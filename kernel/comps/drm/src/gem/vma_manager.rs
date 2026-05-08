// SPDX-License-Identifier: MPL-2.0

use alloc::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use core::ops::Bound;

use ostd::{mm::PAGE_SIZE, sync::Mutex};

use crate::{DrmError, DrmGemObject};

#[cfg(target_pointer_width = "64")]
const DRM_FILE_PAGE_OFFSET_START: u64 = ((u32::MAX as u64) / PAGE_SIZE as u64) + 1;
#[cfg(target_pointer_width = "64")]
const DRM_FILE_PAGE_OFFSET_SIZE: u64 = ((u32::MAX as u64) / PAGE_SIZE as u64) * 256;

#[cfg(target_pointer_width = "32")]
const DRM_FILE_PAGE_OFFSET_START: u64 = ((0x0FFF_FFFFu64) / PAGE_SIZE as u64) + 1;
#[cfg(target_pointer_width = "32")]
const DRM_FILE_PAGE_OFFSET_SIZE: u64 = ((0x0FFF_FFFFu64) / PAGE_SIZE as u64) * 16;

/// Manages the fake mmap offset address space of one DRM device.
///
/// The manager follows Linux DRM's `drm_vma_offset_manager` model:
/// allocations are tracked in page units, offsets returned to userspace are
/// computed as `start_page << PAGE_SHIFT`, and access control is maintained on
/// each node instead of on the manager itself.
#[derive(Debug)]
pub struct DrmVmaOffsetManager {
    base_page: u64,
    size_pages: u64,
    inner: Mutex<DrmVmaOffsetManagerInner>,
}

#[derive(Debug)]
struct DrmVmaOffsetManagerInner {
    free_ranges: BTreeMap<u64, u64>,
    objects_by_start: BTreeMap<u64, Weak<dyn DrmGemObject>>,
}

/// Represents the mmap offset metadata attached to one GEM object.
///
/// The node does not own the object itself. Instead, the owning object keeps
/// the node alive and must remove it from the manager before the object is
/// destroyed.
#[derive(Debug)]
pub struct DrmVmaOffsetNode {
    allocation: Mutex<Option<DrmVmaAllocation>>,
    allowed_files: Mutex<BTreeMap<u32, usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DrmVmaAllocation {
    start_page: u64,
    size_pages: u64,
}

impl DrmVmaOffsetManager {
    /// Creates a manager with Linux-compatible default bounds.
    pub fn new() -> Self {
        Self::with_range(DRM_FILE_PAGE_OFFSET_START, DRM_FILE_PAGE_OFFSET_SIZE)
    }

    /// Creates a manager for a custom page-based offset range.
    pub fn with_range(base_page: u64, size_pages: u64) -> Self {
        let mut free_ranges = BTreeMap::new();
        if size_pages != 0 {
            free_ranges.insert(base_page, size_pages);
        }

        Self {
            base_page,
            size_pages,
            inner: Mutex::new(DrmVmaOffsetManagerInner {
                free_ranges,
                objects_by_start: BTreeMap::new(),
            }),
        }
    }

    /// Returns the first page of the managed fake address space.
    pub fn base_page(&self) -> u64 {
        self.base_page
    }

    /// Returns the size of the managed fake address space in pages.
    pub fn size_pages(&self) -> u64 {
        self.size_pages
    }

    /// Adds an object node to the manager.
    ///
    /// Like Linux `drm_vma_offset_add()`, this is idempotent: if the node was
    /// already added, the existing allocation is kept and `Ok(())` is returned.
    pub fn add(&self, gem_object: &Arc<dyn DrmGemObject>) -> Result<(), DrmError> {
        let pages = pages_from_size(gem_object.size())?;
        if pages == 0 {
            return Err(DrmError::Invalid);
        }

        let mut inner = self.inner.lock();
        let node = gem_object.vma_node();
        if node.is_allocated() {
            return Ok(());
        }
        let allocation = inner.allocate_best_fit(pages)?;

        node.set_allocation(allocation);
        inner
            .objects_by_start
            .insert(allocation.start_page, Arc::downgrade(gem_object));
        Ok(())
    }

