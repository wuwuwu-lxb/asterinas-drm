// SPDX-License-Identifier: MPL-2.0

//! DRM ioctl 定义。
//!
//! NR 值和 magic 都与 Linux DRM 协议对齐：
//! - magic = 0x64 ('d')
//! - NR 值与 Linux DRM ioctl 一致

use crate::{prelude::*, util::ioctl::{ioc, InOutData, OutData}};

/// DRM_IOCTL_VERSION (0x6400)
pub(super) type GetVersion = ioc!(Version, 0x64, 0x00, OutData<DrmVersion>);

/// DRM_IOCTL_GET_CAP (0x6409)
pub(super) type GetCap = ioc!(GetCap, 0x64, 0x09, InOutData<DrmGetCap>);

/// DRM_IOCTL_MODE_GETRESOURCES (0xc04064a0, NR=0xa0)
pub(super) type GetResources = ioc!(GetResources, 0x64, 0xa0, InOutData<DrmModeCardRes>);

/// DRM_IOCTL_MODE_GETENCODER (0xc01464a6, NR=0xa6)
pub(super) type GetEncoder = ioc!(GetEncoder, 0x64, 0xa6, InOutData<DrmModeGetEncoder>);

/// DRM_IOCTL_MODE_GETCONNECTOR (0xc05064a7, NR=0xa7)
pub(super) type GetConnector = ioc!(GetConnector, 0x64, 0xa7, InOutData<DrmModeGetConnector>);

/// DRM_IOCTL_MODE_GETCRTC (0xc06864a1, NR=0xa1)
pub(super) type GetCrtc = ioc!(GetCrtc, 0x64, 0xa1, InOutData<DrmModeCrtc>);

/// DRM_IOCTL_MODE_SETCRTC (0xc06864a2, NR=0xa2)
pub(super) type SetCrtc = ioc!(SetCrtc, 0x64, 0xa2, InOutData<DrmModeCrtc>);

/// DRM_IOCTL_MODE_ADDFB (0xc01c64ae, NR=0xae)
pub(super) type AddFb = ioc!(AddFb, 0x64, 0xae, InOutData<DrmModeFbCmd>);

/// DRM_IOCTL_MODE_RMFB (0xc00464af, NR=0xaf)
pub(super) type RmFb = ioc!(RmFb, 0x64, 0xaf, InOutData<DrmModeRmFb>);

/// DRM_IOCTL_MODE_GETFB (NR=0xac)
pub(super) type GetFb = ioc!(GetFb, 0x64, 0xac, InOutData<DrmModeGetFb>);

/// DRM_IOCTL_MODE_PAGE_FLIP (0xc01864b0, NR=0xb0)
pub(super) type PageFlip = ioc!(PageFlip, 0x64, 0xb0, InOutData<DrmModePageFlip>);

// ============================================================================
// Dumb Buffer 数据结构
// ============================================================================

/// DRM_IOCTL_MODE_CREATE_DUMB (0xc02064b2, NR=0xb2)
pub(super) type CreateDumb = ioc!(CreateDumb, 0x64, 0xb2, InOutData<DrmModeCreateDumb>);

/// DRM_IOCTL_MODE_MAP_DUMB (0xc01064b3, NR=0xb3)
pub(super) type MapDumb = ioc!(MapDumb, 0x64, 0xb3, InOutData<DrmModeMapDumb>);

/// DRM_IOCTL_MODE_DESTROY_DUMB (0xc00464b4, NR=0xb4)
pub(super) type DestroyDumb = ioc!(DestroyDumb, 0x64, 0xb4, InOutData<DrmModeDestroyDumb>);

// ============================================================================
// DRM 数据结构 - 与 libdrm 完全一致的内存布局
// ============================================================================

/// DRM 版本信息结构体 (184 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmVersion {
    pub version_major: i32,
    pub version_minor: i32,
    pub version_patchlevel: i32,
    pub name: [u8; 64],
    pub date: [u8; 32],
    pub desc: [u8; 32],
}

impl Default for DrmVersion {
    fn default() -> Self {
        Self {
            version_major: 0,
            version_minor: 0,
            version_patchlevel: 0,
            name: [0u8; 64],
            date: [0u8; 32],
            desc: [0u8; 32],
        }
    }
}

/// DRM 能力查询结构体 (16 bytes)
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmGetCap {
    pub capability: u64,
    pub value: u64,
}

