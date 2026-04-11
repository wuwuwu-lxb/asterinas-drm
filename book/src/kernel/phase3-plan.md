# Phase 3：/dev/dri/card0 DRM 字符设备实现

## 背景

aster-drm 项目在 `drm` 分支已完成 Phase 1-2（SimpleDrm 双缓冲组件，编译通过）。Phase 1-2 在 `kernel/comps/drm/` 实现了渲染相关的 `SimpleDrm` 组件，但该组件是内核内部组件，并未暴露用户空间接口。

Framebuffer 显示问题根因已查明：GRUB 在 multiboot2 引导协议下**不会**传递 framebuffer tag 给内核，导致 `FRAMEBUFFER.get()` 返回 `None`，因此 `/dev/fb0` 和 `SimpleDrm::new()` 都无法正常工作。这个问题短期内无法解决（需要 OVMF GOP 驱动或 virtio-gpu 设备驱动）。

用户决定**跳过** framebuffer 显示问题，直接进入 Phase 3：在 `kernel/src/device/` 创建标准 DRM 字符设备 `/dev/dri/card0`，暴露符合 Linux DRM 标准的 ioctl 接口。

---

## Phase 3 目标

在 `kernel/src/device/drm/` 创建完整的 DRM 字符设备模块，实现：

| ioctl | 编号 | 功能 |
|-------|------|------|
| `DRM_IOCTL_VERSION` | 0x6400 | 返回驱动版本信息 |
| `DRM_IOCTL_GET_CAP` | 0x6409 | 返回设备能力（如 DUMB_BUFFER=1） |
| `DRM_IOCTL_MODE_GETRESOURCES` | 0x40C0 | 返回 KMS 对象（CRTC/Encoder/Connector）计数和分辨率范围 |

这三个 ioctl 是 DRM 设备的最基础接口，任何符合标准的 DRM 客户端（如 libdrm 工具）都会首先调用这些 ioctl 来探测设备能力。

---

## 架构概览

### 文件布局

```
kernel/src/device/
├── mod.rs              ← 修改：添加 pub mod drm; 并在 init_in_first_kthread() 调用 drm::init()
└── drm/                ← 新建完整模块
    ├── mod.rs          ← Drm 设备结构体，实现 Device trait
    ├── file.rs         ← DrmFileHandle，实现 FileIo + InodeIo + Pollable
    ├── ioctl_defs.rs   ← DRM ioctl 类型别名定义（使用 ioc! 宏）
    ├── kms.rs          ← 简化 KMS 模型（1 CRTC + 1 Encoder + 1 Connector）
    └── init.rs         ← init_in_first_kthread() 设备注册入口
```

### 与 fb.rs 的模式对比

`kernel/src/device/fb.rs` 是完整的参考实现（554 行），Phase 3 的 DRM 设备遵循完全相同的模式：

| 层级 | fb.rs | drm.rs |
|------|-------|--------|
| 设备结构 | `Fb` (实现 `Device` trait) | `Drm` (实现 `Device` trait) |
| 文件句柄 | `FbHandle` (实现 `FileIo` + `InodeIo` + `Pollable`) | `DrmFileHandle` (实现 `FileIo` + `InodeIo` + `Pollable`) |
| ioctl 定义 | `mod ioctl_defs` (使用 `ioc!` 宏) | `mod ioctl_defs` (使用 `ioc!` 宏) |
| 初始化入口 | `pub(super) fn init_in_first_kthread()` | `pub(super) fn init_in_first_kthread()` |
| 设备号 | major 29 (帧缓冲) | **major 226** (Linux 标准 DRM) |
| devtmpfs 路径 | `"fb0"` | `"dri/card0"` |

---

## 详细实现

### 1. 修改 `kernel/src/device/mod.rs`

在 `init_in_first_kthread()` 函数中添加 `drm::init_in_first_kthread()` 调用：

