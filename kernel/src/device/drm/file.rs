// SPDX-License-Identifier: MPL-2.0

//! DRM 文件句柄实现。

use crate::{
    events::IoEvents,
    fs::{
        file::{FileIo, Mappable, StatusFlags},
        vfs::inode::InodeIo,
    },
    prelude::*, process::signal::{PollHandle, Pollable}, util::ioctl::{RawIoctl, dispatch_ioctl},
};

use super::{dumb, fb, ioctl_defs, kms};

#[derive(Debug)]
pub(crate) struct DrmFileHandle;

impl Pollable for DrmFileHandle {
    fn poll(&self, mask: IoEvents, _poller: Option<&mut PollHandle>) -> IoEvents {
        IoEvents::empty() & mask
    }
}

impl InodeIo for DrmFileHandle {
    fn read_at(&self, _offset: usize, _writer: &mut VmWriter, _status_flags: StatusFlags) -> Result<usize> {
        return_errno_with_message!(Errno::ENOTTY, "DRM device does not support read")
    }

    fn write_at(&self, _offset: usize, _reader: &mut VmReader, _status_flags: StatusFlags) -> Result<usize> {
        return_errno_with_message!(Errno::ENOTTY, "DRM device does not support write")
    }
}

impl FileIo for DrmFileHandle {
    fn check_seekable(&self) -> Result<()> {
        Ok(())
    }

    fn is_offset_aware(&self) -> bool {
        true
    }

    fn mappable(&self) -> Result<Mappable> {
        let manager = dumb::get_manager();
        Ok(Mappable::Vmo(manager.vmo()))
    }

    fn ioctl(&self, raw_ioctl: RawIoctl) -> Result<i32> {
        use ioctl_defs::*;

        dispatch_ioctl!(match raw_ioctl {
            cmd @ GetVersion => { kms::handle_version(&cmd)?; Ok(0) }
            cmd @ GetCap => { kms::handle_get_cap(&cmd)?; Ok(0) }
            cmd @ GetResources => { kms::handle_get_resources(&cmd)?; Ok(0) }
            cmd @ GetConnector => { kms::handle_get_connector(&cmd)?; Ok(0) }
            cmd @ GetEncoder => { kms::handle_get_encoder(&cmd)?; Ok(0) }
            cmd @ GetCrtc => { kms::handle_get_crtc(&cmd)?; Ok(0) }
            cmd @ SetCrtc => { kms::handle_set_crtc(&cmd)?; Ok(0) }
            cmd @ PageFlip => { kms::handle_page_flip(&cmd)?; Ok(0) }
            cmd @ CreateDumb => { handle_create_dumb(&cmd)?; Ok(0) }
            cmd @ MapDumb => { handle_map_dumb(&cmd)?; Ok(0) }
            cmd @ DestroyDumb => { handle_destroy_dumb(&cmd)?; Ok(0) }
            cmd @ AddFb => { fb::handle_add_fb(&cmd)?; Ok(0) }
            cmd @ RmFb => { fb::handle_rm_fb(&cmd)?; Ok(0) }
            cmd @ GetFb => { fb::handle_get_fb(&cmd)?; Ok(0) }
            _ => {
                ostd::debug!("unknown DRM ioctl: {:#x}", raw_ioctl.cmd());
                return_errno_with_message!(Errno::ENOTTY, "unsupported DRM ioctl")
            }
        })
    }
}

fn handle_create_dumb(cmd: &ioctl_defs::CreateDumb) -> Result<()> {
    let mut args = cmd.read()?;
    let manager = dumb::get_manager();
    let (handle, pitch, _offset, size) = manager.create_dumb(args.width, args.height, args.bpp)?;
    args.handle = handle;
    args.pitch = pitch;
    args.size = size;
    cmd.write(&args)?;
    ostd::debug!("CREATE_DUMB: {}x{}x{}bpp, handle={}, pitch={}, size={}",
        args.width, args.height, args.bpp, handle, pitch, size);
    Ok(())
}

fn handle_map_dumb(cmd: &ioctl_defs::MapDumb) -> Result<()> {
    let mut args = cmd.read()?;
    let manager = dumb::get_manager();
    let offset = manager.get_offset(args.handle)?;
    args.offset = offset as u64;
    cmd.write(&args)?;
    ostd::info!("MAP_DUMB: handle={}, offset={:#x}", args.handle, offset);
    Ok(())
}

fn handle_destroy_dumb(cmd: &ioctl_defs::DestroyDumb) -> Result<()> {
    let args = cmd.read()?;
    let manager = dumb::get_manager();
    manager.destroy_dumb(args.handle)?;
    ostd::debug!("DESTROY_DUMB: handle={}", args.handle);
    Ok(())
}
