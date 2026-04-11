// SPDX-License-Identifier: MPL-2.0

//! DRM 设备初始化。

use alloc::sync::Arc;

use super::Drm;
use crate::device::registry::char;

/// 在第一个内核线程中初始化 DRM 字符设备
///
/// 该函数在系统启动过程中被 `device::init_in_first_kthread()` 调用。
/// 它将 Drm 设备注册到字符设备注册表，后续 devtmpfs 会自动创建设备节点。
pub(crate) fn init_in_first_kthread() {
    // 注册 /dev/dri/card0
    if let Err(e) = char::register(Arc::new(Drm)) {
        log::warn!("failed to register DRM device: {:?}", e);
    } else {
        log::info!("DRM device /dev/dri/card0 registered (major 226, minor 0)");
    }
}
