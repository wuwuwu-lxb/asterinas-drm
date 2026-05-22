// SPDX-License-Identifier: MPL-2.0

use crate::{
    DrmCrtc, DrmDisplayMode, DrmError, DrmFramebuffer, DrmKmsObjectStore, DrmKmsObjectType,
    DrmPlane, DrmProperty,
    atomic::DrmAtomicEffect,
    kms::object::{KmsObjectId, geometry::RectU32, property::KmsObjectPropValue},
};

#[derive(Debug, Default)]
pub struct DrmPendingPlaneState {
    crtc_rect: Option<RectU32>,
    src_rect: Option<RectU32>,
    pub(crate) crtc_id: Option<Option<KmsObjectId>>,
    fb_id: Option<Option<KmsObjectId>>,
}

impl DrmPendingPlaneState {
    pub fn new(
        crtc_rect: Option<RectU32>,
        src_rect: Option<RectU32>,
        crtc_id: Option<Option<KmsObjectId>>,
        fb_id: Option<Option<KmsObjectId>>,
    ) -> Self {
        Self {
            crtc_rect,
            src_rect,
            crtc_id,
            fb_id,
        }
    }
}

impl DrmPlane {
    pub fn decode_property(
        &self,
        _objects: &DrmKmsObjectStore,
        property: &DrmProperty,
        prop_value: KmsObjectPropValue,
        pending_state: &mut DrmPendingPlaneState,
    ) -> Result<(), DrmError> {
        match property.name() {
            "CRTC_ID" => {
                let new_crtc_id =
                    KmsObjectId::try_from(prop_value).map_err(|_| DrmError::Invalid)?;
                let new_crtc_id = (new_crtc_id != 0).then_some(new_crtc_id);

                pending_state.crtc_id = Some(new_crtc_id);
            }
            "FB_ID" => {
                let new_fb_id = KmsObjectId::try_from(prop_value).map_err(|_| DrmError::Invalid)?;
                let new_fb_id = (new_fb_id != 0).then_some(new_fb_id);

                pending_state.fb_id = Some(new_fb_id);
            }
            "CRTC_W" | "CRTC_H" | "CRTC_X" | "CRTC_Y" => {
                let value = u32::try_from(prop_value).map_err(|_| DrmError::Invalid)?;
                let pending_plane_state = pending_state;
                let mut rect = pending_plane_state
                    .crtc_rect
                    .unwrap_or_else(|| self.snapshot().crtc_rect());

                match property.name() {
                    "CRTC_X" => rect.set_x(value),
                    "CRTC_Y" => rect.set_y(value),
                    "CRTC_W" => rect.set_width(value),
                    "CRTC_H" => rect.set_height(value),
                    _ => unreachable!(),
                }

                pending_plane_state.crtc_rect = Some(rect);
            }
            "SRC_W" | "SRC_H" | "SRC_X" | "SRC_Y" => {
                let value = u32::try_from(prop_value).map_err(|_| DrmError::Invalid)?;
                let pending_plane_state = pending_state;
                let mut rect = pending_plane_state
                    .src_rect
                    .unwrap_or_else(|| self.snapshot().src_rect());

                match property.name() {
                    "SRC_X" => rect.set_x(value),
                    "SRC_Y" => rect.set_y(value),
                    "SRC_W" => rect.set_width(value),
                    "SRC_H" => rect.set_height(value),
                    _ => unreachable!(),
                }

                pending_plane_state.src_rect = Some(rect);
            }
            _ => return Err(DrmError::NotFound),
        }
        Ok(())
    }

