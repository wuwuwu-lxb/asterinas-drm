// SPDX-License-Identifier: MPL-2.0

use alloc::{sync::Arc, vec::Vec};

use ostd::{
    Error, Result,
    boot::boot_info,
    io::IoMem,
    mm::{CachePolicy, HasSize, VmIo},
    sync::Mutex,
};
use spin::Once;

use crate::{Pixel, PixelFormat, RenderedPixel};

/// Default framebuffer dimensions for RAM fallback.
pub const DEFAULT_FB_WIDTH: usize = 1024;
pub const DEFAULT_FB_HEIGHT: usize = 768;
pub const DEFAULT_FB_BPP: usize = 4; // 32bpp = 4 bytes per pixel

/// Maximum number of colormap entries (standard 8-bit palette)
pub const MAX_CMAP_SIZE: usize = 256;

/// The framebuffer used for text or graphical output.
///
/// # Notes
///
/// It is highly recommended to use a synchronization primitive, such as a `SpinLock`, to
/// lock the framebuffer before performing any operation on it.
/// Failing to properly synchronize access can result in corrupted framebuffer content
/// or unspecified behavior during rendering.
#[derive(Debug)]
pub struct FrameBuffer {
    io_mem: IoMem,
    width: usize,
    height: usize,
    line_size: usize,
    pixel_format: PixelFormat,
    cmap: Mutex<FbCmap>,
}

/// A single entry in the color map with 16-bit color values.
///
/// Linux framebuffer colormap uses 16-bit values (0-65535) for each color channel
/// to support high precision color mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorMapEntry {
    /// Red color value (16-bit)
    pub red: u16,
    /// Green color value (16-bit)
    pub green: u16,
    /// Blue color value (16-bit)
    pub blue: u16,
    /// Transparency value (16-bit)
    pub transp: u16,
}

/// Internal framebuffer colormap structure.
#[derive(Debug, Clone)]
struct FbCmap {
    /// Color map entries
    entries: Vec<ColorMapEntry>,
}

/// Trait for framebuffer operations that can be performed on both
/// hardware MMIO-backed framebuffers and RAM-backed fallback framebuffers.
pub trait FrameBufferOps: Send + Sync {
    /// Returns the width of the framebuffer in pixels.
    fn width(&self) -> usize;
    /// Returns the height of the framebuffer in pixels.
    fn height(&self) -> usize;
    /// Returns the line size of the framebuffer in bytes.
    fn line_size(&self) -> usize;
    /// Returns the pixel format of the framebuffer.
    fn pixel_format(&self) -> PixelFormat;
    /// Returns the total size of the framebuffer in bytes.
    fn size(&self) -> usize;
    /// Returns a reference to the `IoMem` instance, if available (MMIO-backed only).
    /// Returns `None` for RAM-backed fallback framebuffers.
    fn io_mem(&self) -> Option<&IoMem>;
    /// Calculates the raw byte offset for a pixel at (x, y).
    fn pixel_offset(&self, x: usize, y: usize) -> usize;
    /// Renders a pixel according to the framebuffer's pixel format.
    fn render_pixel(&self, pixel: Pixel) -> RenderedPixel;
    /// Writes raw bytes at the specified offset.
    fn write_bytes_at(&self, offset: usize, bytes: &[u8]) -> Result<()>;
    /// Reads raw bytes at the specified offset into a buffer.
    fn read_bytes_at(&self, offset: usize, buf: &mut [u8]) -> Result<()>;
    /// Clears the framebuffer with default color (black).
    fn clear(&mut self);
    /// Sets color map entries starting from the given index.
    fn set_color_map(&self, start: usize, entries: &[ColorMapEntry]) -> Result<()>;
    /// Gets color map entries from the given range.
    fn get_color_map(&self, start: usize, len: usize) -> Option<Vec<ColorMapEntry>>;
}

pub static FRAMEBUFFER: Once<Arc<dyn FrameBufferOps + Send + Sync>> = Once::new();

