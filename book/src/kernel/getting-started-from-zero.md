# 从零开始了解 Asterinas DRM 项目

> 本文档为零基础读者设计，帮助你从无到有理解 Asterinas DRM 项目。
> 读完本文档后，你可以继续阅读 [Week 1 学习计划](week-one-learning-plan.md)。

---

## 什么是操作系统？（5 分钟概念）

### 计算机的层级结构

```
┌──────────────────────────────────────┐
│        用户应用程序 (Apps)            │  ← 你平时用的微信、游戏
├──────────────────────────────────────┤
│     操作系统 (Operating System)        │  ← Windows / Linux / macOS
│   ┌────────────┬──────────────┐      │
│   │  用户界面   │  系统调用接口  │      │
│   ├────────────┴──────────────┤      │
│   │     内核 (Kernel)           │      │  ← 资源管理、程序运行
│   │  (文件、网络、内存、进程)   │      │
│   └────────────────────────────┘      │
├──────────────────────────────────────┤
│          硬件 (Hardware)              │  ← CPU、内存、显卡、硬盘
└──────────────────────────────────────┘
```

**操作系统 = 应用程序 + 内核 + 驱动程序**

### 什么是显示驱动？

显示器通过**显卡**（GPU）连接电脑。应用程序想显示图像，需要通过显卡驱动告诉显卡："在这个位置画这个颜色"。

```
应用程序 → 图形库 (OpenGL/Vulkan) → GPU 驱动 → 显卡 → 显示器
```

---

## 什么是 DRM？（10 分钟概念）

### Linux 显示驱动历史

| 时代 | 技术 | 问题 |
|------|------|------|
| 早期 | 直接写显存 | 程序互相冲突 |
| fbdev | 帧缓冲设备 | 只支持固定分辨率，无法多程序共享 |
| **DRM/KMS** | Direct Rendering Manager + Kernel Mode Setting | **现代标准** |

### DRM 的核心思想

**DRM = 设备管理 + 内存管理 + 模式设置**

```
DRM 解决的问题：
1. 多个程序同时需要显示怎么办？→ DRM 设备模型（/dev/dri/card0）
2. 屏幕分辨率怎么设置？         → KMS（Kernel Mode Setting）
3. 怎么高效传输像素数据？        → GEM（Graphics Execution Manager）
4. 怎么避免画面撕裂？           → 双缓冲 / 页面翻转
```

### DRM 核心概念（速查表）

| 概念 | 全称 | 作用 |
|------|------|------|
| **CRTC** | Cathode Ray Tube Controller | 显示控制器，扫描输出到显示器 |
| **Encoder** | Encoder | 编码信号（数字→模拟等） |
| **Connector** | Connector | 物理接口（HDMI/VGA/DP） |
| **Plane** | Plane | 图层（背景层、内容层、光标层） |
| **Framebuffer** | Framebuffer | 显存区域（像素数据） |
| **GEM Object** | Graphics Execution Manager Object | 图形内存对象 |
| **Dumb Buffer** | Dumb Buffer | 最简单的帧缓冲（系统 RAM） |

```
显示流程：
Connector (检测到显示器)
    ↓
Encoder (编码信号)
    ↓
CRTC (控制扫描时序)
    ↓
Plane (选择图层)
    ↓
Framebuffer (像素数据来源)
    ↓
→ 显示器
```

---

## 什么是 Asterinas？

### 一个用 Rust 写的操作系统内核

**为什么用 Rust？**
- 安全：C 语言有内存漏洞，Rust 编译时就检查
- 高效：性能媲美 C
- 现代：类型系统、模块系统、并发安全

**Asterinas 的特点**：
- 兼容 Linux ABI（能用 Linux 的程序）
- 用 Rust 重写，不是 Linux 的分支
- 采用 framekernel 架构

### 什么是 framekernel 架构？

```
┌─────────────────────────────────────────────────────────────┐
│                        Asterinas                            │
│                                                             │
│  ┌──────────────────┐    ┌──────────────────────────────┐  │
│  │  Kernel (safe Rust)│    │     OSTD (unsafe Rust)      │  │
│  │  syscall, VFS,     │    │  MMIO, 中断, 内存, 硬件    │  │
│  │  网络, 进程管理     │    │                             │  │
│  └──────────────────┘    └──────────────────────────────┘  │
│           ↑                          ↑                     │
│           │     只能通过 OSTD API    │                     │
│           └──────────────────────────┘                     │
└─────────────────────────────────────────────────────────────┘

关键思想：
- 所有 unsafe 代码集中在 OSTD（可信代码库小）
- kernel 完全是 safe Rust（无内存漏洞）
```

