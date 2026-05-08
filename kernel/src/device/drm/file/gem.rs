// SPDX-License-Identifier: MPL-2.0

use core::sync::atomic::Ordering;

use aster_drm::{DrmError, DrmGemMapPage, DrmGemObject, DrmIoctlGemCtx};

use crate::{
    device::drm::{file::DrmFile, gem::DrmGemShmemObject, ioctl::*},
    fs::file::{Mappable, MappedObject},
    prelude::*,
    vm::{perms::VmPerms, vmar::MappingHandle},
};

impl DrmIoctlGemCtx for DrmFile {
    fn create_shmem_gem(
        &self,
        size: usize,
        pitch: u32,
    ) -> core::result::Result<Arc<dyn DrmGemObject>, DrmError> {
        let obj = DrmGemShmemObject::new(size, pitch)?;
        Ok(Arc::new(obj))
    }
}

impl Mappable for DrmFile {
    fn map(&self, offset: usize, handle: MappingHandle) -> Result<Box<dyn MappedObject>> {
        if !offset.is_multiple_of(PAGE_SIZE) {
            return_errno!(Errno::EINVAL);
        }

        let map_size = handle.vm_mapping().map_size();
        let map_pages = map_size.div_ceil(PAGE_SIZE);
        let start_page = offset / PAGE_SIZE;

        let Some(gem_object) = self
            .device()
            .vma_manager()
            .lookup(start_page as u64, map_pages as u64)
        else {
            return_errno!(Errno::EFAULT);
        };

        let node = gem_object.vma_node();
        if !node.is_allowed(self.file_id) {
            return_errno!(Errno::EACCES);
        }

        let Some(object_offset) = offset.checked_sub(node.offset_addr() as usize) else {
            return_errno!(Errno::EFAULT);
        };

        Ok(Box::new(DrmGemMappedObject::new(gem_object, object_offset)))
    }
}

#[derive(Debug)]
struct DrmGemMappedObject {
    gem_object: Arc<dyn DrmGemObject>,
    object_offset: usize,
}

impl DrmGemMappedObject {
    fn new(gem_object: Arc<dyn DrmGemObject>, object_offset: usize) -> Self {
        Self {
            gem_object,
            object_offset,
        }
    }
}

impl MappedObject for DrmGemMappedObject {
    fn dup(&self) -> Box<dyn MappedObject> {
        Box::new(Self::new(self.gem_object.clone(), self.object_offset))
    }

    fn split_at(self: Box<Self>, offset: usize) -> (Box<dyn MappedObject>, Box<dyn MappedObject>) {
        let left = Box::new(Self::new(self.gem_object.clone(), self.object_offset));
        let right = Box::new(Self::new(self.gem_object, self.object_offset + offset));
        (left, right)
    }

    fn handle_page_fault(
        &self,
        offset: usize,
        _required_perms: VmPerms,
        mut handle: MappingHandle,
    ) -> Result<()> {
        let object_offset = self
            .object_offset
            .checked_add(offset)
            .ok_or_else(|| Error::new(Errno::EOVERFLOW))?;
        let page_offset = object_offset / PAGE_SIZE * PAGE_SIZE;

        match self.gem_object.map_page(page_offset)? {
            DrmGemMapPage::Frame(frame) => handle.map_frame(offset, frame),
            DrmGemMapPage::IoMem(io_mem) => handle.map_iomem(offset, io_mem),
        }

        Ok(())
    }
}

impl DrmFile {
    fn next_gem_handle(&self) -> u32 {
        self.next_gem_handle.fetch_add(1, Ordering::SeqCst)
    }

    fn add_gem_object(&self, gem_object: Arc<dyn DrmGemObject>) -> Result<u32> {
        gem_object.vma_node().allow(self.file_id)?;
        let handle = self.next_gem_handle();
        self.gem_table.lock().insert(handle, gem_object);
        Ok(handle)
    }

    fn map_gem_handle(&self, handle: u32) -> Result<u64> {
        let gem_object = self
            .gem_table
            .lock()
            .get(&handle)
            .cloned()
            .ok_or(Errno::ENONET)?;

        let dev = self.device();
        dev.vma_manager().add(&gem_object)?;

        Ok(gem_object.vma_node().offset_addr())
    }

    fn remove_gem_object(&self, handle: u32) {
        let gem_object = self.gem_table.lock().remove(&handle);
        if let Some(gem_object) = gem_object {
            gem_object.vma_node().revoke(self.file_id);
        }
    }

    pub(super) fn ioctl_mode_create_dumb(&self, cmd: DrmIoctlModeCreateDumb) -> Result<i32> {
        if !self.device().caps().has_dumb_buffer() {
            return_errno!(Errno::ENOSYS);
        }

        let mut args = cmd.read()?;

        if args.width == 0 || args.height == 0 || args.bpp == 0 {
            return_errno!(Errno::EINVAL);
        }

        let dev = self.device();
        let gem_object = dev.create_dumb(args.width, args.height, args.bpp, self)?;
        args.pitch = gem_object.pitch();
        args.size = gem_object.size() as u64;

        args.handle = self.add_gem_object(gem_object)?;

        cmd.write(&args)?;

        Ok(0)
    }

    pub(super) fn ioctl_mode_map_dumb(&self, cmd: DrmIoctlModeMapDumb) -> Result<i32> {
        if !self.device().caps().has_dumb_buffer() {
            return_errno!(Errno::ENOSYS);
        }

        let mut args = cmd.read()?;
        args.offset = self.map_gem_handle(args.handle)?;
        cmd.write(&args)?;

        Ok(0)
    }

    pub(super) fn ioctl_mode_destroy_dumb(&self, cmd: DrmIoctlModeDestroyDumb) -> Result<i32> {
        if !self.device().caps().has_dumb_buffer() {
            return_errno!(Errno::ENOSYS);
        }

        let args = cmd.read()?;
        self.remove_gem_object(args.handle);

        Ok(0)
    }
}