    pub fn check_pending_state(
        &self,
        objects: &DrmKmsObjectStore,
        pending_state: &DrmPendingPlaneState,
        display_mode: Option<DrmDisplayMode>,
    ) -> Result<DrmAtomicEffect, DrmError> {
        let mut effect = DrmAtomicEffect::default();
        let snapshot = self.snapshot();

        let old_crtc_id = snapshot.crtc_id();
        let old_fb_id = snapshot.fb_id();
        let old_src_rect = snapshot.src_rect();
        let old_crtc_rect = snapshot.crtc_rect();

        let final_crtc_id = pending_state.crtc_id.unwrap_or(old_crtc_id);
        let final_fb_id = pending_state.fb_id.unwrap_or(old_fb_id);
        let final_src_rect = pending_state.src_rect.unwrap_or(old_src_rect);
        let final_crtc_rect = pending_state.crtc_rect.unwrap_or(old_crtc_rect);

        let crtc_changed = pending_state.crtc_id.is_some() && old_crtc_id != final_crtc_id;
        let fb_changed = pending_state.fb_id.is_some() && old_fb_id != final_fb_id;
        let src_changed = pending_state.src_rect.is_some() && old_src_rect != final_src_rect;
        let crtc_rect_changed =
            pending_state.crtc_rect.is_some() && old_crtc_rect != final_crtc_rect;

        if crtc_changed {
            if let Some(old_crtc_id) = old_crtc_id {
                effect.add_affected_crtc(old_crtc_id);
            }
            if let Some(final_crtc_id) = final_crtc_id {
                effect.add_affected_crtc(final_crtc_id);
            }
            effect.set_request_modeset();
        } else if fb_changed || src_changed || crtc_rect_changed {
            if let Some(final_crtc_id) = final_crtc_id {
                effect.add_affected_crtc(final_crtc_id);
            }
        }

        if crtc_changed || fb_changed || src_changed || crtc_rect_changed {
            if let Some(final_crtc_id) = final_crtc_id {
                effect.add_event_crtc(final_crtc_id);
            }
        }

        match (final_crtc_id, final_fb_id) {
            (None, None) => return Ok(effect),
            (Some(_), None) | (None, Some(_)) => return Err(DrmError::Invalid),
            (Some(crtc_id), Some(fb_id)) => {
                objects
                    .get_object::<DrmCrtc>(crtc_id)
                    .ok_or(DrmError::NotFound)?;
                if !objects
                    .collect_object_ids(DrmKmsObjectType::Crtc, Some(self.possible_crtcs()))
                    .contains(&crtc_id)
                {
                    return Err(DrmError::Invalid);
                }
                let fb = objects
                    .get_object::<DrmFramebuffer>(fb_id)
                    .ok_or(DrmError::NotFound)?;
                if !self.format_types().contains(&fb.pixel_format()) {
                    return Err(DrmError::Invalid);
                }
                let display_mode = display_mode.ok_or(DrmError::Invalid)?;

                // Rectangles are checked by their right/bottom edges:
                //
                //   x                         x2 = x + width
                //   +-------------------------+
                //   |                         |
                //   |                         | height
                //   |                         |
                //   +-------------------------+
                //   y                         y2 = y + height
                //
                // For `src_rect`, coordinates are 16.16 framebuffer coordinates.
                // For `crtc_rect`, coordinates are CRTC pixel coordinates.
                let src_x2 = final_src_rect
                    .x()
                    .checked_add(final_src_rect.width())
                    .ok_or(DrmError::Invalid)?;
                let src_y2 = final_src_rect
                    .y()
                    .checked_add(final_src_rect.height())
                    .ok_or(DrmError::Invalid)?;
                let fb_width = fb.width().checked_shl(16).ok_or(DrmError::Invalid)?;
                let fb_height = fb.height().checked_shl(16).ok_or(DrmError::Invalid)?;

                let crtc_x2 = final_crtc_rect
                    .x()
                    .checked_add(final_crtc_rect.width())
                    .ok_or(DrmError::Invalid)?;
                let crtc_y2 = final_crtc_rect
                    .y()
                    .checked_add(final_crtc_rect.height())
                    .ok_or(DrmError::Invalid)?;

                let src_width = final_src_rect.width() >> 16;
                let src_height = final_src_rect.height() >> 16;

                if final_src_rect.width() == 0
                    || final_src_rect.height() == 0
                    || final_crtc_rect.width() == 0
                    || final_crtc_rect.height() == 0
                    || fb_width < src_x2
                    || fb_height < src_y2
                    || (display_mode.hdisplay() as u32) < crtc_x2
                    || (display_mode.vdisplay() as u32) < crtc_y2
                    || src_width != final_crtc_rect.width()
                    || src_height != final_crtc_rect.height()
                {
                    return Err(DrmError::Invalid);
                }
                return Ok(effect);
            }
        }
    }

    pub fn commit_pending_state(
        &self,
        pending_state: &DrmPendingPlaneState,
    ) -> Result<(), DrmError> {
        let mut state = self.state().lock();

        if let Some(src_rect) = pending_state.src_rect {
            state.set_src_rect(src_rect);
        }
        if let Some(crtc_rect) = pending_state.crtc_rect {
            state.set_crtc_rect(crtc_rect);
        }
        if let Some(crtc_id) = pending_state.crtc_id {
            state.set_crtc_id(crtc_id);
        }
        if let Some(fb_id) = pending_state.fb_id {
            state.set_fb_id(fb_id);
        }

        Ok(())
    }
}
