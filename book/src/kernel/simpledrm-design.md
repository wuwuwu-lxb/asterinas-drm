# SimpleDRM 设计与实现

## 项目概述

本计划基于 Asterinas 操作系统的 SimpleDRM 架构设计，实现一个符合 Linux DRM/KMS 标准的轻量级显示驱动。

### 核心目标

1. 替换老旧的 fbdev 接口，提供现代的 DRM 支持
2. 实现双缓冲机制，避免画面撕裂
3. 提供标准的用户空间接口（ioctl）
4. 支持 GEM 对象和 Dumb Buffer 管理

### 关键参考文档

- [DRM 集成指南](drm-integration.md) - 已有的 DRM 集成路线图
- 用户提供的 SimpleDRM 架构设计文档

---

## 架构设计

### SimpleDRM 核心组件

```
┌─────────────────────────────────────────────────────────┐
│                    用户空间                              │
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────┐  │
│  │ libdrm        │  │ Weston        │  │ kmscube     │  │
│  │               │  │ 合成器         │  │ 测试应用    │  │
│  └───────┬───────┘  └───────┬───────┘  └──────┬──────┘  │
│          │                  │                  │         │
│          └──────────────────┼──────────────────┘         │
│                             │ DRM_IOCTL_*                │
└─────────────────────────────┼───────────────────────────┘
                              │
┌─────────────────────────────┼───────────────────────────┐
│                    内核空间                              │
│  ┌─────────────────────────────────────────────────────┐│
│  │              DRM 设备模型                            ││
│  │  ┌───────────────────────────────────────────────┐  ││
│  │  │ /dev/dri/card0                               │  ││
│  │  │  ├─ DRM_IOCTL_VERSION                       │  ││
│  │  │  ├─ DRM_IOCTL_GET_CAP                       │  ││
│  │  │  ├─ DRM_IOCTL_MODE_GETRESOURCES             │  ││
│  │  │  ├─ DRM_IOCTL_MODE_ADDFB / RMFB            │  ││
│  │  │  ├─ DRM_IOCTL_MODE_CREATE_DUMB             │  ││
│  │  │  ├─ DRM_IOCTL_MODE_MAP_DUMB                │  ││
│  │  │  ├─ DRM_IOCTL_MODE_SETCRTC                 │  ││
│  │  │  └─ DRM_IOCTL_MODE_PAGE_FLIP              │  ││
│  │  └───────────────────────────────────────────────┘  ││
│  └─────────────────────────────────────────────────────┘│
│                             │                            │
│  ┌──────────────────────────┼────────────────────────────┐ │
│  │        KMS 对象          │                            │ │
│  │  ┌─────────┐  ┌────────┐ │ ┌────────┐ ┌─────────┐ │ │
│  │  │连接器   │──│ 编码器 │──│ │ CRTC   │──│  平面   │ │ │
│  │  │(显示器) │  │(编码器)│ │ │(控制器)│ │ (图层)  │ │ │
│  │  └─────────┘  └────────┘ │ └────────┘ └─────────┘ │ │
│  └─────────────────────────────────────────────────────┘ │
│                             │                            │
│  ┌──────────────────────────┼────────────────────────────┐ │
│  │       内存管理            │                            │ │
│  │  ┌────────────────────────────────────────────────┐  │ │
│  │  │  GEM (图形执行管理器)                          │  │ │
│  │  │  ├─ Dumb Buffer (系统内存)                    │  │ │
│  │  │  ├─ Framebuffer 对象                          │  │ │
│  │  │  └─ 内存映射 (mmap)                            │  │ │
│  │  └────────────────────────────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────┘ │
│                             │                            │
│  ┌──────────────────────────┼────────────────────────────┐ │
│  │       缓冲区管理           │                            │ │
│  │  ┌────────────┐  ┌────────────────┐                   │ │
│  │  │ 后端缓冲区  │  │ 前端缓冲区     │                   │ │
│  │  │ (系统 RAM) │  │ (MMIO VRAM)   │                   │ │
│  │  └─────┬──────┘  └──────┬────────┘                   │ │
│  │        │                │                             │ │
│  │        └────────────────┴─────────────────────────     │ │
│  │                    present()                           │ │
│  └─────────────────────────────────────────────────────┘ │
│                             │                            │
│  ┌──────────────────────────┼────────────────────────────┐ │
│  │    aster-framebuffer      │                            │ │
│  │  └─ IoMem (MMIO 映射)    │                            │ │
│  │  └─ FrameBuffer Console │                            │ │
│  └─────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

### 数据流

```
用户空间渲染流程:

