// SPDX-License-Identifier: MPL-2.0

//! DRM ioctl 定义。
//!
//! DRM ioctl 编号遵循 Linux DRM 编码规则：
//! - DRM_IOCTL_BASE = 'd' = 0x64
//! - 方向（_IOC_DIR）：READ = 2, WRITE = 1, NONE = 0
//! - 大小（_IOC_SIZE）：参数数据结构体的字节数
//!
//! 编码公式：cmd = (dir << 30) | (magic << 8) | (nr << 0) | (size << 16)
//!
//! 参考：Linux 内核 include/uapi/drm/drm.h

use crate::{prelude::*, util::ioctl::{ioc, InOutData, OutData}};

/// DRM ioctl 使用说明：
/// - DRM 核心 ioctl 使用 magic = 0x64 ('d')
/// - 模式相关 ioctl 使用 magic = 0x42 (0x40 + 2)

/// DRM_IOCTL_VERSION (0x6400)
///
/// 查询驱动版本信息。
///
/// Linux 定义：
/// ```c
/// #define DRM_IOCTL_VERSION \
///     _IOC(_IOC_READ | _IOC_WRITE, DRM_IOCTL_BASE, 0x00, \
///          sizeof(struct drm_version))
/// ```
pub(super) type GetVersion = ioc!(Version, 0x64, 0x00, OutData<DrmVersion>);

/// DRM_IOCTL_GET_CAP (0x6409)
///
/// 查询设备能力。
///
/// Linux 定义：
/// ```c
/// #define DRM_IOCTL_GET_CAP \
///     _IOC(_IOC_READ | _IOC_WRITE, DRM_IOCTL_BASE, 0x09, \
///          sizeof(struct drm_get_cap))
/// ```
pub(super) type GetCap = ioc!(GetCap, 0x64, 0x09, InOutData<DrmGetCap>);

/// DRM_IOCTL_MODE_GETRESOURCES (0x42C0)
///
/// 查询 KMS 资源（CRTC、Encoder、Connector、Framebuffer）数量和分辨率范围。
///
/// Linux 定义：
/// ```c
/// #define DRM_IOCTL_MODE_GETRESOURCES \
///     _IOC(_IOC_READ, 0x40 + 2, 0xC0, sizeof(struct drm_mode_card_res))
/// ```
pub(super) type GetResources = ioc!(
    GetResources,
    0x42,
    0xC0,
    InOutData<DrmModeCardRes>
);

/// DRM_IOCTL_MODE_GETENCODER (0x42C5)
///
/// 获取 Encoder 信息。
///
/// Linux 定义：
/// ```c
/// #define DRM_IOCTL_MODE_GETENCODER \
///     _IOC(_IOC_READ, 0x40 + 2, 0x05, sizeof(struct drm_mode_get_encoder))
/// ```
pub(super) type GetEncoder = ioc!(
    GetEncoder,
    0x42,
    0xC5,
    InOutData<DrmModeGetEncoder>
);

/// DRM_IOCTL_MODE_GETCONNECTOR (0x42C6)
///
/// 获取 Connector 信息。
///
/// Linux 定义：
/// ```c
/// #define DRM_IOCTL_MODE_GETCONNECTOR \
///     _IOC(_IOC_READ, 0x40 + 2, 0x06, sizeof(struct drm_mode_get_connector))
/// ```
pub(super) type GetConnector = ioc!(
    GetConnector,
    0x42,
    0xC6,
    InOutData<DrmModeGetConnector>
);

/// DRM_IOCTL_MODE_GETCRTC (0x42C1)
///
/// 获取 CRTC 信息。
///
/// Linux 定义：
/// ```c
/// #define DRM_IOCTL_MODE_GETCRTC \
///     _IOC(_IOC_READ, 0x40 + 2, 0x01, sizeof(struct drm_mode_crtc))
/// ```
pub(super) type GetCrtc = ioc!(
    GetCrtc,
    0x42,
    0xC1,
    InOutData<DrmModeCrtc>
);

