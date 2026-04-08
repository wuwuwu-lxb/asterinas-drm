# DRM Integration Guide

This document describes how to introduce a
**Direct Rendering Manager (DRM)** subsystem into the Asterinas fork,
starting from `simpledrm` with double-buffering
and evolving toward a complete DRM stack.
It is written for a developer who is already familiar
with the [architecture overview](architecture-overview.md)
of this repository.

---

## Current State

`aster-framebuffer` (`kernel/comps/framebuffer/`) already provides
a minimal display path:

* The bootloader populates `boot_info().framebuffer_arg`
  (address, width, height, bits-per-pixel).
* `FrameBuffer::init()` maps that region with the `WriteCombining`
  cache policy via `ostd::io::IoMem`.
* `FramebufferConsole` renders text (with full ANSI escape support)
  directly into the hardware framebuffer.

Everything above writes pixels directly to the hardware framebuffer
(single-buffer mode).
The goal of the next steps is to:

1. Introduce a **back buffer** in system RAM so rendering is decoupled
   from the visible hardware framebuffer.
2. Add a controlled **flip/present** path so only complete frames
   are made visible.
3. Wrap the whole thing in a **DRM-compatible device model**
   that can eventually expose standard `DRM_IOCTL_*` interfaces
   to userland.

---

## Key Concepts

### simpledrm

`simpledrm` (as known from Linux) is the minimal DRM driver
for firmware-provided framebuffers.
It does **not** require GPU acceleration or a scanout engine:
it simply takes the bootloader framebuffer,
registers it as a KMS device,
and provides a shadow (back) buffer for userland to render into.

In this codebase the equivalent is a new component
`aster-drm` that wraps `aster-framebuffer`
and adds the buffer-management and page-flip logic.

### Double-buffering

Two memory regions are maintained:

| Buffer       | Storage          | Written by   | Read by hardware |
|--------------|------------------|--------------|-----------------|
| Front buffer | MMIO framebuffer | `present()`  | display controller |
| Back buffer  | System RAM       | renderer     | never directly  |

Rendering happens in the back buffer;
`present()` copies (blits) the back buffer into the front buffer
at a controlled point.
Because the copy is fast (write-combining MMIO + contiguous RAM),
tearing is significantly reduced compared to direct rendering.

---

## Integration Points

### 1. `aster-framebuffer` (modify / extend)

`FrameBuffer` already owns the MMIO mapping.
It needs two additions:

* A `flush(src: &[u8])` / `flush_region(...)` method that accepts
  a caller-supplied byte slice and copies it to `IoMem`.
* Exposure of the geometry (`width`, `height`, `line_size`,
  `pixel_format`) — these are already public.

No `unsafe` code is needed; `IoMem::write_bytes` is the existing path.

### 2. New component `aster-drm` (`kernel/comps/drm/`)

Create a new Cargo crate that depends on `aster-framebuffer`.
This is where the back buffer and the DRM device model live.

Minimum viable structure:

```
kernel/comps/drm/
├── Cargo.toml
└── src/
    ├── lib.rs          # #[init_component], public re-exports
    ├── device.rs       # DrmDevice: owns FrameBuffer + back buffer
    ├── buffer.rs       # BackBuffer: allocated Segment<u8>
    └── simple.rs       # SimpleDrm: flush/present logic
```

### 3. `ostd::mm` — back buffer allocation

A contiguous back buffer can be allocated using
`FrameAllocOptions` or a heap `Vec<u8>`.
For the two-week milestone a `Vec<u8>` is sufficient.
For production, a physically-contiguous `Segment` is preferred
because it can later be used for DMA.

```rust
// Rough sketch — actual types from ostd::mm
use alloc::vec;

pub struct BackBuffer {
    data: alloc::vec::Vec<u8>,
    width: usize,
    height: usize,
    stride: usize,
}

impl BackBuffer {
    pub fn new(fb: &FrameBuffer) -> Self {
        let stride = fb.line_size();
        let size = fb.height() * stride;
        BackBuffer {
            data: vec![0u8; size],
            width: fb.width(),
            height: fb.height(),
            stride,
        }
    }

    pub fn as_slice(&self) -> &[u8] { &self.data }
    pub fn as_mut_slice(&mut self) -> &mut [u8] { &mut self.data }
}
```

### 4. Present / page-flip

```rust
/// Copies the back buffer into the hardware framebuffer.
pub fn present(fb: &FrameBuffer, back: &BackBuffer) {
    fb.write_bytes_at(0, back.as_slice())
        .expect("framebuffer write failed");
}
```

This is safe Rust; `FrameBuffer::write_bytes_at` delegates to
`IoMem::write_bytes` which is already part of OSTD's safe API surface.

### 5. VFS device node + `ioctl` (`kernel/src/device/`, `kernel/src/syscall/`)

For userland access, a character device node (e.g. `/dev/dri/card0`)
needs to be registered in devfs.
The `open` / `ioctl` / `mmap` paths are implemented under
`kernel/src/syscall/` and `kernel/src/device/`.

For the two-week milestone this can be skipped;
`present()` can be called directly from a kernel thread or test.

### 6. `Components.toml` — register the new component

Add the new crate to `Components.toml` so the component framework
discovers and initialises it:

```toml
[components]
# existing entries …
drm = { name = "aster-drm" }
```

And add the dependency in `kernel/Cargo.toml`:

```toml
[dependencies]
aster-drm = { path = "comps/drm" }
```

---

## Dependency Considerations