1. open("/dev/dri/card0")
2. DRM_IOCTL_MODE_GETRESOURCES  → 获取 KMS 对象 ID
3. DRM_IOCTL_MODE_CREATE_DUMB   → 创建 Dumb Buffer
4. DRM_IOCTL_MODE_MAP_DUMB      → 获取映射信息
5. mmap()                        → 用户空间映射
6. 写入像素数据到用户空间缓冲区
7. DRM_IOCTL_MODE_ADDFB         → 注册 Framebuffer
8. DRM_IOCTL_MODE_SETCRTC       → 设置显示模式
9. DRM_IOCTL_MODE_PAGE_FLIP     → 触发缓冲区交换
```

---

## 实现阶段

### 阶段 0: 基础设施准备（第 1-2 天）

**目标**: 验证环境依赖和基础设施

#### 0.1 验证引导程序帧缓冲信息

```rust
// 检查 boot_info().framebuffer_arg 是否有效
let Some(framebuffer_arg) = boot_info().framebuffer_arg else {
    log::warn!("Framebuffer not found");
    return;
};
```

#### 0.2 识别像素格式

当前实现中的问题（见 `framebuffer.rs`）:
- BPP 相同但格式不同的情况（如 32bpp 可能是 BGRX 或 XBGR）
- 需要在引导阶段收集更多信息

#### 0.3 验证 line_size 对齐

```rust
// 当前实现假设 line_size = width * bpp/8
// 但实际硬件可能有对齐要求
let line_size = framebuffer_arg.width * pixel_format.nbytes();
```

---

### 阶段 1: SimpleDRM MVP（第 3-7 天）

**目标**: 实现基本的双缓冲功能，无画面撕裂

#### 1.1 创建 aster-drm 组件

**目录结构**:

```
kernel/comps/drm/
├── Cargo.toml
└── src/
    ├── lib.rs                    # 组件入口
    ├── device.rs                 # DRM 设备
    ├── buffer.rs                 # 后端缓冲区管理
    ├── simple.rs                 # SimpleDRM 实现
    ├── kms/
    │   ├── mod.rs               # KMS 对象基类
    │   ├── crtc.rs              # CRTC 控制器
    │   ├── encoder.rs            # 编码器
    │   ├── connector.rs          # 连接器
    │   └── plane.rs              # 平面（图层）
    ├── gem/
    │   ├── mod.rs                # GEM 对象管理
    │   └── dumb.rs               # Dumb Buffer 实现
    └── ioctl/
        ├── mod.rs               # IOCTL 处理
        ├── version.rs            # DRM_IOCTL_VERSION
        ├── resources.rs          # DRM_IOCTL_MODE_GETRESOURCES
        ├── dumb.rs               # Dumb Buffer 操作
        └── crtc.rs               # CRTC 操作
```

#### 1.2 Cargo.toml 配置

```toml
[package]
name = "aster-drm"
version = "0.1.0"
edition.workspace = true

[dependencies]
aster-framebuffer.workspace = true
component.workspace = true
log.workspace = true
ostd.workspace = true
spin.workspace = true
id-alloc.workspace = true

[lints]
workspace = true
```

#### 1.3 实现 BackBuffer

```rust
// kernel/comps/drm/src/buffer.rs

