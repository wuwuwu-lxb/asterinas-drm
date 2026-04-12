// SPDX-License-Identifier: MPL-2.0

//! Simple DRM driver implementation.
//!
//! This module provides `SimpleDrm`, a minimal DRM-like driver that manages
//! double-buffered rendering. It wraps the hardware `FrameBuffer` and coordinates
//! drawing operations with the `BackBuffer`.

use alloc::sync::Arc;

use aster_framebuffer::{FrameBufferOps, DEFAULT_FB_BPP, DEFAULT_FB_HEIGHT, DEFAULT_FB_WIDTH};
use ostd::Result;
use spin::{Mutex, MutexGuard};

use crate::buffer::BackBuffer;

/// A simple DRM driver providing double-buffered rendering.
///
/// `SimpleDrm` wraps the hardware framebuffer and manages a back buffer for
/// tear-free rendering. All drawing operations are performed on the back buffer,
/// and `present()` atomically copies it to the hardware.
pub struct SimpleDrm {
    /// The framebuffer (hardware MMIO or RAM fallback).
    /// None when no framebuffer is available at all.
    fb: Option<Arc<dyn FrameBufferOps + Send + Sync>>,
    /// The back buffer for double-buffered rendering
    back_buffer: Mutex<BackBuffer>,
}

impl SimpleDrm {
    /// Creates a new `SimpleDrm` instance.
    ///
    /// This function acquires the global framebuffer and creates a corresponding
    /// back buffer with matching dimensions.
    pub fn new() -> Option<Self> {
        let fb = aster_framebuffer::FRAMEBUFFER.get().cloned();
        let (width, height, bpp, line_size) = match fb {
            Some(ref f) => (f.width(), f.height(), f.pixel_format().nbytes(), f.line_size()),
            None => {
                // No hardware framebuffer — use default RAM fallback dimensions
                log::info!("SimpleDrm: no hardware framebuffer, using RAM fallback");
                (DEFAULT_FB_WIDTH, DEFAULT_FB_HEIGHT, DEFAULT_FB_BPP,
                 DEFAULT_FB_WIDTH * DEFAULT_FB_BPP)
            }
        };

        let back_buffer = BackBuffer::new(width, height, bpp, line_size);

        Some(Self {
            fb,
            back_buffer: Mutex::new(back_buffer),
        })
    }

    /// Returns a locked reference to the back buffer.
    pub fn back_buffer(&self) -> MutexGuard<'_, BackBuffer> {
        self.back_buffer.lock()
    }

    /// Presents the back buffer to the hardware framebuffer.
    ///
    /// This copies the entire back buffer contents to the hardware framebuffer,
    /// resulting in a tear-free image update.
    /// If no framebuffer is available, this is a no-op.
    pub fn present(&self) -> Result<()> {
        let back = self.back_buffer.lock();
        if let Some(ref fb) = self.fb {
            fb.write_bytes_at(0, back.data())?;
        }
        Ok(())
    }

    /// Returns the width of the framebuffer.
    pub fn width(&self) -> usize {
        self.fb.as_ref().map(|f| f.width()).unwrap_or(DEFAULT_FB_WIDTH)
    }

    /// Returns the height of the framebuffer.
    pub fn height(&self) -> usize {
        self.fb.as_ref().map(|f| f.height()).unwrap_or(DEFAULT_FB_HEIGHT)
    }
}
