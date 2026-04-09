# SimpleDRM Design and Implementation

## Project Overview

This document describes a comprehensive plan for implementing a lightweight display driver based on the Linux DRM/KMS (Direct Rendering Manager / Kernel Mode Setting) subsystem for the Asterinas operating system.

### Core Objectives

1. Replace the legacy fbdev interface with modern DRM support
2. Implement double-buffering to prevent screen tearing
3. Provide standard userspace interfaces (ioctl)
4. Support GEM objects and Dumb Buffer management

### Key Reference Documents

- [DRM Integration Guide](drm-integration.md) - Existing DRM integration roadmap
- User-provided SimpleDRM architecture design document

---

## Architecture Design

### SimpleDRM Core Components

```
┌─────────────────────────────────────────────────────────┐
│                    User Space                           │
│  ┌───────────────┐  ┌───────────────┐  ┌─────────────┐  │
│  │ libdrm        │  │ Weston        │  │ kmscube     │  │
│  │               │  │ Compositor    │  │ Test App    │  │
│  └───────┬───────┘  └───────┬───────┘  └──────┬──────┘  │
│          │                  │                  │         │
│          └──────────────────┼──────────────────┘         │
│                             │ DRM_IOCTL_*                │
└─────────────────────────────┼───────────────────────────┘
                              │
┌─────────────────────────────┼───────────────────────────┐
│                    Kernel Space                          │
│  ┌─────────────────────────────────────────────────────┐│
│  │              DRM Device Model                       ││
│  │  ┌───────────────────────────────────────────────┐  ││
│  │  │ /dev/dri/card0                                │  ││
│  │  │  ├─ DRM_IOCTL_VERSION                        │  ││
│  │  │  ├─ DRM_IOCTL_GET_CAP                        │  ││
│  │  │  ├─ DRM_IOCTL_MODE_GETRESOURCES              │  ││
│  │  │  ├─ DRM_IOCTL_MODE_ADDFB / RMFB             │  ││
│  │  │  ├─ DRM_IOCTL_MODE_CREATE_DUMB              │  ││
│  │  │  ├─ DRM_IOCTL_MODE_MAP_DUMB                 │  ││
│  │  │  ├─ DRM_IOCTL_MODE_SETCRTC                  │  ││
│  │  │  └─ DRM_IOCTL_MODE_PAGE_FLIP               │  ││
│  │  └───────────────────────────────────────────────┘  ││
│  └─────────────────────────────────────────────────────┘│
│                             │                            │
│  ┌──────────────────────────┼───────────────────────────┐ │
│  │        KMS Objects        │                           │ │
│  │  ┌─────────┐  ┌────────┐│ ┌────────┐ ┌─────────┐ │ │
│  │  │Connector│──│ Encoder│──│ │ CRTC   │──│  Plane  │ │ │
│  │  │(Display)│  │(Encoder│ │ │(Ctrl)  │ │(Layer)  │ │ │
│  │  └─────────┘  └────────┘ │ └────────┘ └─────────┘ │ │
│  └─────────────────────────────────────────────────────┘ │
│                             │                            │
│  ┌──────────────────────────┼───────────────────────────┐ │
│  │       Memory Management  │                           │ │
│  │  ┌────────────────────────────────────────────────┐  │ │
│  │  │  GEM (Graphics Execution Manager)              │  │ │
│  │  │  ├─ Dumb Buffer (System RAM)                  │  │ │
│  │  │  ├─ Framebuffer Object                        │  │ │
│  │  │  └─ Memory Mapping (mmap)                    │  │ │
│  │  └────────────────────────────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────┘ │
│                             │                            │
│  ┌──────────────────────────┼───────────────────────────┐ │
│  │       Buffer Management  │                           │ │
│  │  ┌────────────┐  ┌────────────────┐                  │ │
│  │  │ Back Buffer│  │ Front Buffer   │                  │ │
│  │  │(System RAM)│  │(MMIO VRAM)    │                  │ │
│  │  └─────┬──────┘  └──────┬────────┘                  │ │
│  │        │                │                            │ │
│  │        └────────────────┴─────────────────────────    │ │
│  │                    present()                          │ │
│  └─────────────────────────────────────────────────────┘ │
│                             │                            │
│  ┌──────────────────────────┼───────────────────────────┐ │
│  │    aster-framebuffer     │                           │ │
│  │  └─ IoMem (MMIO mapping)│                           │ │
│  │  └─ FrameBuffer Console │                           │ │
│  └─────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

### Data Flow

```
Userspace Rendering Flow:

1. open("/dev/dri/card0")
2. DRM_IOCTL_MODE_GETRESOURCES  → Get KMS object IDs
3. DRM_IOCTL_MODE_CREATE_DUMB   → Create Dumb Buffer
4. DRM_IOCTL_MODE_MAP_DUMB     → Get mapping info
5. mmap()                       → Userspace mapping
6. Write pixel data to userspace buffer
7. DRM_IOCTL_MODE_ADDFB        → Register Framebuffer
8. DRM_IOCTL_MODE_SETCRTC      → Set display mode
9. DRM_IOCTL_MODE_PAGE_FLIP    → Trigger buffer swap
```

---

## Implementation Phases

### Phase 0: Infrastructure Preparation (Days 1-2)

**Goal**: Verify environment dependencies and infrastructure

#### 0.1 Verify Bootloader Framebuffer Info

```rust
// Check if boot_info().framebuffer_arg is valid
let Some(framebuffer_arg) = boot_info().framebuffer_arg else {
    log::warn!("Framebuffer not found");
    return;
};
```

#### 0.2 Identify Pixel Format

Current implementation issues (see `framebuffer.rs`):
- Different formats may share the same BPP (e.g., 32bpp could be BGRX or XBGR)
- Need to collect more information during boot phase

#### 0.3 Verify line_size Alignment

```rust
// Current implementation assumes line_size = width * bpp/8
// But actual hardware may have alignment requirements
let line_size = framebuffer_arg.width * pixel_format.nbytes();
```

---

### Phase 1: SimpleDRM MVP (Days 3-7)

**Goal**: Implement basic double-buffering functionality without screen tearing

#### 1.1 Create aster-drm Component

**Directory Structure**:

```
kernel/comps/drm/
├── Cargo.toml
└── src/
    ├── lib.rs                    # Component entry
    ├── device.rs                 # DRM device
    ├── buffer.rs                 # Backend buffer management
    ├── simple.rs                 # SimpleDRM implementation
    ├── kms/
    │   ├── mod.rs              # KMS object base
    │   ├── crtc.rs             # CRTC controller
    │   ├── encoder.rs           # Encoder
    │   ├── connector.rs         # Connector
    │   └── plane.rs             # Plane (layer)
    ├── gem/
    │   ├── mod.rs              # GEM object management
    │   └── dumb.rs              # Dumb Buffer implementation
    └── ioctl/
        ├── mod.rs              # IOCTL handling
        ├── version.rs           # DRM_IOCTL_VERSION
        ├── resources.rs         # DRM_IOCTL_MODE_GETRESOURCES
        ├── dumb.rs              # Dumb Buffer operations
        └── crtc.rs              # CRTC operations
```

#### 1.2 Cargo.toml Configuration

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

#### 1.3 Implement BackBuffer

```rust
// kernel/comps/drm/src/buffer.rs

use alloc::vec::Vec;
use ostd::sync::SpinLock;

/// Backend buffer - RAM buffer used for rendering
///
/// Double-buffering mechanism:
/// - Back Buffer: Written by userspace
/// - Front Buffer: Hardware display
pub struct BackBuffer {
    data: SpinLock<Vec<u8>>,
    width: usize,
    height: usize,
    stride: usize,
}

impl BackBuffer {
    /// Create a new backend buffer
    pub fn new(width: usize, height: usize, stride: usize) -> Self {
        let size = height * stride;
        Self {
            data: SpinLock::new(vec![0u8; size]),
            width,
            height,
            stride,
        }
    }

    /// Get buffer size
    pub fn size(&self) -> usize {
        self.height * self.stride
    }

    /// Get width
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get height
    pub fn height(&self) -> usize {
        self.height
    }

    /// Get stride (bytes per line)
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// Get buffer data (immutable reference)
    pub fn data(&self) -> &[u8] {
        self.data.lock().as_slice()
    }