use alloc::vec::Vec;
use ostd::sync::SpinLock;

/// 后端缓冲区 - 用于渲染的 RAM 缓冲区
///
/// 双缓冲机制：
/// - Back Buffer: 用户空间写入
/// - Front Buffer: 硬件显示
pub struct BackBuffer {
    data: SpinLock<Vec<u8>>,
    width: usize,
    height: usize,
    stride: usize,
}

impl BackBuffer {
    /// 创建新的后端缓冲区
    pub fn new(width: usize, height: usize, stride: usize) -> Self {
        let size = height * stride;
        Self {
            data: SpinLock::new(vec![0u8; size]),
            width,
            height,
            stride,
        }
    }

    /// 获取缓冲区大小
    pub fn size(&self) -> usize {
        self.height * self.stride
    }

    /// 获取宽度
    pub fn width(&self) -> usize {
        self.width
    }

    /// 获取高度
    pub fn height(&self) -> usize {
        self.height
    }

    /// 获取步长（每行字节数）
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// 获取缓冲区数据（不可变引用）
    pub fn data(&self) -> &[u8] {
        self.data.lock().as_slice()
    }

    /// 清空缓冲区
    pub fn clear(&self) {
        self.data.lock().fill(0);
    }
}
```

#### 1.4 实现 SimpleDrm::present()

```rust
// kernel/comps/drm/src/simple.rs

use alloc::sync::Arc;
use aster_framebuffer::FrameBuffer;
use ostd::sync::SpinLock;

use super::buffer::BackBuffer;

/// SimpleDRM 核心结构
pub struct SimpleDrm {
    fb: Arc<FrameBuffer>,
    back_buffer: SpinLock<BackBuffer>,
}

impl SimpleDrm {
    /// 创建 SimpleDrm 实例
    pub fn new(fb: Arc<FrameBuffer>) -> Self {
        let back_buffer = BackBuffer::new(
            fb.width(),
            fb.height(),
            fb.line_size(),
        );

        Self {
            fb,
            back_buffer: SpinLock::new(back_buffer),
        }
    }

    /// 获取后端缓冲区
    pub fn back_buffer(&self) -> &SpinLock<BackBuffer> {
        &self.back_buffer
    }

    /// 获取帧缓冲信息
    pub fn framebuffer(&self) -> &Arc<FrameBuffer> {
        &self.fb
    }

    /// 将后端缓冲区内容复制到前端缓冲区
    ///
    /// 这是双缓冲的关键操作：
    /// 1. 用户空间渲染到 back buffer
    /// 2. present() 将完整帧复制到硬件帧缓冲
    /// 3. 避免部分更新导致的撕裂
    pub fn present(&self) -> ostd::Result<()> {
        let back_data = {
            let back = self.back_buffer.lock();
            back.data().to_vec()
        };

        self.fb.write_bytes_at(0, &back_data)
    }
}
```

#### 1.5 注册组件

在 `Components.toml` 中添加:

```toml
[components]
# ... 现有条目 ...
drm = { name = "aster-drm" }
```

在 `kernel/Cargo.toml` 中添加:

```toml
[dependencies]
# ... 现有依赖 ...
aster-drm = { path = "comps/drm" }
```

#### 1.6 测试验证

创建测试程序绘制渐变或棋盘格图案，验证：
- 无撕裂
- 稳定的刷新率
- 正确的颜色显示

---

### 阶段 2: 双缓冲与控制台共存（第 8-10 天）

**目标**: 解耦文本控制台和 DRM 后端缓冲区

#### 2.1 扩展 FrameBuffer API

在 `aster-framebuffer` 中添加:

```rust
// kernel/comps/framebuffer/src/framebuffer.rs

