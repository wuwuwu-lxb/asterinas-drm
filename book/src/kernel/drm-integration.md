# DRM 总体文档

这是当前仓库 DRM 方向的唯一总文档。

## 唯一验收标准

只有一个标准：

**在当前 Asterinas 内核上，添加 `device/drm` 能力，并跑通 double framebuffer 全链路。**

---

## 当前状态（已按代码核对）

### 已完成

1. 已存在 `aster-drm` 组件：
   - `kernel/comps/drm/src/lib.rs`
   - `kernel/comps/drm/src/buffer.rs`
   - `kernel/comps/drm/src/simple.rs`
2. 组件接线已完成：
   - `Components.toml` includes `drm = { name = "aster-drm" }`
   - workspace dependencies include `aster-drm`
3. double framebuffer 基础路径已具备：
   - `BackBuffer` stores a RAM back buffer
   - `SimpleDrm::present()` copies back-buffer bytes into hardware framebuffer

### 未完成

1. 目前没有对外可用的 `/dev/dri/card0` 设备节点。
2. 目前没有完整的用户态 DRM ioctl 访问链路。
3. 目前没有“`device/drm` + double framebuffer”端到端跑通的验证结果。

---

## 最小闭环交付

要满足唯一标准，至少需要以下闭环：

1. **设备层**：提供内核可见、用户可访问的 `device/drm`（例如 `/dev/dri/card0`）。
2. **缓冲层**：`BackBuffer -> present() -> 硬件 framebuffer` 链路稳定可用。
3. **验证层**：在目标 VM/QEMU 环境可重复验证。

---

## 通过条件

只有同时满足以下条件才算完成：

1. `device/drm` 可访问；
2. 图像数据可从后缓冲成功提交到前缓冲并显示；
3. 结果可重复复现，而非偶发成功。

---

## 相关代码位置

- DRM 组件：`kernel/comps/drm/`
- 现有 framebuffer 组件：`kernel/comps/framebuffer/`
- 组件注册：`Components.toml`
- 设备与系统调用路径：`kernel/src/device/`、`kernel/src/syscall/`

---

## 参考

- [Architecture Overview](architecture-overview.md)
- [The Framekernel Architecture](the-framekernel-architecture.md)
