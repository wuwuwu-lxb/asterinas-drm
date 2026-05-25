// SPDX-License-Identifier: MPL-2.0

use alloc::{sync::Arc, vec::Vec};
use core::fmt::Debug;

use hashbrown::HashMap;

use crate::{
    DrmConnector, DrmCrtc, DrmDisplayMode, DrmError, DrmFramebuffer, DrmIoctlEventCtx,
    DrmKmsObject, DrmKmsObjectProp, DrmKmsObjectStore, DrmKmsObjectType, DrmKmsOps,
    DrmPendingVblankEvent, DrmPlane, DrmProperty,
    kms::object::{
        KmsObjectId, connector::helper::DrmPendingConnState, crtc::helper::DrmPendingCrtcState,
        geometry::RectU32, plane::helper::DrmPendingPlaneState, property::KmsObjectPropValue,
    },
};

bitflags::bitflags! {
    pub struct DrmAtomicFlags: u32 {
        const PAGE_FLIP_EVENT    = 0x0001;
        const PAGE_FLIP_ASYNC    = 0x0002;

        const TEST_ONLY          = 0x0100;
        const NONBLOCK           = 0x0200;
        const ALLOW_MODESET      = 0x0400;
    }
}

#[derive(Debug)]
pub struct DrmAtomicObjectRequest {
    object_id: KmsObjectId,
    properties: DrmKmsObjectProp,
}

impl DrmAtomicObjectRequest {
    pub fn new(object_id: KmsObjectId) -> Self {
        Self {
            object_id,
            properties: DrmKmsObjectProp::default(),
        }
    }

    pub fn add_property(&mut self, prop_id: KmsObjectId, prop_value: KmsObjectPropValue) {
        self.properties.add_property(prop_id, prop_value);
    }
}

#[derive(Debug, Default)]
pub struct DrmAtomicState {
    plane_states: HashMap<KmsObjectId, DrmPendingPlaneState>,
    crtc_states: HashMap<KmsObjectId, DrmPendingCrtcState>,
    connector_states: HashMap<KmsObjectId, DrmPendingConnState>,

    effect: DrmAtomicEffect,
}

impl DrmAtomicState {
    fn new(
        objects: &DrmKmsObjectStore,
        requests: Vec<DrmAtomicObjectRequest>,
    ) -> Result<Self, DrmError> {
        let mut atomic_state = Self::default();

        for request in requests {
            let object = objects
                .get_unknow_type_object(request.object_id)
                .ok_or(DrmError::NotFound)?;

            for (prop_id, prop_value) in &request.properties.entries() {
                if !object.properties()?.ids().contains(prop_id) {
                    return Err(DrmError::NotFound);
                }
                let property = objects
                    .get_object::<DrmProperty>(*prop_id)
                    .ok_or(DrmError::NotFound)?;

                match object {
                    DrmKmsObject::Plane(plane) => {
                        let pending_state = atomic_state
                            .plane_states
                            .entry(request.object_id)
                            .or_default();
                        plane.decode_property(&objects, property, *prop_value, pending_state)?;
                    }
                    DrmKmsObject::Crtc(crtc) => {
                        let pending_state = atomic_state
                            .crtc_states
                            .entry(request.object_id)
                            .or_default();
                        crtc.decode_property(&objects, property, *prop_value, pending_state)?;
                    }
                    DrmKmsObject::Connector(connector) => {
                        let pending_state = atomic_state
                            .connector_states
                            .entry(request.object_id)
                            .or_default();
                        connector.decode_property(
                            &objects,
                            property,
                            *prop_value,
                            pending_state,
                        )?;
                    }
                    _ => return Err(DrmError::Invalid),
                }
            }
        }

        Ok(atomic_state)
    }
}

#[derive(Debug, Default)]
pub struct DrmAtomicEffect {
    request_modeset: bool,
    affected_crtcs: Vec<KmsObjectId>,
    event_crtcs: Vec<KmsObjectId>,
}

impl DrmAtomicEffect {
    pub fn set_request_modeset(&mut self) {
        self.request_modeset = true;
    }

    pub fn add_affected_crtc(&mut self, crtc_id: KmsObjectId) {
        if !self.affected_crtcs.contains(&crtc_id) {
            self.affected_crtcs.push(crtc_id);
        }
    }