```rust
// kernel/src/device/mod.rs 第 119-125 行

pub fn init_in_first_kthread() {
    registry::init_in_first_kthread();
    mem::init_in_first_kthread();
    misc::init_in_first_kthread();
    evdev::init_in_first_kthread();
    fb::init_in_first_kthread();
    drm::init_in_first_kthread();  // ← 添加这一行
}
```

并在文件开头添加模块声明：

```rust
// kernel/src/device/mod.rs 第 3 行附近
mod evdev;
mod fb;
mod mem;
pub mod misc;
mod pty;
pub mod registry;
pub mod drm;   // ← 添加这一行（如果还没有的话）
mod shm;
pub mod tty;
```

---

### 2. 新建 `kernel/src/device/drm/mod.rs` — DRM 设备结构体

这是设备的核心结构体，实现 `Device` trait：

```rust
// SPDX-License-Identifier: MPL-2.0

//! DRM 字符设备实现。

use alloc::sync::Arc;
use device_id::{DeviceId, MajorId, MinorId};

use super::{Device, DeviceType, registry::char};
use crate::fs::file::FileIo;

/// DRM 设备结构体 — 对应 /dev/dri/card0
///
/// 该结构体在 init_in_first_kthread() 中被注册到字符设备注册表。
/// 用户空间 open("/dev/dri/card0") 时，Device::open() 返回 DrmFileHandle。
#[derive(Debug)]
struct Drm;

/// Drm 文件句柄 — 用户空间与 DRM 设备的每次 open() 对应一个实例
///
/// 每个 DrmFileHandle 持有独立的文件状态，但 KMS 对象（CRTC/Encoder/Connector）
/// 是全局共享的（在 kms.rs 中定义）。
#[derive(Debug)]
pub(super) struct DrmFileHandle {
    // Phase 3 MVP 暂时不在文件句柄中存储状态。
    // Phase 4 会在这里加入 GEM handle 表等。
}

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
```

**关键设计决策：**
- `DrmFileHandle` 在 Phase 3 是无状态的，因为 MVP 只支持只读查询 ioctl
- major 226 是 Linux DRM 的标准设备号，不能用其他值
- `devtmpfs_path = "dri/card0"` 确保设备节点路径符合 Linux 惯例

---

### 3. 新建 `kernel/src/device/drm/file.rs` — 文件句柄和 ioctl 分发

文件句柄负责处理 `ioctl()` 系统调用。参考 fb.rs 的 `FbHandle`：