impl DrmGetCap {
    pub const DRM_CAP_DUMB_BUFFER: u64 = 0x1;
    pub const DRM_CAP_VBLANK_HIGH_CRTC: u64 = 0x2;
    pub const DRM_CAP_DUMB_PREFERRED_DEPTH: u64 = 0x3;
}

/// KMS 资源结构体 (64 bytes) - 与 libdrm 完全一致
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeCardRes {
    pub fb_id_ptr: u64,          // 0
    pub crtc_id_ptr: u64,        // 8
    pub connector_id_ptr: u64,   // 16
    pub encoder_id_ptr: u64,     // 24
    pub count_fbs: u32,          // 32
    pub count_crtcs: u32,        // 36
    pub count_connectors: u32,   // 40
    pub count_encoders: u32,      // 44
    pub min_width: u32,           // 48
    pub max_width: u32,           // 52
    pub min_height: u32,          // 56
    pub max_height: u32,          // 60
}

/// DRM 显示模式结构体 (68 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeModeInfo {
    pub clock: u32,
    pub hdisplay: u16,
    pub hsync_start: u16,
    pub hsync_end: u16,
    pub htotal: u16,
    pub hskew: u16,
    pub vdisplay: u16,
    pub vsync_start: u16,
    pub vsync_end: u16,
    pub vtotal: u16,
    pub vscan: u16,
    pub vrefresh: u32,
    pub flags: u32,
    pub type_: u32,
    pub name: [u8; 32],
}