    pub fn add_event_crtc(&mut self, crtc_id: KmsObjectId) {
        if !self.event_crtcs.contains(&crtc_id) {
            self.event_crtcs.push(crtc_id);
        }
    }

    pub fn merge(&mut self, effect: Self) {
        self.request_modeset |= effect.request_modeset;
        for crtc_id in effect.affected_crtcs {
            self.add_affected_crtc(crtc_id);
        }
        for crtc_id in effect.event_crtcs {
            self.add_event_crtc(crtc_id);
        }
    }
}

/// Provides atomic KMS state handling for a DRM device.
///
/// This trait serves two entry paths. The legacy helpers translate legacy KMS
/// operations, such as dirtyfb and page flip, into atomic state transitions so
/// that drivers can share one validation and commit path. The atomic ioctl path
/// consumes userspace property updates directly and applies them through the
/// same check, commit, event, and flush sequence.
pub trait DrmAtomicOps: DrmKmsOps + Debug + Send + Sync {
    fn atomic_set_crtc(
        &self,
        crtc_id: KmsObjectId,
        fb_id: KmsObjectId,
        x: u32,
        y: u32,
        display_mode: Option<DrmDisplayMode>,
        connector_ids: Vec<KmsObjectId>,
    ) -> Result<(), DrmError> {
        let objects = self.kms_objects().write();

        let flags = DrmAtomicFlags::ALLOW_MODESET;
        let mut atomic_state = DrmAtomicState::default();

        let crtc = objects
            .get_object::<DrmCrtc>(crtc_id)
            .ok_or(DrmError::NotFound)?;
        let plane_id = crtc.primary_plane_id();

        match display_mode {
            Some(display_mode) => {
                if connector_ids.is_empty() {
                    return Err(DrmError::Invalid);
                }
                for (index, connector_id) in connector_ids.iter().enumerate() {
                    if connector_ids[..index].contains(connector_id) {
                        return Err(DrmError::Invalid);
                    }
                }

                let mode_width = display_mode.hdisplay() as u32;
                let mode_height = display_mode.vdisplay() as u32;
                let fixed_point_scale = 1u32 << 16;
                let src_rect = RectU32::new(
                    x.checked_mul(fixed_point_scale).ok_or(DrmError::Invalid)?,
                    y.checked_mul(fixed_point_scale).ok_or(DrmError::Invalid)?,
                    mode_width
                        .checked_mul(fixed_point_scale)
                        .ok_or(DrmError::Invalid)?,
                    mode_height
                        .checked_mul(fixed_point_scale)
                        .ok_or(DrmError::Invalid)?,
                );
                let crtc_rect = RectU32::new(0, 0, mode_width, mode_height);

                atomic_state.crtc_states.insert(
                    crtc_id,
                    DrmPendingCrtcState::new(Some(true), Some(Some(display_mode))),
                );
                atomic_state.plane_states.insert(
                    plane_id,
                    DrmPendingPlaneState::new(
                        Some(crtc_rect),
                        Some(src_rect),
                        Some(Some(crtc_id)),
                        Some(Some(fb_id)),
                    ),
                );
                for connector_id in connector_ids {
                    atomic_state
                        .connector_states
                        .insert(connector_id, DrmPendingConnState::new(Some(Some(crtc_id))));
                }
            }
            None => {
                atomic_state
                    .crtc_states
                    .insert(crtc_id, DrmPendingCrtcState::new(Some(false), Some(None)));
                atomic_state.plane_states.insert(
                    plane_id,
                    DrmPendingPlaneState::new(
                        Some(RectU32::default()),
                        Some(RectU32::default()),
                        Some(None),
                        Some(None),
                    ),
                );
            }
        }

        self.atomic_check(&objects, &mut atomic_state, flags)?;
        self.atomic_commit(&objects, &atomic_state)?;
        drop(objects);

        self.atomic_commit_tail(atomic_state, flags)
    }