/// DRM_IOCTL_MODE_SETCRTC (0x42C0)
///
/// 设置 CRTC 配置。
pub(super) type SetCrtc = ioc!(
    SetCrtc,
    0x42,
    0xC0,
    InOutData<DrmModeCrtc>
);

/// DRM_IOCTL_MODE_ADDFB (0x42C8)
///
/// 创建 Framebuffer。
pub(super) type AddFb = ioc!(
    AddFb,
    0x42,
    0xC8,
    InOutData<DrmModeFbCmd>
);

/// DRM_IOCTL_MODE_RMFB (0x42C9)
///
/// 删除 Framebuffer。
pub(super) type RmFb = ioc!(
    RmFb,
    0x42,
    0xC9,
    InOutData<DrmModeRmFb>
);

/// DRM_IOCTL_MODE_GETFB (0x42CA)
///
/// 获取 Framebuffer 信息。
pub(super) type GetFb = ioc!(
    GetFb,
    0x42,
    0xCA,
    InOutData<DrmModeGetFb>
);

// ============================================================================
// Dumb Buffer 数据结构（与 Linux 兼容的 C 结构体布局）
// ============================================================================

/// DRM_IOCTL_MODE_CREATE_DUMB (0xc0064b2)
///
/// 分配一个 Dumb Buffer。
///
/// Linux 定义：
/// ```c
/// #define DRM_IOCTL_MODE_CREATE_DUMB \
///     _IOC(_IOC_READ | _IOC_WRITE, 0x40 + 2, 0x02, sizeof(struct drm_mode_create_dumb))
/// ```
pub(super) type CreateDumb = ioc!(CreateDumb, 0x42, 0x02, InOutData<DrmModeCreateDumb>);

/// DRM_IOCTL_MODE_MAP_DUMB (0xc01064b3)
///
/// 获取 Dumb Buffer 的 mmap 偏移量。
///
/// Linux 定义：
/// ```c
/// #define DRM_IOCTL_MODE_MAP_DUMB \
///     _IOC(_IOC_READ | _IOC_WRITE, 0x40 + 2, 0x03, sizeof(struct drm_mode_map_dumb))
/// ```
pub(super) type MapDumb = ioc!(MapDumb, 0x42, 0x03, InOutData<DrmModeMapDumb>);

/// DRM_IOCTL_MODE_DESTROY_DUMB (0xc00464b4)
///
/// 销毁一个 Dumb Buffer。
///
/// Linux 定义：
/// ```c
/// #define DRM_IOCTL_MODE_DESTROY_DUMB \
///     _IOC(_IOC_WRITE, 0x40 + 2, 0x04, sizeof(struct drm_mode_destroy_dumb))
/// ```
pub(super) type DestroyDumb = ioc!(DestroyDumb, 0x42, 0x04, InOutData<DrmModeDestroyDumb>);

/// Dumb Buffer 创建结构体 — `struct drm_mode_create_dumb` in Linux
///
/// 用户空间通过 DRM_IOCTL_MODE_CREATE_DUMB ioctl 创建 Dumb Buffer。
///
/// Linux 参考：include/uapi/drm/drm_mode.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeCreateDumb {
    /// 请求的高度（像素）
    pub height: u32,
    /// 请求的宽度（像素）
    pub width: u32,
    /// 每像素位数
    pub bpp: u32,
    /// 标志（未使用，设为 0）
    pub flags: u32,
    /// 返回：GEM handle
    pub handle: u32,
    /// 返回：每行字节数（pitch）
    pub pitch: u32,
    /// 返回：缓冲区大小（字节）
    pub size: u64,
}

/// Dumb Buffer 映射结构体 — `struct drm_mode_map_dumb` in Linux
///
/// 用户空间通过 DRM_IOCTL_MODE_MAP_DUMB ioctl 获取 mmap 偏移量。
///
/// Linux 参考：include/uapi/drm/drm_mode.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeMapDumb {
    /// Buffer 的 GEM handle
    pub handle: u32,
    /// 填充字段（设为 0）
    pub pad: u32,
    /// 返回：mmap 偏移量
    pub offset: u64,
}

