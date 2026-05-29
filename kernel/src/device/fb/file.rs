// SPDX-License-Identifier: MPL-2.0

use alloc::vec::Vec;

use aster_framebuffer::{ColorMapEntry, MAX_CMAP_SIZE};
use ostd::mm::{HasSize, VmIo, VmReader, VmWriter};

use super::{
    backend::{FbBackend, FbInfo},
    uapi::{ioctl_defs, FbCmapUser, FbFixScreenInfo, FbVarScreenInfo},
};
use crate::{
    context::current_userspace,
    events::IoEvents,
    fs::{
        file::{FileIo, Mappable, StatusFlags},
        vfs::inode::InodeIo,
    },
    prelude::*,
    process::signal::{PollHandle, Pollable},
    util::ioctl::{dispatch_ioctl, RawIoctl},
};

#[derive(Debug)]
pub(super) struct FbHandle {
    info: FbInfo,
}

impl FbHandle {
    pub(super) fn new(info: FbInfo) -> Self {
        Self { info }
    }

    pub(super) fn backend(&self) -> &FbBackend {
        self.info.backend()
    }

    /// Reads an array of `u16` color map values from userspace.
    fn read_color_maps_from_user(addr: usize, data: &mut [u16]) -> Result<()> {
        for (i, item) in data.iter_mut().enumerate() {
            let user_addr = addr + i * size_of::<u16>();
            *item = current_userspace!().read_val(user_addr)?;
        }
        Ok(())
    }

    /// Writes an array of `u16` color map values to userspace.
    fn write_color_maps_to_user(addr: usize, data: &[u16]) -> Result<()> {
        for (i, &value) in data.iter().enumerate() {
            let user_addr = addr + i * size_of::<u16>();
            current_userspace!().write_val(user_addr, &value)?;
        }
        Ok(())
    }

    fn collect_var_screen_info(&self) -> FbVarScreenInfo {
        self.backend().collect_var_screen_info()
    }

    fn collect_fix_screen_info(&self) -> FbFixScreenInfo {
        self.backend().collect_fix_screen_info()
    }

    fn validate_var_screen_info(&self, requested: &FbVarScreenInfo) -> Result<()> {
        let current = self.collect_var_screen_info();

        if requested.xres != current.xres
            || requested.yres != current.yres
            || requested.xres_virtual != current.xres_virtual
            || requested.yres_virtual != current.yres_virtual
            || requested.xoffset != 0
            || requested.yoffset != 0
            || requested.bits_per_pixel != current.bits_per_pixel
            || requested.grayscale != current.grayscale
            || requested.nonstd != current.nonstd
            || requested.rotate != current.rotate
            || requested.red != current.red
            || requested.green != current.green
            || requested.blue != current.blue
            || requested.transp != current.transp
        {
            return_errno_with_message!(
                Errno::EINVAL,
                "the requested framebuffer mode cannot be represented by the current DRM scanout"
            );
        }

        Ok(())
    }

    fn validate_pan_display(&self, requested: &FbVarScreenInfo) -> Result<()> {
        let current = self.collect_var_screen_info();
        if requested.xres != current.xres
            || requested.yres != current.yres
            || requested.bits_per_pixel != current.bits_per_pixel
            || requested.red != current.red
            || requested.green != current.green
            || requested.blue != current.blue
            || requested.transp != current.transp
            || requested.xoffset != 0
            || requested.yoffset != 0
            || requested.xres_virtual != current.xres_virtual
            || requested.yres_virtual != current.yres_virtual
        {
            return_errno_with_message!(
                Errno::EINVAL,
                "the requested framebuffer pan is outside the fixed DRM scanout"
            );
        }

        Ok(())
    }