    fn atomic_page_flip(
        &self,
        crtc_id: KmsObjectId,
        fb_id: KmsObjectId,
        user_data: u64,
        event_ctx: Arc<dyn DrmIoctlEventCtx>,
    ) -> Result<(), DrmError> {
        let objects = self.kms_objects().write();

        let flags = DrmAtomicFlags::PAGE_FLIP_EVENT;
        let mut atomic_state = DrmAtomicState::default();

        let crtc = objects
            .get_object::<DrmCrtc>(crtc_id)
            .ok_or(DrmError::NotFound)?;
        let plane_id = crtc.primary_plane_id();
        let plane = objects
            .get_object::<DrmPlane>(plane_id)
            .ok_or(DrmError::NotFound)?;
        if plane.snapshot().crtc_id() != Some(crtc_id) {
            return Err(DrmError::Invalid);
        }

        atomic_state.plane_states.insert(
            plane_id,
            DrmPendingPlaneState::new(None, None, None, Some(Some(fb_id))),
        );

        self.atomic_check(&objects, &mut atomic_state, flags)?;
        self.atomic_commit(&objects, &atomic_state)?;
        self.atomic_queue_vblank_event(&objects, &atomic_state, user_data, event_ctx)?;
        drop(objects);

        self.atomic_commit_tail(atomic_state, flags)
    }

    fn atomic_dirty_fb(&self, fb_id: KmsObjectId) -> Result<(), DrmError> {
        let objects = self.kms_objects().write();

        let flags = DrmAtomicFlags::empty();
        let mut atomic_state = DrmAtomicState::default();

        objects
            .get_object::<DrmFramebuffer>(fb_id)
            .ok_or(DrmError::NotFound)?;

        for plane_id in objects.collect_object_ids(DrmKmsObjectType::Plane, None) {
            let plane = objects
                .get_object::<DrmPlane>(plane_id)
                .ok_or(DrmError::NotFound)?;
            let snapshot = plane.snapshot();

            if snapshot.fb_id() == Some(fb_id) {
                if let Some(crtc_id) = snapshot.crtc_id() {
                    atomic_state.effect.add_affected_crtc(crtc_id);
                }
            }
        }

        drop(objects);

        self.atomic_commit_tail(atomic_state, flags)
    }

    fn atomic_commit_request(
        &self,
        requests: Vec<DrmAtomicObjectRequest>,
        flags: DrmAtomicFlags,
        user_data: u64,
        event_ctx: Arc<dyn DrmIoctlEventCtx>,
    ) -> Result<(), DrmError> {
        if flags.contains(DrmAtomicFlags::TEST_ONLY) {
            let objects = self.kms_objects().read();
            let mut atomic_state = DrmAtomicState::new(&objects, requests)?;

            self.atomic_check(&objects, &mut atomic_state, flags)?;
        } else {
            if flags.contains(DrmAtomicFlags::NONBLOCK)
                || flags.contains(DrmAtomicFlags::PAGE_FLIP_ASYNC)
            {
                // TODO: Support nonblocking commits with a deferred commit worker
                // and event lifetime management. Async page flips also need a
                // backend path that can bypass normal vblank synchronization.
                return Err(DrmError::NotSupported);
            }

            // Hold the KMS object store across check and commit. The committed
            // object state must be the same state that was validated, so commit
            // is expected to be infallible with respect to concurrent KMS
            // changes.
            let objects = self.kms_objects().write();
            let mut atomic_state = DrmAtomicState::new(&objects, requests)?;
            self.atomic_check(&objects, &mut atomic_state, flags)?;
            self.atomic_commit(&objects, &atomic_state)?;
            if flags.contains(DrmAtomicFlags::PAGE_FLIP_EVENT) {
                self.atomic_queue_vblank_event(&objects, &atomic_state, user_data, event_ctx)?;
            }
            drop(objects);
            self.atomic_commit_tail(atomic_state, flags)?;
        }

        Ok(())
    }