/// Dumb Buffer 销毁结构体 — `struct drm_mode_destroy_dumb` in Linux
///
/// 用户空间通过 DRM_IOCTL_MODE_DESTROY_DUMB ioctl 销毁 Dumb Buffer。
///
/// Linux 参考：include/uapi/drm/drm_mode.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeDestroyDumb {
    /// Buffer 的 GEM handle
    pub handle: u32,
}

// ============================================================================
// DRM 数据结构（与 Linux 兼容的 C 结构体布局）
// ============================================================================

/// DRM 版本信息结构体 — `struct drm_version` in Linux
///
/// 用户空间通过 DRM_IOCTL_VERSION ioctl 获取此结构体。
///
/// Linux 参考：include/uapi/drm/drm.h
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmVersion {
    /// 驱动主版本号
    pub version_major: i32,
    /// 驱动次版本号
    pub version_minor: i32,
    /// 驱动补丁级别
    pub version_patchlevel: i32,
    /// 驱动名称（以 null 结尾的字符串）
    pub name: [u8; 64],
    /// 驱动构建日期（YYYYMMDD）
    pub date: [u8; 32],
    /// 驱动描述
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

/// DRM 能力查询结构体 — `struct drm_get_cap` in Linux
///
/// 用户空间通过 DRM_IOCTL_GET_CAP ioctl 查询设备能力。
///
/// Linux 参考：include/uapi/drm/drm.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmGetCap {
    /// 能力类型（DRM_CAP_* 常量）
    pub capability: u64,
    /// 能力值
    pub value: u64,
}

/// DRM 能力常量 — DRM_CAP_* in Linux
impl DrmGetCap {
    /// 支持 Dumb Buffer（用户空间可分配的简单像素缓冲区）
    pub const DRM_CAP_DUMB_BUFFER: u64 = 0x1;
    /// Vblank 高 CRTC 编号支持
    pub const DRM_CAP_VBLANK_HIGH_CRTC: u64 = 0x2;
    /// Dumb Buffer 首选色深
    pub const DRM_CAP_DUMB_PREFERRED_DEPTH: u64 = 0x3;
}

/// KMS 资源结构体 — `struct drm_mode_card_res` in Linux
///
/// 用户空间通过 DRM_IOCTL_MODE_GETRESOURCES ioctl 获取 KMS 对象信息。
/// Phase 3 MVP 返回硬编码的简化值：1 个 CRTC + 1 个 Encoder + 1 个 Connector。
///
/// Linux 参考：include/uapi/drm/drm_mode.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeCardRes {
    /// Framebuffer ID 数组指针（用户空间地址）
    pub fb_id_ptr: u64,
    /// CRTC ID 数组指针（用户空间地址）
    pub crtc_id_ptr: u64,
    /// Connector ID 数组指针（用户空间地址）
    pub connector_id_ptr: u64,
    /// Encoder ID 数组指针（用户空间地址）
    pub encoder_id_ptr: u64,
    /// Framebuffer 数量
    pub count_fbs: u32,
    /// CRTC 数量
    pub count_crtcs: u32,
    /// Connector 数量
    pub count_connectors: u32,
    /// Encoder 数量
    pub count_encoders: u32,
    /// 支持的最小宽度（像素）
    pub min_width: u32,
    /// 支持的最大宽度（像素）
    pub max_width: u32,
    /// 支持的最小高度（像素）
    pub min_height: u32,
    /// 支持的最大高度（像素）
    pub max_height: u32,
}

// ============================================================================
// KMS 数据结构
// ============================================================================