```rust
// SPDX-License-Identifier: MPL-2.0

//! DRM 文件句柄实现。

use alloc::sync::Arc;

use crate::{
    current_userspace,
    events::IoEvents,
    fs::{
        file::{FileIo, Mappable, StatusFlags},
        vfs::inode::InodeIo,
    },
    prelude::*,
    process::signal::{PollHandle, Pollable},
    util::ioctl::{RawIoctl, dispatch_ioctl},
};

use super::kms;
use super::ioctl_defs;

/// DRM 文件句柄
///
/// 每个用户空间进程 open("/dev/dri/card0") 都会创建一个独立的 DrmFileHandle 实例。
/// 该结构体实现了 FileIo trait，处理所有 DRM ioctl 命令。
#[derive(Debug)]
pub(super) struct DrmFileHandle;

impl Pollable for DrmFileHandle {
    fn poll(&self, mask: IoEvents, _poller: Option<&mut PollHandle>) -> IoEvents {
        // DRM 设备支持 poll，用于 eventfd 风格的事件通知
        // Phase 3 MVP 暂时不支持 DRM events，返回空事件
        IoEvents::empty() & mask
    }
}

impl InodeIo for DrmFileHandle {
    fn read_at(
        &self,
        _offset: usize,
        _writer: &mut VmWriter,
        _status_flags: StatusFlags,
    ) -> Result<usize> {
        // DRM 字符设备不支持 read()，返回 ENOTTY
        return_errno_with_message!(Errno::ENOTTY, "DRM device does not support read")
    }

    fn write_at(
        &self,
        _offset: usize,
        _reader: &mut VmReader,
        _status_flags: StatusFlags,
    ) -> Result<usize> {
        // DRM 字符设备不支持 write()，返回 ENOTTY
        return_errno_with_message!(Errno::ENOTTY, "DRM device does not support write")
    }
}

impl FileIo for DrmFileHandle {
    fn check_seekable(&self) -> Result<()> {
        Ok(())
    }

    fn is_offset_aware(&self) -> bool {
        true
    }

    fn mappable(&self) -> Result<Mappable> {
        // Phase 3 不支持 mmap（属于 Phase 4 的 DUMB_BUFFER 范畴）
        return_errno_with_message!(Errno::ENODEV, "mmap is not supported in Phase 3")
    }

    fn ioctl(&self, raw_ioctl: RawIoctl) -> Result<i32> {
        use ioctl_defs::*;

        dispatch_ioctl!(match raw_ioctl {
            // DRM_IOCTL_VERSION (0x6400) — 返回驱动版本
            cmd @ GetVersion => {
                kms::handle_version(cmd)?;
                Ok(0)
            }
            // DRM_IOCTL_GET_CAP (0x6409) — 返回设备能力
            cmd @ GetCap => {
                kms::handle_get_cap(cmd)?;
                Ok(0)
            }
            // DRM_IOCTL_MODE_GETRESOURCES (0x40C0) — 返回 KMS 对象信息
            cmd @ GetResources => {
                kms::handle_get_resources(cmd)?;
                Ok(0)
            }
            _ => {
                log::debug!(
                    "unknown DRM ioctl: {:#x}",
                    raw_ioctl.cmd()
                );
                return_errno_with_message!(Errno::ENOTTY, "unsupported DRM ioctl")
            }
        })
    }
}
```

**参考 fb.rs 的 FbHandle::ioctl() 模式（第 487-545 行）：**
- `dispatch_ioctl!` 宏会自动展开为类型安全的 ioctl 分发
- `cmd @ GetVersion` 语法将匹配到的 ioctl 绑定到 `cmd` 变量
- `cmd.read()` 从用户空间读取输入数据（如果是 InData 类型）
- `cmd.write(&val)` 向用户空间写入输出数据（如果是 OutData 类型）

---

### 4. 新建 `kernel/src/device/drm/ioctl_defs.rs` — DRM ioctl 类型定义

使用 `ioc!` 宏定义 DRM ioctl 类型。这是 ioctl 系统的核心——每个 ioctl 编号对应一个类型别名：

```rust
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

use crate::util::ioctl::{ioc, InData, OutData};

/// DRM 魔法数字 = 'd' = 0x64
///
/// 所有 DRM 核心 ioctl 都使用 'd' 作为魔法数字。
/// 模式相关 ioctl（如 DRM_IOCTL_MODE_*）使用 0x40 + 2 = 0x42 作为魔法数字。
const DRM_IOCTL_BASE: u8 = b'd';  // 0x64

/// DRM_IOCTL_VERSION (0x6400)
///
/// 查询驱动版本信息。输入输出参数都是 `struct drm_version`。
///
/// Linux 定义：
/// ```c
/// #define DRM_IOCTL_VERSION \
///     _IOC(_IOC_READ | _IOC_WRITE, DRM_IOCTL_BASE, 0x00, \
///          sizeof(struct drm_version))
/// ```
///
/// 在 asterinas 中，我们使用简化的 OutData（只写出版本信息到用户空间）：
pub(super) type GetVersion = ioc!(DRM_IOCTL_VERSION, DRM_IOCTL_BASE, 0x00, OutData<DrmVersion>);

/// DRM_IOCTL_GET_CAP (0x6409)
///
/// 查询设备能力。输入 `struct drm_get_cap`，输出 u64 值。
///
/// Linux 定义：
/// ```c
/// #define DRM_IOCTL_GET_CAP \
///     _IOC(_IOC_READ | _IOC_WRITE, DRM_IOCTL_BASE, 0x09, \
///          sizeof(struct drm_get_cap))
/// ```
pub(super) type GetCap = ioc!(DRM_IOCTL_GET_CAP, DRM_IOCTL_BASE, 0x09, InOutData<DrmGetCap>);

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
    0x42,   // magic = 0x40 + 2
    0xC0,   // nr
    OutData<DrmModeCardRes>
);

