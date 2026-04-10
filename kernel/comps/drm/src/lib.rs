// SPDX-License-Identifier: MPL-2.0

//! The DRM component for Asterinas.
//!
//! This component provides a simple DRM (Direct Rendering Manager) implementation
//! starting with a double-buffered framebuffer.

#![no_std]
#![deny(unsafe_code)]

extern crate alloc;

mod buffer;
mod simple;

pub use buffer::BackBuffer;
pub use simple::SimpleDrm;

use component::{ComponentInitError, init_component};

#[init_component]
fn init() -> Result<(), ComponentInitError> {
    Ok(())
}