    /// Clear the buffer
    pub fn clear(&self) {
        self.data.lock().fill(0);
    }
}
```

#### 1.4 Implement SimpleDrm::present()

```rust
// kernel/comps/drm/src/simple.rs

use alloc::sync::Arc;
use aster_framebuffer::FrameBuffer;
use ostd::sync::SpinLock;

use super::buffer::BackBuffer;

/// SimpleDRM core structure
pub struct SimpleDrm {
    fb: Arc<FrameBuffer>,
    back_buffer: SpinLock<BackBuffer>,
}

impl SimpleDrm {
    /// Create a new SimpleDrm instance
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

    /// Get backend buffer
    pub fn back_buffer(&self) -> &SpinLock<BackBuffer> {
        &self.back_buffer
    }

    /// Get framebuffer info
    pub fn framebuffer(&self) -> &Arc<FrameBuffer> {
        &self.fb
    }

    /// Copy backend buffer content to frontend buffer
    ///
    /// This is the key operation for double-buffering:
    /// 1. Userspace renders to back buffer
    /// 2. present() copies the complete frame to hardware framebuffer
    /// 3. Prevents tearing caused by partial updates
    pub fn present(&self) -> ostd::Result<()> {
        let back_data = {
            let back = self.back_buffer.lock();
            back.data().to_vec()
        };

        self.fb.write_bytes_at(0, &back_data)
    }
}
```

#### 1.5 Register Component

Add to `Components.toml`:

```toml
[components]
# ... existing entries ...
drm = { name = "aster-drm" }
```

Add to `kernel/Cargo.toml`:

```toml
[dependencies]
# ... existing dependencies ...
aster-drm = { path = "comps/drm" }
```

#### 1.6 Test Verification

Create test programs to draw gradient or checkerboard patterns, verifying:
- No tearing
- Stable refresh rate
- Correct color display

---

### Phase 2: Double-buffering and Console Coexistence (Days 8-10)

**Goal**: Decouple text console from DRM backend buffer

#### 2.1 Extend FrameBuffer API

Add to `aster-framebuffer`:

```rust
// kernel/comps/framebuffer/src/framebuffer.rs

impl FrameBuffer {
    /// Flush specified region data to hardware framebuffer
    ///
    /// Parameters:
    /// - `offset`: Starting offset in framebuffer
    /// - `data`: Data to flush
    /// - `len`: Length of data to flush
    pub fn flush_region(&self, offset: usize, data: &[u8], len: usize) -> Result<()> {
        let slice = &data[..len];
        self.io_mem.write_bytes(offset, slice)
    }
}
```

#### 2.2 Protect BackBuffer Access

Use `SpinLock<LocalIrqDisabled>` for interrupt-safe access:

```rust
use ostd::sync::{LocalIrqDisabled, SpinLock};

pub struct SimpleDrm {
    fb: Arc<FrameBuffer>,
    back_buffer: SpinLock<BackBuffer, LocalIrqDisabled>,
}
```

#### 2.3 VSync-style Periodic Flushing

```rust
use ostd::timer::Timer;

// Timer callback
fn vsync_callback(timer: &Timer) {
    if let Some(drm) = SIMPLE_DRM.get() {
        let _ = drm.present();
    }
}
```

---

### Phase 3: DRM Device Node and Basic IOCTLs (Weeks 3-4)

**Goal**: Expose `/dev/dri/card0` for userspace programs

#### 3.1 Register Character Device

Create new device module in `kernel/src/device/`:

```rust
// kernel/src/device/drm/mod.rs

pub mod device;
pub mod file;

use device::DrmDevice;

pub fn init_in_first_kthread() {
    // Register /dev/dri/card0
    let drm_device = Arc::new(DrmDevice::new());
    register(Arc::new(drm_device)).unwrap();
}
```

#### 3.2 Implement DRM_IOCTL_VERSION

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

#### 3.3 Implement DRM_IOCTL_GET_CAP

```rust
// kernel/src/device/drm/ioctl/capability.rs

const DRM_IOCTL_GET_CAP: u32 = 0x09;

#[repr(C)]
struct DrmCapability {
    capability: u64,
    value: u64,
}

