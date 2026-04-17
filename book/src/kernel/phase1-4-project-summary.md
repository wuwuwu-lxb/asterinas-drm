# Asterinas DRM 项目完整技术总结

**分支**: `drm`  
**最后更新**: 2026-04-16  
**文档状态**: 与当前代码和验证结果对齐

> **2026-04-16 关键结论**：最小 DRM/KMS bring-up 已跑通。`double_framebuffer` 的核心流程 `GETRESOURCES -> GETCONNECTOR -> GETENCODER -> CREATE_DUMB -> ADDFB -> MAP_DUMB -> SET_CRTC -> PAGE_FLIP` 已能驱动可见输出；当前实现属于 **firmware framebuffer 上的最小 SimpleDRM/KMS 框架**，还不是完整 GPU DRM 驱动。

---

## 目录

1. [项目目标与当前结论](#1-项目目标与当前结论)
2. [我们在 Add minimal DRM 文档里的哪个位置](#2-我们在-add-minimal-drm-文档里的哪个位置)
3. [当前架构总览](#3-当前架构总览)
4. [最小显示链路是怎么跑通的](#4-最小显示链路是怎么跑通的)
5. [各模块怎么实现](#5-各模块怎么实现)
6. [关键修复与踩坑记录](#6-关键修复与踩坑记录)
7. [当前能力边界](#7-当前能力边界)
8. [后续工作建议](#8-后续工作建议)
9. [关键文件索引](#9-关键文件索引)

---

## 1. 项目目标与当前结论

### 1.1 项目目标

本项目的目标是为 Asterinas 提供一条 **Linux 兼容的最小 DRM/KMS 路径**，让用户态能够按 libdrm 预期访问 `/dev/dri/card0`，枚举 KMS 对象，创建 dumb buffer，注册 framebuffer，并通过 `SET_CRTC` / `PAGE_FLIP` 把图像真正显示到屏幕上。

这条路径优先解决的是：

- 设备节点存在：`/dev/dri/card0`
- ioctl 编号和结构体布局与 Linux DRM/libdrm 对齐
- 用户态能够 `mmap` dumb buffer
- KMS ioctl 不只是“返回成功”，而是能真的把像素刷到可见 framebuffer

### 1.2 当前已经完成的结果

截至 2026-04-16，当前分支已经完成：

- `/dev/dri/card0` 已注册并可打开
- 最小 KMS 对象模型已建立：1 个 CRTC、1 个 Encoder、1 个 Connector
- Dumb buffer 已支持 `CREATE_DUMB / MAP_DUMB / DESTROY_DUMB`
- Framebuffer 已支持 `ADDFB / RMFB / GETFB`
- `SET_CRTC` / `PAGE_FLIP` 已接入真正的 present 路径
- `double_framebuffer` 已跑通，VNC 可见输出已验证
- 当前模式信息会根据真实 `FRAMEBUFFER` 动态上报，而不是写死 1024x768

### 1.3 当前实现的定位

这套实现的定位不是“完整 Linux DRM 子系统”，而是：

> **把 firmware/boot framebuffer 包装成一个最小可用的 DRM/KMS 设备，并让用户态 dumb buffer 内容通过 KMS 路径显示出来。**

也就是说，它更接近设计文档里的 **SimpleDRM minimal bring-up**，而不是完整的 GPU 原生驱动。

---

## 2. 我们在 Add minimal DRM 文档里的哪个位置

参考文档：`/home/wuwuwu/文档/Add minimal DRM framework and initial simple ioctl support.md`

这份设计文档的主线可以概括成四部分：

1. **Background**：从旧 fbdev 走向 SimpleDRM / KMS
2. **Minimal DRM framework**：建立最小 KMS 对象模型与内存模型
3. **Driver**：注册 DRM 设备并对外暴露 `/dev/dri/cardX`
4. **Simple user usage**：让用户态按标准 libdrm/KMS 流程跑起来

当前所处位置如下：

| 设计文档部分 | 当前状态 | 说明 |
|---|---|---|
| Background：从 fbdev 迁移到 DRM/KMS | ✅ 已落地 | 当前主路径已经是 `/dev/dri/card0` + KMS ioctl，而不是依赖 `/dev/fb0` |
| Minimal DRM framework：KMS 基础对象 | ✅ 已完成最小实现 | 已提供固定的 CRTC / Encoder / Connector 拓扑 |
| Minimal DRM framework：memory backend | ✅ 已完成最小实现 | 采用 dumb buffer + 全局 VMO 槽位方案，支持 `mmap` |
| Driver：DRM 设备注册 | ✅ 已完成 | 已注册 `major=226` 的 `/dev/dri/card0` |
| Simple user usage：最小 modeset 流程 | ✅ 已跑通 | `double_framebuffer` 已完成完整链路并显示成功 |
| 文档中的 GEM 抽象化方向 | 🔄 只做了最小骨架 | 当前 `gem.rs` 仍是预留位，尚未形成完整通用 GEM 框架 |
| 多 plane / 原子模式设置 / 热插拔 | ❌ 未实现 | 还不在当前里程碑范围内 |
| 原生 Virtio-GPU 扫描输出 | 🔄 未完成 | 当前显示依赖已有 `FRAMEBUFFER`，不是完整 virtio-gpu modeset |

### 2.1 用一句话描述当前位置

如果按那份 Add minimal 文档来定位：

> **我们已经完成了“Minimal DRM framework + initial simple ioctl support”的核心可运行版本，并且把文档里那条 simple user usage 的最小 KMS 流程真正跑通了。**

但还没有进入“完整 GPU DRM driver / 通用 GEM / 高级 KMS 功能”的阶段。

---

## 3. 当前架构总览

### 3.1 总体分层

```text
Userspace (libdrm / test program)
    │
    │ open + ioctl + mmap
    ▼
/dev/dri/card0
    │
    ▼
DrmFileHandle (ioctl dispatch + mappable VMO)
    │
    ├── kms.rs   : KMS 对象与模式设置
    ├── dumb.rs  : dumb buffer / VMO 槽位管理
    ├── fb.rs    : fb_id 管理与 present
    └── ioctl_defs.rs : Linux DRM ABI 定义
    │
    ▼
aster_framebuffer::FRAMEBUFFER
    │
    ▼
Firmware/boot framebuffer
(VNC 可见输出)
```

### 3.2 关键设计思想

当前架构的核心不是让 GPU 直接扫描用户态分配的缓冲，而是走一条更小、更容易 bring-up 的路径：

1. 用户态通过 DRM 创建 dumb buffer；
2. dumb buffer 通过 VMO 暴露给 `mmap`；
3. 用户态往 dumb buffer 写像素；
4. `ADDFB` 把 dumb buffer 注册成 `fb_id`；
5. `SET_CRTC` / `PAGE_FLIP` 调用 present 逻辑；
6. present 逻辑把 dumb buffer 内容逐行拷贝到 `FRAMEBUFFER`；
7. `FRAMEBUFFER` 对应的 firmware framebuffer 被 VNC 看到。

所以这是一套：

> **DRM userspace buffer -> 内核 present -> firmware framebuffer**

的桥接架构。

### 3.3 为什么这样设计

这是当前阶段最合理的方案，因为它同时满足：

- 对外接口是标准 DRM/KMS
- 内核实现复杂度可控
- 不需要一开始就做完整 GPU 内存管理
- 能尽快验证 libdrm userspace 是否能与内核协议打通
- 能尽快获得可见显示结果

---

## 4. 最小显示链路是怎么跑通的

### 4.1 用户态最小流程

当前已经跑通的最小链路就是设计文档里的这条：

```text
GETRESOURCES
  -> GETCONNECTOR
  -> GETENCODER
  -> CREATE_DUMB
  -> ADDFB
  -> MAP_DUMB
  -> mmap
  -> 写像素
  -> SET_CRTC
  -> PAGE_FLIP
```

### 4.2 内核侧对应的数据流

```text
CREATE_DUMB
  -> dumb.rs 分配 handle、pitch、size、VMO slot

MAP_DUMB
  -> 返回 fake offset（实际上是 VMO slot offset）

mmap(/dev/dri/card0, offset)
  -> file.rs::mappable() 返回整个 dumb-buffer VMO
  -> 用户态映射到对应 slot

ADDFB
  -> fb.rs 建立 fb_id -> handle/width/height/bpp 的关联

SET_CRTC / PAGE_FLIP
  -> kms.rs 校验 CRTC / connector / mode / fb_id
  -> fb.rs::present_fb(fb_id)
  -> dumb.rs::snapshot(handle) 读取 VMO 中真实像素
  -> fb.rs 逐行 blit 到 FRAMEBUFFER
  -> VNC 看到更新后的画面
```

### 4.3 为什么最后能显示

关键点在于：

- 用户态写入的是 **mmap 出来的 dumb buffer VMO**；
- 显示时不能读旧的 shadow buffer，必须读 **VMO 里的真实内容**；
- `present_fb()` 把读出来的数据写入 `aster_framebuffer::FRAMEBUFFER`；
- 当 `FRAMEBUFFER` 背后是可见 framebuffer（例如 `linux-efi-handover64` 路径下的 firmware framebuffer）时，VNC 就能看到结果。

---

## 5. 各模块怎么实现

### 5.1 `mod.rs` / `init.rs`：DRM 设备入口

- `kernel/src/device/drm/mod.rs`
- `kernel/src/device/drm/init.rs`

职责：

- 注册字符设备 `/dev/dri/card0`
- 使用 Linux 标准 DRM major 号 `226`
- 启动时初始化 dumb buffer manager 和 framebuffer manager

这部分对应设计文档里的 **Driver**。

### 5.2 `file.rs`：文件句柄与 ioctl 分发

- `kernel/src/device/drm/file.rs`

职责：

- 提供 `DrmFileHandle`
- 实现 `ioctl()` 分发
- 实现 `mappable()`，把 dumb buffer 所在的 VMO 暴露给用户态 `mmap`

这里是 `/dev/dri/card0` 的核心入口。

当前已实现的 ioctl 包括：

- 通用：`VERSION`、`GET_CAP`
- KMS：`GETRESOURCES`、`GETCONNECTOR`、`GETENCODER`、`GETCRTC`、`SETCRTC`、`PAGE_FLIP`
- Framebuffer：`ADDFB`、`RMFB`、`GETFB`
- Dumb buffer：`CREATE_DUMB`、`MAP_DUMB`、`DESTROY_DUMB`

### 5.3 `ioctl_defs.rs`：DRM ABI 层

- `kernel/src/device/drm/ioctl_defs.rs`

职责：

- 定义 ioctl magic 和 NR
- 定义所有与 libdrm 对齐的结构体布局
- 保证用户态和内核态对同一块内存的解释一致

这是整个项目最关键的 ABI 文件之一。

当前实现遵循：

- magic = `0x64`（`'d'`）
- NR 与 Linux DRM 标准一致
- `#[repr(C)]`
- 数据结构按 libdrm 布局对齐

### 5.4 `kms.rs`：最小 KMS 对象模型

- `kernel/src/device/drm/kms.rs`

职责：

- 实现固定的 KMS 拓扑：1 CRTC、1 Encoder、1 Connector
- 响应资源枚举 ioctl
- 维护当前模式与当前显示 framebuffer 状态
- 在 `SET_CRTC` / `PAGE_FLIP` 中驱动 present

当前对象模型是简化版：

- `CRTC_ID = 1`
- `ENCODER_ID = 1`
- `CONNECTOR_ID = 1`

当前模式通过 `FRAMEBUFFER` 动态推导，这样上报给用户态的 mode 与真实可见 framebuffer 尺寸一致，避免了 modeset 时的 `EINVAL`。

### 5.5 `dumb.rs`：dumb buffer 与 mmap 方案

- `kernel/src/device/drm/dumb.rs`

职责：

- 分配 dumb buffer handle
- 计算 pitch / size
- 管理全局 VMO
- 为每个 dumb buffer 分配固定 slot
- 为 `MAP_DUMB` 返回可供 `mmap` 使用的 fake offset
- 在 present 时返回指定 handle 的像素快照

#### 当前内存模型

```text
一个全局 VMO
┌──────────────────────────────────────────────┐
│ slot 0 | slot 1 | slot 2 | ... | slot 255  │
└──────────────────────────────────────────────┘
```

- 每个 slot 大小固定为 16MB
- `handle = 1` 对应 slot 0
- `handle = 2` 对应 slot 1
- `MAP_DUMB` 返回的 offset 其实是 slot 起始偏移

这就是设计文档和学长建议里提到的那个 hack：

> DRM 语义里的 offset 不是“真实物理偏移”，而是供 `mmap` 再解释的 token。当前实现通过“一个大 VMO + 固定 slot offset”把这件事跑通了。

#### 当前快照逻辑

现在 `snapshot()` 会从 **VMO** 读取真实内容，而不是读取旧的 shadow `Vec<u8>`。这是最后跑通显示的关键修复之一。

### 5.6 `fb.rs`：`fb_id` 管理与 present 桥接

- `kernel/src/device/drm/fb.rs`

职责：

- 管理 `fb_id -> handle/width/height/bpp`
- 支持 `ADDFB / RMFB / GETFB`
- 记录当前 active framebuffer
- 在 `present_fb(fb_id)` 中完成真正的屏幕更新

这里的 `present_fb()` 是当前架构的核心桥：

```text
fb_id
  -> handle
  -> dumb buffer snapshot
  -> 校验尺寸/stride/bpp
  -> row-by-row blit
  -> FRAMEBUFFER
```

当前采用 **逐行拷贝**，而不是整块 memcpy，这是为了兼容：

- dumb buffer 自身 pitch
- 真实 framebuffer 的 line size
- 两边 stride 不完全相等的情况

### 5.7 `FRAMEBUFFER`：最终可见输出目标

- `kernel/comps/framebuffer/src/framebuffer.rs`

职责：

- 表示当前系统实际使用的 framebuffer
- 暴露 `FrameBufferOps`
- 提供 `write_bytes_at()` 让 DRM present 路径写入像素

当前 DRM 层并不直接操控 GPU 硬件扫描，而是把 `FRAMEBUFFER` 当作最终扫描目标。

因此，当前实现能否“看见画面”，取决于 `FRAMEBUFFER` 背后是否是真实可见 framebuffer。

---

## 6. 关键修复与踩坑记录

### 6.1 ioctl magic 和 NR 不对

早期实现里，DRM ioctl 的 magic / NR 与 Linux 标准不一致，会导致 libdrm 发起的 ioctl 无法正确命中。

修复后：

- 使用 `magic = 0x64`
- 使用 Linux DRM 标准 NR

### 6.2 struct ABI 不对齐

如果 `DrmModeModeInfo`、`DrmModeCrtc`、`DrmModeGetConnector` 等结构布局不对，内核虽然“收到了 ioctl”，但字段解释会错位，最终表现为：

- `ENOTTY`
- `EINVAL`
- 或者用户态拿到脏数据

这一轮已经把关键结构改为与 libdrm ABI 对齐。

### 6.3 模式上报和真实 framebuffer 尺寸不一致

早期把 mode 写死成 `1024x768`，而实际 `FRAMEBUFFER` 可能是 `1280x800`。这样 `SET_CRTC` 会因为用户态请求模式与真实显示路径不匹配而失败。

现在修复为：

- `current_mode()` 从 `FRAMEBUFFER` 动态生成 mode
- `GETCONNECTOR` / `GETCRTC` / `SET_CRTC` 统一使用这套动态 mode

### 6.4 `SET_CRTC` / `PAGE_FLIP` 之前只改状态，不改画面

早期实现只更新 `current_fb_id`，但没有把选中的 framebuffer 内容真正刷到屏幕，所以 userspace 看起来“成功了”，VNC 却没变化。

现在修复为：

- `SET_CRTC` -> `present_fb()`
- `PAGE_FLIP` -> `present_fb()`

也就是说，这两个 ioctl 都已经接入真实 present 路径。

### 6.5 present 之前读的是 stale 数据

这是最后一个真正导致黑屏的问题。

问题根因：

- 用户态通过 `mmap` 写入的是 VMO
- 但内核 present 早期读的是 `buffer.data.clone()`
- 两边不是同一份真实数据源

最终修复：

- `snapshot()` 改为从 `self.vmo.read_bytes(...)` 读取对应 slot 的真实内容

这一步之后，`double_framebuffer` 才真正显示成功。

### 6.6 启动协议差异

当前可见输出与启动协议强相关：

- `multiboot2`：经常拿不到真实 firmware framebuffer，容易落到 `RamFrameBuffer`
- `linux-efi-handover64`：更容易拿到真实可见 framebuffer，因此 VNC 验证链路更稳定

所以这次 bring-up 跑通，关键验证路径是：

```bash
make BOOT_PROTOCOL=linux-efi-handover64 run_kernel
```

---

## 7. 当前能力边界

当前实现已经够支撑“最小 DRM/KMS 跑通”，但边界也很明确。

### 7.1 已具备的能力

- 最小 DRM 字符设备
- 最小 KMS 资源枚举
- dumb buffer 创建 / 映射 / 销毁
- framebuffer 创建 / 查询 / 删除
- modeset / page flip 的基础显示路径
- 用户态软件渲染后显示到可见 framebuffer

### 7.2 仍然没有的能力

- 完整通用 GEM 对象模型
- plane / overlay / cursor plane
- atomic modesetting
- connector hotplug
- 完整 property 体系
- fbdev 兼容层 `/dev/fb0`
- 原生 virtio-gpu scanout / command submission
- 零拷贝显示

因此当前版本是：

> **能跑通最小 libdrm/KMS 流程的 SimpleDRM 风格实现**，不是完整 GPU DRM 驱动。

---

## 8. 后续工作建议

### 8.1 第一优先级：巩固最小 DRM 路径

- 用更多标准 libdrm 示例程序验证
- 增加回归测试，覆盖：
  - `SET_CRTC`
  - `PAGE_FLIP`
  - `GETFB`
  - 非法 `fb_id` / `handle` / mode 的错误路径

### 8.2 第二优先级：补足兼容层与工具链验证

- 评估是否需要 `/dev/fb0` 兼容层
- 引入 `modetest` 等工具验证最小 KMS 能力
- 补上文档中尚未完全落地的 GEM 抽象

### 8.3 第三优先级：向真实 GPU DRM 驱动演进

- 把当前 firmware framebuffer bridge 演进成真正的 virtio-gpu scanout 路径
- 研究 plane / property / atomic 模型
- 为后续 Weston / Mesa bring-up 做准备

---

## 9. 关键文件索引

### DRM 入口与设备注册

- `kernel/src/device/drm/mod.rs`
- `kernel/src/device/drm/init.rs`
- `kernel/src/device/drm/file.rs`

### 协议与 ABI

- `kernel/src/device/drm/ioctl_defs.rs`

### KMS / Dumb Buffer / Framebuffer

- `kernel/src/device/drm/kms.rs`
- `kernel/src/device/drm/dumb.rs`
- `kernel/src/device/drm/fb.rs`

### 最终显示目标

- `kernel/comps/framebuffer/src/framebuffer.rs`

### 相关组件

- `kernel/comps/virtio/src/device/gpu/device.rs`
- `kernel/comps/framebuffer/src/framebuffer.rs`

---

*本文档描述的是当前分支实际已经跑通的最小 DRM/KMS 架构。后续如果开始实现完整 GEM、原生 virtio-gpu 扫描输出或 atomic KMS，需要重新更新本文档的架构定位。*
