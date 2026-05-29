// SPDX-License-Identifier: MPL-2.0

use alloc::sync::Arc;

use aster_drm::{DrmDevice, DrmGemMapPage, DrmGemObject};
use ostd::mm::{HasSize, PAGE_SIZE};

use super::{backend::FbBackend, file::FbHandle};
use crate::{
    fs::file::{Mappable, MappedObject},
    prelude::*,
    vm::{perms::VmPerms, vmar::MappingHandle},
};

impl Mappable for FbHandle {
    fn map(&self, offset: usize, mut handle: MappingHandle) -> Result<Box<dyn MappedObject>> {
        match self.backend() {
            FbBackend::Drm(backend) => {
                if offset % PAGE_SIZE != 0 {
                    return_errno_with_message!(
                        Errno::EINVAL,
                        "the framebuffer mmap offset must be page-aligned"
                    );
                }

                Ok(Box::new(FbMapHandle::Drm(FbDrmMapHandle {
                    device: backend.device.clone(),
                    gem: backend.gem.clone(),
                    fb_id: backend.fb_id,
                    object_offset: offset,
                })))
            }
            FbBackend::Legacy(framebuffer) => {
                let io_mem = framebuffer.io_mem();
                let mapped_handle = Box::new(FbMapHandle::Legacy);

                let io_mem_sliced = if offset >= io_mem.size() {
                    return Ok(mapped_handle);
                } else if offset != 0 {
                    io_mem.slice(offset..io_mem.size())
                } else {
                    io_mem.clone()
                };

                handle.map_iomem(0, io_mem_sliced);

                Ok(mapped_handle)
            }
        }
    }
}

#[derive(Debug)]
enum FbMapHandle {
    Drm(FbDrmMapHandle),
    Legacy,
}

#[derive(Debug)]
struct FbDrmMapHandle {
    device: Arc<dyn DrmDevice>,
    gem: Arc<dyn DrmGemObject>,
    fb_id: u32,
    object_offset: usize,
}

impl MappedObject for FbMapHandle {
    fn dup(&self) -> Box<dyn MappedObject> {
        match self {
            Self::Drm(handle) => Box::new(Self::Drm(FbDrmMapHandle {
                device: handle.device.clone(),
                gem: handle.gem.clone(),
                fb_id: handle.fb_id,
                object_offset: handle.object_offset,
            })),
            Self::Legacy => Box::new(Self::Legacy),
        }
    }

    fn split_at(self: Box<Self>, offset: usize) -> (Box<dyn MappedObject>, Box<dyn MappedObject>) {
        match *self {
            Self::Drm(handle) => {
                let right_offset = handle.object_offset.saturating_add(offset);
                (
                    Box::new(Self::Drm(FbDrmMapHandle {
                        device: handle.device.clone(),
                        gem: handle.gem.clone(),
                        fb_id: handle.fb_id,
                        object_offset: handle.object_offset,
                    })),
                    Box::new(Self::Drm(FbDrmMapHandle {
                        device: handle.device,
                        gem: handle.gem,
                        fb_id: handle.fb_id,
                        object_offset: right_offset,
                    })),
                )
            }
            Self::Legacy => (Box::new(Self::Legacy), Box::new(Self::Legacy)),
        }
    }

    fn handle_page_fault(
        &self,
        offset: usize,
        _required_perms: VmPerms,
        mut handle: MappingHandle,
    ) -> Result<()> {
        let Self::Drm(drm_handle) = self else {
            return_errno_with_message!(
                Errno::EFAULT,
                "legacy framebuffer mappings must be populated eagerly"
            );
        };

        let object_offset = drm_handle
            .object_offset
            .checked_add(offset)
            .ok_or_else(|| Error::new(Errno::EOVERFLOW))?;
        if object_offset % PAGE_SIZE != 0 {
            return_errno_with_message!(
                Errno::EINVAL,
                "the framebuffer page fault offset must be page-aligned"
            );
        }

        match drm_handle.gem.map_page(object_offset)? {
            DrmGemMapPage::Frame(frame) => handle.map_frame(offset, frame),
            DrmGemMapPage::IoMem(io_mem) => handle.map_iomem(offset, io_mem),
        }

        // `fbdev` mmap writes are not trapped after the page is mapped, so this
        // best-effort flush makes newly faulted pages visible without adding a
        // separate display path outside DRM.
        let _ = drm_handle.device.dirty_fb(drm_handle.fb_id);
        Ok(())
    }
}
