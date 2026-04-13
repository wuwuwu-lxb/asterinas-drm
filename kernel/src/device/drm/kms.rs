// SPDX-License-Identifier: MPL-2.0

//! 简化 KMS (Kernel Mode Setting) 模型。
//!
//! 提供硬编码的 KMS 对象：
//! - 1 个 CRTC（显示控制器，ID=1）
//! - 1 个 Encoder（编码器，ID=1）
//! - 1 个 Connector（连接器，ID=1）

use core::mem::size_of;

use crate::{
    current_userspace,
    prelude::*,
    return_errno_with_message,
    util::ioctl::{Ioctl, InOutData, OutData},
};

use super::ioctl_defs::{
    DrmGetCap, DrmModeCardRes, DrmModeCrtc, DrmModeGetConnector,
    DrmModeGetEncoder, DrmModeModeInfo, DrmVersion,
};

/// DRM 驱动名称
const DRIVER_NAME: &[u8] = b"AsterinasSimpleDrm\0";

/// DRM 驱动描述
const DRIVER_DESC: &[u8] = b"Simple DRM driver for Asterinas OS\0";

/// DRM 驱动构建日期（YYYYMMDD）
const DRIVER_DATE: &[u8] = b"20260413\0";

/// KMS 对象 ID 常量
const CRTC_ID: u32 = 1;
const ENCODER_ID: u32 = 1;
const CONNECTOR_ID: u64 = 1;

/// 处理 DRM_IOCTL_VERSION ioctl
///
/// 填充 `struct drm_version` 并写入用户空间。
pub fn handle_version(cmd: &Ioctl<0x64, 0x00, true, OutData<DrmVersion>>) -> Result<()> {
    let mut version = DrmVersion::default();

    version.version_major = 1;
    version.version_minor = 0;
    version.version_patchlevel = 0;

    let name_len = DRIVER_NAME.len().min(64);
    version.name[..name_len].copy_from_slice(&DRIVER_NAME[..name_len]);

    let desc_len = DRIVER_DESC.len().min(32);
    version.desc[..desc_len].copy_from_slice(&DRIVER_DESC[..desc_len]);

    let date_len = DRIVER_DATE.len().min(32);
    version.date[..date_len].copy_from_slice(&DRIVER_DATE[..date_len]);

    cmd.write(&version)?;
    Ok(())
}

