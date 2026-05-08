// SPDX-License-Identifier: MPL-2.0

use aster_drm::{DrmError, DrmGemMapPage, DrmGemObject, DrmVmaOffsetNode};
use ostd::mm::{PAGE_SIZE, VmReader, VmWriter};

use crate::{
    prelude::*,
    vm::vmo::{CommitFlags, Vmo, VmoFlags, VmoOptions},
};

#[derive(Debug)]
pub(super) struct DrmGemShmemObject {
    vmo: Arc<Vmo>,
    size: usize,
    pitch: u32,
    vma_node: Arc<DrmVmaOffsetNode>,
}

impl DrmGemShmemObject {
    pub fn new(size: usize, pitch: u32) -> core::result::Result<Self, DrmError> {
        let vmo = VmoOptions::new(size)
            .flags(VmoFlags::RESIZABLE)
            .alloc()
            .map_err(|_| DrmError::NoMemory)?;

        let vma_node = Arc::new(DrmVmaOffsetNode::new());

        Ok(Self {
            vmo,
            size,
            pitch,
            vma_node,
        })
    }
}

impl DrmGemObject for DrmGemShmemObject {
    fn read(&self, offset: usize, writer: &mut VmWriter) -> core::result::Result<(), DrmError> {
        self.vmo.read(offset, writer).map_err(|_| DrmError::Invalid)
    }

    fn write(&self, offset: usize, reader: &mut VmReader) -> core::result::Result<(), DrmError> {
        self.vmo
            .write(offset, reader)
            .map_err(|_| DrmError::Invalid)
    }

    fn size(&self) -> usize {
        self.size
    }

    fn pitch(&self) -> u32 {
        self.pitch
    }

    fn vma_node(&self) -> &Arc<DrmVmaOffsetNode> {
        &self.vma_node
    }

    fn map_page(&self, offset: usize) -> core::result::Result<DrmGemMapPage, DrmError> {
        if offset % PAGE_SIZE != 0 {
            return Err(DrmError::Invalid);
        }

        let mapped_size = self
            .size
            .div_ceil(PAGE_SIZE)
            .checked_mul(PAGE_SIZE)
            .ok_or(DrmError::NoMemory)?;
        if offset >= mapped_size {
            return Err(DrmError::Invalid);
        }

        let page_idx = offset / PAGE_SIZE;
        let frame =
            self.vmo
                .commit_on(page_idx, CommitFlags::empty())
                .map_err(|error| match error.error() {
                    Errno::EINVAL => DrmError::Invalid,
                    Errno::ENOMEM => DrmError::NoMemory,
                    Errno::EBUSY | Errno::EIO => DrmError::Busy,
                    _ => DrmError::NoMemory,
                })?;
        Ok(DrmGemMapPage::Frame(frame))
    }
}