/// DRM 显示模式结构体 — `struct drm_mode_modeinfo` in Linux
///
/// 描述一个显示模式（分辨率、刷新率等）。
///
/// Linux 参考：include/uapi/drm/drm_mode.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeModeInfo {
    /// 像素时钟（kHz）
    pub clock: u32,
    /// 水平显示
    pub hdisplay: u16,
    /// 水平同步开始
    pub hsync_start: u16,
    /// 水平同步结束
    pub hsync_end: u16,
    /// 水平总像素
    pub htotal: u16,
    /// 水平偏斜
    pub hskew: u16,
    /// 垂直显示
    pub vdisplay: u16,
    /// 垂直同步开始
    pub vsync_start: u16,
    /// 垂直同步结束
    pub vsync_end: u16,
    /// 垂直总行
    pub vtotal: u16,
    /// 垂直扫描
    pub vscan: u16,
    /// 垂直刷新率（Hz）
    pub vrefresh: u32,
    /// 标志位
    pub flags: u32,
    /// 模式名称
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
            name: *b"1024x768\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        }
    }
}

/// DRM Encoder 信息结构体 — `struct drm_mode_get_encoder` in Linux
///
/// Linux 参考：include/uapi/drm/drm_mode.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeGetEncoder {
    /// Encoder ID
    pub encoder_id: u32,
    /// Encoder 类型（DRM_MODE_ENCODER_*）
    pub encoder_type: u32,
    /// 当前关联的 CRTC ID
    pub crtc_id: u32,
    /// 可能连接的 CRTC 位掩码
    pub possible_crtcs: u32,
    /// 可能克隆的 Encoder 位掩码
    pub possible_clones: u32,
}

/// DRM Encoder 类型常量
impl DrmModeGetEncoder {
    pub const DRM_MODE_ENCODER_DAC: u32 = 0;
    pub const DRM_MODE_ENCODER_TVDAC: u32 = 1;
    pub const DRM_MODE_ENCODER_LVDS: u32 = 2;
    pub const DRM_MODE_ENCODER_TMDS: u32 = 3;
    pub const DRM_MODE_ENCODER_VIRTUAL: u32 = 4;
    pub const DRM_MODE_ENCODER_DPI: u32 = 5;
}

/// DRM Connector 信息结构体 — `struct drm_mode_get_connector` in Linux
///
/// Linux 参考：include/uapi/drm/drm_mode.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeGetConnector {
    /// Connector ID
    pub connector_id: u64,
    /// Encoder ID 数组指针
    pub encoders_ptr: u64,
    /// Mode 数组指针
    pub modes_ptr: u64,
    /// Property 数组指针
    pub props_ptr: u64,
    /// Property values 数组指针
    pub prop_values_ptr: u64,
    /// Connector ID (u32 version)
    pub connector_id_val: u32,
    /// 当前关联的 Encoder ID
    pub encoder_id: u32,
    /// 连接状态（0=断开, 1=连接, 2=未知）
    pub connection: u32,
    /// Connector 类型（DRM_MODE_CONNECTOR_*）
    pub connector_type: u32,
    /// Connector 类型 ID
    pub connector_type_id: u32,
    /// 支持的模式数量
    pub count_modes: u32,
    /// 属性数量
    pub count_props: u32,
    /// 可用 Encoder 数量
    pub count_encoders: u32,
}

/// DRM Connector 类型常量
impl DrmModeGetConnector {
    pub const DRM_MODE_CONNECTOR_Unknown: u32 = 0;
    pub const DRM_MODE_CONNECTOR_VGA: u32 = 1;
    pub const DRM_MODE_CONNECTOR_DVII: u32 = 2;
    pub const DRM_MODE_CONNECTOR_DVID: u32 = 3;
    pub const DRM_MODE_CONNECTOR_DVIA: u32 = 4;
    pub const DRM_MODE_CONNECTOR_Composite: u32 = 5;
    pub const DRM_MODE_CONNECTOR_SVIDEO: u32 = 6;
    pub const DRM_MODE_CONNECTOR_LVDS: u32 = 7;
    pub const DRM_MODE_CONNECTOR_Component: u32 = 8;
    pub const DRM_MODE_CONNECTOR_9PinDIN: u32 = 9;
    pub const DRM_MODE_CONNECTOR_DisplayPort: u32 = 10;
    pub const DRM_MODE_CONNECTOR_HDMIA: u32 = 11;
    pub const DRM_MODE_CONNECTOR_HDMIB: u32 = 12;
    pub const DRM_MODE_CONNECTOR_TV: u32 = 13;
    pub const DRM_MODE_CONNECTOR_E_DP: u32 = 14;
    pub const DRM_MODE_CONNECTOR_VIRTUAL: u32 = 15;
}