// DRM Capability Constants
const DRM_CAP_DUMB_BUFFER: u64 = 0x1;
const DRM_CAP_VBLANK_HIGH_CRTC: u64 = 0x2;
const DRM_CAP_DUMB_PREFERRED_DEPTH: u64 = 0x3;

pub fn handle_get_cap(arg: Vaddr) -> Result<()> {
    let cap = unsafe { &*(arg as *const DrmCapability) };

    let value = match cap.capability {
        DRM_CAP_DUMB_BUFFER => 1,              // Dumb Buffer supported
        DRM_CAP_VBLANK_HIGH_CRTC => 0,         // Not supported
        DRM_CAP_DUMB_PREFERRED_DEPTH => 32,     // Preferred 32-bit depth
        _ => return Err(Error::EINVAL),
    };

    copy_to_user(arg as usize + 8, &value)?;
    Ok(())
}
```

#### 3.4 Implement DRM_IOCTL_MODE_GETRESOURCES

```rust
// kernel/src/device/drm/ioctl/resources.rs

const DRM_IOCTL_MODE_GETRESOURCES: u32 = 0xC0;

#[repr(C)]
struct DrmModeCardRes {
    fb_id_ptr: u64,
    crtc_id_ptr: u64,
    connector_id_ptr: u64,
    encoder_id_ptr: u64,
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
    // Returns:
    // - 1 CRTC
    // - 1 Encoder
    // - 1 Connector
    // - Supported resolution range

    let mut res = DrmModeCardRes {
        fb_id_ptr: 0,
        crtc_id_ptr: 0,
        connector_id_ptr: 0,
        encoder_id_ptr: 0,
        count_fbs: 0,
        count_crtcs: 1,
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

#### 3.5 Implement Dumb Buffer Operations

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
    handle: u32,
    pitch: u32,
    size: u64,
    padding: u64,
};

#[repr(C)]
struct DrmModeMapDumb {
    handle: u32,
    padding: u32,
    offset: u64,
};
```

#### 3.6 Implement DRM_IOCTL_MODE_SETCRTC

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
    mode: u64,
    x: u32,
    y: u32,
};

pub fn handle_set_crtc(arg: Vaddr) -> Result<()> {
    let cmd = unsafe { &*(arg as *const DrmModeSetCrtc) };

    // Validate FB ID
    let fb = get_framebuffer(cmd.fb_id)?;

    // Validate CRTC ID
    let crtc = get_crtc(cmd.crtc_id)?;

    // Apply display mode
    crtc.set_mode(&cmd.mode)?;

    // Bind Framebuffer to CRTC
    crtc.bind_framebuffer(fb)?;

    Ok(())
}
```

#### 3.7 Userspace Test Program

Create simple test in `test/` directory:

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
        perror("Failed to open DRM device");
        return 1;
    }

    // Get resource info
    drmModeResPtr resources = drmModeGetResources(fd);
    if (!resources) {
        printf("Failed to get DRM resources\n");
        close(fd);
        return 1;
    }

    printf("CRTCs: %d, Connectors: %d\n",
           resources->count_crtcs,
           resources->count_connectors);

    // Cleanup
    drmModeFreeResources(resources);
    close(fd);

    return 0;
}
```

---

### Phase 4: GEM Objects and mmap (Weeks 5-8)

**Goal**: Implement standard libdrm dumb-buffer workflow

#### 4.1 Implement GEM Object Lifecycle

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

#### 4.2 Implement mmap Support

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
        // offset contains GEM handle information
        // Use Vmar to map GEM object to userspace

        let gem_handle = offset_to_handle(offset)?;
        let object = self.gem_manager.get_object(gem_handle)
            .ok_or(Error::EINVAL)?;

        // Create anonymous VMO
        let vmo = Vmo::new(len);
        vmo.write(0, &object.data)?;

        // Map to userspace address
        let vmar = ctx.vm_space().lock();
        vmar.map_at(vmar.root_vmar(), start, &vmo, 0, len, prot)?;

        Ok(start)
    }
}
```

---

### Phase 5: KMS Plane/CRTC/Encoder/Connector Model (Week 9+)

**Goal**: Standard-compliant KMS device

