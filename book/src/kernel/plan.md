# Asterinas DRM 项目计划

**分支**: `drm`  
**最后更新**: 2026-04-16  
**文档状态**: 活跃

---

## 项目目标

实现 Linux 兼容的最小 DRM/KMS 框架，让用户态能够通过 `/dev/dri/card0` 完成资源枚举、dumb buffer 分配、framebuffer 注册和基本 modeset/present，并为后续向完整 GPU DRM 驱动演进打基础。

---

## 当前状态总览

| 组件 | 状态 | 说明 |
|------|------|------|
| `/dev/dri/card0` | ✅ 完成 | DRM 字符设备，major=226 |
| DRM ABI 对齐 | ✅ 完成 | ioctl magic/NR/struct layout 已对齐 libdrm |
| KMS 基础对象 | ✅ 完成 | 1 CRTC + 1 Encoder + 1 Connector 的最小拓扑 |
| Dumb Buffer | ✅ 完成 | `CREATE_DUMB / MAP_DUMB / DESTROY_DUMB` 已可用 |
| Framebuffer 管理 | ✅ 完成 | `ADDFB / RMFB / GETFB` 已可用 |
| Present 路径 | ✅ 完成 | `SET_CRTC / PAGE_FLIP` 会真正把像素刷到可见 framebuffer |
| double_framebuffer 测试 | ✅ 通过 | 2026-04-16 验证通过 |
| Add minimal 文档主流程 | ✅ 跑通 | 最小 libdrm/KMS 流程已走通 |
| `/dev/fb0` 兼容层 | ❌ 未实现 | 当前没有 fbdev 字符设备 |
| 完整 GEM 抽象 | ❌ 未实现 | `gem.rs` 仍是预留骨架 |
| 原生 Virtio-GPU scanout | 🔄 进行中 | 当前仍依赖 `FRAMEBUFFER` bridge |
| 高级 KMS 功能 | ❌ 未实现 | plane/property/atomic/hotplug 均未做 |

---

## 里程碑结论

### 已完成里程碑

当前已经完成的是：

> **Minimal DRM framework and initial simple ioctl support 的可运行版本。**

具体体现在：

- 用户态可以按标准 DRM/KMS 顺序访问设备；
- dumb buffer 能 `mmap`；
- framebuffer 能注册为 `fb_id`；
- `SET_CRTC` / `PAGE_FLIP` 不再只是逻辑状态更新，而是能驱动真实 present；
- `double_framebuffer` 已经完成从用户态写像素到 VNC 可见输出的闭环。

### 当前仍不是的东西

当前还不是：

- 完整 Linux DRM 子系统
- 通用 GEM/PRIME 实现
- 原生 GPU 扫描输出驱动
- 支持现代 compositor 的完整 KMS 实现

---

## 当前测试验证矩阵

| 测试 | 状态 | 日期 | 说明 |
|------|------|------|------|
| `double_framebuffer` 最小链路 | ✅ | 2026-04-16 | 可见输出已跑通 |
| `GETRESOURCES/GETCONNECTOR/GETENCODER` | ✅ | 2026-04-16 | 资源枚举正常 |
| `CREATE_DUMB/MAP_DUMB/ADDFB` | ✅ | 2026-04-16 | 映射和 framebuffer 注册正常 |
| `SET_CRTC` | ✅ | 2026-04-16 | 已接入真实 present |
| `PAGE_FLIP` | ✅ | 2026-04-16 | 已复用同一 present 路径 |
| `GETFB` | ✅ | 2026-04-16 | 返回实际 pitch/handle |
| 标准 libdrm 工具链更多样例 | ⏳ | — | 仍需补测 |
| `modetest` / 更复杂 KMS 工具 | ⏳ | — | 仍需补测 |

---

## 当前架构定位

当前架构是：

```text
userspace dumb buffer
    ↓ mmap writes
VMO-backed dumb buffer
    ↓ present on SET_CRTC / PAGE_FLIP
fb_id -> handle -> snapshot -> row-by-row blit
    ↓
aster_framebuffer::FRAMEBUFFER
    ↓
firmware / boot framebuffer
```

这意味着当前实现本质上是：

