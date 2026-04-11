// SPDX-License-Identifier: MPL-2.0

//! DRM 字符设备实现。

mod file;
mod init;
mod ioctl_defs;
mod kms;

use device_id::{DeviceId, MajorId, MinorId};

use crate::prelude::*;

use super::{Device, DeviceType};
use crate::fs::file::FileIo;

pub(crate) use file::DrmFileHandle;
pub(crate) use init::init_in_first_kthread;

/// DRM 设备结构体 — 对应 /dev/dri/card0
///
/// 该结构体在 init_in_first_kthread() 中被注册到字符设备注册表。
/// 用户空间 open("/dev/dri/card0") 时，Device::open() 返回 DrmFileHandle。
#[derive(Debug)]
struct Drm;

impl Device for Drm {
    fn type_(&self) -> DeviceType {
        DeviceType::Char
    }

    fn id(&self) -> DeviceId {
        // Linux DRM 的标准 major 号是 226
        // minor 号 0 对应第一个 DRM 设备 card0
        DeviceId::new(MajorId::new(226), MinorId::new(0))
    }

    fn devtmpfs_path(&self) -> Option<String> {
        // devtmpfs 会在 /dev/dri/ 下自动创建设备节点
        Some("dri/card0".into())
    }

    fn open(&self) -> Result<Box<dyn FileIo>> {
        Ok(Box::new(DrmFileHandle))
    }
}
