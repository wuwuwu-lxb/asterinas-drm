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

use super::{ioctl_defs, kms};

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
        // Phase 3 不支持 mmap（属于 Phase 4 的 DUMB_BUFFER 范畴）
        return_errno_with_message!(Errno::ENODEV, "mmap is not supported in Phase 3")
    }

    fn ioctl(&self, raw_ioctl: RawIoctl) -> Result<i32> {
        use ioctl_defs::*;

        dispatch_ioctl!(match raw_ioctl {
            // DRM_IOCTL_VERSION (0x6400) — 返回驱动版本
            cmd @ GetVersion => {
                kms::handle_version(&cmd)?;
                Ok(0)
            }
            // DRM_IOCTL_GET_CAP (0x6409) — 返回设备能力
            cmd @ GetCap => {
                kms::handle_get_cap(&cmd)?;
                Ok(0)
            }
            // DRM_IOCTL_MODE_GETRESOURCES (0x40C0) — 返回 KMS 对象信息
            cmd @ GetResources => {
                kms::handle_get_resources(&cmd)?;
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