- 对外暴露标准 DRM/KMS 接口；
- 对内使用 dumb buffer + VMO 作为最小内存后端；
- 最终通过 present 桥接到已有 `FRAMEBUFFER`；
- 先把最小 userspace 协议跑通，再逐步向真正 GPU scanout 演进。

---

## 技术债务

### 高优先级

1. **补充标准 libdrm 回归验证**
   - 现在已经有 `double_framebuffer` 跑通结果
   - 但还需要更多真实 libdrm 示例或工具验证
   - 目标是确认 ABI 对齐在更广泛场景下稳定成立

2. **补充错误路径与回归测试**
   - 覆盖非法 `fb_id`
   - 覆盖非法 `handle`
   - 覆盖 mode 尺寸不匹配
   - 覆盖 stride/bpp 不匹配

### 中优先级

3. **/dev/fb0 兼容层**
   - 当前没有 fbdev 字符设备
   - 如果要兼容旧程序，需要补一层转发或桥接

4. **梳理 GEM 抽象层**
   - 当前 `gem.rs` 仍然是预留位
   - 如果后续要做更标准的 DRM 内存对象管理，需要把 dumb buffer 背后的对象抽象正规化

### 中长期

5. **原生 Virtio-GPU scanout**
   - 当前成功显示依赖已有 `FRAMEBUFFER`
   - 后续需要把输出链路逐步迁移到真正 GPU/scanout 路径

6. **高级 KMS 能力**
   - plane
   - property
   - atomic modesetting
   - hotplug
   - 多输出支持

---

## 下一步计划

### 立即执行

- [ ] 用更多标准 libdrm 程序验证当前 `/dev/dri/card0`
- [ ] 为 `SET_CRTC / PAGE_FLIP / GETFB / MAP_DUMB` 增加回归测试
- [ ] 补一份最小 bring-up 的用户态验证说明

### 短期计划

- [ ] 评估是否需要 `/dev/fb0` 兼容层
- [ ] 评估 `modetest` 适配与移植成本
- [ ] 梳理 `gem.rs` 的最小可演进方向

### 中期计划

- [ ] 推进 virtio-gpu 原生扫描输出
- [ ] 评估 Weston/Mesa bring-up 需要的缺失能力
- [ ] 逐步从“firmware framebuffer bridge”过渡到“真实 DRM scanout”

---

## 已解决的问题（按时间）

| 日期 | 问题 | 解决方案 |
|------|------|---------|
| 2026-04-10 | `/dev/dri/card0` 不存在 | 注册 DRM 字符设备，路径固定为 `dri/card0` |
| 2026-04-11 | ioctl magic/NR 不符合 Linux DRM 标准 | 修正为 `magic=0x64` 和 Linux 标准 NR |
| 2026-04-14 | dumb buffer `mmap` 路径存在 offset 语义问题 | 使用“大 VMO + 固定 slot offset”方案跑通 |
| 2026-04-14 | 关键 struct ABI 错误 | 按 libdrm 布局修复结构体定义 |
| 2026-04-16 | `SET_CRTC` / `PAGE_FLIP` 返回 `EINVAL` | 用真实 `FRAMEBUFFER` 动态生成 mode |
| 2026-04-16 | ioctl 成功但 VNC 无变化 | 将 `SET_CRTC` / `PAGE_FLIP` 接到 `present_fb()` |
| 2026-04-16 | present 读取 stale shadow buffer 导致黑屏 | `snapshot()` 改为从 VMO 读取真实像素 |
| 2026-04-16 | `double_framebuffer` 最小显示链路未闭环 | 完成闭环并验证通过 |

---

## 相关文档

- [Phase 1-4 项目总结](./phase1-4-project-summary.md) — 当前架构、实现和定位说明
- [启动协议与 framebuffer 背景](./phase1-4-project-summary.md#6-关键修复与踩坑记录) — 包含 bring-up 中的启动协议差异说明

---

*本文档描述的是当前“最小 DRM/KMS 已跑通”的项目状态。若后续开始做真正的 GEM 层、Virtio-GPU 原生 scanout 或高级 KMS，需要同步更新本计划。*