// ============================================================================
// DRM 数据结构（与 Linux 兼容的 C 结构体布局）
// ============================================================================

/// DRM 版本信息结构体 — `struct drm_version` in Linux
///
/// 用户空间通过 DRM_IOCTL_VERSION ioctl 获取此结构体。
/// 所有字段都是输出参数，由内核填充后返回用户空间。
///
/// Linux 参考：include/uapi/drm/drm.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmVersion {
    /// 驱动主版本号
    pub version_major: i32,
    /// 驱动次版本号
    pub version_minor: i32,
    /// 驱动补丁级别
    pub version_patchlevel: i32,
    /// 驱动名称（以 null 结尾的字符串，不足部分填 0）
    pub name: [u8; 64],
    /// 驱动构建日期（YYYYMMDD，不足部分填 0）
    pub date: [u8; 32],
    /// 驱动描述（以 null 结尾的字符串，不足部分填 0）
    pub desc: [u8; 32],
}

/// DRM 能力查询结构体 — `struct drm_get_cap` in Linux
///
/// 用户空间通过 DRM_IOCTL_GET_CAP ioctl 查询设备能力。
/// 输入：capability 字段指定要查询的能力类型
/// 输出：value 字段返回该能力的值
///
/// Linux 参考：include/uapi/drm/drm.h
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod)]
pub struct DrmGetCap {
    /// 能力类型（DRM_CAP_* 常量）
    pub capability: u64,
    /// 能力值（根据 capability 类型解释）
    pub value: u64,
}

