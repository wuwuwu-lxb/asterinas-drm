// SPDX-License-Identifier: MPL-2.0

//! 简化 KMS (Kernel Mode Setting) 模型。
//!
//! Phase 3 MVP 提供硬编码的 KMS 对象：
//! - 1 个 CRTC（显示控制器）
//! - 1 个 Encoder（编码器）
//! - 1 个 Connector（连接器）

use crate::{prelude::*, return_errno_with_message, util::ioctl::{Ioctl, InOutData, OutData}};

use super::ioctl_defs::{DrmGetCap, DrmModeCardRes, DrmVersion};

/// DRM 驱动名称
const DRIVER_NAME: &[u8] = b"AsterinasSimpleDrm\0";

/// DRM 驱动描述
const DRIVER_DESC: &[u8] = b"Simple DRM driver for Asterinas OS\0";

/// DRM 驱动构建日期（YYYYMMDD）
const DRIVER_DATE: &[u8] = b"20260411\0";

/// 处理 DRM_IOCTL_VERSION ioctl
///
/// 填充 `struct drm_version` 并写入用户空间。
pub fn handle_version(cmd: &Ioctl<0x64, 0x00, true, OutData<DrmVersion>>) -> Result<()> {
    let mut version = DrmVersion::default();

    version.version_major = 1;
    version.version_minor = 0;
    version.version_patchlevel = 0;

    // 填充 name 字段（64 字节）
    let name_len = DRIVER_NAME.len().min(64);
    version.name[..name_len].copy_from_slice(&DRIVER_NAME[..name_len]);

    // 填充 desc 字段（32 字节）
    let desc_len = DRIVER_DESC.len().min(32);
    version.desc[..desc_len].copy_from_slice(&DRIVER_DESC[..desc_len]);

    // 填充 date 字段（32 字节）
    let date_len = DRIVER_DATE.len().min(32);
    version.date[..date_len].copy_from_slice(&DRIVER_DATE[..date_len]);

    // 写入用户空间
    cmd.write(&version)?;
    Ok(())
}

/// 处理 DRM_IOCTL_GET_CAP ioctl
///
/// 根据 capability 查询设备能力。
pub fn handle_get_cap(
    cmd: &Ioctl<0x64, 0x09, true, InOutData<DrmGetCap>>,
) -> Result<()> {
    let mut cap = cmd.read()?;
    cap.value = match cap.capability {
        DrmGetCap::DRM_CAP_DUMB_BUFFER => 1,
        DrmGetCap::DRM_CAP_VBLANK_HIGH_CRTC => 0,
        DrmGetCap::DRM_CAP_DUMB_PREFERRED_DEPTH => 0,
        _ => {
            log::debug!("unknown DRM capability: {}", cap.capability);
            return_errno_with_message!(Errno::EINVAL, "unknown DRM capability");
        }
    };
    cmd.write(&cap)?;
    Ok(())
}

/// 处理 DRM_IOCTL_MODE_GETRESOURCES ioctl
///
/// 返回 KMS 对象数量和分辨率范围。
pub fn handle_get_resources(
    cmd: &Ioctl<0x42, 0xC0, true, OutData<DrmModeCardRes>>,
) -> Result<()> {
    let mut res = DrmModeCardRes::default();

    // Phase 3 不返回对象 ID（设为 0 表示空指针）
    res.fb_id_ptr = 0;
    res.crtc_id_ptr = 0;
    res.connector_id_ptr = 0;
    res.encoder_id_ptr = 0;

    // 对象计数（硬编码简化值）
    res.count_fbs = 0;
    res.count_crtcs = 1;
    res.count_connectors = 1;
    res.count_encoders = 1;

    // 分辨率范围（硬编码）
    res.min_width = 64;
    res.max_width = 4096;
    res.min_height = 64;
    res.max_height = 4096;

    cmd.write(&res)?;
    Ok(())
}
