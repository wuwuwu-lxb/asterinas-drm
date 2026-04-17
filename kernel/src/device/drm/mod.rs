// SPDX-License-Identifier: MPL-2.0

//! DRM 字符设备实现。

mod dumb;
mod fb;
mod file;
mod gem;
mod init;
mod ioctl_defs;
mod kms;

use device_id::{DeviceId, MajorId, MinorId};

use crate::prelude::*;

use super::{Device, DeviceType};
use crate::fs::file::FileIo;

pub(crate) use file::DrmFileHandle;
pub(crate) use init::init_in_first_kthread;

#[derive(Debug)]
struct Drm;

impl Device for Drm {
    fn type_(&self) -> DeviceType {
        DeviceType::Char
    }

    fn id(&self) -> DeviceId {
        DeviceId::new(MajorId::new(226), MinorId::new(0))
    }

    fn devtmpfs_path(&self) -> Option<String> {
        Some("dri/card0".into())
    }

    fn open(&self) -> Result<Box<dyn FileIo>> {
        Ok(Box::new(DrmFileHandle))
    }
}
