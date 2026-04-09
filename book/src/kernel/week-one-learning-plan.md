# 一周学习计划：从零到跑通 Double Buffer

> 本计划为零基础学习者设计，目标是一周内（每天 2-4 小时）掌握 Asterinas DRM 项目基础知识，并在 Week 1 结束时跑通 `simpledrm` MVP（双缓冲测试图案）。

---

## 背景知识速成（Day 0 — 预习）

> 如果你已有 OS/Rust/内核基础，可跳过 Day 0 直接进入 Day 1。

### 0.1 什么是显示驱动？为什么需要双缓冲？

**单缓冲的问题**：程序直接往屏幕显存写像素，如果写到一半屏幕刷新，就会看到半成品画面——这叫"撕裂"（tearing）。

**双缓冲的原理**：
```
┌──────────────────────────────────────────────────────────┐
│  缓冲区A (RAM)   →   缓冲区B (硬件显存)  →   显示器       │
│  后缓冲区          前缓冲区               扫描输出        │
│  (程序写入)        (present时复制)                         │
└──────────────────────────────────────────────────────────┘

程序只写后缓冲区 → 完整一帧后 → present() 一次性复制到前缓冲区 → 显示器
```

**为什么 present 能避免撕裂**：复制操作极快（RAM → MMIO），在屏幕刷新窗口之间完成。

### 0.2 Rust 语言速成（你只需要懂这些）

| 概念 | 含义 | 代码示例 |
|------|------|---------|
| `&self` | 不可变借用 | `fn width(&self) -> usize` |
| `&mut self` | 可变借用 | `fn write_bytes_at(&mut self, ...)` |
| `Arc<T>` | 原子引用计数（多线程共享） | `Arc<FrameBuffer>` |
| `Mutex<T>` | 互斥锁（线程安全） | `Mutex<FbCmap>` |
| `Result<T, E>` | 可能失败的操作 | `fn write_bytes(...) -> Result<()>` |
| `?` | 错误传播 | `foo()?.bar()` |
| `impl Trait` | 返回迭代器等 | `impl Iterator<Item=u8>` |
| `#[init_component]` | 组件初始化宏 | `#[init_component] fn init() -> ...` |

### 0.3 Asterinas 项目结构速懂

```
asterinas-drm/
├── kernel/          # Safe Rust 内核（无 unsafe）
│   └── comps/       # 可插拔组件（drivers、subsystems）
│       └── framebuffer/  # 帧缓冲组件 ← 你要改这里
│       └── drm/          # DRM 组件 ← 你要创建这里
├── ostd/           # OS 框架（唯一允许 unsafe 的地方）
│   └── src/
│       ├── io.rs    # IoMem（MMIO 内存映射）
│       └── mm.rs    # 内存管理
├── book/           # 文档
└── Makefile        # 构建命令
```

---

## Day 1 — 搭建环境 + 跑通现有代码

### 目标
- 在 Docker 中编译并运行 Asterinas
- 看到帧缓冲输出
- 理解项目构建流程

### 任务清单

- [ ] **1.1 Docker 环境**
  ```bash
  docker run -it --privileged --network=host -v /dev:/dev \
    -v $(pwd)/asterinas:/root/asterinas \
    asterinas/asterinas:0.17.1-20260319
  ```

- [ ] **1.2 编译内核**
  ```bash
  cd /root/asterinas
  make kernel
  ```

- [ ] **1.3 运行 QEMU**
  ```bash
  make run_kernel
  ```
  观察：能看到帧缓冲控制台输出吗？记录分辨率和颜色。

- [ ] **1.4 理解 Makefile 目标**
  ```bash
  make help    # 查看所有可用目标
  make check   # 运行 lint (fmt, clippy, typos)
  ```

### 验收标准
✅ `make kernel` 编译成功
✅ `make run_kernel` 在 QEMU 中看到输出

---

## Day 2 — 读懂 aster-framebuffer 代码

### 目标
- 理解 `FrameBuffer` 核心结构
- 理解像素格式转换
- 找到所有公开 API

### 任务清单

- [ ] **2.1 精读 `framebuffer.rs`**
  关键问题：
  - `FrameBuffer` 有哪些公开方法？
  - `write_bytes_at` 是怎么工作的？
  - `PixelOffset` 怎么计算像素偏移？

- [ ] **2.2 精读 `pixel.rs`**
  关键问题：
  - `Pixel` 和 `RenderedPixel` 的区别？
  - 4 种像素格式的字节大小？

- [ ] **2.3 精读 `console.rs` 第 1 部分（ConsoleState）**
  关键问题：
  - `ConsoleState` 的 `bytes: Vec<u8>` 是什么用途？
  - `draw_char` 如何渲染一个字符到帧缓冲？
  - `shift_lines_up` 什么时候被调用？

- [ ] **2.4 画架构图**
  用自己的话，画出从 bootloader 到屏幕的数据流。

### 验收标准
✅ 能回答上面的关键问题
✅ 能画出数据流架构图