---

## 这个项目的目标是什么？

### 现状
Asterinas 已有 `aster-framebuffer` 组件，能显示文字到屏幕。

### 目标：添加 DRM 支持

```
阶段目标：
Phase 1 (Week 1): 双缓冲基础        → 不撕裂的测试图案
Phase 2 (Week 2): 控制台共存        → 文字 + 图形同时工作
Phase 3 (Week 3-4): /dev/dri/card0  → 用户程序可以打开 DRM 设备
Phase 4 (Week 5-8): GEM + mmap      → 用户程序可以分配显存
Phase 5 (Week 9+): 完整 KMS         → 标准的 Linux DRM/KMS
```

---

## aster-framebuffer 现有代码解析

### 文件结构

```
kernel/comps/framebuffer/
├── src/
│   ├── lib.rs         # 入口，初始化组件
│   ├── framebuffer.rs # FrameBuffer 结构体（MMIO 映射）
│   ├── pixel.rs       # 像素格式转换
│   ├── console.rs     # 文本控制台渲染
│   ├── ansi_escape.rs # ANSI 转义序列解析
│   └── dummy_console.rs
```

### 核心概念

#### 1. Bootloader 提供帧缓冲信息

开机时，bootloader（OVMF/QEMU固件）会告诉内核：
- 帧缓冲地址（显存物理地址）
- 宽度、高度、每像素位数（BPP）

代码位置：`framebuffer.rs:63-66`

```rust
let Some(framebuffer_arg) = boot_info().framebuffer_arg else {
    log::warn!("Framebuffer not found");
    return;
};
```

#### 2. IoMem — 安全的 MMIO 读写

MMIO（Memory-Mapped I/O）是把硬件寄存器映射到内存地址，CPU 直接读写内存就能操作硬件。

`IoMem` 是 OSTD 提供的安全封装：

```rust
// 获取 MMIO 内存区域
let io_mem = IoMem::acquire_with_cache_policy(
    fb_base..fb_base.checked_add(fb_size).unwrap(),
    CachePolicy::WriteCombining,  // 写合并，加速顺序写入
)
.unwrap();

// 写字节到 MMIO（等价于写显存）
io_mem.write_bytes(offset, bytes)?;
```

#### 3. 像素格式

像素在显存中的存储方式有多种：

| 格式名 | 字节/像素 | 说明 |
|--------|----------|------|
| `Grayscale8` | 1 | 灰度（黑白） |
| `Rgb565` | 2 | 16位彩色（红5绿6蓝5） |
| `Rgb888` | 3 | 24位彩色（每通道8位） |
| `BgrReserved` | 4 | 32位 BGR + 保留 |

转换代码在 `pixel.rs:31-66`：

```rust
impl Pixel {
    pub fn render(&self, format: PixelFormat) -> RenderedPixel {
        match format {
            PixelFormat::Rgb888 => RenderedPixel {
                buf: [self.red, self.green, self.blue],
                len: 3,
            },
            // ...
        }
    }
}
```

#### 4. FrameBuffer 的 API

```rust
pub struct FrameBuffer {
    io_mem: IoMem,           // MMIO 内存映射
    width: usize,            // 宽度（像素）
    height: usize,           // 高度（像素）
    line_size: usize,        // 每行字节数（可能含对齐）
    pixel_format: PixelFormat,
    cmap: Mutex<FbCmap>,     // 颜色表（调色板）
}

impl FrameBuffer {
    pub fn width(&self) -> usize;
    pub fn height(&self) -> usize;
    pub fn line_size(&self) -> usize;
    pub fn pixel_format(&self) -> PixelFormat;
    pub fn write_bytes_at(&self, offset: usize, bytes: &[u8]) -> Result<()>;
    pub fn write_pixel_at(&self, offset: PixelOffset, pixel: RenderedPixel) -> Result<()>;
    pub fn render_pixel(&self, pixel: Pixel) -> RenderedPixel;
}
```

#### 5. PixelOffset — 像素坐标计算

GPU 显存是扁平的字节数组，需要计算 (x, y) 的偏移：

```rust
impl FrameBuffer {
    pub fn calc_offset(&self, x: usize, y: usize) -> PixelOffset<'_> {
        PixelOffset {
            fb: self,
            // 计算：y * 每行字节数 + x * 每像素字节数
            offset: (x * self.pixel_format.nbytes() + y * self.line_size) as isize,
        }
    }
}
```