pub(crate) fn init() {
    let fb_arg = boot_info().framebuffer_arg;
    log::info!("Framebuffer boot arg: {:?}", fb_arg);

    let Some(framebuffer_arg) = fb_arg else {
        // No hardware framebuffer — use RAM fallback
        log::info!("No hardware framebuffer, using RAM fallback {}x{}x32bpp",
            DEFAULT_FB_WIDTH, DEFAULT_FB_HEIGHT);
        let ram_fb = RamFrameBuffer::new();
        let fb_ops: Arc<dyn FrameBufferOps + Send + Sync> = Arc::new(ram_fb);
        FRAMEBUFFER.call_once(|| fb_ops);
        return;
    };

    if framebuffer_arg.address == 0 {
        log::error!("Framebuffer address is zero");
        // Fallback to RAM
        log::info!("Using RAM fallback {}x{}x32bpp", DEFAULT_FB_WIDTH, DEFAULT_FB_HEIGHT);
        let ram_fb = RamFrameBuffer::new();
        let fb_ops: Arc<dyn FrameBufferOps + Send + Sync> = Arc::new(ram_fb);
        FRAMEBUFFER.call_once(|| fb_ops);
        return;
    }

    // FIXME: There are several pixel formats that have the same BPP. We lost the information
    // during the boot phase, so here we guess the pixel format on a best effort basis.
    let pixel_format = match framebuffer_arg.bpp {
        8 => PixelFormat::Grayscale8,
        16 => PixelFormat::Rgb565,
        24 => PixelFormat::Rgb888,
        32 => PixelFormat::BgrReserved,
        _ => {
            log::error!(
                "Unsupported framebuffer pixel format: {} bpp",
                framebuffer_arg.bpp
            );
            // Fallback to RAM
            log::info!("Using RAM fallback {}x{}x32bpp", DEFAULT_FB_WIDTH, DEFAULT_FB_HEIGHT);
            let ram_fb = RamFrameBuffer::new();
            let fb_ops: Arc<dyn FrameBufferOps + Send + Sync> = Arc::new(ram_fb);
            FRAMEBUFFER.call_once(|| fb_ops);
            return;
        }
    };

    let framebuffer = {
        // FIXME: There can be more than `width` pixels per framebuffer line due to alignment
        // purposes. We need to collect this information during the boot phase.
        let line_size = framebuffer_arg
            .width
            .checked_mul(pixel_format.nbytes())
            .unwrap();
        let fb_size = framebuffer_arg.height.checked_mul(line_size).unwrap();

        let fb_base = framebuffer_arg.address;
        // Use write-combining for framebuffer to enable faster write operations.
        // Write-combining allows the CPU to combine multiple writes into fewer bus transactions,
        // which is ideal for framebuffer access patterns (sequential writes).
        let io_mem = IoMem::acquire_with_cache_policy(
            fb_base..fb_base.checked_add(fb_size).unwrap(),
            CachePolicy::WriteCombining,
        )
        .unwrap();

        let default_cmap = FbCmap {
            entries: Vec::new(),
        };

        FrameBuffer {
            io_mem,
            width: framebuffer_arg.width,
            height: framebuffer_arg.height,
            line_size,
            pixel_format,
            cmap: Mutex::new(default_cmap),
        }
    };

    framebuffer.clear();
    let fb_ops: Arc<dyn FrameBufferOps + Send + Sync> = Arc::new(framebuffer);
    FRAMEBUFFER.call_once(|| fb_ops);
}

impl FrameBuffer {
    /// Returns the width of the framebuffer in pixels.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the height of the framebuffer in pixels.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Returns the line size of the framebuffer in bytes.
    pub fn line_size(&self) -> usize {
        self.line_size
    }

    /// Returns a reference to the `IoMem` instance of the framebuffer.
    pub fn io_mem(&self) -> &IoMem {
        &self.io_mem
    }

    /// Returns the pixel format of the framebuffer.
    pub fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    /// Renders the pixel according to the pixel format of the framebuffer.
    pub fn render_pixel(&self, pixel: Pixel) -> RenderedPixel {
        pixel.render(self.pixel_format)
    }