    /// Validates a pending atomic state and records its display side effects.
    ///
    /// This step resolves each object's final state, checks object-specific
    /// constraints, and computes which CRTCs need a modeset, event, or flush.
    fn atomic_check(
        &self,
        objects: &DrmKmsObjectStore,
        atomic_state: &mut DrmAtomicState,
        flags: DrmAtomicFlags,
    ) -> Result<(), DrmError> {
        for (plane_id, plane_state) in atomic_state.plane_states.iter() {
            let plane = objects
                .get_object::<DrmPlane>(*plane_id)
                .ok_or(DrmError::NotFound)?;
            let snapshot = plane.snapshot();

            let final_crtc_id = match plane_state.crtc_id {
                Some(crtc_id) => crtc_id,
                None => snapshot.crtc_id(),
            };

            let final_display_mode = match final_crtc_id {
                Some(crtc_id) => match atomic_state
                    .crtc_states
                    .get(&crtc_id)
                    .and_then(|state| state.display_mode)
                {
                    Some(display_mode) => display_mode,
                    None => {
                        let crtc = objects
                            .get_object::<DrmCrtc>(crtc_id)
                            .ok_or(DrmError::NotFound)?;
                        crtc.snapshot().display_mode()
                    }
                },
                None => None,
            };

            atomic_state.effect.merge(plane.check_pending_state(
                &objects,
                plane_state,
                final_display_mode,
            )?);
        }

        for (crtc_id, crtc_state) in atomic_state.crtc_states.iter() {
            let crtc = objects
                .get_object::<DrmCrtc>(*crtc_id)
                .ok_or(DrmError::NotFound)?;
            atomic_state
                .effect
                .merge(crtc.check_pending_state(*crtc_id, &objects, crtc_state)?)
        }

        for (conn_id, conn_state) in atomic_state.connector_states.iter_mut() {
            let connector = objects
                .get_object::<DrmConnector>(*conn_id)
                .ok_or(DrmError::NotFound)?;
            atomic_state
                .effect
                .merge(connector.check_pending_state(&objects, conn_state)?)
        }

        if atomic_state.effect.request_modeset && !flags.contains(DrmAtomicFlags::ALLOW_MODESET) {
            return Err(DrmError::Invalid);
        }

        Ok(())
    }

    /// Commits the checked atomic state into the software KMS objects.
    ///
    /// This step updates plane, CRTC, and connector state after validation has
    /// succeeded. It does not program the display backend directly.
    fn atomic_commit(
        &self,
        objects: &DrmKmsObjectStore,
        atomic_state: &DrmAtomicState,
    ) -> Result<(), DrmError> {
        for (plane_id, plane_state) in atomic_state.plane_states.iter() {
            let plane = objects
                .get_object::<DrmPlane>(*plane_id)
                .ok_or(DrmError::NotFound)?;
            plane.commit_pending_state(plane_state)?;
        }

        for (crtc_id, crtc_state) in atomic_state.crtc_states.iter() {
            let crtc = objects
                .get_object::<DrmCrtc>(*crtc_id)
                .ok_or(DrmError::NotFound)?;
            crtc.commit_pending_state(crtc_state)?;
        }

        for (conn_id, conn_state) in atomic_state.connector_states.iter() {
            let connector = objects
                .get_object::<DrmConnector>(*conn_id)
                .ok_or(DrmError::NotFound)?;
            connector.commit_pending_state(&objects, conn_state)?;
        }

        Ok(())
    }

    /// Queues flip-complete events for CRTCs affected by this commit.
    ///
    /// Events are armed after the software state is committed and are delivered
    /// when the target CRTC reaches the next vblank sequence.
    fn atomic_queue_vblank_event(
        &self,
        objects: &DrmKmsObjectStore,
        atomic_state: &DrmAtomicState,
        user_data: u64,
        event_ctx: Arc<dyn DrmIoctlEventCtx>,
    ) -> Result<(), DrmError> {
        for crtc_id in &atomic_state.effect.event_crtcs {
            let crtc = objects
                .get_object::<DrmCrtc>(*crtc_id)
                .ok_or(DrmError::NotFound)?;

            let target_sequence = crtc
                .vblank_sequence()
                .checked_add(1)
                .ok_or(DrmError::Invalid)?;

            let event = DrmPendingVblankEvent::new_flip_complete(
                target_sequence,
                user_data,
                *crtc_id,
                event_ctx.clone(),
            );
            crtc.queue_vblank_event(event);
        }

        Ok(())
    }

    /// Applies a committed atomic state to the display backend.
    ///
    /// The default implementation flushes every affected CRTC. Drivers may
    /// override this step when hardware programming, cleanup, or asynchronous
    /// ordering requires backend-specific handling.
    fn atomic_commit_tail(
        &self,
        atomic_state: DrmAtomicState,
        _flags: DrmAtomicFlags,
    ) -> Result<(), DrmError> {
        // TODO: Commit tail applies the already-swapped software state to the display backend.
        // Future implementations need to handle nonblocking ordering, vblank
        // waits, page-flip events, cleanup of old framebuffer references, and
        // driver-specific hardware programming.
        for crtc_id in atomic_state.effect.affected_crtcs {
            self.atomic_flush(crtc_id)?;
        }

        Ok(())
    }

    fn atomic_flush(&self, crtc_id: KmsObjectId) -> Result<(), DrmError>;
}
