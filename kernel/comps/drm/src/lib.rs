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

use alloc::sync::Arc;
use component::{ComponentInitError, init_component};
use spin::Once;

static SIMPLE_DRM: Once<Arc<SimpleDrm>> = Once::new();

#[init_component]
fn init() -> Result<(), ComponentInitError> {
    let Some(drm) = SimpleDrm::new() else {
        log::warn!("aster-drm: no framebuffer available, skipping");
        return Ok(());
    };
    let drm = Arc::new(drm);
    SIMPLE_DRM.call_once(|| drm.clone());
    log::info!("aster-drm: initialized");
    Ok(())
}

pub fn get_simple_drm() -> Option<Arc<SimpleDrm>> {
    SIMPLE_DRM.get().cloned()
}