impl FrameBuffer {
    /// 将指定区域的数据刷新到硬件帧缓冲
    ///
    /// 参数:
    /// - `offset`: 数据在帧缓冲中的起始偏移
    /// - `data`: 要刷新的数据
    /// - `len`: 要刷新的数据长度
    pub fn flush_region(&self, offset: usize, data: &[u8], len: usize) -> Result<()> {
        let slice = &data[..len];
        self.io_mem.write_bytes(offset, slice)
    }
}
```

#### 2.2 保护 BackBuffer 访问

使用 `SpinLock<LocalIrqDisabled>` 允许中断上下文中安全访问:

```rust
use ostd::sync::{LocalIrqDisabled, SpinLock};

pub struct SimpleDrm {
    fb: Arc<FrameBuffer>,
    back_buffer: SpinLock<BackBuffer, LocalIrqDisabled>,
}
```

#### 2.3 VSync 风格的周期性刷新

```rust
use ostd::timer::Timer;

// 定时器回调
fn vsync_callback(timer: &Timer) {
    if let Some(drm) = SIMPLE_DRM.get() {
        let _ = drm.present();
    }
}
```

---

### 阶段 3: DRM 设备节点和基础 IOCTL（第 3-4 周）

**目标**: 暴露 `/dev/dri/card0` 供用户空间程序使用

#### 3.1 注册字符设备

在 `kernel/src/device/` 中创建新的设备模块:

```rust
// kernel/src/device/drm/mod.rs

pub mod device;
pub mod file;

use device::DrmDevice;

pub fn init_in_first_kthread() {
    // 注册 /dev/dri/card0
    let drm_device = Arc::new(DrmDevice::new());
    register(Arc::new(drm_device)).unwrap();
}
```

#### 3.2 实现 DRM_IOCTL_VERSION

```rust
// kernel/src/device/drm/ioctl/version.rs

use core::mem::size_of;

const DRM_VERSION: u32 = 0x00;
const DRM_IOCTL_VERSION_BASE: u32 = 0x40;

#[repr(C)]
struct DrmVersion {
    version_major: i32,
    version_minor: i32,
    version_patchlevel: i32,
    name: [u8; 64],
    date: [u8; 32],
    desc: [u8; 32],
}

impl DrmVersion {
    fn new() -> Self {
        let name = b"Asterinas SimpleDRM\0";
        let date = b"2025\0";
        let desc = b"Simple DRM driver for firmware framebuffer\0";

        let mut result = Self {
            version_major: 1,
            version_minor: 0,
            version_patchlevel: 0,
            name: [0; 64],
            date: [0; 32],
            desc: [0; 32],
        };

        result.name[..name.len()].copy_from_slice(name);
        result.date[..date.len()].copy_from_slice(date);
        result.desc[..desc.len()].copy_from_slice(desc);

        result
    }
}

pub fn handle_version(arg: Vaddr) -> Result<()> {
    let version = DrmVersion::new();
    copy_to_user(arg, &version)?;
    Ok(())
}
```

#### 3.3 实现 DRM_IOCTL_GET_CAP

```rust
// kernel/src/device/drm/ioctl/capability.rs

const DRM_IOCTL_GET_CAP: u32 = 0x09;

#[repr(C)]
struct DrmCapability {
    capability: u64,
    value: u64,
}

// DRM Capability 常量
const DRM_CAP_DUMB_BUFFER: u64 = 0x1;
const DRM_CAP_VBLANK_HIGH_CRTC: u64 = 0x2;
const DRM_CAP_DUMB_PREFERRED_DEPTH: u64 = 0x3;

pub fn handle_get_cap(arg: Vaddr) -> Result<()> {
    let cap = unsafe { &*(arg as *const DrmCapability) };

    let value = match cap.capability {
        DRM_CAP_DUMB_BUFFER => 1,              // 支持 Dumb Buffer
        DRM_CAP_VBLANK_HIGH_CRTC => 0,         // 不支持
        DRM_CAP_DUMB_PREFERRED_DEPTH => 32,     // 首选 32 位色深
        _ => return Err(Error::EINVAL),
    };

    copy_to_user(arg as usize + 8, &value)?;
    Ok(())
}
```

#### 3.4 实现 DRM_IOCTL_MODE_GETRESOURCES

```rust
// kernel/src/device/drm/ioctl/resources.rs

