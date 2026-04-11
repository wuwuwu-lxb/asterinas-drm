// SPDX-License-Identifier: MPL-2.0

//! Simple DRM driver implementation.
//!
//! This module provides `SimpleDrm`, a minimal DRM-like driver that manages
//! double-buffered rendering. It wraps the hardware `FrameBuffer` and coordinates
//! drawing operations with the `BackBuffer`.

use alloc::sync::Arc;

use ostd::Result;
use spin::{Mutex, MutexGuard};

use crate::buffer::BackBuffer;

/// A simple DRM driver providing double-buffered rendering.
///
/// `SimpleDrm` wraps the hardware framebuffer and manages a back buffer for
/// tear-free rendering. All drawing operations are performed on the back buffer,
/// and `present()` atomically copies it to the hardware.
pub struct SimpleDrm {
    /// The hardware framebuffer
    fb: Arc<aster_framebuffer::FrameBuffer>,
    /// The back buffer for double-buffered rendering
    back_buffer: Mutex<BackBuffer>,
}

impl SimpleDrm {
    /// Creates a new `SimpleDrm` instance.
    ///
    /// This function acquires the global framebuffer and creates a corresponding
    /// back buffer with matching dimensions.
    pub fn new() -> Option<Self> {
        let fb = aster_framebuffer::FRAMEBUFFER.get()?;
        let width = fb.width();
        let height = fb.height();
        let bpp = fb.pixel_format().nbytes();
        let line_size = fb.line_size();

        let back_buffer = BackBuffer::new(width, height, bpp, line_size);

        Some(Self {
            fb: fb.clone(),
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
    pub fn present(&self) -> Result<()> {
        let back = self.back_buffer.lock();
        self.fb.write_bytes_at(0, back.data())?;
        Ok(())
    }

    /// Returns the width of the framebuffer.
    pub fn width(&self) -> usize {
        self.fb.width()
    }

    /// Returns the height of the framebuffer.
    pub fn height(&self) -> usize {
        self.fb.height()
    }
}