impl DrmModeModeInfo {
    pub const fn new(width: u16, height: u16, refresh: u32) -> Self {
        let clock = (width as u32 * height as u32 * refresh as u32 / 1000) as u32;
        Self {
            clock,
            hdisplay: width,
            hsync_start: width + 48,
            hsync_end: width + 88,
            htotal: width + 168,
            hskew: 0,
            vdisplay: height,
            vsync_start: height + 3,
            vsync_end: height + 6,
            vtotal: height + 29,
            vscan: 0,
            vrefresh: refresh,
            flags: 0x20,
            type_: 0,
            name: *b"1024x768\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        }
    }
}

impl Default for DrmModeModeInfo {
    fn default() -> Self {
        Self {
            clock: 0,
            hdisplay: 0,
            hsync_start: 0,
            hsync_end: 0,
            htotal: 0,
            hskew: 0,
            vdisplay: 0,
            vsync_start: 0,
            vsync_end: 0,
            vtotal: 0,
            vscan: 0,
            vrefresh: 0,
            flags: 0,
            type_: 0,
            name: [0u8; 32],
        }
    }
}

/// DRM Encoder 信息结构体 (20 bytes)
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeGetEncoder {
    pub encoder_id: u32,         // 0
    pub encoder_type: u32,       // 4
    pub crtc_id: u32,            // 8
    pub possible_crtcs: u32,     // 12
    pub possible_clones: u32,    // 16
}

impl DrmModeGetEncoder {
    pub const DRM_MODE_ENCODER_DAC: u32 = 0;
    pub const DRM_MODE_ENCODER_TVDAC: u32 = 1;
    pub const DRM_MODE_ENCODER_LVDS: u32 = 2;
    pub const DRM_MODE_ENCODER_TMDS: u32 = 3;
    pub const DRM_MODE_ENCODER_VIRTUAL: u32 = 4;
    pub const DRM_MODE_ENCODER_DPI: u32 = 5;
}

/// DRM Connector 信息结构体 (80 bytes) - 与 libdrm 完全一致
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeGetConnector {
    pub encoders_ptr: u64,       // 0
    pub modes_ptr: u64,          // 8
    pub props_ptr: u64,          // 16
    pub prop_values_ptr: u64,    // 24
    pub count_modes: u32,        // 32
    pub count_props: u32,        // 36
    pub count_encoders: u32,     // 40
    pub encoder_id: u32,         // 44 (OUTPUT: 当前连接的 encoder)
    pub connector_id: u32,       // 48 (INPUT: 查询哪个 connector)
    pub connector_type: u32,     // 52
    pub connector_type_id: u32,  // 56
    pub connection: u32,         // 60 (OUTPUT)
    pub mm_width: u32,           // 64
    pub mm_height: u32,          // 68
    pub subpixel: u32,           // 72
    pub pad: [u8; 4],            // 76 (padding to reach 80 bytes)
}

impl Default for DrmModeGetConnector {
    fn default() -> Self {
        Self {
            encoders_ptr: 0,
            modes_ptr: 0,
            props_ptr: 0,
            prop_values_ptr: 0,
            count_modes: 0,
            count_props: 0,
            count_encoders: 0,
            encoder_id: 0,
            connector_id: 0,
            connector_type: 0,
            connector_type_id: 0,
            connection: 0,
            mm_width: 0,
            mm_height: 0,
            subpixel: 0,
            pad: [0u8; 4],
        }
    }
}

impl DrmModeGetConnector {
    pub const DRM_MODE_CONNECTED: u32 = 1;
    pub const DRM_MODE_DISCONNECTED: u32 = 2;
    pub const DRM_MODE_UNKNOWNCONNECTION: u32 = 3;
}

/// DRM CRTC 信息结构体 (104 bytes) - 与 libdrm 完全一致
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeCrtc {
    pub set_connectors_ptr: u64,  // 0
    pub count_connectors: u32,    // 8
    pub crtc_id: u32,             // 12
    pub fb_id: u32,               // 16
    pub x: u32,                   // 20
    pub y: u32,                   // 24
    pub gamma_size: u32,          // 28
    pub mode_valid: u32,          // 32
    pub mode: DrmModeModeInfo,    // 36
}

impl DrmModeCrtc {
    pub const fn new() -> Self {
        Self {
            set_connectors_ptr: 0,
            count_connectors: 0,
            crtc_id: 0,
            fb_id: 0,
            x: 0,
            y: 0,
            gamma_size: 0,
            mode_valid: 0,
            mode: DrmModeModeInfo {
                clock: 0,
                hdisplay: 0,
                hsync_start: 0,
                hsync_end: 0,
                htotal: 0,
                hskew: 0,
                vdisplay: 0,
                vsync_start: 0,
                vsync_end: 0,
                vtotal: 0,
                vscan: 0,
                vrefresh: 0,
                flags: 0,
                type_: 0,
                name: [0u8; 32],
            },
        }
    }
}

/// DRM Framebuffer 创建结构体 (28 bytes) - 与 libdrm 完全一致
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeFbCmd {
    pub fb_id: u32,      // 0
    pub width: u32,      // 4
    pub height: u32,     // 8
    pub pitch: u32,      // 12
    pub bpp: u32,        // 16
    pub depth: u32,      // 20
    pub handle: u32,     // 24
}

impl DrmModeFbCmd {
    pub const fn new() -> Self {
        Self {
            fb_id: 0,
            width: 0,
            height: 0,
            pitch: 0,
            bpp: 0,
            depth: 0,
            handle: 0,
        }
    }
}

/// DRM RMFB 结构体 (4 bytes)
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeRmFb {
    pub fb_id: u32,  // 0
}

/// DRM GETFB 结构体 (36 bytes)
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeGetFb {
    pub fb_id: u32,   // 0
    pub width: u32,   // 4
    pub height: u32,  // 8
    pub pitch: u32,   // 12
    pub bpp: u32,     // 16
    pub depth: u32,   // 20
    pub handle: u32,  // 24
    pub pad: [u8; 4], // 28 (padding before offset)
    pub offset: u64,  // 32
}

/// DRM PAGE_FLIP 结构体 (32 bytes)
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModePageFlip {
    pub crtc_id: u32,   // 0
    pub fb_id: u32,     // 4
    pub flags: u32,     // 8
    pub sequence: u32,  // 12
    pub user_data: u64, // 16
    pub reserved: u64,   // 24
}

// ============================================================================
// Dumb Buffer 数据结构
// ============================================================================

/// Dumb Buffer 创建结构体 (32 bytes) - 与 libdrm 完全一致
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeCreateDumb {
    pub height: u32,   // 0
    pub width: u32,   // 4
    pub bpp: u32,     // 8
    pub flags: u32,   // 12
    pub handle: u32,  // 16
    pub pitch: u32,   // 20
    pub size: u64,    // 24
}

/// Dumb Buffer 映射结构体 (16 bytes) - 与 libdrm 完全一致
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeMapDumb {
    pub handle: u32,  // 0
    pub pad: u32,     // 4
    pub offset: u64,  // 8
}

/// Dumb Buffer 销毁结构体 (4 bytes)
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeDestroyDumb {
    pub handle: u32,  // 0
}