    /// Calculates the offset of a pixel at the specified position.
    pub fn calc_offset(&self, x: usize, y: usize) -> PixelOffset<'_> {
        PixelOffset {
            fb: self,
            offset: (x * self.pixel_format.nbytes() + y * self.line_size) as isize,
        }
    }

    /// Writes a pixel at the specified position.
    pub fn write_pixel_at(&self, offset: PixelOffset, pixel: RenderedPixel) -> Result<()> {
        self.io_mem.write_bytes(offset.as_usize(), pixel.as_slice())
    }

    /// Writes raw bytes at the specified offset.
    pub fn write_bytes_at(&self, offset: usize, bytes: &[u8]) -> Result<()> {
        self.io_mem.write_bytes(offset, bytes)
    }

    /// Clears the framebuffer with default color (black).
    pub fn clear(&self) {
        let frame = alloc::vec![0u8; self.io_mem().size()];
        self.write_bytes_at(0, &frame).unwrap();
    }

    /// Sets color map entries starting from the given index.
    ///
    /// For efifb devices, hardware color map is not supported, so we maintain
    /// an in-memory map for software emulation.
    pub fn set_color_map(&self, start: usize, entries: &[ColorMapEntry]) -> Result<()> {
        if start > MAX_CMAP_SIZE || entries.len() > MAX_CMAP_SIZE - start {
            return Err(Error::InvalidArgs);
        }

        let mut cmap = self.cmap.lock();
        let required_len = start + entries.len();

        // Ensure the colormap has enough space
        if cmap.entries.len() < required_len {
            cmap.entries.resize(
                required_len,
                ColorMapEntry {
                    red: 0,
                    green: 0,
                    blue: 0,
                    transp: 0,
                },
            );
        }

        // Copy the entries
        cmap.entries[start..start + entries.len()].copy_from_slice(entries);

        Ok(())
    }

    /// Gets color map entries from the given range.
    pub fn get_color_map(&self, start: usize, len: usize) -> Option<Vec<ColorMapEntry>> {
        let cmap = self.cmap.lock();

        if start >= cmap.entries.len() || len > cmap.entries.len() - start {
            return None;
        }

        Some(cmap.entries[start..start + len].to_vec())
    }
}

impl FrameBufferOps for FrameBuffer {
    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn line_size(&self) -> usize {
        self.line_size
    }

    fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    fn size(&self) -> usize {
        self.io_mem.size()
    }

    fn io_mem(&self) -> Option<&IoMem> {
        Some(&self.io_mem)
    }

    fn pixel_offset(&self, x: usize, y: usize) -> usize {
        x * self.pixel_format.nbytes() + y * self.line_size
    }

    fn render_pixel(&self, pixel: Pixel) -> RenderedPixel {
        pixel.render(self.pixel_format)
    }

    fn write_bytes_at(&self, offset: usize, bytes: &[u8]) -> Result<()> {
        self.io_mem.write_bytes(offset, bytes)
    }

    fn read_bytes_at(&self, offset: usize, buf: &mut [u8]) -> Result<()> {
        self.io_mem.read_bytes(offset, buf)
    }

    fn clear(&mut self) {
        let frame = alloc::vec![0u8; self.io_mem.size()];
        self.io_mem.write_bytes(0, &frame).unwrap();
    }

    fn set_color_map(&self, start: usize, entries: &[ColorMapEntry]) -> Result<()> {
        if start > MAX_CMAP_SIZE || entries.len() > MAX_CMAP_SIZE - start {
            return Err(Error::InvalidArgs);
        }

        let mut cmap = self.cmap.lock();
        let required_len = start + entries.len();
        if cmap.entries.len() < required_len {
            cmap.entries.resize(
                required_len,
                ColorMapEntry {
                    red: 0,
                    green: 0,
                    blue: 0,
                    transp: 0,
                },
            );
        }
        cmap.entries[start..start + entries.len()].copy_from_slice(entries);
        Ok(())
    }

    fn get_color_map(&self, start: usize, len: usize) -> Option<Vec<ColorMapEntry>> {
        let cmap = self.cmap.lock();
        if start >= cmap.entries.len() || len > cmap.entries.len() - start {
            return None;
        }
        Some(cmap.entries[start..start + len].to_vec())
    }
}

