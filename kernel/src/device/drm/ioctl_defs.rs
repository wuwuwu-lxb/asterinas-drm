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

use crate::{
    prelude::*,
    util::ioctl::{ioc, InOutData, OutData},
};

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
pub(super) type GetVersion = ioc!(DRM_IOCTL_VERSION, 0x64, 0x00, OutData<DrmVersion>);

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
pub(super) type GetCap = ioc!(DRM_IOCTL_GET_CAP, 0x64, 0x09, InOutData<DrmGetCap>);

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
    DRM_IOCTL_MODE_GETRESOURCES,
    0x42,
    0xC0,
    OutData<DrmModeCardRes>
);

// ============================================================================
// DRM 数据结构（与 Linux 兼容的 C 结构体布局）
// ============================================================================

/// DRM 版本信息结构体 — `struct drm_version` in Linux
///
/// 用户空间通过 DRM_IOCTL_VERSION ioctl 获取此结构体。
///
/// Linux 参考：include/uapi/drm/drm.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
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
