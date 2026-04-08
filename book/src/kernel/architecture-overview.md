# Architecture Overview

This page provides a concise reference of the Asterinas repository
structure, core subsystems, build/tooling layout,
and the extension points most relevant
to adding a DRM graphics stack (such as `simpledrm` and double-buffering).

## Project Positioning

Asterinas is a _secure_, _fast_, and _general-purpose_ OS kernel
that provides a Linux-compatible ABI.
It is written in Rust and targets modern 64-bit platforms
(x86-64 as Tier 1, RISC-V 64 and LoongArch 64 as Tier 2/3).

Its defining design principle is the
[framekernel architecture](the-framekernel-architecture.md):
the kernel lives in a single address space (like a monolithic kernel),
but is partitioned into two strict halves:

| Layer              | `unsafe` Rust | Responsibility                                                 |
|--------------------|:-------------:|----------------------------------------------------------------|
| **OSTD** (`ostd/`) | Allowed       | Encapsulate all hardware-touching code; expose safe high-level APIs |
| **Kernel** (`kernel/`) | **Forbidden** | Implement syscalls, file systems, drivers, and other OS services in _safe_ Rust |

This hard boundary keeps the Trusted Computing Base (TCB) small:
every bug that could cause undefined behaviour is confined to OSTD.

---

## Repository Layout

```
asterinas-drm/
├── kernel/          # Safe-Rust OS kernel (see below)
│   ├── comps/       # Pluggable kernel components (drivers, subsystems)
│   ├── libs/        # Shared in-kernel libraries (bigtcp, rights, util…)
│   └── src/         # Core kernel: syscalls, VFS, net, sched, process…
│
├── ostd/            # OS framework — the only crate with unsafe Rust
│   └── src/         # arch, boot, mm, io, irq, sync, task, timer, user…
│
├── osdk/            # cargo-osdk: build, run, test, debug OSDK crates
│
├── book/            # mdBook documentation (this document lives here)
│
├── test/            # User-space regression / syscall tests (C programs)
├── distro/          # Asterinas NixOS distribution configuration
├── tools/           # Helper scripts (formatting, Docker, benchmarking)
│
├── Cargo.toml       # Workspace manifest
├── Components.toml  # Component dependency/ordering metadata
├── OSDK.toml        # OSDK manifest (QEMU flags, boot protocol, etc.)
├── Makefile         # Top-level convenience targets
└── rust-toolchain.toml  # Pinned nightly Rust toolchain
```

---

## Core Subsystems

### OSTD (`ostd/`)

OSTD exposes safe Rust APIs for every operation that requires `unsafe`.
Key modules:

| Module          | Purpose                                                            |
|-----------------|--------------------------------------------------------------------|
| `mm`            | Physical frame allocator, page tables, MMIO (`IoMem`), DMA        |
| `io`            | `IoMem` (MMIO) and `IoPort` (x86 PIO) allocators                  |
| `irq`           | Interrupt registration and dispatch                                |
| `boot`          | Bootloader info (framebuffer address/geometry, memory map…)        |
| `task` / `sync` | Kernel tasks, `SpinLock`, `Mutex`, `RwLock`                        |
| `timer`         | Timer callbacks                                                    |
| `user`          | Safe user-space memory access                                      |
| `bus`           | PCI bus enumeration and device access                              |
| `arch`          | Architecture-specific code (x86-64, RISC-V 64, LoongArch 64)      |

The most important API for display drivers is `ostd::io::IoMem`,
which provides a safe interface for memory-mapped I/O regions,
including support for the `WriteCombining` cache policy
that accelerates sequential framebuffer writes.

### Kernel (`kernel/`)

The main kernel crate is `aster-kernel`.
It is composed of the following top-level modules:

| Directory      | Responsibility                                         |
|----------------|--------------------------------------------------------|
| `src/syscall/` | Linux-ABI system call dispatch and implementation      |
| `src/fs/`      | VFS layer, ext2, tmpfs, procfs, devfs                  |
| `src/net/`     | Networking stack (bigtcp)                              |
| `src/process/` | Process/thread management, signals                     |
| `src/sched/`   | CPU scheduler                                          |
| `src/vm/`      | User virtual memory management                         |
| `src/ipc/`     | Pipes, sockets (IPC)                                   |
| `src/driver/`  | Driver init glue (registers components with subsystems)|
| `src/device/`  | Device abstraction layer                               |

### Component System (`kernel/comps/`)

Drivers and optional subsystems live as independent Cargo crates
under `kernel/comps/`.
Each component is registered via the `#[init_component]` attribute macro
and follows a two-phase initialisation (`Bootstrap` → `PostBoot`).

Current components:

| Crate                  | Function                                      |
|------------------------|-----------------------------------------------|
| `aster-framebuffer`    | Boot-time framebuffer (MMIO, pixel rendering) |
| `aster-console`        | Text console abstraction + ANSI escape        |
| `aster-input`          | Input device abstraction                      |
| `aster-i8042`          | PS/2 keyboard / mouse driver                  |
| `aster-pci`            | PCI bus enumeration                           |
| `aster-virtio`         | VirtIO device drivers (block, net, GPU, etc.) |
| `aster-block`          | Block device abstraction                      |
| `aster-network`        | Network device abstraction                    |
| `aster-uart`           | UART serial driver                            |
| `aster-logger`         | Early-boot kernel logger                      |
| `aster-time`           | Timer subsystem                               |
| `aster-softirq`        | Deferred interrupt work                       |
| `aster-systree`        | Sysfs-like device tree                        |

---

## Existing Display Infrastructure

`aster-framebuffer` (`kernel/comps/framebuffer/`) is the starting point
for any graphics work.  It already provides:

* **`FrameBuffer`** — wraps an `IoMem` region obtained from bootloader
  info (`boot_info().framebuffer_arg`).
* **`PixelFormat`** — supports `Grayscale8`, `Rgb565`, `Rgb888`,
  and `BgrReserved` (32-bpp).
* **Pixel rendering** — `Pixel::render()` converts an RGB triple
  into the hardware format.
* **`PixelOffset`** — a safe cursor for addressing individual pixels
  in the framebuffer.
* **Color map** (`FbCmap`) — an in-memory 256-entry palette
  compatible with the Linux `FBIOPUTCMAP`/`FBIOGETCMAP` ioctls.
* **`FramebufferConsole`** — a full ANSI text console that renders
  glyphs onto the framebuffer, registered via `aster-console`.
* **Write-combining cache policy** — used when mapping the framebuffer
  region for faster sequential writes.

### Bootloader Framebuffer Path

```
boot_info().framebuffer_arg   (ostd::boot)
    └─> IoMem::acquire_with_cache_policy(WriteCombining)
            └─> FrameBuffer::init()   (aster-framebuffer)
                    └─> FramebufferConsole::init()
                            └─> aster_console::register_device()
```

---

## Build and Tooling

| Command              | Action                                               |
|----------------------|------------------------------------------------------|
| `make kernel`        | Build the initramfs and kernel image                 |
| `make run_kernel`    | Build and launch in QEMU                             |
| `make test`          | Unit tests for non-OSDK crates (`cargo test`)        |
| `make ktest`         | Kernel-mode unit tests via `cargo osdk test` in QEMU |
| `make check`         | Full lint: rustfmt, clippy, typos, license checks    |
| `make format`        | Auto-format Rust, Nix, and C code                    |
| `make docs`          | Build rustdocs for all crates                        |

All kernel development runs inside the project Docker container.
The Rust toolchain version is pinned in `rust-toolchain.toml`
(nightly-2025-12-06).
`OSDK_TARGET_ARCH` selects the target
(`x86_64` by default, also `riscv64` or `loongarch64`).

---

## Extension Points for a DRM Stack

The following integration points are the natural landing zones
for adding DRM support to this fork.

### 1. `aster-framebuffer` — the immediate foundation

The existing `FrameBuffer` type exposes the bootloader framebuffer
through safe MMIO.
`simpledrm` would sit directly on top of this:
allocating a second (back) buffer in system RAM,
rendering into it, and flushing to `FrameBuffer` on `page_flip`.

### 2. `ostd::mm` — buffer allocation

`FrameAllocOptions` and `Segment` provide safe physical-memory
allocation, which is needed for off-screen back buffers and
GEM-style buffer objects.

### 3. `ostd::bus` (PCI) + `aster-pci` — GPU enumeration

Future GPU drivers (e.g., virtio-gpu, bochs-display)
discover devices via PCI and perform MMIO over `IoMem`.
`aster-pci` is already wired into the boot sequence.

### 4. `kernel/src/device/` and `kernel/src/syscall/` — userland ABI

DRM file operations (`open`, `ioctl`, `mmap`, `poll`)
would be surfaced through the VFS device layer and
the system call dispatch table already present in the kernel.

### 5. `ostd::sync` — display synchronisation

`SpinLock<LocalIrqDisabled>` (used by `FramebufferConsole` today)
and `Mutex` (used for the color map) are the canonical primitives
for protecting shared display state.

---

## See Also

* [The Framekernel Architecture](the-framekernel-architecture.md)
* [Advanced Build and Test Instructions](advanced-instructions.md)
* [An Overview of OSTD](../ostd/README.md)
* [DRM Integration Guide](drm-integration.md)