    /// Handles the [`ioctl_defs::GetColorMap`] ioctl command.
    fn handle_get_cmap(&self, cmap_user: &FbCmapUser) -> Result<()> {
        if cmap_user.len == 0 {
            return Ok(());
        }

        let start = cmap_user.start as usize;
        let len = cmap_user.len as usize;

        let entries = match self.backend() {
            FbBackend::Drm(backend) => {
                let cmap = backend.cmap.lock();
                if start >= cmap.len() || len > cmap.len() - start {
                    return_errno_with_message!(
                        Errno::EINVAL,
                        "the color map index is out of bounds"
                    );
                }
                cmap[start..start + len].to_vec()
            }
            FbBackend::Legacy(framebuffer) => {
                framebuffer.get_color_map(start, len).ok_or_else(|| {
                    Error::with_message(Errno::EINVAL, "the color map index is out of bounds")
                })?
            }
        };

        let red: Vec<u16> = entries.iter().map(|e| e.red).collect();
        let green: Vec<u16> = entries.iter().map(|e| e.green).collect();
        let blue: Vec<u16> = entries.iter().map(|e| e.blue).collect();
        let transp: Vec<u16> = entries.iter().map(|e| e.transp).collect();

        Self::write_color_maps_to_user(cmap_user.red, &red)?;
        Self::write_color_maps_to_user(cmap_user.green, &green)?;
        Self::write_color_maps_to_user(cmap_user.blue, &blue)?;
        if cmap_user.transp != 0 {
            Self::write_color_maps_to_user(cmap_user.transp, &transp)?;
        }

        Ok(())
    }

    /// Handles the [`ioctl_defs::PutColorMap`] ioctl command.
    fn handle_set_cmap(&self, cmap_user: &FbCmapUser) -> Result<()> {
        if cmap_user.len == 0 {
            return Ok(());
        }

        let start = cmap_user.start as usize;
        let len = cmap_user.len as usize;

        // Check the size to prevent excessive memory allocation.
        if start > MAX_CMAP_SIZE || len > MAX_CMAP_SIZE - start {
            return_errno_with_message!(
                Errno::EINVAL,
                "the color map range exceeds its maximum size"
            );
        }

        let mut red = vec![0u16; len];
        let mut green = vec![0u16; len];
        let mut blue = vec![0u16; len];
        let mut transp = vec![0u16; len];

        Self::read_color_maps_from_user(cmap_user.red, &mut red)?;
        Self::read_color_maps_from_user(cmap_user.green, &mut green)?;
        Self::read_color_maps_from_user(cmap_user.blue, &mut blue)?;
        if cmap_user.transp != 0 {
            Self::read_color_maps_from_user(cmap_user.transp, &mut transp)?;
        }

        let entries: Vec<ColorMapEntry> = (0..len)
            .map(|i| ColorMapEntry {
                red: red[i],
                green: green[i],
                blue: blue[i],
                transp: transp[i],
            })
            .collect();

        match self.backend() {
            FbBackend::Drm(backend) => {
                let mut cmap = backend.cmap.lock();
                let required_len = start
                    .checked_add(entries.len())
                    .ok_or_else(|| Error::new(Errno::EOVERFLOW))?;
                if cmap.len() < required_len {
                    cmap.resize(
                        required_len,
                        ColorMapEntry {
                            red: 0,
                            green: 0,
                            blue: 0,
                            transp: 0,
                        },
                    );
                }
                cmap[start..required_len].copy_from_slice(&entries);
            }
            FbBackend::Legacy(framebuffer) => framebuffer.set_color_map(start, &entries)?,
        }

        Ok(())
    }
}

impl Pollable for FbHandle {
    fn poll(&self, mask: IoEvents, _poller: Option<&mut PollHandle>) -> IoEvents {
        let events = IoEvents::IN | IoEvents::OUT;
        events & mask
    }
}

impl InodeIo for FbHandle {
    fn read_at(
        &self,
        offset: usize,
        writer: &mut VmWriter,
        _status_flags: StatusFlags,
    ) -> Result<usize> {
        match self.backend() {
            FbBackend::Drm(backend) => {
                if !writer.has_avail() {
                    return Ok(0);
                }

                if offset >= backend.size_bytes {
                    return Ok(0);
                }

                let len = writer.avail().min(backend.size_bytes - offset);
                if len == 0 {
                    return Ok(0);
                }

                let mut new_writer = writer.clone_exclusive();
                new_writer.limit(len);
                backend.gem.read(offset, &mut new_writer)?;
                writer.skip(len);
                Ok(len)
            }
            FbBackend::Legacy(framebuffer) => {
                if !writer.has_avail() {
                    return Ok(0);
                }

                let io_mem = framebuffer.io_mem();
                let size = io_mem.size();
                if offset >= size {
                    return Ok(0);
                }

                let len = writer.avail().min(size - offset);
                if len == 0 {
                    return Ok(0);
                }

                let mut new_writer = writer.clone_exclusive();
                new_writer.limit(len);

                let result = io_mem.read_fallible(offset, &mut new_writer);
                let copied = match result {
                    Ok(copied) => copied,
                    Err((err, copied)) => {
                        if copied > 0 {
                            copied
                        } else {
                            return Err(err.into());
                        }
                    }
                };

                writer.skip(copied);
                Ok(copied)
            }
        }
    }

