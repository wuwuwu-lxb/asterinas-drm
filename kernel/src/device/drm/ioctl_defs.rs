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

/// DRM_IOCTL_MODE_GETRESOURCES (0x40C0)
///
/// 查询 KMS 资源（CRTC、Encoder、Connector、Framebuffer）数量和分辨率范围。
/// 注意：这是模式相关 ioctl，使用 magic = 0x40 + 2 = 0x42。
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
    OutData<DrmModeCardRes>
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
