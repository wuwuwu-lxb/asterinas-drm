// SPDX-License-Identifier: MPL-2.0

use alloc::sync::Arc;
use core::fmt::Debug;

use crate::{
    DrmDisplayFormat, DrmError, DrmGemObject, DrmKmsObject, kms::object::DrmKmsObjectCast,
};

pub const DRM_FORMAT_MAX_PLANES: usize = 4;

#[derive(Debug)]
pub struct DrmFramebuffer {
    width: u32,
    height: u32,
    pixel_format: DrmDisplayFormat,
    flags: u32,
    pitches: [u32; DRM_FORMAT_MAX_PLANES],
    offsets: [u32; DRM_FORMAT_MAX_PLANES],
    modifiers: [u64; DRM_FORMAT_MAX_PLANES],
    gem_objects: [Option<Arc<dyn DrmGemObject>>; DRM_FORMAT_MAX_PLANES],
}

impl DrmFramebuffer {
    pub fn new(
        width: u32,
        height: u32,
        pixel_format: DrmDisplayFormat,
        flags: u32,
        pitches: [u32; DRM_FORMAT_MAX_PLANES],
        offsets: [u32; DRM_FORMAT_MAX_PLANES],
        modifiers: [u64; DRM_FORMAT_MAX_PLANES],
        gems: [Option<Arc<dyn DrmGemObject>>; DRM_FORMAT_MAX_PLANES],
    ) -> Result<Self, DrmError> {
        if width == 0 || height == 0 || matches!(pixel_format, DrmDisplayFormat::Unknown) {
            return Err(DrmError::Invalid);
        }

        if pitches[0] == 0 || gems[0].is_none() {
            return Err(DrmError::Invalid);
        }

        Ok(Self {
            width,
            height,
            pixel_format,
            flags,
            pitches,
            offsets,
            modifiers,
            gem_objects: gems,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixel_format(&self) -> DrmDisplayFormat {
        self.pixel_format
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn pitch(&self, plane_index: usize) -> Option<u32> {
        self.gem_objects.get(plane_index)?.as_ref()?;
        Some(self.pitches[plane_index])
    }

    pub fn offset(&self, plane_index: usize) -> Option<u32> {
        self.gem_objects.get(plane_index)?.as_ref()?;
        Some(self.offsets[plane_index])
    }

    pub fn modifier(&self, plane_index: usize) -> Option<u64> {
        self.gem_objects.get(plane_index)?.as_ref()?;
        Some(self.modifiers[plane_index])
    }

    pub fn gem_object(&self, plane_index: usize) -> Option<&Arc<dyn DrmGemObject>> {
        self.gem_objects.get(plane_index)?.as_ref()
    }
}

impl DrmKmsObjectCast for DrmFramebuffer {
    fn cast(obj: &DrmKmsObject) -> Option<&Self> {
        if let DrmKmsObject::Framebuffer(fb) = obj {
            Some(fb)
        } else {
            None
        }
    }
}
