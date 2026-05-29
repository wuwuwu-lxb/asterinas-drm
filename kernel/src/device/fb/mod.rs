// SPDX-License-Identifier: MPL-2.0

use alloc::sync::Arc;

use device_id::{DeviceId, MajorId, MinorId};
use spin::Once;

use super::{registry::char, Device, DeviceType, DevtmpfsInodeMeta};
use crate::{fs::file::FileIo, prelude::*};

mod backend;
mod drm_fbdev;
mod file;
mod mmap;
mod uapi;

use backend::FbInfo;
use file::FbHandle;

static FB_INFO: Once<FbInfo> = Once::new();

#[derive(Debug)]
struct Fb;

impl Device for Fb {
    fn type_(&self) -> DeviceType {
        DeviceType::Char
    }

    fn id(&self) -> DeviceId {
        // Same value with Linux: major 29, minor 0.
        DeviceId::new(MajorId::new(29), MinorId::new(0))
    }

    fn devtmpfs_meta(&self) -> Option<DevtmpfsInodeMeta<'_>> {
        // Linux names framebuffer device nodes as `fbN`.
        // TODO: We currently expose only one framebuffer device,
        // so the devtmpfs node is fixed to `fb0`.
        // Reference: <https://elixir.bootlin.com/linux/v6.18/source/drivers/video/fbdev/core/fbsysfs.c#L482>.
        Some(DevtmpfsInodeMeta::new("fb0"))
    }

    fn open(&self) -> Result<Box<dyn FileIo>> {
        let Some(info) = FB_INFO.get() else {
            return Err(Error::with_message(
                Errno::ENODEV,
                "the framebuffer device is not present",
            ));
        };
        Ok(Box::new(FbHandle::new(info.clone())))
    }
}

pub(super) fn init_in_first_kthread() {
    let Some(info) = FbInfo::new() else {
        return;
    };

    FB_INFO.call_once(|| info);
    char::register(Arc::new(Fb)).expect("failed to register framebuffer char device");
}
