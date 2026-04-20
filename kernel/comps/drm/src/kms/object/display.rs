// SPDX-License-Identifier: MPL-2.0

const DRM_DISPLAY_MODE_LEN: usize = 32;
const DRM_DEFAULT_VREFRESH_HZ: u32 = 60;

bitflags::bitflags! {
    pub struct DrmModeFlag: u32 {
        const PHSYNC    = 1 << 0;
        const NHSYNC    = 1 << 1;
        const PVSYNC    = 1 << 2;
        const NVSYNC    = 1 << 3;
        const INTERLACE = 1 << 4;
        const DBLSCAN   = 1 << 5;
        const CSYNC     = 1 << 6;
        const PCSYNC    = 1 << 7;
        const NCSYNC    = 1 << 8;
        const HSKEW     = 1 << 9;
        const BCAST     = 1 << 10; // deprecated
        const PIXMUX    = 1 << 11; // deprecated
        const DBLCLK    = 1 << 12;
        const CLKDIV2   = 1 << 13;
    }
}

bitflags::bitflags! {
    pub struct DrmModeType: u32 {
        const BUILTIN   = 1 << 0; // deprecated
        const CLOCK_C   = (1 << 1) | Self::BUILTIN.bits(); // deprecated
        const CRTC_C    = (1 << 2) | Self::BUILTIN.bits(); // deprecated
        const PREFERRED = 1 << 3;
        const DEFAULT   = 1 << 4; // deprecated
        const USERDEF   = 1 << 5;
        const DRIVER    = 1 << 6;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DrmDisplayMode {
    clock: u32,
    hdisplay: u16,
    hsync_start: u16,
    hsync_end: u16,
    htotal: u16,
    hskew: u16,
    vdisplay: u16,
    vsync_start: u16,
    vsync_end: u16,
    vtotal: u16,
    vscan: u16,

    flags: u32,
    type_: u32,

    name: [u8; DRM_DISPLAY_MODE_LEN],
}

impl Default for DrmDisplayMode {
    fn default() -> Self {
        Self::new(1280, 800, DRM_DEFAULT_VREFRESH_HZ)
    }
}

impl DrmDisplayMode {
    /// Creates a simple fixed-refresh display mode.
    ///
    /// This follows Linux's `DRM_SIMPLE_MODE()` / `DRM_MODE_INIT()` style
    /// semantics: callers provide one resolution and one refresh rate, and the
    /// resulting mode is a simple fallback mode rather than a detailed timing
    /// mode discovered from EDID or a panel description.
    pub fn new(width: u16, height: u16, vrefresh_hz: u32) -> Self {
        let mut name = [0u8; DRM_DISPLAY_MODE_LEN];
        let s = alloc::format!("{width}x{height}");
        let n = s.as_bytes();
        let len = n.len().min(DRM_DISPLAY_MODE_LEN - 1);
        name[..len].copy_from_slice(&n[..len]);

        Self {
            clock: (width as u32) * (height as u32) * vrefresh_hz / 1000,
            hdisplay: width,
            hsync_start: width,
            hsync_end: width,
            htotal: width,
            hskew: 0,
            vdisplay: height,
            vsync_start: height,
            vsync_end: height,
            vtotal: height,
            vscan: 0,
            flags: 0,
            type_: DrmModeType::DRIVER.bits(),
            name,
        }
    }

    fn vrefresh(&self) -> u32 {
        if self.htotal == 0 || self.vtotal == 0 {
            return 0;
        }

        let mut num = self.clock as u64;
        let mut den = (self.htotal as u64) * (self.vtotal as u64);

        let flags = DrmModeFlag::from_bits_truncate(self.flags);

        if flags.contains(DrmModeFlag::INTERLACE) {
            num *= 2;
        }

        if flags.contains(DrmModeFlag::DBLSCAN) {
            den *= 2;
        }

        if self.vscan > 1 {
            den *= self.vscan as u64;
        }

        ((num * 1000 + den / 2) / den) as u32
    }
}

impl Into<DrmModeModeInfo> for DrmDisplayMode {
    fn into(self) -> DrmModeModeInfo {
        DrmModeModeInfo {
            clock: self.clock,
            hdisplay: self.hdisplay,
            hsync_start: self.hsync_start,
            hsync_end: self.hsync_end,
            htotal: self.htotal,
            hskew: self.hskew,
            vdisplay: self.vdisplay,
            vsync_start: self.vsync_start,
            vsync_end: self.vsync_end,
            vtotal: self.vtotal,
            vscan: self.vscan,
            vrefresh: self.vrefresh(),
            flags: self.flags,
            type_: self.type_,
            name: self.name,
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum SubpixelOrder {
    Unknown = 0,
    HorizontalRgb = 1,
    HorizontalBgr = 2,
    VerticalRgb = 3,
    VerticalBgr = 4,
    None = 5,
}

#[derive(Debug)]
pub struct DrmDisplayInfo {
    mm_width: u32,
    mm_height: u32,
    subpixel_order: SubpixelOrder,
}

impl Default for DrmDisplayInfo {
    fn default() -> Self {
        Self {
            mm_width: 0,
            mm_height: 0,
            subpixel_order: SubpixelOrder::Unknown,
        }
    }
}

impl DrmDisplayInfo {
    pub fn new(mm_width: u32, mm_height: u32, subpixel_order: SubpixelOrder) -> Self {
        Self {
            mm_width,
            mm_height,
            subpixel_order,
        }
    }

    pub fn mm_width(&self) -> u32 {
        self.mm_width
    }

    pub fn mm_height(&self) -> u32 {
        self.mm_height
    }

    pub fn subpixel_order(&self) -> u32 {
        self.subpixel_order as u32
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmModeModeInfo {
    pub clock: u32,
    pub hdisplay: u16,
    pub hsync_start: u16,
    pub hsync_end: u16,
    pub htotal: u16,
    pub hskew: u16,
    pub vdisplay: u16,
    pub vsync_start: u16,
    pub vsync_end: u16,
    pub vtotal: u16,
    pub vscan: u16,

    pub vrefresh: u32,

    pub flags: u32,
    pub type_: u32,

    pub name: [u8; DRM_DISPLAY_MODE_LEN],
}

impl Into<DrmDisplayMode> for DrmModeModeInfo {
    fn into(self) -> DrmDisplayMode {
        DrmDisplayMode {
            clock: self.clock,
            hdisplay: self.hdisplay,
            hsync_start: self.hsync_start,
            hsync_end: self.hsync_end,
            htotal: self.htotal,
            hskew: self.hskew,
            vdisplay: self.vdisplay,
            vsync_start: self.vsync_start,
            vsync_end: self.vsync_end,
            vtotal: self.vtotal,
            vscan: self.vscan,
            flags: self.flags,
            type_: self.type_,
            name: self.name,
        }
    }
}

const fn fourcc_code(a: u8, b: u8, c: u8, d: u8) -> u32 {
    (a as u32) | ((b as u32) << 8) | ((c as u32) << 16) | ((d as u32) << 24)
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum DrmDisplayFormat {
    XRGB8888 = fourcc_code(b'X', b'R', b'2', b'4'),
    ARGB8888 = fourcc_code(b'A', b'R', b'2', b'4'),
    XBGR8888 = fourcc_code(b'X', b'B', b'2', b'4'),
    RGBX8888 = fourcc_code(b'R', b'X', b'2', b'4'),
    BGRX8888 = fourcc_code(b'B', b'X', b'2', b'4'),
    C8 = fourcc_code(b'C', b'8', b' ', b' '),
    XRGB1555 = fourcc_code(b'X', b'R', b'1', b'5'),
    RGB565 = fourcc_code(b'R', b'G', b'1', b'6'),
    RGB888 = fourcc_code(b'R', b'G', b'2', b'4'),
    XRGB2101010 = fourcc_code(b'X', b'R', b'3', b'0'),
    Unknown = fourcc_code(b' ', b' ', b' ', b' '),
}
