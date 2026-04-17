// SPDX-License-Identifier: MPL-2.0

//! DRM KMS Framebuffer 管理模块。

use alloc::vec::Vec;

use aster_framebuffer::{FRAMEBUFFER, FrameBufferOps};
use spin::Once;

use super::dumb::{self, DumbBufferSnapshot};
use super::ioctl_defs::{AddFb, GetFb, RmFb};

use crate::prelude::*;

#[derive(Debug, Clone)]
struct FbObject {
    fb_id: u32,
    handle: u32,
    width: u32,
    height: u32,
    bpp: u32,
}

#[derive(Debug, Clone, Copy)]
struct PresentLayout {
    height: usize,
    row_bytes: usize,
    src_pitch: usize,
    dst_pitch: usize,
}

pub(crate) struct FbManager {
    fbs: spin::Mutex<Vec<FbObject>>,
    next_id: spin::Mutex<u32>,
    /// The currently active FB ID set by SET_CRTC.
    /// None means no FB has been made active yet.
    current_fb_id: spin::Mutex<Option<u32>>,
}

impl FbManager {
    fn new() -> Self {
        FbManager {
            fbs: spin::Mutex::new(Vec::new()),
            next_id: spin::Mutex::new(1),
            current_fb_id: spin::Mutex::new(None),
        }
    }

    pub(crate) fn fb_exists(&self, fb_id: u32) -> bool {
        self.fbs.lock().iter().any(|fb| fb.fb_id == fb_id)
    }

    pub(crate) fn get_fb(&self, fb_id: u32) -> Option<FbObject> {
        self.fbs.lock().iter().find(|fb| fb.fb_id == fb_id).cloned()
    }

    pub(crate) fn fb_count(&self) -> usize {
        self.fbs.lock().len()
    }

    pub(crate) fn all_fb_ids(&self) -> Vec<u32> {
        self.fbs.lock().iter().map(|fb| fb.fb_id).collect()
    }

    pub(crate) fn add_fb(&self, handle: u32, width: u32, height: u32, bpp: u32) -> Result<u32> {
        let fb_id = {
            let mut next_id = self.next_id.lock();
            let id = *next_id;
            *next_id += 1;
            id
        };
        self.fbs.lock().push(FbObject {
            fb_id,
            handle,
            width,
            height,
            bpp,
        });
        Ok(fb_id)
    }

    pub(crate) fn remove_fb(&self, fb_id: u32) -> Result<()> {
        let mut fbs = self.fbs.lock();
        let idx = fbs.iter().position(|fb| fb.fb_id == fb_id)
            .ok_or(Error::from(Errno::ENOENT))?;
        fbs.remove(idx);
        Ok(())
    }

    pub(crate) fn current_fb_id(&self) -> Option<u32> {
        *self.current_fb_id.lock()
    }

    pub(crate) fn set_current_fb(&self, fb_id: Option<u32>) {
        *self.current_fb_id.lock() = fb_id;
    }

    pub(crate) fn present_fb(&self, fb_id: u32) -> Result<()> {
        let fb = self.get_fb(fb_id).ok_or(Error::from(Errno::ENOENT))?;
        let snapshot = dumb::get_manager().snapshot(fb.handle)?;
        let visible = FRAMEBUFFER.get().ok_or(Error::from(Errno::ENODEV))?;
        let layout = present_layout(&fb, &snapshot, visible.as_ref())?;
        blit_fb(&snapshot, visible.as_ref(), layout)?;
        self.set_current_fb(Some(fb_id));
        Ok(())
    }
}

fn bytes_per_pixel(bpp: u32) -> Result<usize> {
    if bpp == 0 || bpp % 8 != 0 {
        return Err(Error::from(Errno::EINVAL));
    }
    Ok((bpp / 8) as usize)
}

fn checked_row_bytes(width: u32, bytes_per_pixel: usize) -> Result<usize> {
    (width as usize)
        .checked_mul(bytes_per_pixel)
        .ok_or(Error::from(Errno::EINVAL))
}

