// SPDX-License-Identifier: MPL-2.0

//! The framebuffer of Asterinas.
#![no_std]
#![deny(unsafe_code)]

extern crate alloc;

mod ansi_escape;
mod console;
mod dummy_console;
mod framebuffer;
mod pixel;

use component::{ComponentInitError, init_component};
pub use console::{CONSOLE_NAME, ConsoleCallbacks, FRAMEBUFFER_CONSOLE};
pub use dummy_console::DummyFramebufferConsole;
pub use framebuffer::{ColorMapEntry, FRAMEBUFFER, FrameBuffer, FrameBufferOps, MAX_CMAP_SIZE, RamFrameBuffer, DEFAULT_FB_WIDTH, DEFAULT_FB_HEIGHT, DEFAULT_FB_BPP};
pub use pixel::{Pixel, PixelFormat, RenderedPixel};

#[init_component]
fn init() -> Result<(), ComponentInitError> {
    framebuffer::init();
    console::init();
    Ok(())
}
