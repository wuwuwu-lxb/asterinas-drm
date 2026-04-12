// SPDX-License-Identifier: MPL-2.0

//! Virtio-GPU control queue command structures.
//!
//! Reference: Virtio-GPU Spec Section 5.10


/// Virtio GPU control header, common to all commands and responses.
#[derive(Debug, Clone, Copy, Pod)]
#[repr(C)]
pub struct VirtioGpuCtrlHdr {
    /// Command type (see VIRTIO_GPU_CMD_* constants)
    pub type_: u32,
    /// Flags for the command
    pub flags: u32,
}

/// Command types for control queue
pub mod cmd {
    use super::VirtioGpuCtrlHdr;

    // Display info commands (0x0100-0x0110)
    pub const VIRTIO_GPU_CMD_GET_DISPLAY_INFO: u32 = 0x0100;
    pub const VIRTIO_GPU_CMD_RESOURCE_CREATE_2D: u32 = 0x0101;
    pub const VIRTIO_GPU_CMD_RESOURCE_UNREF: u32 = 0x0102;
    pub const VIRTIO_GPU_CMD_SET_SCANOUT: u32 = 0x0103;
    pub const VIRTIO_GPU_CMD_FLUSH: u32 = 0x0104;
    pub const VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D: u32 = 0x0105;
    pub const VIRTIO_GPU_CMD_ATTACH_BACKING: u32 = 0x0106;
    pub const VIRTIO_GPU_CMD_DETACH_BACKING: u32 = 0x0107;
    pub const VIRTIO_GPU_CMD_UPDATE_CURSOR: u32 = 0x0110;
    pub const VIRTIO_GPU_CMD_MOVE_CURSOR: u32 = 0x0111;

    // Capability commands (0x0200-0x0210)
    pub const VIRTIO_GPU_CMD_GET_CAPSET_INFO: u32 = 0x0200;
    pub const VIRTIO_GPU_CMD_GET_EDID: u32 = 0x0201;
}

/// Pixel formats supported by virtio-gpu
pub mod format {
    /// 32-bit BGRX format (8:8:8:8)
    pub const VIRTIO_GPU_FORMAT_B8G8R8X8: u32 = 1;
    /// 32-bit BGRA format (8:8:8:8)
    pub const VIRTIO_GPU_FORMAT_B8G8R8A8: u32 = 2;
    /// 16-bit RGB565 format (5:6:5)
    pub const VIRTIO_GPU_FORMAT_R5G6B5: u32 = 3;
}

/// Command: Resource Create 2D
#[derive(Debug, Clone, Copy, Pod)]
#[repr(C)]
pub struct VirtioGpuCmdResourceCreate2D {
    /// Control header
    pub hdr: VirtioGpuCtrlHdr,
    /// Resource ID to create
    pub resource_id: u32,
    /// Padding
    _padding1: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Pixel format (see format::* constants)
    pub format: u32,
    /// Resource flags
    pub flags: u32,
}

impl VirtioGpuCmdResourceCreate2D {
    /// Creates a new RESOURCE_CREATE_2D command
    pub fn new(resource_id: u32, width: u32, height: u32, format: u32) -> Self {
        Self {
            hdr: VirtioGpuCtrlHdr {
                type_: cmd::VIRTIO_GPU_CMD_RESOURCE_CREATE_2D,
                flags: 0,
            },
            resource_id,
            _padding1: 0,
            width,
            height,
            format,
            flags: 0,
        }
    }
}

/// Command: Set Scanout
#[derive(Debug, Clone, Copy, Pod)]
#[repr(C)]
pub struct VirtioGpuCmdSetScanout {
    /// Control header
    pub hdr: VirtioGpuCtrlHdr,
    /// Scanout ID
    pub scanout_id: u32,
    /// Resource ID to display
    pub resource_id: u32,
    /// X coordinate
    pub x: u32,
    /// Y coordinate
    pub y: u32,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
}

impl VirtioGpuCmdSetScanout {
    /// Creates a new SET_SCANOUT command
    pub fn new(scanout_id: u32, resource_id: u32, x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            hdr: VirtioGpuCtrlHdr {
                type_: cmd::VIRTIO_GPU_CMD_SET_SCANOUT,
                flags: 0,
            },
            scanout_id,
            resource_id,
            x,
            y,
            width,
            height,
        }
    }
}