/// 处理 DRM_IOCTL_GET_CAP ioctl
///
/// 根据 capability 查询设备能力。
pub fn handle_get_cap(cmd: &Ioctl<0x64, 0x09, true, InOutData<DrmGetCap>>) -> Result<()> {
    let mut cap = cmd.read()?;
    cap.value = match cap.capability {
        DrmGetCap::DRM_CAP_DUMB_BUFFER => 1,
        DrmGetCap::DRM_CAP_VBLANK_HIGH_CRTC => 0,
        DrmGetCap::DRM_CAP_DUMB_PREFERRED_DEPTH => 32,
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
/// 返回 KMS 对象数量、ID 数组和分辨率范围。
pub fn handle_get_resources(
    cmd: &Ioctl<0x42, 0xC0, true, InOutData<DrmModeCardRes>>,
) -> Result<()> {
    let mut res = cmd.read()?;

    let count_crtcs = 1;
    let count_encoders = 1;
    let count_connectors = 1;
    let count_fbs = super::fb::get_manager().get_fb_count() as u32;

    res.count_crtcs = count_crtcs;
    res.count_encoders = count_encoders;
    res.count_connectors = count_connectors;
    res.count_fbs = count_fbs;

    res.min_width = 64;
    res.max_width = 4096;
    res.min_height = 64;
    res.max_height = 4096;

    if res.crtc_id_ptr != 0 && count_crtcs > 0 {
        current_userspace!().writer(res.crtc_id_ptr as Vaddr, 4)?.write_val(&CRTC_ID)?;
    }

    if res.encoder_id_ptr != 0 && count_encoders > 0 {
        current_userspace!().writer(res.encoder_id_ptr as Vaddr, 4)?.write_val(&ENCODER_ID)?;
    }

    if res.connector_id_ptr != 0 && count_connectors > 0 {
        current_userspace!().writer(res.connector_id_ptr as Vaddr, 4)?.write_val(&(CONNECTOR_ID as u32))?;
    }

    if res.fb_id_ptr != 0 && count_fbs > 0 {
        let fb_ids = super::fb::get_manager().get_all_fb_ids();
        for (i, &fb_id) in fb_ids.iter().enumerate() {
            current_userspace!().writer((res.fb_id_ptr as Vaddr) + (i as Vaddr * 4), 4)?.write_val(&fb_id)?;
        }
    }

    cmd.write(&res)?;
    Ok(())
}

/// 处理 DRM_IOCTL_MODE_GETENCODER ioctl
///
/// 返回 Encoder 信息。
pub fn handle_get_encoder(cmd: &Ioctl<0x42, 0xC5, true, InOutData<DrmModeGetEncoder>>) -> Result<()> {
    let mut args = cmd.read()?;

    if args.encoder_id != ENCODER_ID && args.encoder_id != 0 {
        return_errno_with_message!(Errno::ENOENT, "encoder not found");
    }

    args.encoder_id = ENCODER_ID;
    args.encoder_type = DrmModeGetEncoder::DRM_MODE_ENCODER_TMDS;
    args.crtc_id = CRTC_ID;
    args.possible_crtcs = 1 << 0;
    args.possible_clones = 0;

    cmd.write(&args)?;
    Ok(())
}

/// 处理 DRM_IOCTL_MODE_GETCONNECTOR ioctl
///
/// 返回 Connector 信息。
pub fn handle_get_connector(cmd: &Ioctl<0x42, 0xC6, true, InOutData<DrmModeGetConnector>>) -> Result<()> {
    let mut args = cmd.read()?;

    if args.connector_id != 1 && args.connector_id != 0 {
        return_errno_with_message!(Errno::ENOENT, "connector not found");
    }

    args.connector_id = 1;
    args.connector_id_val = 1;
    args.encoder_id = 1;
    args.connector_type = 4; // HDMI-A
    args.connector_type_id = 1;
    args.connection = 1; // CONNECTED

    if args.count_encoders > 0 && args.encoders_ptr != 0 {
        current_userspace!().writer(args.encoders_ptr as Vaddr, 4)?.write_val(&1u32)?;
    }
    args.count_encoders = 1;

    let default_mode = DrmModeModeInfo::new(1024, 768, 60);
    if args.count_modes > 0 && args.modes_ptr != 0 {
        current_userspace!().writer(args.modes_ptr as Vaddr, size_of::<DrmModeModeInfo>())?.write_val(&default_mode)?;
    }
    args.count_modes = 1;

    args.count_props = 0;

    cmd.write(&args)?;
    Ok(())
}

/// 处理 DRM_IOCTL_MODE_GETCRTC ioctl
///
/// 返回 CRTC 配置。
pub fn handle_get_crtc(cmd: &Ioctl<0x42, 0xC1, true, InOutData<DrmModeCrtc>>) -> Result<()> {
    let mut args = cmd.read()?;

    if args.crtc_id != CRTC_ID && args.crtc_id != 0 {
        return_errno_with_message!(Errno::ENOENT, "crtc not found");
    }

    args.crtc_id = CRTC_ID;
    args.fb_id = 0;
    args.x = 0;
    args.y = 0;
    args.mode_valid = 0;
    args.mode = DrmModeModeInfo::default();
    args.count_modes = 0;
    args.gamma_size = 0;

    cmd.write(&args)?;
    Ok(())
}

/// 处理 DRM_IOCTL_MODE_SETCRTC ioctl
///
/// 设置 CRTC 配置并启用显示输出。
pub fn handle_set_crtc(cmd: &Ioctl<0x42, 0xC0, true, InOutData<DrmModeCrtc>>) -> Result<()> {
    let args = cmd.read()?;

    if args.crtc_id != CRTC_ID {
        return_errno_with_message!(Errno::ENOENT, "crtc not found");
    }

    if args.fb_id != 0 && !super::fb::get_manager().fb_exists(args.fb_id) {
        return_errno_with_message!(Errno::ENOENT, "framebuffer not found");
    }

    log::info!("SET_CRTC: crtc={}, fb={}, mode={:?}", args.crtc_id, args.fb_id, args.mode);

    cmd.write(&args)?;
    Ok(())
}
