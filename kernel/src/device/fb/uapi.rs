// SPDX-License-Identifier: MPL-2.0

use aster_framebuffer::PixelFormat;

use crate::prelude::*;

/// Bitfields describing the color channel layout; `struct fb_bitfield` in Linux.
///
/// Reference: <https://elixir.bootlin.com/linux/v6.17/source/include/uapi/linux/fb.h#L189>.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Pod)]
pub(super) struct FbBitfield {
    /// Bit offset of the field
    pub offset: u32,
    /// Length of the field in bits
    pub length: u32,
    /// Most significant bit position (0 = left, 1 = right)
    pub msb_right: u32,
}

impl FbBitfield {
    /// Converts pixel format to framebuffer bitfields for Linux compatibility.
    #[rustfmt::skip]
    pub(super) fn from_pixel_format(pixel_format: PixelFormat) -> (Self, Self, Self, Self) {
        match pixel_format {
            PixelFormat::Grayscale8 => (
                Self { offset: 0, length: 8, msb_right: 0 },
                Self { offset: 0, length: 8, msb_right: 0 },
                Self { offset: 0, length: 8, msb_right: 0 },
                Self::default(),
            ),
            PixelFormat::Rgb565 => (
                Self { offset: 11, length: 5, msb_right: 0 },
                Self { offset: 5, length: 6, msb_right: 0 },
                Self { offset: 0, length: 5, msb_right: 0 },
                Self::default(),
            ),
            PixelFormat::Rgb888 => (
                Self { offset: 16, length: 8, msb_right: 0 },
                Self { offset: 8, length: 8, msb_right: 0 },
                Self { offset: 0, length: 8, msb_right: 0 },
                Self::default(),
            ),
            PixelFormat::BgrReserved => (
                Self { offset: 16, length: 8, msb_right: 0 },
                Self { offset: 8, length: 8, msb_right: 0 },
                Self { offset: 0, length: 8, msb_right: 0 },
                Self { offset: 24, length: 8, msb_right: 0 },
            ),
        }
    }
}

/// Variable screen information for framebuffer devices; `struct fb_var_screeninfo` in Linux.
///
/// Reference: <https://elixir.bootlin.com/linux/v6.17/source/include/uapi/linux/fb.h#L243>.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
pub(super) struct FbVarScreenInfo {
    /// Visible resolution width
    pub xres: u32,
    /// Visible resolution height
    pub yres: u32,
    /// Virtual resolution width
    pub xres_virtual: u32,
    /// Virtual resolution height
    pub yres_virtual: u32,
    /// Offset from virtual to visible (horizontal)
    pub xoffset: u32,
    /// Offset from virtual to visible (vertical)
    pub yoffset: u32,
    /// Color depth in bits per pixel
    pub bits_per_pixel: u32,
    /// 0 = color, 1 = grayscale, >1 = FOURCC
    pub grayscale: u32,
    /// Red color bitfield in framebuffer memory
    pub red: FbBitfield,
    /// Green color bitfield in framebuffer memory
    pub green: FbBitfield,
    /// Blue color bitfield in framebuffer memory
    pub blue: FbBitfield,
    /// Transparency bitfield
    pub transp: FbBitfield,
    /// Non-standard pixel format indicator
    pub nonstd: u32,
    /// Activation control flags
    pub activate: u32,
    /// Height of display in millimeters
    pub height: u32,
    /// Width of display in millimeters
    pub width: u32,
    /// Acceleration capabilities (obsolete)
    pub accel_flags: u32,
    /// Pixel clock period in picoseconds
    pub pixclock: u32,
    /// Time from horizontal sync to picture
    pub left_margin: u32,
    /// Time from picture to horizontal sync
    pub right_margin: u32,
    /// Time from vertical sync to picture
    pub upper_margin: u32,
    /// Time from picture to vertical sync
    pub lower_margin: u32,
    /// Length of horizontal sync
    pub hsync_len: u32,
    /// Length of vertical sync
    pub vsync_len: u32,
    /// Synchronization flags
    pub sync: u32,
    /// Video mode flags
    pub vmode: u32,
    /// Screen rotation angle (counter-clockwise)
    pub rotate: u32,
    /// Colorspace for FOURCC-based modes
    pub colorspace: u32,
    /// Reserved for future compatibility
    pub reserved: [u32; 4],
}

