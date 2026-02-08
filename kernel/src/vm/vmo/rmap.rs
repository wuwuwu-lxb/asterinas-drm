// SPDX-License-Identifier: MPL-2.0

use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
use core::ops::Range;

use keyable_arc::KeyableWeak;
use ostd::{
    mm::{PAGE_SIZE, Vaddr, VmSpace},
    task::disable_preempt,
};

/// Reverse mappings from a [`Vmo`] to [`VmSpace`]s.
///
/// [`Vmo`]: super::Vmo
#[derive(Debug)]
pub struct Rmap {
    entries: BTreeMap<KeyableWeak<VmSpace>, Vec<RmapEntry>>,
}

/// A reverse mapping entry.
#[derive(Copy, Clone, Debug)]
pub struct RmapEntry {
    /// The virtual address.
    pub vaddr: Vaddr,
    /// The VMO offset.
    pub offset: usize,
    /// The mapping size.
    pub size: usize,
}

impl Rmap {
    pub(super) const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Inserts a new reverse mapping entry.
    pub fn insert(&mut self, vm_space: &Arc<VmSpace>, entry: RmapEntry) {
        self.entries
            .entry(KeyableWeak::from(Arc::downgrade(vm_space)))
            .or_default()
            .push(entry)
    }

    /// Removes a reverse mapping entry.
    ///
    /// # Panics
    ///
    /// This method will panic if the reverse mapping entry does not exist.
    pub fn remove(&mut self, vm_space: &Arc<VmSpace>, vaddr: Vaddr) {
        use alloc::collections::btree_map::Entry;

        let key = KeyableWeak::from(Arc::downgrade(vm_space));
        let Entry::Occupied(mut map_entry) = self.entries.entry(key) else {
            panic!("the entry to remove does not exist")
        };

        let entries = map_entry.get_mut();
        let index = entries
            .iter()
            .position(|entry| entry.vaddr == vaddr)
            .expect("the entry to remove does not exist");
        entries.swap_remove(index);
    }

    /// Iterates over all reverse mappings and unmaps the given offset range.
    ///
    /// # Panics
    ///
    /// This method may panic if the offset range is not aligned to the page boundary.
    pub fn unmap(&mut self, offset: Range<usize>) {
        debug_assert!(offset.start.is_multiple_of(PAGE_SIZE));
        debug_assert!(offset.end.is_multiple_of(PAGE_SIZE));

        self.entries.retain(|vm_space, entries| {
            let Some(vm_space) = vm_space.upgrade() else {
                return false;
            };

            for entry in entries {
                let vmo_range =
                    entry.offset.max(offset.start)..(entry.offset + entry.size).min(offset.end);
                if vmo_range.is_empty() {
                    continue;
                }

                let addr_range = (vmo_range.start - entry.offset + entry.vaddr)
                    ..(vmo_range.end - entry.offset + entry.vaddr);

                let preempt_guard = disable_preempt();
                let mut cursor_mut = vm_space.cursor_mut(&preempt_guard, &addr_range).unwrap();
                cursor_mut.unmap(addr_range.len());
                cursor_mut.flusher().dispatch_tlb_flush();
                cursor_mut.flusher().sync_tlb_flush();
            }

            true
        });
    }
}