const DRM_IOCTL_MODE_GETRESOURCES: u32 = 0xC0;

#[repr(C)]
struct DrmModeCardRes {
    fb_id_ptr: u64,       // Framebuffer IDs
    crtc_id_ptr: u64,     // CRTC IDs
    connector_id_ptr: u64, // Connector IDs
    encoder_id_ptr: u64,  // Encoder IDs
    count_fbs: u32,
    count_crtcs: u32,
    count_connectors: u32,
    count_encoders: u32,
    min_width: u32,
    max_width: u32,
    min_height: u32,
    max_height: u32,
}

pub fn handle_get_resources(arg: Vaddr) -> Result<()> {
    // 返回:
    // - 1 个 CRTC
    // - 1 个 Encoder
    // - 1 个 Connector
    // - 支持的分辨率范围

    let mut res = DrmModeCardRes {
        fb_id_ptr: 0,
        crtc_id_ptr: 0,
        connector_id_ptr: 0,
        encoder_id_ptr: 0,
        count_fbs: 0,
        count_crtcs: 1,    // 只有一个 CRTC
        count_connectors: 1,
        count_encoders: 1,
        min_width: 64,
        max_width: 4096,
        min_height: 64,
        max_height: 4096,
    };

    copy_to_user(arg, &res)?;
    Ok(())
}
```

#### 3.5 实现 Dumb Buffer 操作

```rust
// kernel/src/device/drm/ioctl/dumb.rs

const DRM_IOCTL_MODE_CREATE_DUMB: u32 = 0xC2;
const DRM_IOCTL_MODE_MAP_DUMB: u32 = 0xC3;
const DRM_IOCTL_MODE_DESTROY_DUMB: u32 = 0xC4;
const DRM_IOCTL_MODE_ADDFB: u32 = 0xAE;
const DRM_IOCTL_MODE_RMFB: u32 = 0xAF;

#[repr(C)]
struct DrmModeCreateDumb {
    height: u32,
    width: u32,
    bpp: u32,
    flags: u32,
    handle: u32,    // 返回的 GEM handle
    pitch: u32,     // 返回的步长
    size: u64,      // 返回的缓冲区大小
    padding: u64,
};

#[repr(C)]
struct DrmModeMapDumb {
    handle: u32,
    padding: u32,
    offset: u64,    // 返回的 mmap 偏移
};
```

#### 3.6 实现 DRM_IOCTL_MODE_SETCRTC

```rust
// kernel/src/device/drm/ioctl/crtc.rs

const DRM_IOCTL_MODE_SETCRTC: u32 = 0xA6;

#[repr(C)]
struct DrmModeSetCrtc {
    crtc_id: u32,
    fb_id: u32,
    flags: u32,
    connector_count: u32,
    connectors_ptr: u64,
    mode: u64,       // 显示模式指针
    x: u32,
    y: u32,
};

pub fn handle_set_crtc(arg: Vaddr) -> Result<()> {
    let cmd = unsafe { &*(arg as *const DrmModeSetCrtc) };

    // 验证 FB ID
    let fb = get_framebuffer(cmd.fb_id)?;

    // 验证 CRTC ID
    let crtc = get_crtc(cmd.crtc_id)?;

    // 应用显示模式
    crtc.set_mode(&cmd.mode)?;

    // 绑定 Framebuffer 到 CRTC
    crtc.bind_framebuffer(fb)?;

    Ok(())
}
```

#### 3.7 用户空间测试程序

在 `test/` 目录创建简单测试:

```c
// test/drm_basic/drm_test.c

#include <stdio.h>
#include <fcntl.h>
#include <unistd.h>
#include <xf86drm.h>
#include <xf86drmMode.h>