#### 5.1 Implement KMS Object Model

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
        // Set display timing
        // Configure scanout engine
        // Enable vertical sync
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
            encoder_type: EncoderType::Dpi,
            possible_crtcs: 1,
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
            clock: 148500,
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
            connection: ConnectionStatus::Connected,
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
            id: crtc_id,
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

#### 5.2 Implement Atomic Mode Setting (Optional)

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

    // Validate all objects and properties
    // Prepare all changes
    // Apply atomically
    // On failure, rollback all changes

    Ok(())
}
```

---

## Technical Details

### Pixel Format Support

```rust
// Supported pixel formats (corresponding to DRM_FORMAT_*)
pub const SUPPORTED_FORMATS: &[u32] = &[
    0x34324258, // DRM_FORMAT_XRGB8888
    0x34324742, // DRM_FORMAT_BGRX8888
    0x56524742, // DRM_FORMAT_RGB565
    0x20585352, // DRM_FORMAT_RGBX8888
];
```

### Synchronization Mechanisms

```rust
// Use spinlocks to protect shared resources
use ostd::sync::SpinLock;

// For interrupt-safe operations
use ostd::sync::LocalIrqDisabled;

// Vertical sync (future extension)
const DRM_MODE_VSYNC_MEM_FENCE: u32 = 1 << 2;
```

### Error Handling

Follow Linux DRM error code conventions:
- `-EINVAL`: Invalid arguments
- `-ENOMEM`: Out of memory
- `-ENOENT`: Object does not exist
- `-EBUSY`: Resource busy
- `-ENODEV`: Device does not exist

---

## Testing Plan

### Unit Tests

1. **BackBuffer Tests**
   - Creation and destruction
   - Data read/write
   - Size calculation

2. **GEM Object Tests**
   - Creation and destruction
   - Memory allocation
   - Handle allocation

3. **KMS Object Tests**
   - CRTC/Encoder/Connector relationships
   - Mode validation

### Integration Tests

1. **Basic Functionality Tests**
   ```bash
   # Compile test program
   gcc test/drm_basic/drm_test.c -o test/drm_basic/drm_test -ldrm

   # Run in QEMU
   ./test/drm_basic/drm_test
   ```

2. **Double-buffering Tests**
   - Draw test patterns
   - Verify no tearing

3. **Performance Tests**
   - Frame rate testing
   - Memory usage testing

### Userspace Test Programs

Use `kmscube` or custom test programs to verify:
- Open `/dev/dri/card0`
- Get resource information
- Create Dumb Buffer
- Map to userspace
- Render and display

---

## Documentation

### API Documentation

Write documentation comments for every public API:

```rust
/// Get the backend buffer
///
/// Returns the lock on the backend buffer for safe access.
///
/// # Example
///
/// ```ignore
/// let back = drm.back_buffer();
/// let mut data = back.lock();
/// data[0] = 0xFF; // Write pixel data
/// ```
pub fn back_buffer(&self) -> &SpinLock<BackBuffer> {
    &self.back_buffer
}
```

### Architecture Documentation

Add in `book/src/kernel/`:
- SimpleDRM design documentation
- API reference manual
- Porting guide

---

## Future Work

### Short-term Goals

- [ ] Implement basic double-buffering
- [ ] Support more pixel formats
- [ ] Add VSync support

### Mid-term Goals

- [ ] Implement atomic mode setting
- [ ] Support multiple displays
- [ ] Add hardware acceleration

### Long-term Goals

- [ ] VirtIO GPU backend
- [ ] Wayland compositor support
- [ ] OpenGL/Vulkan acceleration

---

## Summary

This plan provides a complete roadmap for implementing SimpleDRM in Asterinas from scratch. Through phased implementation, we can:

1. **Quick Verification**: Phase 1 provides visible double-buffering effects
2. **Progressive Enhancement**: Each phase adds new features
3. **Backward Compatibility**: fbdev compatibility layer ensures legacy software works
4. **Standard Interface**: Following Linux DRM API makes porting existing software easier

Implementing this plan requires:
- **Core Team**: 2-3 people
- **Time**: 6-10 weeks (depending on target feature completeness)
- **Skills**: Operating system kernel, graphics stack, driver development experience