/// DRM Connector 连接状态
impl DrmModeGetConnector {
    pub const DRM_MODE_CONNECTED: u32 = 1;
    pub const DRM_MODE_DISCONNECTED: u32 = 2;
    pub const DRM_MODE_UNKNOWNCONNECTION: u32 = 3;
}

/// DRM CRTC 信息结构体 — `struct drm_mode_crtc` in Linux
///
/// Linux 参考：include/uapi/drm/drm_mode.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeCrtc {
    /// CRTC ID
    pub crtc_id: u32,
    /// 关联的 Framebuffer ID
    pub fb_id: u32,
    /// 显示区域 X 偏移
    pub x: u32,
    /// 显示区域 Y 偏移
    pub y: u32,
    /// mode 是否有效
    pub mode_valid: u32,
    /// 显示模式
    pub mode: DrmModeModeInfo,
    /// 模式数量
    pub count_modes: u32,
    /// X1
    pub x1: u32,
    /// Y1
    pub y1: u32,
    /// gamma size
    pub gamma_size: u32,
}

/// DRM Framebuffer 创建结构体 — `struct drm_mode_fb_cmd2` in Linux
///
/// 用于 ADDFB2 ioctl（支持更多参数）。
///
/// Linux 参考：include/uapi/drm/drm_mode.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeFbCmd2 {
    /// Framebuffer ID（输出）
    pub fb_id: u32,
    /// 宽度
    pub width: u32,
    /// 高度
    pub height: u32,
    /// 每像素位数
    pub bpp: u32,
    /// 标志
    pub flags: u32,
    /// 像素格式
    pub pixel_format: u32,
    /// 缓冲区句柄
    pub handles: [u32; 4],
    /// 偏移
    pub pitches: [u32; 4],
    /// 起始偏移
    pub offsets: [u64; 4],
    /// 修饰符
    pub modifier: [u64; 4],
}

/// DRM Framebuffer 简单创建结构体 — `struct drm_mode_fb_cmd` in Linux
///
/// 用于 ADDFB ioctl（简化版本）。
///
/// Linux 参考：include/uapi/drm/drm_mode.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeFbCmd {
    /// Framebuffer ID（输出）
    pub fb_id: u32,
    /// 宽度
    pub width: u32,
    /// 高度
    pub height: u32,
    /// 格式
    pub format: u32,
    /// pitch
    pub pitch: u32,
    /// Buffer 句柄
    pub handle: u32,
    /// 深度
    pub depth: u32,
    /// bpp
    pub bpp: u32,
    /// 偏移
    pub offset: u64,
    /// 大小
    pub size: u64,
}

/// DRM RMFB 结构体 — `struct drm_mode_fb` in Linux
///
/// Linux 参考：include/uapi/drm/drm_mode.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeRmFb {
    /// 要删除的 Framebuffer ID
    pub fb_id: u32,
}

/// DRM GETFB 结构体 — `struct drm_mode_fb` in Linux
///
/// Linux 参考：include/uapi/drm/drm_mode.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmModeGetFb {
    /// Framebuffer ID
    pub fb_id: u32,
    /// 宽度
    pub width: u32,
    /// 高度
    pub height: u32,
    /// 每像素位数
    pub bpp: u32,
    /// 扫描线宽度
    pub pitch: u32,
    /// Buffer 句柄
    pub handle: u32,
    /// 缓冲区偏移
    pub offset: u64,
}

/// Framebuffer ID 起始值
pub const FB_ID_START: u32 = 1;
