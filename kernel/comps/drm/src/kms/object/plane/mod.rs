// SPDX-License-Identifier: MPL-2.0

use alloc::vec::Vec;
use core::fmt::Debug;

use ostd::sync::Mutex;

use crate::kms::object::{
    DrmKmsObject, DrmKmsObjectCast, KmsObjectId, KmsObjectIndex, display::DrmDisplayFormat,
    geometry::RectU32, property::DrmKmsObjectProp,
};

pub mod property;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrmPlaneType {
    Overlay = 0,
    Primary = 1,
    Cursor = 2,
}

#[derive(Debug, Default)]
pub struct DrmPlaneState {
    src_rect_px: RectU32,
    crtc_rect_px: RectU32,

    fb_id: Option<KmsObjectId>,
    crtc_id: Option<KmsObjectId>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DrmPlaneSnapshot {
    src_rect_px: RectU32,
    crtc_rect_px: RectU32,
    fb_id: Option<KmsObjectId>,
    crtc_id: Option<KmsObjectId>,
}

impl DrmPlaneSnapshot {
    pub fn src_rect(&self) -> RectU32 {
        self.src_rect_px
    }

    pub fn crtc_rect(&self) -> RectU32 {
        self.crtc_rect_px
    }

    pub fn fb_id(&self) -> Option<KmsObjectId> {
        self.fb_id
    }

    pub fn crtc_id(&self) -> Option<KmsObjectId> {
        self.crtc_id
    }
}

#[derive(Debug)]
pub struct DrmPlane {
    type_: DrmPlaneType,
    state: Mutex<DrmPlaneState>,
    possible_crtcs: u32,
    format_types: Vec<DrmDisplayFormat>,
    properties: DrmKmsObjectProp,
}

impl DrmPlane {
    pub fn new(
        type_: DrmPlaneType,
        format_types: Vec<DrmDisplayFormat>,
        possible_crtcs: &[KmsObjectIndex],
        properties: DrmKmsObjectProp,
    ) -> Self {
        let mut possible_crtcs_mask = 0;
        for &index in possible_crtcs {
            possible_crtcs_mask |= 1 << index;
        }

        Self {
            type_,
            state: Mutex::new(DrmPlaneState::default()),
            possible_crtcs: possible_crtcs_mask,
            format_types,
            properties,
        }
    }

    pub fn type_(&self) -> DrmPlaneType {
        self.type_
    }

    pub fn state(&self) -> &Mutex<DrmPlaneState> {
        &self.state
    }

    pub fn snapshot(&self) -> DrmPlaneSnapshot {
        let state = self.state.lock();
        DrmPlaneSnapshot {
            src_rect_px: state.src_rect_px,
            crtc_rect_px: state.crtc_rect_px,
            fb_id: state.fb_id,
            crtc_id: state.crtc_id,
        }
    }

    pub fn properties(&self) -> &DrmKmsObjectProp {
        &self.properties
    }

    pub fn possible_crtcs(&self) -> u32 {
        self.possible_crtcs
    }

    pub fn format_types(&self) -> &[DrmDisplayFormat] {
        &self.format_types
    }

    pub fn set_crtc_id(&self, crtc_id: Option<KmsObjectId>) {
        self.state.lock().crtc_id = crtc_id;
    }

    pub fn set_fb_id(&self, fb_id: Option<KmsObjectId>) {
        self.state.lock().fb_id = fb_id;
    }
}

impl DrmKmsObjectCast for DrmPlane {
    fn cast(obj: &DrmKmsObject) -> Option<&Self> {
        if let DrmKmsObject::Plane(p) = obj {
            Some(p)
        } else {
            None
        }
    }
}