| Requirement                    | Provided by              | Notes                          |
|--------------------------------|--------------------------|--------------------------------|
| MMIO framebuffer mapping       | `aster-framebuffer`      | Already exists                 |
| Safe memory allocation         | `ostd::mm` / `alloc`     | `Vec<u8>` sufficient for MVP   |
| Pixel format conversion        | `aster-framebuffer` pixel module | Already exists          |
| Synchronisation (back buffer)  | `ostd::sync::Mutex`      | Wrap `BackBuffer` in `Mutex`   |
| PCI device discovery (future)  | `aster-pci`              | Needed for GPU drivers         |
| DMA transfers (future)         | `ostd::mm::dma`          | Needed for zero-copy display   |
| User-space buffer sharing (future) | `ostd::mm::vm_space` | Needed for `mmap`              |
| VirtIO GPU (future)            | `aster-virtio`           | virtio-gpu backend             |

All immediate dependencies (`aster-framebuffer`, `ostd`, `alloc`)
are already present in the workspace.
No new external crates are required for the two-week milestone.

---

## Phased Roadmap

### Phase 0 — Prerequisite audit (Day 1–2)

- [ ] Confirm bootloader passes valid framebuffer info
      (`boot_info().framebuffer_arg.is_some()` on target VM).
- [ ] Identify the pixel format actually used by the QEMU/VM firmware
      (currently guessed from BPP — see the `FIXME` in `framebuffer.rs`).
- [ ] Verify `line_size` matches `width × bpp/8`
      (the `FIXME` for stride/alignment in `framebuffer.rs`).

### Phase 1 — simpledrm MVP (Days 3–7, ~Week 1)

**Goal:** render a full-screen test pattern without tearing.

1. Create `kernel/comps/drm/` as a new Cargo crate.
2. Implement `BackBuffer` backed by a heap-allocated `Vec<u8>`.
3. Implement `SimpleDrm::present()` using `FrameBuffer::write_bytes_at`.
4. Add a kernel thread that draws a gradient or checkerboard
   into the back buffer and calls `present()` once per tick.
5. Register the component in `Components.toml`.
6. Boot in QEMU and verify the test pattern is stable.

Acceptance criterion: a static test image is visible on screen
with no corruption or flicker.

### Phase 2 — Double-buffering + console coexistence (Days 8–10, ~Week 2 start)

**Goal:** decouple text console from the DRM back buffer.

1. Extend `aster-framebuffer` with a `flush_region` API
   (dirty-rectangle optimisation for partial updates).
2. Gate `FramebufferConsole` so it renders into the back buffer
   rather than directly into MMIO.
3. Add a `vsync`-style periodic flush (timer callback or explicit call).
4. Protect `BackBuffer` with `SpinLock<LocalIrqDisabled>`
   to allow interrupt-safe access.

Acceptance criterion: text console output and a concurrently-drawn
graphic region both update cleanly without visible tearing.

### Phase 3 — DRM device node + basic ioctls (Weeks 3–4)

**Goal:** expose `/dev/dri/card0` so a userland program can
draw to the screen.

1. Register a character device in devfs (`/dev/dri/card0`).
2. Implement `DRM_IOCTL_VERSION`, `DRM_IOCTL_GET_CAP`,
   and `DRM_IOCTL_MODE_GETRESOURCES` to satisfy `libdrm` probing.
3. Implement `DRM_IOCTL_MODE_ADDFB` / `DRM_IOCTL_MODE_RMFB`
   to let userland allocate a dumb (CPU-rendered) framebuffer.
4. Implement `DRM_IOCTL_MODE_DIRTYFB` or `DRM_IOCTL_MODE_PAGE_FLIP`
   to trigger `present()`.
5. Add a minimal userland test program in `test/` that opens
   `/dev/dri/card0` and paints pixels.

Acceptance criterion: `test/drm_basic` runs inside the VM
and shows a visible image.

### Phase 4 — GEM buffer objects + mmap (Weeks 5–8)

**Goal:** standard `libdrm` dumb-buffer workflow.

1. Implement GEM object lifecycle
   (`DRM_IOCTL_MODE_CREATE_DUMB`,
    `DRM_IOCTL_MODE_MAP_DUMB`,
    `DRM_IOCTL_MODE_DESTROY_DUMB`).
2. Wire `mmap` of a GEM handle to a user virtual memory region
   backed by the back buffer `Segment`.
3. Run `kmscube` or similar minimal DRM test suite in the VM.

### Phase 5 — KMS plane / CRTC / encoder / connector model (Weeks 9+)

**Goal:** a conformant KMS device that `Weston` or a simple Wayland
compositor can drive.

1. Model the display pipeline as KMS objects:
   connector → encoder → CRTC → plane.
2. Implement `DRM_IOCTL_MODE_SETCRTC` and atomic modesetting
   (`DRM_IOCTL_MODE_ATOMIC`).
3. Add VirtIO-GPU backend as an alternative to the MMIO framebuffer.
4. Progressively enable more of the `DRM_*` ioctl surface
   as needed by target applications.

---

## File Naming and Code Style

Follow the Asterinas coding guidelines:

* New crate name: `aster-drm` (snake-case, `aster-` prefix).
* Every public item requires a doc comment
  (third-person singular present tense, identifier in backticks).
* No `unsafe` code in `kernel/comps/drm/`;
  all unsafe operations are delegated to OSTD.
* `#[deny(unsafe_code)]` at the crate root.
* Use `ostd::sync::Mutex` / `SpinLock` — never raw `std::sync` types.
* Propagate errors with `?`; no `.unwrap()` except in tests.
* Log with `log::info!` / `log::warn!` — no `println!`.

---

## See Also

* [Architecture Overview](architecture-overview.md)
* [The Framekernel Architecture](the-framekernel-architecture.md)
* [An Overview of OSTD](../ostd/README.md)
* [OSDK User Guide](../osdk/guide/README.md)
