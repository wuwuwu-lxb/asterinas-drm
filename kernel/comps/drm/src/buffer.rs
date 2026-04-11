// SPDX-License-Identifier: MPL-2.0

//! Back buffer management for double-buffered rendering.
//!
//! The `BackBuffer` provides an in-memory buffer that serves as the intermediate
//! drawing surface. All rendering operations write to this buffer first,
//! and the final `present()` call copies it to the hardware framebuffer.

use alloc::vec;
use alloc::vec::Vec;

use ostd::Result;

/// The back buffer for double-buffered rendering.
///
/// The back buffer is an in-memory representation of the framebuffer surface.
/// Rendering is performed on this buffer first, then presented to the hardware
/// framebuffer via `SimpleDrm::present()`.
#[derive(Debug)]
pub struct BackBuffer {
    /// Raw pixel data buffer
    data: Vec<u8>,
    /// Width in pixels
    width: usize,
    /// Height in pixels
    height: usize,
    /// Bytes per pixel
    bpp: usize,
    /// Line size in bytes (width * bpp, potentially aligned)
    line_size: usize,
}

impl BackBuffer {
    /// Creates a new back buffer with the given dimensions and bytes per pixel.
    pub fn new(width: usize, height: usize, bpp: usize, line_size: usize) -> Self {
        let size = height * line_size;
        Self {
            data: vec![0u8; size],
            width,
            height,
            bpp,
            line_size,
        }
    }

    /// Returns the width of the back buffer in pixels.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the height of the back buffer in pixels.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Returns the bytes per pixel.
    pub fn bpp(&self) -> usize {
        self.bpp
    }

    /// Returns the line size in bytes.
    pub fn line_size(&self) -> usize {
        self.line_size
    }

    /// Returns a mutable slice of the underlying buffer.
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Returns the raw buffer data.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Clears the back buffer with the specified color.
    pub fn clear(&mut self, color: u32) {
        let bytes = self.bpp;
        let mut color_bytes = [0u8; 4];
        for (i, byte) in color_bytes.iter_mut().enumerate().take(bytes) {
            *byte = ((color >> (i * 8)) & 0xFF) as u8;
        }

        for chunk in self.data.chunks_exact_mut(self.line_size) {
            for pixel_bytes in chunk.chunks_exact_mut(bytes) {
                pixel_bytes.copy_from_slice(&color_bytes[..bytes]);
            }
        }
    }

    /// Writes a single pixel at the specified offset.
    pub fn write_pixel_at(&mut self, offset: usize, pixel: &[u8]) -> Result<()> {
        if offset + pixel.len() > self.data.len() {
            return Err(ostd::Error::InvalidArgs);
        }
        self.data[offset..offset + pixel.len()].copy_from_slice(pixel);
        Ok(())
    }

    /// Writes raw bytes at the specified offset.
    pub fn write_bytes_at(&mut self, offset: usize, bytes: &[u8]) -> Result<()> {
        if offset + bytes.len() > self.data.len() {
            return Err(ostd::Error::InvalidArgs);
        }
        self.data[offset..offset + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
}