/// Command: Transfer to Host 2D
#[derive(Debug, Clone, Copy, Pod)]
#[repr(C)]
pub struct VirtioGpuCmdTransferToHost2D {
    /// Control header
    pub hdr: VirtioGpuCtrlHdr,
    /// Resource ID
    pub resource_id: u32,
    /// Padding
    _padding1: u32,
    /// Offset within resource
    pub offset: u64,
    /// X coordinate
    pub x: u32,
    /// Y coordinate
    pub y: u32,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
}

impl VirtioGpuCmdTransferToHost2D {
    /// Creates a new TRANSFER_TO_HOST_2D command
    pub fn new(resource_id: u32, offset: u64, x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            hdr: VirtioGpuCtrlHdr {
                type_: cmd::VIRTIO_GPU_CMD_TRANSFER_TO_HOST_2D,
                flags: 0,
            },
            resource_id,
            _padding1: 0,
            offset,
            x,
            y,
            width,
            height,
        }
    }
}

/// Command: Flush
#[derive(Debug, Clone, Copy, Pod)]
#[repr(C)]
pub struct VirtioGpuCmdFlush {
    /// Control header
    pub hdr: VirtioGpuCtrlHdr,
    /// Resource ID
    pub resource_id: u32,
    /// Padding
    _padding1: u32,
    /// X coordinate
    pub x: u32,
    /// Y coordinate
    pub y: u32,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
}

impl VirtioGpuCmdFlush {
    /// Creates a new FLUSH command
    pub fn new(resource_id: u32, x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            hdr: VirtioGpuCtrlHdr {
                type_: cmd::VIRTIO_GPU_CMD_FLUSH,
                flags: 0,
            },
            resource_id,
            _padding1: 0,
            x,
            y,
            width,
            height,
        }
    }
}

/// Memory backing structure for attach/detach backing commands
#[derive(Debug, Clone, Copy, Pod)]
#[repr(C)]
pub struct VirtioGpuMemEntry {
    /// Physical address (guest)
    pub addr: u64,
    /// Length of the region
    pub length: u32,
    /// Padding
    _padding: u32,
}

/// Command: Attach Backing
#[derive(Debug, Clone, Copy, Pod)]
#[repr(C)]
pub struct VirtioGpuCmdAttachBacking {
    /// Control header
    pub hdr: VirtioGpuCtrlHdr,
    /// Resource ID
    pub resource_id: u32,
    /// Padding
    _padding1: u32,
    /// Number of memory entries
    pub nr_entries: u32,
    /// Padding
    _padding2: u32,
}

impl VirtioGpuCmdAttachBacking {
    /// Creates a new ATTACH_BACKING command
    pub fn new(resource_id: u32, nr_entries: u32) -> Self {
        Self {
            hdr: VirtioGpuCtrlHdr {
                type_: cmd::VIRTIO_GPU_CMD_ATTACH_BACKING,
                flags: 0,
            },
            resource_id,
            _padding1: 0,
            nr_entries,
            _padding2: 0,
        }
    }
}

/// Display information returned by GET_DISPLAY_INFO
#[derive(Debug, Clone, Copy, Pod)]
#[repr(C)]
pub struct VirtioGpuDisplayOne {
    /// X coordinate
    pub x: u32,
    /// Y coordinate
    pub y: u32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Enabled flag
    pub enabled: u32,
    /// Flags
    pub flags: u32,
}

/// Response: Display info (multiple displays)
#[derive(Debug, Clone, Copy, Pod)]
#[repr(C)]
pub struct VirtioGpuRespDisplayInfo {
    /// Header
    pub hdr: VirtioGpuCtrlHdr,
    /// Display information (up to 16)
    pub displays: [VirtioGpuDisplayOne; 16],
}

/// Response: OK (empty success response)
#[derive(Debug, Clone, Copy, Pod)]
#[repr(C)]
pub struct VirtioGpuRespOk {
    /// Header
    pub hdr: VirtioGpuCtrlHdr,
}