int main() {
    int fd = open("/dev/dri/card0", O_RDWR);
    if (fd < 0) {
        perror("打开 DRM 设备失败");
        return 1;
    }

    // 获取资源信息
    drmModeResPtr resources = drmModeGetResources(fd);
    if (!resources) {
        printf("获取 DRM 资源失败\n");
        close(fd);
        return 1;
    }

    printf("CRTCs: %d, Connectors: %d\n",
           resources->count_crtcs,
           resources->count_connectors);

    // 清理
    drmModeFreeResources(resources);
    close(fd);

    return 0;
}
```

---

### 阶段 4: GEM 对象和 mmap（第 5-8 周）

**目标**: 实现标准的 libdrm dumb-buffer 工作流程

#### 4.1 实现 GEM 对象生命周期

```rust
// kernel/comps/drm/src/gem/mod.rs

use alloc::vec::Vec;
use ostd::sync::SpinLock;
use id_alloc::IdAllocator;

pub struct GemObject {
    handle: u32,
    size: usize,
    data: Vec<u8>,
}

pub struct GemManager {
    objects: SpinLock<BTreeMap<u32, Arc<GemObject>>>,
    id_allocator: SpinLock<IdAllocator<u32>>,
}

impl GemManager {
    pub fn new() -> Self {
        Self {
            objects: SpinLock::new(BTreeMap::new()),
            id_allocator: SpinLock::new(IdAllocator::new(1, u32::MAX)),
        }
    }

    pub fn create_object(&self, size: usize) -> Result<u32> {
        let handle = {
            let mut alloc = self.id_allocator.lock();
            alloc.alloc().ok_or(Error::ENOMEM)?
        };

        let object = Arc::new(GemObject {
            handle,
            size,
            data: vec![0u8; size],
        });

        self.objects.lock().insert(handle, object);
        Ok(handle)
    }

    pub fn get_object(&self, handle: u32) -> Option<Arc<GemObject>> {
        self.objects.lock().get(&handle).cloned()
    }

    pub fn destroy_object(&self, handle: u32) -> Result<()> {
        let mut objects = self.objects.lock();
        if objects.remove(&handle).is_none() {
            return Err(Error::EINVAL);
        }

        let mut alloc = self.id_allocator.lock();
        alloc.free(handle);

        Ok(())
    }
}
```

#### 4.2 实现 mmap 支持

```rust
// kernel/src/device/drm/file.rs

use crate::vm::vmar::Vmar;

impl DrmFile {
    fn mmap(
        &self,
        start: usize,
        len: usize,
        prot: usize,
        flags: usize,
        offset: usize,
        ctx: &Context,
    ) -> Result<usize> {
        // offset 包含 GEM handle 信息
        // 使用 Vmar 将 GEM 对象映射到用户空间

        let gem_handle = offset_to_handle(offset)?;
        let object = self.gem_manager.get_object(gem_handle)
            .ok_or(Error::EINVAL)?;

        // 创建匿名 VMO
        let vmo = Vmo::new(len);
        vmo.write(0, &object.data)?;

        // 映射到用户地址空间
        let vmar = ctx.vm_space().lock();
        vmar.map_at(vmar.root_vmar(), start, &vmo, 0, len, prot)?;

        Ok(start)
    }
}
```

---

### 阶段 5: KMS 平面/CRTC/编码器/连接器模型（第 9 周+）

**目标**: 符合标准的 KMS 设备

#### 5.1 实现 KMS 对象模型

```rust
// kernel/comps/drm/src/kms/crtc.rs

pub struct Crtc {
    id: u32,
    device: Arc<SimpleDrm>,
    encoder: SpinLock<Option<Arc<Encoder>>>,
    plane: Arc<Plane>,
    mode: SpinLock<Option<DisplayMode>>,
}