    fn write_at(
        &self,
        offset: usize,
        reader: &mut VmReader,
        _status_flags: StatusFlags,
    ) -> Result<usize> {
        match self.backend() {
            FbBackend::Drm(backend) => {
                if !reader.has_remain() {
                    return Ok(0);
                }

                if offset >= backend.size_bytes {
                    return_errno_with_message!(
                        Errno::ENOSPC,
                        "the write offset is beyond the framebuffer size"
                    );
                }

                let len = reader.remain().min(backend.size_bytes - offset);
                if len == 0 {
                    return Ok(0);
                }

                let mut new_reader = reader.clone();
                new_reader.limit(len);
                backend.gem.write(offset, &mut new_reader)?;
                reader.skip(len);
                backend.device.dirty_fb(backend.fb_id)?;
                Ok(len)
            }
            FbBackend::Legacy(framebuffer) => {
                if !reader.has_remain() {
                    return Ok(0);
                }

                let io_mem = framebuffer.io_mem();
                let size = io_mem.size();
                if offset >= size {
                    return_errno_with_message!(
                        Errno::ENOSPC,
                        "the write offset is beyond the framebuffer size"
                    );
                }

                let len = reader.remain().min(size - offset);
                if len == 0 {
                    return Ok(0);
                }

                let mut new_reader = reader.clone();
                new_reader.limit(len);

                let result = io_mem.write_fallible(offset, &mut new_reader);
                let copied = match result {
                    Ok(copied) => copied,
                    Err((err, copied)) => {
                        if copied > 0 {
                            copied
                        } else {
                            return Err(err.into());
                        }
                    }
                };

                reader.skip(copied);
                Ok(copied)
            }
        }
    }
}

impl FileIo for FbHandle {
    fn check_seekable(&self) -> Result<()> {
        Ok(())
    }

    fn is_offset_aware(&self) -> bool {
        true
    }

    fn mappable(&self) -> Result<&dyn Mappable> {
        Ok(self as &dyn Mappable)
    }

    fn ioctl(&self, raw_ioctl: RawIoctl) -> Result<i32> {
        use ioctl_defs::*;

        dispatch_ioctl!(match raw_ioctl {
            cmd @ GetVarScreenInfo => {
                cmd.write(&self.collect_var_screen_info())?;
                Ok(0)
            }
            cmd @ PutVarScreenInfo => {
                let requested = cmd.read()?;
                self.validate_var_screen_info(&requested)?;
                cmd.write(&self.collect_var_screen_info())?;
                Ok(0)
            }
            cmd @ GetFixScreenInfo => {
                cmd.write(&self.collect_fix_screen_info())?;
                Ok(0)
            }
            cmd @ GetColorMap => {
                self.handle_get_cmap(&cmd.read()?)?;
                Ok(0)
            }
            cmd @ PutColorMap => {
                self.handle_set_cmap(&cmd.read()?)?;
                Ok(0)
            }
            cmd @ PanDisplay => {
                let requested = cmd.read()?;
                self.validate_pan_display(&requested)?;
                if let FbBackend::Drm(backend) = self.backend() {
                    backend.restore_scanout()?;
                    backend.dirty()?;
                }
                cmd.write(&self.collect_var_screen_info())?;
                Ok(0)
            }
            cmd @ Blank => {
                let _level = cmd.get();
                if self.backend().is_drm() {
                    Ok(0)
                } else {
                    return_errno_with_message!(
                        Errno::EINVAL,
                        "blanking is not supported by the legacy framebuffer backend"
                    )
                }
            }
            _ => {
                ostd::debug!(
                    "the ioctl command {:#x} is unknown for framebuffer devices",
                    raw_ioctl.cmd()
                );
                return_errno_with_message!(Errno::ENOTTY, "the ioctl command is unknown");
            }
        })
    }
}
