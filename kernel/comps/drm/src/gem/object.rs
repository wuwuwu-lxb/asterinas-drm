// SPDX-License-Identifier: MPL-2.0

use core::{any::Any, fmt::Debug};

use alloc::sync::Arc;
use ostd::{
    io::IoMem,
    mm::{UFrame, VmReader, VmWriter},
};

use crate::{DrmError, gem::vma_manager::DrmVmaOffsetNode};

#[derive(Debug)]
pub enum DrmGemMapPage {
    /// Maps one page of regular memory.
    Frame(UFrame),
    /// Maps one page of device I/O memory.
    IoMem(IoMem),
}

pub trait DrmGemObject: Debug + Any + Sync + Send {
    fn read(&self, offset: usize, writer: &mut VmWriter) -> Result<(), DrmError>;
    fn write(&self, offset: usize, reader: &mut VmReader) -> Result<(), DrmError>;
    fn size(&self) -> usize;
    fn pitch(&self) -> u32;
    fn vma_node(&self) -> &Arc<DrmVmaOffsetNode>;
    /// Returns the page backing the page-aligned object-relative byte offset.
    fn map_page(&self, offset: usize) -> Result<DrmGemMapPage, DrmError>;
}
