// SPDX-License-Identifier: MPL-2.0

use alloc::sync::Arc;

use aster_framebuffer::{FRAMEBUFFER, FrameBuffer, PixelFormat};
use ostd::mm::{HasPaddr, HasSize};

use super::{
    drm_fbdev::DrmFbdevBackend,
    uapi::{FbBitfield, FbFixScreenInfo, FbVarScreenInfo},
};
use crate::prelude::*;

const FB_TYPE_PACKED_PIXELS: u32 = 0;
const FB_VISUAL_TRUECOLOR: u32 = 2;
const FB_ACCEL_NONE: u32 = 0;

#[derive(Clone, Debug)]
pub(super) struct FbInfo {
    backend: FbBackend,
}

#[derive(Clone, Debug)]
pub(super) enum FbBackend {
    Drm(Arc<DrmFbdevBackend>),
    Legacy(Arc<FrameBuffer>),
}

impl FbInfo {
    pub(super) fn new() -> Option<Self> {
        let backend = match DrmFbdevBackend::new() {
            Ok(drm_backend) => {
                let drm_backend = Arc::new(drm_backend);
                drm_backend.spawn_refresh_thread();
                FbBackend::Drm(drm_backend)
            }
            Err(error) => {
                ostd::warn!("failed to initialize DRM fbdev emulation: {:?}", error);
                let legacy = FRAMEBUFFER.get()?.clone();
                FbBackend::Legacy(legacy)
            }
        };

        Some(Self { backend })
    }

    pub(super) fn backend(&self) -> &FbBackend {
        &self.backend
    }
}

impl FbBackend {
    pub(super) fn is_drm(&self) -> bool {
        matches!(self, Self::Drm(_))
    }

    pub(super) fn collect_var_screen_info(&self) -> FbVarScreenInfo {
        // Default pixel clock calculation for efifb compatibility.
        const DEFAULT_PIXEL_CLOCK_DIVISOR: u32 = 10_000_000;

        // Default timing parameters for efifb compatibility.
        const DEFAULT_RIGHT_MARGIN: u32 = 32;
        const DEFAULT_UPPER_MARGIN: u32 = 16;
        const DEFAULT_LOWER_MARGIN: u32 = 4;
        const DEFAULT_VSYNC_LEN: u32 = 4;

        match self {
            FbBackend::Drm(backend) => {
                let (red, green, blue, transp) =
                    FbBitfield::from_pixel_format(PixelFormat::BgrReserved);

                FbVarScreenInfo {
                    xres: backend.width,
                    yres: backend.height,
                    xres_virtual: backend.width,
                    yres_virtual: backend.height,
                    bits_per_pixel: 32,
                    red,
                    green,
                    blue,
                    transp,
                    height: backend.height_mm,
                    width: backend.width_mm,
                    pixclock: DEFAULT_PIXEL_CLOCK_DIVISOR / backend.width * 1000 / backend.height,
                    left_margin: (backend.width / 8) & 0xf8,
                    right_margin: DEFAULT_RIGHT_MARGIN,
                    upper_margin: DEFAULT_UPPER_MARGIN,
                    lower_margin: DEFAULT_LOWER_MARGIN,
                    vsync_len: DEFAULT_VSYNC_LEN,
                    hsync_len: (backend.width / 8) & 0xf8,
                    ..Default::default()
                }
            }
            FbBackend::Legacy(framebuffer) => {
                let pixel_format = framebuffer.pixel_format();
                let (red, green, blue, transp) = FbBitfield::from_pixel_format(pixel_format);

                FbVarScreenInfo {
                    xres: framebuffer.width() as u32,
                    yres: framebuffer.height() as u32,
                    xres_virtual: framebuffer.width() as u32,
                    yres_virtual: framebuffer.height() as u32,
                    bits_per_pixel: (8 * pixel_format.nbytes()) as u32,
                    red,
                    green,
                    blue,
                    transp,
                    pixclock: DEFAULT_PIXEL_CLOCK_DIVISOR / framebuffer.width() as u32 * 1000
                        / framebuffer.height() as u32,
                    left_margin: (framebuffer.width() as u32 / 8) & 0xf8,
                    right_margin: DEFAULT_RIGHT_MARGIN,
                    upper_margin: DEFAULT_UPPER_MARGIN,
                    lower_margin: DEFAULT_LOWER_MARGIN,
                    vsync_len: DEFAULT_VSYNC_LEN,
                    hsync_len: (framebuffer.width() as u32 / 8) & 0xf8,
                    ..Default::default()
                }
            }
        }
    }

    pub(super) fn collect_fix_screen_info(&self) -> FbFixScreenInfo {
        match self {
            FbBackend::Drm(backend) => FbFixScreenInfo {
                id: *b"drmfb\0\0\0\0\0\0\0\0\0\0\0",
                smem_len: backend.size_bytes as u32,
                type_: FB_TYPE_PACKED_PIXELS,
                visual: FB_VISUAL_TRUECOLOR,
                line_length: backend.pitch_bytes,
                accel: FB_ACCEL_NONE,
                ..Default::default()
            },
            FbBackend::Legacy(framebuffer) => FbFixScreenInfo {
                id: *b"asterfb\0\0\0\0\0\0\0\0\0",
                smem_start: framebuffer.io_mem().paddr() as u64,
                smem_len: framebuffer.io_mem().size() as u32,
                type_: FB_TYPE_PACKED_PIXELS,
                visual: FB_VISUAL_TRUECOLOR,
                line_length: framebuffer.line_size() as u32,
                accel: FB_ACCEL_NONE,
                ..Default::default()
            },
        }
    }
}
