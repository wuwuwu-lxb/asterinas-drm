// SPDX-License-Identifier: MPL-2.0

use crate::{
    DrmConnector, DrmCrtc, DrmEncoder, DrmError, DrmKmsObjectStore, DrmKmsObjectType, DrmProperty,
    atomic::DrmAtomicEffect,
    kms::object::{KmsObjectId, property::KmsObjectPropValue},
};

#[derive(Debug, Default)]
pub struct DrmPendingConnState {
    crtc_id: Option<Option<KmsObjectId>>,
    encoder_id: Option<Option<KmsObjectId>>,
    old_encoder_id: Option<KmsObjectId>,
    old_crtc_id: Option<KmsObjectId>,
}

impl DrmPendingConnState {
    pub(crate) fn new(crtc_id: Option<Option<KmsObjectId>>) -> Self {
        Self {
            crtc_id,
            encoder_id: None,
            old_encoder_id: None,
            old_crtc_id: None,
        }
    }
}

impl DrmConnector {
    fn get_possible_encoder_for_crtc(
        &self,
        objects: &DrmKmsObjectStore,
        crtc_id: KmsObjectId,
    ) -> Result<KmsObjectId, DrmError> {
        for encoder_id in
            objects.collect_object_ids(DrmKmsObjectType::Encoder, Some(self.possible_encoders()))
        {
            let encoder = objects
                .get_object::<DrmEncoder>(encoder_id)
                .ok_or(DrmError::NotFound)?;

            if objects
                .collect_object_ids(DrmKmsObjectType::Crtc, Some(encoder.possible_crtcs()))
                .contains(&crtc_id)
            {
                return Ok(encoder_id);
            }
        }

        Err(DrmError::NotFound)
    }

    pub fn decode_property(
        &self,
        _objects: &DrmKmsObjectStore,
        property: &DrmProperty,
        prop_value: KmsObjectPropValue,
        pending_state: &mut DrmPendingConnState,
    ) -> Result<(), DrmError> {
        match property.name() {
            "CRTC_ID" => {
                let new_crtc_id =
                    KmsObjectId::try_from(prop_value).map_err(|_| DrmError::Invalid)?;
                let new_crtc_id = (new_crtc_id != 0).then_some(new_crtc_id);

                pending_state.crtc_id = Some(new_crtc_id);
            }
            _ => return Err(DrmError::NotFound),
        }
        Ok(())
    }

    pub fn check_pending_state(
        &self,
        objects: &DrmKmsObjectStore,
        pending_state: &mut DrmPendingConnState,
    ) -> Result<DrmAtomicEffect, DrmError> {
        let mut effect = DrmAtomicEffect::default();

        let Some(final_crtc_id) = pending_state.crtc_id else {
            return Ok(effect);
        };

        let snapshot = self.snapshot();
        let old_encoder_id = snapshot.encoder_id();
        let old_crtc_id = match old_encoder_id {
            Some(encoder_id) => {
                let encoder = objects
                    .get_object::<DrmEncoder>(encoder_id)
                    .ok_or(DrmError::NotFound)?;
                encoder.crtc_id()
            }
            None => None,
        };
        pending_state.old_crtc_id = old_crtc_id;

        let final_encoder_id = match final_crtc_id {
            Some(crtc_id) => {
                objects
                    .get_object::<DrmCrtc>(crtc_id)
                    .ok_or(DrmError::NotFound)?;

                let encoder_id = match old_encoder_id {
                    Some(encoder_id) => {
                        let encoder = objects
                            .get_object::<DrmEncoder>(encoder_id)
                            .ok_or(DrmError::NotFound)?;

                        if objects
                            .collect_object_ids(
                                DrmKmsObjectType::Crtc,
                                Some(encoder.possible_crtcs()),
                            )
                            .contains(&crtc_id)
                        {
                            encoder_id
                        } else {
                            self.get_possible_encoder_for_crtc(objects, crtc_id)?
                        }
                    }
                    None => self.get_possible_encoder_for_crtc(objects, crtc_id)?,
                };

                Some(encoder_id)
            }
            None => None,
        };
        pending_state.encoder_id = Some(final_encoder_id);

        let crtc_changed = old_crtc_id != final_crtc_id;
        let encoder_changed = old_encoder_id != final_encoder_id;
        let connector_changed = crtc_changed || encoder_changed;

        pending_state.old_encoder_id = encoder_changed.then_some(old_encoder_id).flatten();

        if connector_changed {
            if let Some(old_crtc_id) = old_crtc_id {
                effect.add_affected_crtc(old_crtc_id);
            }
            if let Some(final_crtc_id) = final_crtc_id {
                effect.add_affected_crtc(final_crtc_id);
                effect.add_event_crtc(final_crtc_id);
            }
            effect.set_request_modeset();
        }

        Ok(effect)
    }

    pub fn commit_pending_state(
        &self,
        objects: &DrmKmsObjectStore,
        pending_state: &DrmPendingConnState,
    ) -> Result<(), DrmError> {
        let mut state = self.state().lock();
        if let Some(crtc_id) = pending_state.crtc_id {
            if let Some(old_encoder_id) = pending_state.old_encoder_id {
                let old_encoder = objects
                    .get_object::<DrmEncoder>(old_encoder_id)
                    .ok_or(DrmError::NotFound)?;
                old_encoder.set_crtc_id(None);
            }

            let Some(crtc_id) = crtc_id else {
                state.set_current_encoder_id(None);
                return Ok(());
            };

            let encoder_id = pending_state
                .encoder_id
                .flatten()
                .ok_or(DrmError::Invalid)?;
            let encoder = objects
                .get_object::<DrmEncoder>(encoder_id)
                .ok_or(DrmError::NotFound)?;

            state.set_current_encoder_id(Some(encoder_id));
            encoder.set_crtc_id(Some(crtc_id));
        }

        Ok(())
    }
}