    /// Removes a node from the manager.
    ///
    /// Like Linux `drm_vma_offset_remove()`, this clears the allocated offset
    /// range but preserves the node's allowed-file list.
    pub fn remove(&self, gem_object: &dyn DrmGemObject) {
        let mut inner = self.inner.lock();
        let node = gem_object.vma_node();
        let Some(allocation) = node.take_allocation() else {
            return;
        };

        inner.objects_by_start.remove(&allocation.start_page);
        inner.free_range(allocation.start_page, allocation.size_pages);
    }

    /// Looks up the node that fully covers `[start_page, start_page + pages)`.
    ///
    /// This mirrors Linux `drm_vma_offset_lookup_locked()`, which allows
    /// lookups into the middle of an existing node as long as the requested
    /// page range stays within the node.
    pub fn lookup(&self, start_page: u64, pages: u64) -> Option<Arc<dyn DrmGemObject>> {
        if pages == 0 {
            return None;
        }
        let end_page = start_page.checked_add(pages)?;

        let mut inner = self.inner.lock();
        let (node_start, gem_object) = inner
            .objects_by_start
            .range(..=start_page)
            .next_back()
            .map(|(start, gem_object)| (*start, gem_object.clone()))?;
        let gem_object = match gem_object.upgrade() {
            Some(gem_object) => gem_object,
            None => {
                inner.objects_by_start.remove(&node_start);
                return None;
            }
        };
        let node = gem_object.vma_node();
        let allocation = match node.allocation() {
            Some(allocation) => allocation,
            None => {
                inner.objects_by_start.remove(&node_start);
                return None;
            }
        };

        if allocation.start_page != node_start {
            inner.objects_by_start.remove(&node_start);
            return None;
        }

        let node_end = allocation.start_page.checked_add(allocation.size_pages)?;
        if allocation.start_page <= start_page && end_page <= node_end {
            return Some(gem_object);
        }

        None
    }

    /// Looks up the node whose start page matches exactly.
    pub fn exact_lookup(
        &self,
        start_page: u64,
        pages: u64,
    ) -> Option<Arc<dyn DrmGemObject>> {
        let gem_object = self.lookup(start_page, pages)?;
        let node = gem_object.vma_node();
        (node.start_page() == start_page).then_some(gem_object)
    }
}

impl DrmVmaOffsetNode {
    /// Creates an unallocated node.
    pub fn new() -> Self {
        Self {
            allocation: Mutex::new(None),
            allowed_files: Mutex::new(BTreeMap::new()),
        }
    }

    /// Returns the start page of the allocation, or 0 if not allocated.
    pub fn start_page(&self) -> u64 {
        self.allocation()
            .map_or(0, |allocation| allocation.start_page)
    }

    /// Returns the allocation size in pages, or 0 if not allocated.
    pub fn size_pages(&self) -> u64 {
        self.allocation()
            .map_or(0, |allocation| allocation.size_pages)
    }

    /// Returns the userspace mmap offset in bytes, or 0 if not allocated.
    pub fn offset_addr(&self) -> u64 {
        self.start_page().checked_mul(PAGE_SIZE as u64).unwrap_or(0)
    }

    /// Adds an open file to the allowed-user list.
    ///
    /// This is ref-counted, matching Linux `drm_vma_node_allow()`.
    pub fn allow(&self, open_id: u32) -> Result<(), DrmError> {
        let mut allowed_files = self.allowed_files.lock();
        let count = allowed_files.entry(open_id).or_insert(0);
        *count = count
            .checked_add(1)
            .ok_or_else(|| DrmError::NoMemory)?;
        Ok(())
    }

