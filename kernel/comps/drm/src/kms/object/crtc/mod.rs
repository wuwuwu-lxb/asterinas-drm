// SPDX-License-Identifier: MPL-2.0

use core::{fmt::Debug, time::Duration};

use aster_time::read_monotonic_time;
use ostd::sync::{Mutex, WaitQueue};

use crate::{
    DrmError, DrmPendingVblankEvent,
    kms::{
        object::{
            DrmKmsObject, DrmKmsObjectCast, KmsObjectId, display::DrmDisplayMode,
            property::DrmKmsObjectProp,
        },
        vblank::DrmVblankState,
    },
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

pub struct DrmCrtc {
    state: Mutex<DrmCrtcState>,
    gamma_size_px: u32,
    primary_plane_id: KmsObjectId,
    cursor_plane_id: Option<KmsObjectId>,
    properties: DrmKmsObjectProp,

    vblank_state: Mutex<DrmVblankState>,
    vblank_wait_queue: WaitQueue,
}

impl Debug for DrmCrtc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DrmCrtc")
            .field("state", &self.state)
            .field("gamma_size_px", &self.gamma_size_px)
            .field("primary_plane_id", &self.primary_plane_id)
            .field("cursor_plane_id", &self.cursor_plane_id)
            .field("properties", &self.properties)
            .field("vblank_state", &self.vblank_state)
            .finish_non_exhaustive()
    }
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
            vblank_state: Mutex::new(DrmVblankState::new()),
            vblank_wait_queue: WaitQueue::new(),
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

    pub fn set_display_mode(&self, display_mode: Option<DrmDisplayMode>) {
        self.state().lock().display_mode = display_mode;
    }

    pub fn set_enable(&self, enable: bool) {
        self.state().lock().enable = enable;
    }

    pub fn set_active(&self, active: bool) {
        self.state().lock().active = active;
    }

    pub fn wait_vblank(&self, target_sequence: u64) -> (u64, Duration) {
        self.vblank_wait_queue.wait_until(|| {
            let state = self.vblank_state.lock();

            (state.sequence() >= target_sequence)
                .then_some((state.sequence(), state.last_time().clone()))
        })
    }

    pub fn queue_vblank_event(&self, event: DrmPendingVblankEvent) {
        self.vblank_state.lock().queue_event(event);
    }

    pub fn vblank_sequence(&self) -> u64 {
        self.vblank_state.lock().sequence()
    }

    pub fn handle_vblank(&self) -> Result<(), DrmError> {
        let timestamp = read_monotonic_time();

        let (sequence, mut ready_events) = {
            let mut vblank_state = self.vblank_state.lock();

            let sequence = vblank_state.increment();
            vblank_state.update_time(timestamp);
            let ready_events = vblank_state.take_pending_events(sequence);

            (sequence, ready_events)
        };

        self.vblank_wait_queue.wake_all();

        let tv_sec = timestamp.as_secs() as u32;
        let tv_usec = timestamp.subsec_micros();

        for event in ready_events.iter_mut() {
            event.send(sequence, tv_sec, tv_usec);
        }

        Ok(())
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