#### 6. FramebufferConsole — 文本控制台

`FramebufferConsole` 把字符渲染到位图字体，然后写到帧缓冲：

```rust
struct ConsoleState {
    x_pos: usize,           // 光标 x
    y_pos: usize,           // 光标 y
    fg_color: Pixel,        // 前景色
    bg_color: Pixel,        // 背景色
    font: BitmapFont,       // 8x8 位图字体
    bytes: Vec<u8>,         // 控制台内部缓冲区（用于滚动回溯）
    backend: Arc<FrameBuffer>,  // 底层帧缓冲
}
```

关键方法：`draw_char` 渲染一个字符，`newline` 处理换行。

---

## 双缓冲的原理（重点）

### 单缓冲的问题

```
时间线：
T1: 程序写像素 0-100    → 屏幕正在读 0-200（显示上半帧）
T2: 程序写像素 100-200  → 屏幕刷新！（显示混和了新旧内容）
结果：撕裂（tearing）
```

### 双缓冲的解决方案

```
┌─────────────────────────────────────────────────────────────┐
│                         RAM                                 │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                  Back Buffer (后缓冲区)              │   │
│  │         程序在这里写入完整的帧（无中断）               │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                           ↓ present() 一次性复制
┌─────────────────────────────────────────────────────────────┐
│                    MMIO Framebuffer                         │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                Front Buffer (前缓冲区)               │   │
│  │              硬件持续扫描输出到显示器                  │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                           ↓
                    ┌─────────────┐
                    │   显示器    │
                    └─────────────┘

present() 何时调用？
- 方案A：定时器（VSync，60Hz/144Hz）
- 方案B：程序主动通知（Page Flip）
```

### 代码实现

```rust
pub struct SimpleDrm {
    fb: Arc<FrameBuffer>,           // 前缓冲区（硬件 MMIO）
    back_buffer: SpinLock<BackBuffer>, // 后缓冲区（RAM）
}

pub struct BackBuffer {
    data: Vec<u8>,   // RAM 中的像素数据
    width: usize,
    height: usize,
    stride: usize,   // 每行字节数
}

impl SimpleDrm {
    /// 将后缓冲区复制到前缓冲区（present）
    pub fn present(&self) -> ostd::Result<()> {
        let back_data = {
            let back = self.back_buffer.lock();
            back.data().to_vec()  // 复制 RAM 数据
        };
        self.fb.write_bytes_at(0, &back_data)  // 写入 MMIO
    }
}
```

---

## 项目目录结构

```
asterinas-drm/
├── kernel/
│   ├── comps/                    # 可插拔组件
│   │   ├── framebuffer/         # 现有帧缓冲（你需要在旁边创建）
│   │   │   └── src/
│   │   │       ├── lib.rs       # 组件入口
│   │   │       ├── framebuffer.rs # FrameBuffer 结构
│   │   │       ├── pixel.rs      # 像素格式
│   │   │       └── console.rs    # 控制台
│   │   └── drm/                  # ← 你要创建这个！
│   │       └── src/
│   │           ├── lib.rs       # 组件入口
│   │           ├── buffer.rs     # BackBuffer
│   │           └── simple.rs     # SimpleDrm
│   └── src/                      # 内核核心（不要动）
├── ostd/                         # OS 框架（unsafe Rust）
│   └── src/
│       ├── io.rs                 # IoMem（MMIO）
│       ├── mm.rs                 # 内存管理
│       └── sync.rs               # 锁
├── book/                         # 文档
│   └── src/
│       └── kernel/
│           ├── drm-integration.md # DRM 路线图
│           ├── architecture-overview.md # 架构总览
│           ├── week-one-learning-plan.md # Week 1 学习计划
│           └── getting-started-from-zero.md # ← 本文
├── Components.toml               # 组件注册表
└── Makefile                      # 构建命令
```

---

## 常用命令

| 命令 | 作用 |
|------|------|
| `make kernel` | 编译内核 + initramfs |
| `make run_kernel` | 在 QEMU 中运行内核 |
| `make check` | 运行所有 lint |
| `make format` | 格式化代码 |
| `make docs` | 生成 API 文档 |
| `make clean` | 清理编译产物 |

---

## 下一步

1. 阅读 [Week 1 学习计划](week-one-learning-plan.md)，开始动手
2. 阅读 [DRM Integration Guide](drm-integration.md)，了解项目路线图
3. 阅读 [Architecture Overview](architecture-overview.md)，深入架构