/// Fixed screen information for framebuffer devices; `struct fb_fix_screeninfo` in Linux.
///
/// Reference: <https://elixir.bootlin.com/linux/v6.17/source/include/uapi/linux/fb.h#L158>.
#[padding_struct]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod)]
pub(super) struct FbFixScreenInfo {
    /// Identification string (e.g., "EFI VGA")
    pub id: [u8; 16],
    /// Start of framebuffer memory (physical address)
    pub smem_start: u64,
    /// Length of framebuffer memory in bytes
    pub smem_len: u32,
    /// Framebuffer type identifier
    pub type_: u32,
    /// Auxiliary type information (e.g., interleave)
    pub type_aux: u32,
    /// Visual type (mono, pseudo-color, true-color, etc.)
    pub visual: u32,
    /// Horizontal panning step size (0 = no panning)
    pub xpanstep: u16,
    /// Vertical panning step size (0 = no panning)
    pub ypanstep: u16,
    /// Y-axis wrapping step size (0 = no wrapping)
    pub ywrapstep: u16,
    /// Length of a screen line in bytes
    pub line_length: u32,
    /// Start of memory-mapped I/O (physical address)
    pub mmio_start: u64,
    /// Length of memory-mapped I/O region
    pub mmio_len: u32,
    /// Hardware acceleration type identifier
    pub accel: u32,
    /// Hardware capability flags
    pub capabilities: u16,
    /// Reserved for future compatibility
    pub reserved: [u16; 2],
}

/// Framebuffer colormap structure for userspace communication; `struct fb_cmap` in Linux.
///
/// Reference: <https://elixir.bootlin.com/linux/v6.17/source/include/uapi/linux/fb.h#L283>.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod)]
pub(super) struct FbCmapUser {
    /// Starting offset in colormap
    pub start: u32,
    /// Number of colormap entries
    pub len: u32,
    /// Pointer to red color values in userspace
    pub red: usize,
    /// Pointer to green color values in userspace
    pub green: usize,
    /// Pointer to blue color values in userspace
    pub blue: usize,
    /// Pointer to transparency values in userspace (may be null)
    pub transp: usize,
}

pub(in crate::device::fb) mod ioctl_defs {
    use super::{FbCmapUser, FbFixScreenInfo, FbVarScreenInfo};
    use crate::util::ioctl::{ioc, InData, InOutData, OutData, PassByVal};

    // Reference: <https://elixir.bootlin.com/linux/v6.17/source/include/uapi/linux/fb.h#L13-L38>

    pub(in crate::device::fb) type GetVarScreenInfo = ioc!(FBIOGET_VSCREENINFO, 0x4600, OutData<FbVarScreenInfo>);
    pub(in crate::device::fb) type PutVarScreenInfo = ioc!(FBIOPUT_VSCREENINFO, 0x4601, InOutData<FbVarScreenInfo>);
    pub(in crate::device::fb) type GetFixScreenInfo = ioc!(FBIOGET_FSCREENINFO, 0x4602, OutData<FbFixScreenInfo>);
    pub(in crate::device::fb) type GetColorMap      = ioc!(FBIOGETCMAP,         0x4604, InData<FbCmapUser>);
    pub(in crate::device::fb) type PutColorMap      = ioc!(FBIOPUTCMAP,         0x4605, InData<FbCmapUser>);

    pub(in crate::device::fb) type PanDisplay       = ioc!(FBIOPAN_DISPLAY,     0x4606, InOutData<FbVarScreenInfo>);
    pub(in crate::device::fb) type Blank            = ioc!(FBIOBLANK,           0x4611, InData<i32, PassByVal>);
}