impl Crtc {
    pub fn new(id: u32, device: Arc<SimpleDrm>) -> Self {
        Self {
            id,
            device,
            encoder: SpinLock::new(None),
            plane: Arc::new(Plane::primary(id)),
            mode: SpinLock::new(None),
        }
    }

    pub fn set_encoder(&self, encoder: Arc<Encoder>) {
        *self.encoder.lock() = Some(encoder);
    }

    pub fn set_mode(&self, mode: &DisplayMode) -> Result<()> {
        *self.mode.lock() = Some(*mode);
        self.apply_mode()
    }

    fn apply_mode(&self) -> Result<()> {
        // 设置显示时序
        // 配置扫描引擎
        // 启用垂直同步
        Ok(())
    }
}
```

```rust
// kernel/comps/drm/src/kms/encoder.rs

pub struct Encoder {
    id: u32,
    encoder_type: EncoderType,
    possible_crtcs: u32,
    possible_clones: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum EncoderType {
    None,
    Dac,
    Tmds,
    Lvds,
    TvDac,
    Virtual,
    Dsi,
    Dpmst,
    Dpi,
}

impl Encoder {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            encoder_type: EncoderType::Dpi, // 模拟 DPI 编码器
            possible_crtcs: 1,              // 只能连接到 CRTC 0
            possible_clones: 0,
        }
    }
}
```

```rust
// kernel/comps/drm/src/kms/connector.rs

pub struct Connector {
    id: u32,
    connector_type: ConnectorType,
    connection: ConnectionStatus,
    modes: Vec<DisplayMode>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub enum ConnectorType {
    Unknown,
    Vga,
    DviI,
    DviD,
    Composite,
    SVideo,
    Lvds,
    Component,
    DIN9P,
    DisplayPort,
    HdmIA,
    HdmIB,
    Tv,
    EmbeddedDisplayPort,
}

impl Connector {
    pub fn new(id: u32, fb: &FrameBuffer) -> Self {
        let mode = DisplayMode {
            width: fb.width() as u32,
            height: fb.height() as u32,
            clock: 148500,          // 假设 60Hz 刷新率
            hdisplay: fb.width() as u16,
            hsync_start: 0,
            hsync_end: 0,
            htotal: 0,
            vdisplay: fb.height() as u16,
            vsync_start: 0,
            vsync_end: 0,
            vtotal: 0,
            flags: 0,
            type_: 0,
            name: [0; 32],
        };

        Self {
            id,
            connector_type: ConnectorType::Lvds,
            connection: ConnectionStatus::Connected, // 固件提供，始终连接
            modes: vec![mode],
        }
    }
}
```

```rust
// kernel/comps/drm/src/kms/plane.rs

pub struct Plane {
    id: u32,
    plane_type: PlaneType,
    possible_crtcs: u32,
    formats: Vec<u32>,
}

#[derive(Debug, Clone, Copy)]
pub enum PlaneType {
    Overlay,
    Primary,
    Cursor,
}

impl Plane {
    pub fn primary(crtc_id: u32) -> Self {
        Self {
            id: crtc_id, // 复用 CRTC ID 作为 plane ID
            plane_type: PlaneType::Primary,
            possible_crtcs: 1 << crtc_id,
            formats: vec![
                0x34324258, // DRM_FORMAT_XRGB8888
                0x34324742, // DRM_FORMAT_BGRX8888
                0x56524742, // DRM_FORMAT_RGB565
            ],
        }
    }
}
```

#### 5.2 实现原子模式设置（可选）

```rust
const DRM_IOCTL_MODE_ATOMIC: u32 = 0xB8;

#[repr(C)]
struct DrmModeAtomic {
    flags: u32,
    count_objs: u32,
    objs_ptr: u64,
    props_ptr: u64,
    prop_values_ptr: u64,
};

pub fn handle_atomic(fd: u32, arg: Vaddr) -> Result<()> {
    let atomic = unsafe { &*(arg as *const DrmModeAtomic) };

    // 验证所有对象和属性
    // 准备所有更改
    // 一次性应用（原子操作）
    // 如果失败，回滚所有更改

    Ok(())
}
```

---

## 技术细节

### 像素格式支持

```rust
// 支持的像素格式（对应 DRM_FORMAT_*）
pub const SUPPORTED_FORMATS: &[u32] = &[
    0x34324258, // DRM_FORMAT_XRGB8888
    0x34324742, // DRM_FORMAT_BGRX8888
    0x56524742, // DRM_FORMAT_RGB565
    0x20585352, // DRM_FORMAT_RGBX8888
];
```

### 同步机制

```rust
// 使用自旋锁保护共享资源
use ostd::sync::SpinLock;

// 对于中断安全的操作
use ostd::sync::LocalIrqDisabled;

// 垂直同步（未来扩展）
const DRM_MODE_VSYNC_MEM_FENCE: u32 = 1 << 2;
```

### 错误处理

遵循 Linux DRM 错误码约定:
- `-EINVAL`: 无效参数
- `-ENOMEM`: 内存不足
- `-ENOENT`: 对象不存在
- `-EBUSY`: 资源忙
- `-ENODEV`: 设备不存在

---

## 测试计划

### 单元测试

1. **BackBuffer 测试**
   - 创建和销毁
   - 数据读写
   - 大小计算

2. **GEM 对象测试**
   - 创建和销毁
   - 内存分配
   - Handle 分配

3. **KMS 对象测试**
   - CRTC/Encoder/Connector 关系
   - 模式验证

### 集成测试

1. **基础功能测试**
   ```bash
   # 编译测试程序
   gcc test/drm_basic/drm_test.c -o test/drm_basic/drm_test -ldrm