---

## Day 3 — 理解 framekernel 架构 + OSTD API

### 目标
- 理解 safe/unsafe 边界
- 学会查 OSTD API 文档
- 理解组件初始化机制

### 任务清单

- [ ] **3.1 阅读 `AGENTS.md` 架构部分**
  重点理解：
  - 为什么 `kernel/` 不能有 `unsafe`？
  - OSTD 负责什么？
  - 组件系统怎么工作的？

- [ ] **3.2 查看 OSTD API 文档**
  ```bash
  make docs
  # 在 target/doc/ 下查看
  ```
  重点关注：
  - `ostd::io::IoMem` — MMIO 读写
  - `ostd::sync::Mutex` / `SpinLock` — 锁
  - `ostd::boot::boot_info` — 启动信息

- [ ] **3.3 理解组件初始化**
  阅读 `kernel/comps/framebuffer/src/lib.rs`：
  - `#[init_component]` 的作用？
  - `FRAMEBUFFER.call_once` 的含义？

- [ ] **3.4 理解 `Components.toml`**
  查看 `Components.toml`，找到 `framebuffer` 条目。
  问自己：添加新组件需要改哪里？

### 验收标准
✅ 能解释 safe/unsafe 边界
✅ 能找到 `IoMem::write_bytes` 的文档

---

## Day 4 — 设计 aster-drm 组件结构

### 目标
- 设计 `aster-drm` 组件的代码结构
- 编写 `Cargo.toml` 和目录布局
- 写 `BackBuffer` 和 `SimpleDrm` 的骨架代码

### 任务清单

- [ ] **4.1 创建目录结构**
  ```
  kernel/comps/drm/
  ├── Cargo.toml
  └── src/
      ├── lib.rs
      ├── buffer.rs    # BackBuffer
      └── simple.rs    # SimpleDrm
  ```

- [ ] **4.2 编写 `Cargo.toml`**
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

  [lints]
  workspace = true
  ```

- [ ] **4.3 实现 `BackBuffer`（`buffer.rs`）**
  ```rust
  pub struct BackBuffer {
      data: SpinLock<Vec<u8>>,
      width: usize,
      height: usize,
      stride: usize,
  }

  impl BackBuffer {
      pub fn new(width: usize, height: usize, stride: usize) -> Self;
      pub fn width(&self) -> usize;
      pub fn height(&self) -> usize;
      pub fn stride(&self) -> usize;
      pub fn data(&self) -> &[u8];
      pub fn clear(&self);
  }
  ```

- [ ] **4.4 实现 `SimpleDrm`（`simple.rs`）**
  ```rust
  pub struct SimpleDrm {
      fb: Arc<FrameBuffer>,
      back_buffer: SpinLock<BackBuffer>,
  }

  impl SimpleDrm {
      pub fn new(fb: Arc<FrameBuffer>) -> Self;
      pub fn back_buffer(&self) -> &SpinLock<BackBuffer>;
      pub fn present(&self) -> ostd::Result<()>;
  }
  ```

- [ ] **4.5 编写 `lib.rs`**
  ```rust
  #![deny(unsafe_code)]
  extern crate alloc;

  mod buffer;
  mod simple;

  use component::{init_component, ComponentInitError};
  pub use buffer::BackBuffer;
  pub use simple::SimpleDrm;

  #[init_component]
  fn init() -> Result<(), ComponentInitError> {
      // TODO: 注册 DRM 设备
      Ok(())
  }
  ```

- [ ] **4.6 注册到 `Components.toml`**
  ```toml
  drm = { name = "aster-drm" }
  ```

### 验收标准
✅ 目录结构创建完成
✅ `Cargo.toml` 写对
✅ 骨架代码编译通过（`cargo build -p aster-drm`）

---

## Day 5 — 实现测试图案 + 跑通 present()

### 目标
- 实现 `SimpleDrm::present()`
- 编写内核线程生成测试图案
- 在 QEMU 中验证双缓冲效果

### 任务清单

- [ ] **5.1 实现 `present()` 逻辑**
  ```rust
  pub fn present(&self) -> ostd::Result<()> {
      let back_data = {
          let back = self.back_buffer.lock();
          back.data().to_vec()
      };
      self.fb.write_bytes_at(0, &back_data)
  }
  ```

- [ ] **5.2 编写测试图案生成器**
  在 `simple.rs` 中添加：
  ```rust
  /// 绘制棋盘格测试图案到后缓冲区
  pub fn draw_checkerboard(&self) {
      let mut back = self.back_buffer.lock();
      let data = back.data_mut();
      let pixel_size = self.fb.pixel_format().nbytes();
      let block_size = 32; // 每块 32 像素

      for y in 0..back.height() {
          for x in 0..back.width() {
              let is_white = ((x / block_size) + (y / block_size)) % 2 == 0;
              let color = if is_white { 0xFF } else { 0x00 };
              let offset = (y * back.stride() + x * pixel_size) as usize;
              // ... 写入像素
          }
      }
  }
  ```

- [ ] **5.3 启动内核线程调用测试**
  修改 `lib.rs` 的 `init()`：
  ```rust
  use ostd::task::Task;

  #[init_component]
  fn init() -> Result<(), ComponentInitError> {
      let drm = SimpleDrm::new(/* 从 FRAMEBUFFER 获取 */);

      Task::new_kernel_thread(|| {
          loop {
              drm.draw_checkerboard();
              drm.present().unwrap();
              // 简单延时
          }
      }).spawn();

      Ok(())
  }
  ```

- [ ] **5.4 编译 + 运行**
  ```bash
  make kernel
  make run_kernel
  ```

### 验收标准
✅ `cargo build -p aster-drm` 成功
✅ `make kernel` 成功
✅ QEMU 中看到棋盘格图案（无撕裂）

---

## Day 6 — 理解并发安全 + 优化 present()

### 目标
- 理解为什么 `present()` 需要锁
- 实现双缓冲的完整安全保证
- 添加调试日志

### 任务清单

- [ ] **6.1 分析并发问题**
  问自己：
  - 如果两个线程同时调用 `present()` 会发生什么？
  - 如果渲染线程写 `BackBuffer` 的同时 `present()` 在读，会发生什么？
  - 当前的 `SpinLock` 足够吗？

- [ ] **6.2 添加日志**
  ```rust
  log::info!("SimpleDrm: present called, copying {} bytes", back_data.len());
  ```

- [ ] **6.3 尝试渐变图案**
  ```rust
  pub fn draw_gradient(&self) {
      let mut back = self.back_buffer.lock();
      for y in 0..back.height() {
          let intensity = (y * 255 / back.height()) as u8;
          // 写入渐变行
      }
  }
  ```

### 验收标准
✅ 理解并发问题
✅ 有日志输出
✅ 渐变和棋盘格都能正常显示

---

## Day 7 — 总结 + 准备 Phase 2

### 目标
- 整理学习笔记
- 总结遇到的问题和解决方案
- 制定 Phase 2 计划

### 任务清单

- [ ] **7.1 写学习总结**
  回答以下问题：
  - Asterinas 的 framekernel 架构核心思想是什么？
  - `FrameBuffer` 的核心 API 有哪些？
  - 双缓冲的原理是什么？
  - `SimpleDrm` 的结构是怎样的？

- [ ] **7.2 Code Review 自查**
  - [ ] 代码符合 Asterinas 编码规范吗？
  - [ ] 公开 API 有文档注释吗？
  - [ ] 错误处理用 `?` 了吗？
  - [ ] 有 `#[deny(unsafe_code)]` 吗？

