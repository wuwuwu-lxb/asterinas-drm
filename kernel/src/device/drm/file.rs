// SPDX-License-Identifier: MPL-2.0

//! DRM 文件句柄实现。

use crate::{
    events::IoEvents,
    fs::{
        file::{FileIo, Mappable, StatusFlags},
        vfs::inode::InodeIo,
    },
    prelude::*,
    process::signal::{PollHandle, Pollable},
    util::ioctl::{RawIoctl, dispatch_ioctl},
};

use super::{dumb, fb, ioctl_defs, kms};

/// DRM 文件句柄
///
/// 每个用户空间进程 open("/dev/dri/card0") 都会创建一个独立的 DrmFileHandle 实例。
#[derive(Debug)]
pub(crate) struct DrmFileHandle;

impl Pollable for DrmFileHandle {
    fn poll(&self, mask: IoEvents, _poller: Option<&mut PollHandle>) -> IoEvents {
        // DRM 设备支持 poll，用于 eventfd 风格的事件通知
        // Phase 3 MVP 暂时不支持 DRM events，返回空事件
        IoEvents::empty() & mask
    }
}

impl InodeIo for DrmFileHandle {
    fn read_at(
        &self,
        _offset: usize,
        _writer: &mut VmWriter,
        _status_flags: StatusFlags,
    ) -> Result<usize> {
        // DRM 字符设备不支持 read()
        return_errno_with_message!(Errno::ENOTTY, "DRM device does not support read")
    }

    fn write_at(
        &self,
        _offset: usize,
        _reader: &mut VmReader,
        _status_flags: StatusFlags,
    ) -> Result<usize> {
        // DRM 字符设备不支持 write()
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
        // Phase 4: Dumb Buffer mmap 支持
        // 返回 DumbBufferManager 的 VMO
        // mmap offset 指定具体的 buffer 槽位
        let manager = dumb::get_manager();
        Ok(Mappable::Vmo(manager.vmo()))
    }

    fn ioctl(&self, raw_ioctl: RawIoctl) -> Result<i32> {
        use ioctl_defs::*;

        dispatch_ioctl!(match raw_ioctl {
            // DRM 核心 ioctl
            cmd @ GetVersion => {
                kms::handle_version(&cmd)?;
                Ok(0)
            }
            cmd @ GetCap => {
                kms::handle_get_cap(&cmd)?;
                Ok(0)
            }

            // KMS 资源查询
            cmd @ GetResources => {
                kms::handle_get_resources(&cmd)?;
                Ok(0)
            }
            cmd @ GetConnector => {
                kms::handle_get_connector(&cmd)?;
                Ok(0)
            }
            cmd @ GetEncoder => {
                kms::handle_get_encoder(&cmd)?;
                Ok(0)
            }
            cmd @ GetCrtc => {
                kms::handle_get_crtc(&cmd)?;
                Ok(0)
            }

            // KMS 显示设置
            cmd @ SetCrtc => {
                kms::handle_set_crtc(&cmd)?;
                Ok(0)
            }

            // Dumb Buffer 管理
            cmd @ CreateDumb => {
                handle_create_dumb(&cmd)?;
                Ok(0)
            }
            cmd @ MapDumb => {
                handle_map_dumb(&cmd)?;
                Ok(0)
            }
            cmd @ DestroyDumb => {
                handle_destroy_dumb(&cmd)?;
                Ok(0)
            }

            // Framebuffer 管理
            cmd @ AddFb => {
                fb::handle_add_fb(&cmd)?;
                Ok(0)
            }
            cmd @ RmFb => {
                fb::handle_rm_fb(&cmd)?;
                Ok(0)
            }
            cmd @ GetFb => {
                fb::handle_get_fb(&cmd)?;
                Ok(0)
            }

            _ => {
                log::debug!(
                    "unknown DRM ioctl: {:#x}",
                    raw_ioctl.cmd()
                );
                return_errno_with_message!(Errno::ENOTTY, "unsupported DRM ioctl")
            }
        })
    }
}

/// 处理 DRM_IOCTL_MODE_CREATE_DUMB ioctl
fn handle_create_dumb(cmd: &ioctl_defs::CreateDumb) -> Result<()> {
    let mut args = cmd.read()?;

    let manager = dumb::get_manager();
    let (handle, pitch, offset, size) = manager.create_dumb(args.width, args.height, args.bpp)?;

    args.handle = handle;
    args.pitch = pitch;
    args.size = size;
    cmd.write(&args)?;

    // 注意：offset 通过 MAP_DUMB ioctl 获取，而不是在这里返回
    log::debug!(
        "CREATE_DUMB: {}x{}x{}bpp, handle={}, pitch={}, size={}",
        args.width, args.height, args.bpp, handle, pitch, size
    );

    Ok(())
}

/// 处理 DRM_IOCTL_MODE_MAP_DUMB ioctl
fn handle_map_dumb(cmd: &ioctl_defs::MapDumb) -> Result<()> {
    let mut args = cmd.read()?;

    let manager = dumb::get_manager();
    let offset = manager.get_offset(args.handle)?;

    args.offset = offset as u64;
    cmd.write(&args)?;

    log::info!("MAP_DUMB: handle={}, offset={:#x}", args.handle, offset);

    Ok(())
}

/// 处理 DRM_IOCTL_MODE_DESTROY_DUMB ioctl
fn handle_destroy_dumb(cmd: &ioctl_defs::DestroyDumb) -> Result<()> {
    let args = cmd.read()?;

    let manager = dumb::get_manager();
    manager.destroy_dumb(args.handle)?;

    log::debug!("DESTROY_DUMB: handle={}", args.handle);

    Ok(())
}
