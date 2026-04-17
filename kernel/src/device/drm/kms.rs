// SPDX-License-Identifier: MPL-2.0

//! 简化 KMS (Kernel Mode Setting) 模型。

use core::mem::size_of;

use aster_framebuffer::FRAMEBUFFER;

use crate::{
    current_userspace,
    prelude::*,
    return_errno_with_message,
    util::ioctl::{Ioctl, InOutData, OutData},
};

use super::ioctl_defs::{
    DrmGetCap, DrmModeCardRes, DrmModeCrtc, DrmModeGetConnector,
    DrmModeGetEncoder, DrmModeModeInfo, DrmModePageFlip, DrmVersion,
    GetResources, GetEncoder, GetConnector, GetCrtc, SetCrtc, PageFlip,
};
use super::fb;

const DRIVER_NAME: &[u8] = b"AsterinasSimpleDrm\0";
const DRIVER_DESC: &[u8] = b"Simple DRM driver for Asterinas OS\0";
const DRIVER_DATE: &[u8] = b"20260413\0";

const CRTC_ID: u32 = 1;
const ENCODER_ID: u32 = 1;
const CONNECTOR_ID: u32 = 1;

fn current_mode() -> DrmModeModeInfo {
    let Some(framebuffer) = FRAMEBUFFER.get() else {
        return DrmModeModeInfo::new(1024, 768, 60);
    };
    DrmModeModeInfo::new(framebuffer.width() as u16, framebuffer.height() as u16, 60)
}

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

pub fn handle_get_cap(cmd: &Ioctl<0x64, 0x09, true, InOutData<DrmGetCap>>) -> Result<()> {
    let mut cap = cmd.read()?;
    cap.value = match cap.capability {
        DrmGetCap::DRM_CAP_DUMB_BUFFER => 1,
        DrmGetCap::DRM_CAP_VBLANK_HIGH_CRTC => 0,
        DrmGetCap::DRM_CAP_DUMB_PREFERRED_DEPTH => 32,
        _ => {
            ostd::debug!("unknown DRM capability: {}", cap.capability);
            return_errno_with_message!(Errno::EINVAL, "unknown DRM capability");
        }
    };
    cmd.write(&cap)?;
    Ok(())
}

pub fn handle_get_resources(
    cmd: &Ioctl<0x64, 0xa0, true, InOutData<DrmModeCardRes>>,
) -> Result<()> {
    let mut res = cmd.read()?;
    let count_crtcs = 1;
    let count_encoders = 1;
    let count_connectors = 1;
    let count_fbs = super::fb::get_manager().fb_count() as u32;
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
        current_userspace!().writer(res.connector_id_ptr as Vaddr, 4)?.write_val(&CONNECTOR_ID)?;
    }
    if res.fb_id_ptr != 0 && count_fbs > 0 {
        let fb_ids = super::fb::get_manager().all_fb_ids();
        for (i, &fb_id) in fb_ids.iter().enumerate() {
            current_userspace!().writer((res.fb_id_ptr as Vaddr) + (i as Vaddr * 4), 4)?.write_val(&fb_id)?;
        }
    }
    cmd.write(&res)?;
    Ok(())
}