- [ ] **7.3 制定 Phase 2 计划**
  根据 `drm-integration.md` 的 Phase 2 内容，制定：
  - 如何让 `FramebufferConsole` 和 DRM 双缓冲共存？
  - 需要什么同步机制？
  - 如何验证？

### 验收标准
✅ 学习笔记完整
✅ 代码通过 `make check`
✅ 能回答所有总结问题

---

## 里程碑验收

```
┌─────────────────────────────────────────────────────────────┐
│                    Week 1 验收清单                          │
├──────────────────┬──────────────────────────────────────────┤
│ Day 1 ✅         │ 环境搭建成功，QEMU 运行正常              │
│ Day 2 ✅         │ 能读懂 framebuffer 代码，回答问题        │
│ Day 3 ✅         │ 理解 framekernel 和 OSTD API             │
│ Day 4 ✅         │ aster-drm 骨架编译通过                   │
│ Day 5 ✅         │ 棋盘格测试图案跑通，present() 工作      │
│ Day 6 ✅         │ 并发安全理解，日志添加                   │
│ Day 7 ✅         │ 学习总结完成，Phase 2 计划制定            │
└──────────────────┴──────────────────────────────────────────┘

🎯 最终目标：QEMU 中看到稳定的棋盘格/渐变图案，无撕裂
```

---

## 常见问题

### Q: `make kernel` 编译失败
A: 检查 Docker 环境是否正确。尝试：
```bash
make clean
make kernel
```

### Q: `cargo build -p aster-drm` 找不到依赖
A: 确认在 `/root/asterinas` 目录下，并检查 `kernel/Cargo.toml` 是否添加了依赖路径。

### Q: QEMU 中看不到图案
A: 检查 QEMU 是否有帧缓冲支持。确认虚拟机有配置 virtio-gpu 或 stdvga。

### Q: present() 后图案闪烁
A: 这是正常的——如果没有使用垂直同步（VSync），每帧都可能闪烁。Phase 2 会添加 VSync 支持。

---

## 参考资源

- [DRM Integration Guide](drm-integration.md) — 项目 DRM 路线图
- [Architecture Overview](architecture-overview.md) — 项目架构总览
- [aster-framebuffer 源码](../comps/framebuffer/src/) — 现有帧缓冲实现
- [SimpleDRM Design (English)](simpledrm-design-en.md) — 详细设计文档