fn present_layout(
    fb: &FbObject,
    snapshot: &DumbBufferSnapshot,
    visible: &dyn FrameBufferOps,
) -> Result<PresentLayout> {
    if snapshot.width != fb.width || snapshot.height != fb.height || snapshot.bpp != fb.bpp {
        return Err(Error::from(Errno::EINVAL));
    }

    let bytes_per_pixel = bytes_per_pixel(snapshot.bpp)?;
    let row_bytes = checked_row_bytes(snapshot.width, bytes_per_pixel)?;
    let src_pitch = snapshot.pitch as usize;
    let height = snapshot.height as usize;
    let snapshot_size = snapshot.size as usize;

    if snapshot.data.len() != snapshot_size {
        return Err(Error::from(Errno::EINVAL));
    }
    if src_pitch < row_bytes {
        return Err(Error::from(Errno::EINVAL));
    }
    if snapshot_size < src_pitch.checked_mul(height).ok_or(Error::from(Errno::EINVAL))? {
        return Err(Error::from(Errno::EINVAL));
    }

    if visible.width() != fb.width as usize || visible.height() != fb.height as usize {
        return Err(Error::from(Errno::EINVAL));
    }
    if visible.pixel_format().nbytes() != bytes_per_pixel {
        return Err(Error::from(Errno::EINVAL));
    }
    if visible.line_size() < row_bytes {
        return Err(Error::from(Errno::EINVAL));
    }
    if visible.size() < visible.line_size().checked_mul(height).ok_or(Error::from(Errno::EINVAL))? {
        return Err(Error::from(Errno::EINVAL));
    }

    Ok(PresentLayout {
        height,
        row_bytes,
        src_pitch,
        dst_pitch: visible.line_size(),
    })
}

fn blit_fb(
    snapshot: &DumbBufferSnapshot,
    visible: &dyn FrameBufferOps,
    layout: PresentLayout,
) -> Result<()> {
    for row in 0..layout.height {
        let src_offset = row.checked_mul(layout.src_pitch).ok_or(Error::from(Errno::EINVAL))?;
        let src_end = src_offset.checked_add(layout.row_bytes).ok_or(Error::from(Errno::EINVAL))?;
        let dst_offset = row.checked_mul(layout.dst_pitch).ok_or(Error::from(Errno::EINVAL))?;
        visible.write_bytes_at(dst_offset, &snapshot.data[src_offset..src_end])?;
    }
    Ok(())
}

static FB_MANAGER: Once<FbManager> = Once::new();

pub fn init() {
    FB_MANAGER.call_once(|| FbManager::new());
}

pub(crate) fn get_manager() -> &'static FbManager {
    FB_MANAGER.get().expect("FB_MANAGER not initialized")
}

pub(crate) fn fb_exists(fb_id: u32) -> bool {
    get_manager().fb_exists(fb_id)
}

pub(crate) fn get_fb_count() -> usize {
    get_manager().fb_count()
}

pub(crate) fn get_all_fb_ids() -> Vec<u32> {
    get_manager().all_fb_ids()
}

pub fn handle_add_fb(cmd: &AddFb) -> Result<()> {
    let mut args = cmd.read()?;
    let _ = dumb::get_manager().get_offset(args.handle)?;
    let fb_id = get_manager().add_fb(args.handle, args.width, args.height, args.bpp)?;
    args.fb_id = fb_id;
    cmd.write(&args)?;
    Ok(())
}

pub fn handle_rm_fb(cmd: &RmFb) -> Result<()> {
    let args = cmd.read()?;
    get_manager().remove_fb(args.fb_id)?;
    Ok(())
}

pub fn handle_get_fb(cmd: &GetFb) -> Result<()> {
    let mut args = cmd.read()?;
    let fb = get_manager().get_fb(args.fb_id)
        .ok_or(Error::from(Errno::ENOENT))?;
    let snapshot = dumb::get_manager().snapshot(fb.handle)?;
    args.width = fb.width;
    args.height = fb.height;
    args.bpp = fb.bpp;
    args.pitch = snapshot.pitch;
    args.handle = fb.handle;
    args.offset = 0;
    cmd.write(&args)?;
    Ok(())
}
