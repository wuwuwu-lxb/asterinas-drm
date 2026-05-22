// SPDX-License-Identifier: MPL-2.0

use crate::{
    DrmCrtc, DrmDisplayMode, DrmError, DrmKmsObjectStore, DrmProperty, DrmPropertyBlob,
    atomic::DrmAtomicEffect,
    kms::object::{KmsObjectId, property::KmsObjectPropValue},
};

#[derive(Debug, Default)]
pub struct DrmPendingCrtcState {
    pub(crate) active: Option<bool>,
    pub(crate) display_mode: Option<Option<DrmDisplayMode>>,
}

impl DrmPendingCrtcState {
    pub(crate) fn new(active: Option<bool>, display_mode: Option<Option<DrmDisplayMode>>) -> Self {
        Self {
            active,
            display_mode,
        }
    }
}

impl DrmCrtc {
    pub fn decode_property(
        &self,
        objects: &DrmKmsObjectStore,
        property: &DrmProperty,
        prop_value: KmsObjectPropValue,
        pending_state: &mut DrmPendingCrtcState,
    ) -> Result<(), DrmError> {
        match property.name() {
            "ACTIVE" => {
                if prop_value > 1 {
                    return Err(DrmError::Invalid);
                }

                let active = prop_value != 0;
                pending_state.active = Some(active);
            }
            "MODE_ID" => {
                let blob_id = KmsObjectId::try_from(prop_value).map_err(|_| DrmError::Invalid)?;

                if blob_id == 0 {
                    pending_state.display_mode = Some(None);
                } else {
                    let blob = objects
                        .get_object::<DrmPropertyBlob>(blob_id)
                        .ok_or(DrmError::NotFound)?;

                    let display_mode = DrmDisplayMode::from_blob(blob)?;
                    pending_state.display_mode = Some(Some(display_mode));
                }
            }
            _ => return Err(DrmError::NotFound),
        }
        Ok(())
    }

    pub fn check_pending_state(
        &self,
        crtc_id: KmsObjectId,
        _objects: &DrmKmsObjectStore,
        pending_state: &DrmPendingCrtcState,
    ) -> Result<DrmAtomicEffect, DrmError> {
        let mut effect = DrmAtomicEffect::default();
        let snapshot = self.snapshot();

        let old_active = snapshot.active();
        let old_display_mode = snapshot.display_mode();

        let final_active = pending_state.active.unwrap_or(old_active);
        let final_display_mode = pending_state.display_mode.unwrap_or(old_display_mode);

        let active_changed = pending_state.active.is_some() && old_active != final_active;
        let display_mode_changed =
            pending_state.display_mode.is_some() && old_display_mode != final_display_mode;

        if active_changed || display_mode_changed {
            effect.add_affected_crtc(crtc_id);
            effect.set_request_modeset();
        }
        if final_active && (active_changed || display_mode_changed) {
            effect.add_event_crtc(crtc_id);
        }

        if final_active && final_display_mode.is_none() {
            return Err(DrmError::Invalid);
        }
        Ok(effect)
    }

    pub fn commit_pending_state(
        &self,
        pending_state: &DrmPendingCrtcState,
    ) -> Result<(), DrmError> {
        let mut state = self.state().lock();

        if let Some(active) = pending_state.active {
            state.set_active(active);
            state.set_enable(active);
        }
        if let Some(display_mode) = pending_state.display_mode {
            state.set_display_mode(display_mode);
        }

        Ok(())
    }
}
