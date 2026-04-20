// SPDX-License-Identifier: MPL-2.0

use core::fmt::Debug;

use ostd::sync::Mutex;

use crate::kms::object::{DrmKmsObject, DrmKmsObjectCast, KmsObjectId, KmsObjectIndex};

#[derive(Debug, Default)]
pub struct DrmEncoderState {
    crtc_id: Option<KmsObjectId>,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum DrmEncoderType {
    NONE = 0,
    DAC = 1,
    TMDS = 2,
    LVDS = 3,
    TVDAC = 4,
    VIRTUAL = 5,
    DSI = 6,
    DPMST = 7,
    DPI = 8,
}

#[derive(Debug)]
pub struct DrmEncoder {
    type_: DrmEncoderType,
    state: Mutex<DrmEncoderState>,
    possible_crtcs: u32,
    // TODO: Add `possible_clones` when the builder supports encoder clone topology.
}

impl DrmEncoder {
    pub fn new(type_: DrmEncoderType, possible_crtcs: &[KmsObjectIndex]) -> Self {
        let mut possible_crtcs_mask = 0;
        for &index in possible_crtcs {
            possible_crtcs_mask |= 1 << index;
        }

        Self {
            type_,
            state: Mutex::new(DrmEncoderState::default()),
            possible_crtcs: possible_crtcs_mask,
        }
    }

    pub fn type_(&self) -> DrmEncoderType {
        self.type_
    }

    pub fn state(&self) -> &Mutex<DrmEncoderState> {
        &self.state
    }

    pub fn possible_crtcs(&self) -> u32 {
        self.possible_crtcs
    }

    pub fn crtc_id(&self) -> Option<KmsObjectId> {
        self.state().lock().crtc_id
    }

    pub fn set_crtc_id(&self, crtc_id: Option<KmsObjectId>) {
        self.state().lock().crtc_id = crtc_id;
    }
}

impl DrmKmsObjectCast for DrmEncoder {
    fn cast(obj: &DrmKmsObject) -> Option<&Self> {
        if let DrmKmsObject::Encoder(encoder) = obj {
            Some(encoder)
        } else {
            None
        }
    }
}
