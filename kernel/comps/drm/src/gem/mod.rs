// SPDX-License-Identifier: MPL-2.0

use alloc::sync::Arc;
use core::fmt::Debug;

use crate::{DrmError, gem::object::DrmGemObject};

pub mod object;
pub mod vma_manager;

pub trait DrmIoctlGemCtx: Send + Sync {
    fn create_shmem_gem(&self, size: usize, pitch: u32) -> Result<Arc<dyn DrmGemObject>, DrmError>;
}

pub trait DrmGemOps: Debug + Send + Sync {
    fn create_dumb(
        &self,
        _width: u32,
        _height: u32,
        _bpp: u32,
        _ctx: &dyn DrmIoctlGemCtx,
    ) -> Result<Arc<dyn DrmGemObject>, DrmError> {
        Err(DrmError::NotSupported)
    }
}