   # 在 QEMU 中运行
   ./test/drm_basic/drm_test
   ```

2. **双缓冲测试**
   - 绘制测试图案
   - 验证无撕裂

3. **性能测试**
   - 帧率测试
   - 内存使用测试

### 用户空间测试程序

使用 `kmscube` 或自定义测试程序验证:
- 打开 `/dev/dri/card0`
- 获取资源信息
- 创建 Dumb Buffer
- 映射到用户空间
- 渲染和显示

---

## 文档

### API 文档

为每个公共 API 编写文档注释:

```rust
/// 获取后端缓冲区
///
/// 返回后端缓冲区的锁，用于安全访问。
///
/// # 示例
///
/// ```ignore
/// let back = drm.back_buffer();
/// let mut data = back.lock();
/// data[0] = 0xFF; // 写入像素数据
/// ```
pub fn back_buffer(&self) -> &SpinLock<BackBuffer> {
    &self.back_buffer
}
```

### 架构文档

在 `book/src/kernel/` 中添加:
- SimpleDRM 设计文档
- API 参考手册
- 移植指南

---

## 未来工作

### 短期目标

- [ ] 实现基本的双缓冲
- [ ] 支持更多像素格式
- [ ] 添加 VSync 支持

### 中期目标

- [ ] 实现原子模式设置
- [ ] 支持多显示器
- [ ] 添加硬件加速

### 长期目标

- [ ] VirtIO GPU 后端
- [ ] Wayland compositor 支持
- [ ] OpenGL/Vulkan 加速

---

## 总结

本计划提供了一个从零开始在 Asterinas 中实现 SimpleDRM 的完整路线图。通过分阶段实现，我们可以:

1. **快速验证**: 阶段 1 就能看到可用的双缓冲效果
2. **逐步完善**: 每个阶段都增加新功能
3. **向后兼容**: fbdev 兼容性层保证旧软件可用
4. **标准接口**: 遵循 Linux DRM API，便于移植现有软件

实施这个计划需要:
- **核心团队**: 2-3 人
- **时间**: 6-10 周（取决于目标功能的完整性）
- **技能**: 操作系统内核、图形栈、驱动开发经验