pub fn handle_get_encoder(cmd: &Ioctl<0x64, 0xa6, true, InOutData<DrmModeGetEncoder>>) -> Result<()> {
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

pub fn handle_get_connector(cmd: &Ioctl<0x64, 0xa7, true, InOutData<DrmModeGetConnector>>) -> Result<()> {
    let mut args = cmd.read()?;
    if args.connector_id != CONNECTOR_ID && args.connector_id != 0 {
        return_errno_with_message!(Errno::ENOENT, "connector not found");
    }
    args.encoder_id = ENCODER_ID;
    args.connector_type = 4; // HDMI-A
    args.connector_type_id = 1;
    args.connection = 1; // CONNECTED
    args.mm_width = 2560;
    args.mm_height = 1600;
    args.subpixel = 0;
    if args.count_encoders > 0 && args.encoders_ptr != 0 {
        current_userspace!().writer(args.encoders_ptr as Vaddr, 4)?.write_val(&ENCODER_ID)?;
    }
    args.count_encoders = 1;
    let default_mode = current_mode();
    if args.count_modes > 0 && args.modes_ptr != 0 {
        current_userspace!().writer(args.modes_ptr as Vaddr, size_of::<DrmModeModeInfo>())?.write_val(&default_mode)?;
    }
    args.count_modes = 1;
    args.count_props = 0;
    cmd.write(&args)?;
    Ok(())
}

pub fn handle_get_crtc(cmd: &Ioctl<0x64, 0xa1, true, InOutData<DrmModeCrtc>>) -> Result<()> {
    let mut args = cmd.read()?;
    if args.crtc_id != CRTC_ID && args.crtc_id != 0 {
        return_errno_with_message!(Errno::ENOENT, "crtc not found");
    }
    args.crtc_id = CRTC_ID;
    // Report the current framebuffer if one is active
    args.fb_id = fb::get_manager().current_fb_id().unwrap_or(0);
    args.x = 0;
    args.y = 0;
    // If a mode was set via SET_CRTC, report mode_valid=1
    if fb::get_manager().current_fb_id().is_some() {
        args.mode_valid = 1;
        args.mode = current_mode();
    } else {
        args.mode_valid = 0;
        args.mode = DrmModeModeInfo::default();
    }
    args.gamma_size = 0;
    cmd.write(&args)?;
    Ok(())
}

pub fn handle_set_crtc(cmd: &Ioctl<0x64, 0xa2, true, InOutData<DrmModeCrtc>>) -> Result<()> {
    let mut args = cmd.read()?;
    if args.crtc_id != CRTC_ID {
        return_errno_with_message!(Errno::ENOENT, "crtc not found");
    }
    if args.x != 0 || args.y != 0 {
        return_errno_with_message!(Errno::EINVAL, "crtc offsets are not supported");
    }
    if args.fb_id != 0 && !super::fb::get_manager().fb_exists(args.fb_id) {
        return_errno_with_message!(Errno::ENOENT, "framebuffer not found");
    }
    if args.count_connectors > 0 {
        if args.set_connectors_ptr == 0 {
            return_errno_with_message!(Errno::EINVAL, "connector array is missing");
        }
        for i in 0..args.count_connectors as usize {
            let connector_id = current_userspace!()
                .reader((args.set_connectors_ptr as Vaddr) + (i as Vaddr * 4), 4)?
                .read_val::<u32>()?;
            if connector_id != CONNECTOR_ID {
                return_errno_with_message!(Errno::ENOENT, "connector not found");
            }
        }
    }
    if args.mode_valid != 0 {
        let current_mode = current_mode();
        if args.mode.hdisplay != current_mode.hdisplay
            || args.mode.vdisplay != current_mode.vdisplay
        {
            return_errno_with_message!(Errno::EINVAL, "display mode is not supported");
        }
    }
    if args.fb_id == 0 {
        fb::get_manager().set_current_fb(None);
    } else {
        fb::get_manager().present_fb(args.fb_id)?;
    }
    ostd::info!("SET_CRTC: crtc={}, fb={}, connectors={}, mode={:?}", args.crtc_id, args.fb_id, args.count_connectors, args.mode);
    cmd.write(&args)?;
    Ok(())
}

pub fn handle_page_flip(cmd: &Ioctl<0x64, 0xb0, true, InOutData<DrmModePageFlip>>) -> Result<()> {
    let args = cmd.read()?;
    if args.crtc_id != CRTC_ID {
        return_errno_with_message!(Errno::ENOENT, "crtc not found");
    }
    if !fb::get_manager().fb_exists(args.fb_id) {
        return_errno_with_message!(Errno::ENOENT, "framebuffer not found");
    }
    ostd::info!("PAGE_FLIP: crtc={}, fb={}, flags={:#x}", args.crtc_id, args.fb_id, args.flags);
    fb::get_manager().present_fb(args.fb_id)?;
    cmd.write(&args)?;
    Ok(())
}