    /// Adds an open file to the allowed-user list without incrementing an
    /// existing reference count.
    ///
    /// This matches Linux `drm_vma_node_allow_once()`.
    pub fn allow_once(&self, open_id: u32) -> Result<(), DrmError> {
        let mut allowed_files = self.allowed_files.lock();
        allowed_files.entry(open_id).or_insert(1);
        Ok(())
    }

    /// Removes one reference from the allowed-user list.
    pub fn revoke(&self, open_id: u32) {
        let mut allowed_files = self.allowed_files.lock();
        let Some(count) = allowed_files.get_mut(&open_id) else {
            return;
        };

        *count -= 1;
        if *count == 0 {
            allowed_files.remove(&open_id);
        }
    }

    /// Returns whether the given open file is currently allowed to map this
    /// node.
    pub fn is_allowed(&self, open_id: u32) -> bool {
        self.allowed_files.lock().contains_key(&open_id)
    }

    fn is_allocated(&self) -> bool {
        self.allocation.lock().is_some()
    }

    fn allocation(&self) -> Option<DrmVmaAllocation> {
        *self.allocation.lock()
    }

    fn set_allocation(&self, allocation: DrmVmaAllocation) {
        let mut slot = self.allocation.lock();
        debug_assert!(slot.is_none());
        *slot = Some(allocation);
    }

    fn take_allocation(&self) -> Option<DrmVmaAllocation> {
        self.allocation.lock().take()
    }
}

impl DrmVmaOffsetManagerInner {
    fn allocate_best_fit(&mut self, pages: u64) -> Result<DrmVmaAllocation, DrmError> {
        let Some((best_start, best_len)) = self.find_best_fit(pages) else {
            return Err(DrmError::NoMemory);
        };

        self.free_ranges.remove(&best_start);
        if best_len > pages {
            let remaining_start = best_start
                .checked_add(pages)
                .ok_or_else(|| DrmError::NoMemory)?;
            let remaining_len = best_len - pages;
            self.free_ranges.insert(remaining_start, remaining_len);
        }

        Ok(DrmVmaAllocation {
            start_page: best_start,
            size_pages: pages,
        })
    }

    fn find_best_fit(&self, pages: u64) -> Option<(u64, u64)> {
        self.free_ranges
            .iter()
            .filter(|(_, len)| **len >= pages)
            .min_by_key(|(_, len)| **len)
            .map(|(start, len)| (*start, *len))
    }

    fn free_range(&mut self, start_page: u64, size_pages: u64) {
        if size_pages == 0 {
            return;
        }

        let mut free_start = start_page;
        let Some(mut free_end) = start_page.checked_add(size_pages) else {
            return;
        };

        if let Some((prev_start, prev_len)) = self
            .free_ranges
            .range(..free_start)
            .next_back()
            .map(|(start, len)| (*start, *len))
        {
            let Some(prev_end) = prev_start.checked_add(prev_len) else {
                return;
            };
            if prev_end == free_start {
                self.free_ranges.remove(&prev_start);
                free_start = prev_start;
            }
        }

        if let Some((next_start, next_len)) = self
            .free_ranges
            .range((Bound::Excluded(free_start), Bound::Unbounded))
            .next()
            .map(|(start, len)| (*start, *len))
            && free_end == next_start
        {
            self.free_ranges.remove(&next_start);
            let Some(merged_end) = free_end.checked_add(next_len) else {
                return;
            };
            free_end = merged_end;
        }

        self.free_ranges.insert(free_start, free_end - free_start);
    }
}

fn pages_from_size(size: usize) -> Result<u64, DrmError> {
    if size == 0 {
        return Err(DrmError::Invalid);
    }

    let aligned_size = size
        .checked_add(PAGE_SIZE - 1)
        .ok_or(DrmError::NoMemory)?
        / PAGE_SIZE;
    u64::try_from(aligned_size).map_err(|_| DrmError::NoMemory)
}
