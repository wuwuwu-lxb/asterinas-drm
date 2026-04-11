# DRM Integration Guide

This is the single canonical document for DRM work in this repository.

## Acceptance Standard

There is only one acceptance standard:

**On top of this Asterinas kernel, add `device/drm` capability and run double framebuffer end-to-end.**

---

## Current Status (Verified Against Code)

### Already Implemented

1. `aster-drm` component exists:
   - `kernel/comps/drm/src/lib.rs`
   - `kernel/comps/drm/src/buffer.rs`
   - `kernel/comps/drm/src/simple.rs`
2. Component wiring exists:
   - `Components.toml` includes `drm = { name = "aster-drm" }`
   - workspace dependencies include `aster-drm`
3. Double framebuffer base path exists:
   - `BackBuffer` stores a RAM back buffer
   - `SimpleDrm::present()` copies back-buffer bytes into hardware framebuffer

### Not Yet Implemented

1. No externally available `/dev/dri/card0` device node.
2. No complete userspace DRM ioctl access path yet.
3. No end-to-end validation result proving “`device/drm` + double framebuffer” is fully working.

---

## Minimum End-to-End Deliverable

To satisfy the acceptance standard, the implementation must provide:

1. **Device layer**: kernel-visible and user-accessible `device/drm` (for example `/dev/dri/card0`).
2. **Buffer layer**: stable `BackBuffer -> present() -> hardware framebuffer` path.
3. **Validation layer**: reproducible runtime verification in the target VM/QEMU environment.

---

## Pass Criteria

The target is considered complete only if all conditions hold:

1. `device/drm` can be accessed.
2. Image data is successfully presented from back buffer to front buffer and displayed.
3. The result is reproducible, not a one-off success.

---

## Relevant Code Locations

- DRM component: `kernel/comps/drm/`
- Existing framebuffer component: `kernel/comps/framebuffer/`
- Component registry: `Components.toml`
- Device/syscall paths: `kernel/src/device/`, `kernel/src/syscall/`

---

## See Also

- [Architecture Overview](architecture-overview.md)
- [The Framekernel Architecture](the-framekernel-architecture.md)
