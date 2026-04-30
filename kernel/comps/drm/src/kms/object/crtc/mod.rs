// SPDX-License-Identifier: MPL-2.0

use core::fmt::Debug;

use ostd::sync::Mutex;

use crate::kms::object::{
    DrmKmsObject, DrmKmsObjectCast, KmsObjectId, display::DrmDisplayMode,
    property::DrmKmsObjectProp,
};

pub mod property;

#[derive(Debug, Default)]
pub struct DrmCrtcState {
    display_mode: Option<DrmDisplayMode>,
    enable: bool,
    active: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DrmCrtcSnapshot {
    display_mode: Option<DrmDisplayMode>,
    enable: bool,
    active: bool,
}

impl DrmCrtcSnapshot {
    pub fn display_mode(&self) -> Option<DrmDisplayMode> {
        self.display_mode
    }

    pub fn enable(&self) -> bool {
        self.enable
    }

    pub fn active(&self) -> bool {
        self.active
    }
}

#[derive(Debug)]
pub struct DrmCrtc {
    state: Mutex<DrmCrtcState>,
    gamma_size_px: u32,
    primary_plane_id: KmsObjectId,
    cursor_plane_id: Option<KmsObjectId>,
    properties: DrmKmsObjectProp,
}

impl DrmCrtc {
    pub fn new(
        gamma_size_px: u32,
        primary_plane_id: KmsObjectId,
        cursor_plane_id: Option<KmsObjectId>,
        properties: DrmKmsObjectProp,
    ) -> Self {
        Self {
            state: Mutex::new(DrmCrtcState::default()),
            gamma_size_px,
            primary_plane_id,
            cursor_plane_id,
            properties,
        }
    }

    pub fn state(&self) -> &Mutex<DrmCrtcState> {
        &self.state
    }

    pub fn snapshot(&self) -> DrmCrtcSnapshot {
        let state = self.state.lock();
        DrmCrtcSnapshot {
            display_mode: state.display_mode,
            enable: state.enable,
            active: state.active,
        }
    }

    pub fn properties(&self) -> &DrmKmsObjectProp {
        &self.properties
    }

    pub fn gamma_size_px(&self) -> u32 {
        self.gamma_size_px
    }

    pub fn primary_plane_id(&self) -> KmsObjectId {
        self.primary_plane_id
    }

    pub fn cursor_plane_id(&self) -> Option<KmsObjectId> {
        self.cursor_plane_id
    }

    pub fn set_display_mode(&self, display_mode: DrmDisplayMode) {
        self.state().lock().display_mode = Some(display_mode);
    }

    pub fn set_active(&self, active: bool) {
        self.state().lock().active = active;
    }
}

impl DrmKmsObjectCast for DrmCrtc {
    fn cast(obj: &DrmKmsObject) -> Option<&Self> {
        if let DrmKmsObject::Crtc(crtc) = obj {
            Some(crtc)
        } else {
            None
        }
    }
}