/// DRM 能力常量 — DRM_CAP_* in Linux
impl DrmGetCap {
    /// 支持 Dumb Buffer（用户空间可分配的简单像素缓冲区）
    pub const DRM_CAP_DUMB_BUFFER: u64 = 0x1;
    /// Vblank 高 CRTC 编号支持（不使用）
    pub const DRM_CAP_VBLANK_HIGH_CRTC: u64 = 0x2;
    /// Dumb Buffer 首选色深（不使用）
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
    /// Phase 3 不返回 Framebuffer ID，设为 0
    pub fb_id_ptr: u64,
    /// CRTC ID 数组指针（用户空间地址）
    /// Phase 3 不返回 CRTC ID，设为 0
    pub crtc_id_ptr: u64,
    /// Connector ID 数组指针（用户空间地址）
    /// Phase 3 不返回 Connector ID，设为 0
    pub connector_id_ptr: u64,
    /// Encoder ID 数组指针（用户空间地址）
    /// Phase 3 不返回 Encoder ID，设为 0
    pub encoder_id_ptr: u64,
    /// Framebuffer 数量
    pub count_fbs: u32,
    /// CRTC 数量（Phase 3 = 1）
    pub count_crtcs: u32,
    /// Connector 数量（Phase 3 = 1）
    pub count_connectors: u32,
    /// Encoder 数量（Phase 3 = 1）
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
```

**ioctl 编号详解：**

| ioctl | 原始编号 | magic | nr | 大小 | 说明 |
|-------|----------|-------|-----|------|------|
| `DRM_IOCTL_VERSION` | 0x6400 | 'd'=0x64 | 0x00 | 148 | DRM_BASE 0x64 + 编号 0x00 |
| `DRM_IOCTL_GET_CAP` | 0x6409 | 'd'=0x64 | 0x09 | 16 | DRM_BASE 0x64 + 编号 0x09 |
| `DRM_IOCTL_MODE_GETRESOURCES` | 0x40C0 | 0x42 | 0xC0 | 56 | magic=0x40+2, nr=0xC0 |

**`ioc!` 宏的三种用法（参考 `kernel/src/util/ioctl/mod.rs` 第 179-202 行）：**

```rust
// 用法 1：旧式编码（16 位命令码，直接给 raw 数字）
type OldStyle = ioc!(TIOCNOTTY, 0x5422, NoData);

// 用法 2：新式编码（给定 magic, nr, 数据类型）
type NewStyle = ioc!(TIOCSPTLCK, b'T', 0x31, InData<i32>);

// 用法 3：新式编码，输出数据
type GetVersion = ioc!(DRM_IOCTL_VERSION, b'd', 0x00, OutData<DrmVersion>);
```

---

### 5. 新建 `kernel/src/device/drm/kms.rs` — 简化 KMS 模型

KMS（Kernel Mode Setting）对象模型。Phase 3 使用硬编码的简化模型：

```rust
// SPDX-License-Identifier: MPL-2.0

//! 简化 KMS (Kernel Mode Setting) 模型。
//!
//! Phase 3 MVP 提供硬编码的 KMS 对象：
//! - 1 个 CRTC（显示控制器）
//! - 1 个 Encoder（编码器，在硬件层面连接 CRTC 和 Connector）
//! - 1 个 Connector（连接器，对应物理显示输出）
//!
//! 所有 KMS 对象使用固定 ID：CRTC=1, Encoder=2, Connector=3
//!
//! 分辨率范围从 SimpleDrm 获取（如果可用），否则使用默认值。

use alloc::sync::Arc;

use super::ioctl_defs::{DrmGetCap, DrmModeCardRes, DrmVersion};
use crate::util::ioctl::{Ioctl, OutData};

/// DRM 驱动名称
const DRIVER_NAME: &[u8] = b"AsterinasSimpleDrm\0";

/// DRM 驱动描述
const DRIVER_DESC: &[u8] = b"Simple DRM driver for Asterinas\0";

/// DRM 驱动构建日期（YYYYMMDD）
const DRIVER_DATE: &[u8] = b"20260411\0";

/// 处理 DRM_IOCTL_VERSION ioctl
///
/// 填充 `struct drm_version` 并写入用户空间。
pub fn handle_version(cmd: &Ioctl<b'd', 0x00, true, OutData<DrmVersion>>) -> Result<()> {
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
pub fn handle_get_cap(cmd: &Ioctl<b'd', 0x09, true, super::ioctl_defs::InOutData<DrmGetCap>>) -> Result<()> {
    let cap = cmd.read()?;

    let value = match cap.capability {
        DrmGetCap::DRM_CAP_DUMB_BUFFER => 1,           // 支持 Dumb Buffer
        DrmGetCap::DRM_CAP_VBLANK_HIGH_CRTC => 0,      // 不支持
        DrmGetCap::DRM_CAP_DUMB_PREFERRED_DEPTH => 0,  // 不使用
        _ => {
            log::debug!("unknown DRM capability: {}", cap.capability);
            return_errno_with_message!(Errno::EINVAL, "unknown DRM capability");
        }
    };

    // 修改 cap.value 并写回（InOutData 可以同时读写）
    // 由于 DrmGetCap 是 Copy 的，我们重新构造并写入
    let mut new_cap = cap;
    new_cap.value = value;

    // 使用 with_data_ptr 获取可变的 SafePtr 来写入
    // 注意：这里需要用 InOutData 的 write 方法，但 cmd 已经消耗了
    // 所以我们重新构造命令...
    // 实际上 InOutData 的 write 会同时读写，这里用 read 拿到值后
    // 构造一个新的 DrmGetCap 再用 cmd.write 写入整个结构
    cmd.write(&new_cap)?;
    Ok(())
}

/// 处理 DRM_IOCTL_MODE_GETRESOURCES ioctl
///
/// 返回 KMS 对象数量和分辨率范围。
pub fn handle_get_resources(cmd: &Ioctl<0x42, 0xC0, true, OutData<DrmModeCardRes>>) -> Result<()> {
    // 尝试从 SimpleDrm 获取分辨率（如果 framebuffer 可用）
    // 否则使用默认值
    let (width, height) = if let Some(drm) = aster_drm::get_simple_drm() {
        (drm.width() as u32, drm.height() as u32)
    } else {
        // 默认分辨率（VBE 常用模式）
        (1024, 768)
    };

    let mut res = DrmModeCardRes::default();

    // Phase 3 不返回对象 ID（设为 0 表示空指针）
    res.fb_id_ptr = 0;
    res.crtc_id_ptr = 0;
    res.connector_id_ptr = 0;
    res.encoder_id_ptr = 0;

    // 对象计数（硬编码简化值）
    res.count_fbs = 0;           // Phase 3 不支持 Framebuffer 对象
    res.count_crtcs = 1;         // 1 个 CRTC
    res.count_connectors = 1;    // 1 个 Connector
    res.count_encoders = 1;      // 1 个 Encoder

    // 分辨率范围（从 SimpleDrm 或默认值）
    res.min_width = 64;
    res.max_width = 4096;
    res.min_height = 64;
    res.max_height = 4096;

    cmd.write(&res)?;
    Ok(())
}
```

**注意：`handle_get_cap` 的 InOutData 处理方式**

InOutData 是同时读写的类型。在上面的代码中，`cmd.read()?` 获取用户空间的输入，然后 `cmd.write(&new_cap)?` 将完整结构体写回用户空间。这是 InOutData 的标准用法。

但是 `handle_get_cap` 中有一个微妙之处：调用 `cmd.read()?` 会消耗 `cmd`，所以不能再调用 `cmd.write()`。正确的处理方式是使用 `with_data_ptr` 方法直接操作底层数据：

```rust
pub fn handle_get_cap(cmd: &Ioctl<b'd', 0x09, true, InOutData<DrmGetCap>>) -> Result<()> {
    cmd.with_data_ptr(|ptr| {
        let mut cap = ptr.read()?;
        cap.value = match cap.capability {
            DrmGetCap::DRM_CAP_DUMB_BUFFER => 1,
            _ => return Err(Error::with_message(Errno::EINVAL, "unknown capability")),
        };
        ptr.write(&cap)?;
        Ok(())
    })?
}
```

**KMS 对象 ID 硬编码（Phase 3 简化）：**

| 对象类型 | ID | 说明 |
|---------|-----|------|
| CRTC | 1 | 第一个（也是唯一一个）显示控制器 |
| Encoder | 2 | 第一个（也是唯一一个）编码器 |
| Connector | 3 | 第一个（也是唯一一个）连接器 |

Phase 4 会在这些 ID 的分配上引入真正的 ID allocator。

---

### 6. 新建 `kernel/src/device/drm/init.rs` — 初始化入口

```rust
// SPDX-License-Identifier: MPL-2.0

//! DRM 设备初始化。

use alloc::sync::Arc;

use super::Drm;
use crate::device::registry::char;

/// 在第一个内核线程中初始化 DRM 字符设备
///
/// 该函数在系统启动过程中被 `device::init_in_first_kthread()` 调用。
/// 它将 Drm 设备注册到字符设备注册表，后续 devtmpfs 会自动创建设备节点。
pub(super) fn init_in_first_kthread() {
    // 注册 /dev/dri/card0
    // 如果注册失败（设备已存在），输出警告但不 panic
    if let Err(e) = char::register(Arc::new(Drm)) {
        log::warn!("failed to register DRM device: {:?}", e);
    } else {
        log::info!("DRM device /dev/dri/card0 registered (major 226, minor 0)");
    }
}
```

---

## 修改 `kernel/src/device/mod.rs` 的完整 diff

```diff
--- a/kernel/src/device/mod.rs
+++ b/kernel/src/device/mod.rs
@@ -3,6 +3,7 @@
 mod evdev;
 mod fb;
 mod mem;
 pub mod misc;
+pub mod drm;
 mod pty;
 mod registry;
 mod shm;
@@ -121,5 +122,6 @@ pub fn init_in_first_kthread() {
     evdev::init_in_first_kthread();
     fb::init_in_first_kthread();
+    drm::init_in_first_kthread();
 }
```

---

## 新建文件清单

| 文件路径 | 行数（预估） | 说明 |
|---------|-------------|------|
| `kernel/src/device/drm/mod.rs` | ~50 | `Device` trait 实现 |
| `kernel/src/device/drm/file.rs` | ~100 | `FileIo` + `InodeIo` + `Pollable` 实现 |
| `kernel/src/device/drm/ioctl_defs.rs` | ~150 | `ioc!` 类型别名 + C 结构体 |
| `kernel/src/device/drm/kms.rs` | ~120 | KMS 处理器函数 |
| `kernel/src/device/drm/init.rs` | ~20 | 初始化入口 |

总计约 **440 行**新代码。

---

## 依赖关系

```
drm/mod.rs
  ├── Device trait (device/mod.rs)
  ├── char::register (device/registry/char.rs)
  └── DrmFileHandle (file.rs)

drm/file.rs
  ├── FileIo trait (fs/file/mod.rs)
  ├── InodeIo trait (fs/vfs/inode.rs)
  ├── Pollable trait (process/signal.rs)
  ├── RawIoctl (util/ioctl/mod.rs)
  ├── dispatch_ioctl! macro (util/ioctl/mod.rs)
  ├── kms::* (kms.rs)
  └── ioctl_defs::* (ioctl_defs.rs)

drm/ioctl_defs.rs
  ├── ioc! macro (util/ioctl/mod.rs)
  ├── InData / OutData / InOutData (util/ioctl/mod.rs)
  └── Pod trait (用于 #[repr(C)] 结构体)

drm/kms.rs
  ├── aster_drm::get_simple_drm (kernel/comps/drm/)
  └── ioctl_defs::* (ioctl_defs.rs)

drm/init.rs
  └── char::register (device/registry/char.rs)
```

**特别注意：** `kms.rs` 中调用了 `aster_drm::get_simple_drm()`，这意味着 `kernel/comps/drm/` 组件必须先于 `kernel/src/device/drm/` 模块初始化。由于 Asterinas 的组件初始化顺序按照 `Components.toml` 的拓扑排序，只要 `aster-drm` 在 `aster-device-drm` 之前定义即可。

---

## Phase 3 禁区（不要做什么）

以下内容属于 Phase 4 或更后期的范围，**不要**在 Phase 3 中实现：

1. **GEM/mmap**：任何与内存映射相关的 ioctl（`MAP_DUMB`, `CREATE_DUMB`, `DESTROY_DUMB`）
2. **Framebuffer 对象**：`DRM_IOCTL_MODE_ADDFB`, `DRM_IOCTL_MODE_RMFB`
3. **CRTC/Encoder/Connector 详细状态**：`SETCRTC`, `GETCONNECTOR`, `SETENCODER` 等 ioctl
4. **VBlank**：`DRM_IOCTL_VBLANK` 等垂直同步相关 ioctl
5. **Atomic Mode Setting**：`DRM_IOCTL_MODE_ATOMIC` 等
6. **DRM Properties**：`GET_PROPERTY`, `SET_PROPERTY` 等

Phase 3 的 ioctl 分发表只需要处理 3 个 ioctl，其余全部返回 `ENOTTY`。

---

## 验收标准

### 编译验证

```bash
# 在容器内编译
docker exec asterinas bash -c "cd /root/asterinas && make kernel 2>&1 | tail -30"

# 检查是否有 error
docker exec asterinas bash -c "cd /root/asterinas && cargo build -p aster-kernel 2>&1 | grep -i error"
```

预期结果：编译通过，无任何 error。

### 设备节点验证

系统启动后，检查 `/dev/dri/card0` 是否存在：

```bash
ls -la /dev/dri/card0
# 预期输出：crw------- 1 root root 226, 0 /dev/dri/card0
```

### ioctl 功能验证

使用 `libdrm` 测试工具或 Python 脚本验证：

```python
#!/usr/bin/env python3
# test_drm.py

import os
import struct

DRM_IOCTL_VERSION = 0x6400
DRM_IOCTL_GET_CAP = 0x6409
DRM_IOCTL_MODE_GETRESOURCES = 0x40C0

DRM_CAP_DUMB_BUFFER = 0x1

# struct drm_version { int version_major; int version_minor; int version_patchlevel; char name[64]; char date[32]; char desc[32]; }
drm_version_fmt = "iii64s32s32s"
drm_version_size = struct.calcsize(drm_version_fmt)

# struct drm_get_cap { __u64 capability; __u64 value; }
drm_get_cap_fmt = "QQ"
drm_get_cap_size = struct.calcsize(drm_get_cap_fmt)

# struct drm_mode_card_res { __u64 fb_id_ptr; __u64 crtc_id_ptr; __u64 connector_id_ptr; __u64 encoder_id_ptr; __u32 count_fbs; __u32 count_crtcs; __u32 count_connectors; __u32 count_encoders; __u32 min_width; __u32 max_width; __u32 min_height; __u32 max_height; }
drm_mode_card_res_fmt = "QQQQIIIIIIII"
drm_mode_card_res_size = struct.calcsize(drm_mode_card_res_fmt)

fd = os.open("/dev/dri/card0", os.O_RDWR)

# Test VERSION
version = os.ioctl(fd, DRM_IOCTL_VERSION, struct.pack("i" * (drm_version_size // 4), 0) * 1)
# Parse version
major, minor, patchlevel = struct.unpack("iii", version[:12])
name = version[12:76].rstrip(b'\x00').decode()
print(f"VERSION: {major}.{minor}.{patchlevel} ({name})")

# Test GET_CAP with DUMB_BUFFER
cap_data = struct.pack(drm_get_cap_fmt, DRM_CAP_DUMB_BUFFER, 0)
cap_result = os.ioctl(fd, DRM_IOCTL_GET_CAP, cap_data)
cap_value = struct.unpack(drm_get_cap_fmt, cap_result)[1]
print(f"GET_CAP(DUMB_BUFFER): {cap_value}")

# Test GETRESOURCES
res_data = b'\x00' * drm_mode_card_res_size
res_result = os.ioctl(fd, DRM_IOCTL_MODE_GETRESOURCES, res_data)
res = struct.unpack(drm_mode_card_res_fmt, res_result)
print(f"GETRESOURCES: crtcs={res[5]}, connectors={res[6]}, encoders={res[7]}")
print(f"  resolution range: {res[8]}x{res[9]} - {res[10]}x{res[11]}")

os.close(fd)
```

预期输出：
```
VERSION: 1.0.0 (AsterinasSimpleDrm)
GET_CAP(DUMB_BUFFER): 1
GETRESOURCES: crtcs=1, connectors=1, encoders=1
  resolution range: 64x64 - 4096x4096
```

---

## Phase 4 预告

Phase 4 将实现 DUMB_BUFFER 支持，使用户空间可以真正分配 GPU 缓冲区：

- `DRM_IOCTL_MODE_CREATE_DUMB` — 创建用户空间可映射的像素缓冲区
- `DRM_IOCTL_MODE_MAP_DUMB` — 获取 mmap 偏移量
- `DRM_IOCTL_MODE_DESTROY_DUMB` — 销毁缓冲区
- `DRM_IOCTL_MODE_ADDFB` — 将 DUMB Buffer 注册为 Framebuffer 对象
- `DRM_IOCTL_MODE_RMFB` — 移除 Framebuffer 对象
- `DRM_IOCTL_MODE_SETCRTC` — 将 Framebuffer 绑定到 CRTC 显示

Phase 4 还需要实现真正的 KMS 对象 ID 分配器（GEM handle 表），而不是 Phase 3 的硬编码 ID。