/// A RAM-backed framebuffer used as fallback when no hardware framebuffer is available.
///
/// This provides a software-only framebuffer that stores pixels in RAM.
/// It implements `FrameBufferOps` but does NOT provide MMIO access (io_mem).
#[derive(Debug)]
pub struct RamFrameBuffer {
    width: usize,
    height: usize,
    line_size: usize,
    pixel_format: PixelFormat,
    /// Protected by Mutex for interior mutability via &self.
    data: Mutex<Vec<u8>>,
    cmap: Mutex<FbCmap>,
}

impl RamFrameBuffer {
    /// Creates a new RAM-backed framebuffer with default dimensions.
    pub fn new() -> Self {
        let width = DEFAULT_FB_WIDTH;
        let height = DEFAULT_FB_HEIGHT;
        let pixel_format = PixelFormat::BgrReserved;
        let line_size = width * pixel_format.nbytes();
        let size = height * line_size;
        let data = alloc::vec![0u8; size];

        RamFrameBuffer {
            width,
            height,
            line_size,
            pixel_format,
            data: Mutex::new(data),
            cmap: Mutex::new(FbCmap { entries: Vec::new() }),
        }
    }
}

impl FrameBufferOps for RamFrameBuffer {
    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn line_size(&self) -> usize {
        self.line_size
    }

    fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    fn size(&self) -> usize {
        self.data.lock().len()
    }

    fn io_mem(&self) -> Option<&IoMem> {
        None
    }

    fn pixel_offset(&self, x: usize, y: usize) -> usize {
        x * self.pixel_format.nbytes() + y * self.line_size
    }

    fn render_pixel(&self, pixel: Pixel) -> RenderedPixel {
        pixel.render(self.pixel_format)
    }

    fn write_bytes_at(&self, offset: usize, bytes: &[u8]) -> Result<()> {
        let mut data = self.data.lock();
        if offset >= data.len() {
            return Err(Error::InvalidArgs);
        }
        let end = (offset + bytes.len()).min(data.len());
        data[offset..end].copy_from_slice(&bytes[..end - offset]);
        Ok(())
    }

    fn read_bytes_at(&self, offset: usize, buf: &mut [u8]) -> Result<()> {
        let data = self.data.lock();
        if offset >= data.len() {
            return Err(Error::InvalidArgs);
        }
        let end = (offset + buf.len()).min(data.len());
        buf[..end - offset].copy_from_slice(&data[offset..end]);
        if end - offset < buf.len() {
            buf[end - offset..].fill(0);
        }
        Ok(())
    }

    fn clear(&mut self) {
        self.data.lock().fill(0);
    }

    fn set_color_map(&self, start: usize, entries: &[ColorMapEntry]) -> Result<()> {
        if start > MAX_CMAP_SIZE || entries.len() > MAX_CMAP_SIZE - start {
            return Err(Error::InvalidArgs);
        }

        let mut cmap = self.cmap.lock();
        let required_len = start + entries.len();
        if cmap.entries.len() < required_len {
            cmap.entries.resize(
                required_len,
                ColorMapEntry {
                    red: 0,
                    green: 0,
                    blue: 0,
                    transp: 0,
                },
            );
        }
        cmap.entries[start..start + entries.len()].copy_from_slice(entries);
        Ok(())
    }

    fn get_color_map(&self, start: usize, len: usize) -> Option<Vec<ColorMapEntry>> {
        let cmap = self.cmap.lock();
        if start >= cmap.entries.len() || len > cmap.entries.len() - start {
            return None;
        }
        Some(cmap.entries[start..start + len].to_vec())
    }
}

/// The offset of a pixel in the framebuffer.
#[derive(Debug, Clone, Copy)]
pub struct PixelOffset<'a> {
    fb: &'a FrameBuffer,
    offset: isize,
}

impl PixelOffset<'_> {
    /// Adds the specified delta to the x coordinate.
    pub fn x_add(&mut self, x_delta: isize) {
        let delta = x_delta * self.fb.pixel_format.nbytes() as isize;
        self.offset += delta;
    }

    /// Adds the specified delta to the y coordinate.
    pub fn y_add(&mut self, y_delta: isize) {
        let delta = y_delta * self.fb.line_size as isize;
        self.offset += delta;
    }

    /// Returns the offset value as a `usize`.
    pub fn as_usize(&self) -> usize {
        self.offset as usize
    }
}
