// SPDX-License-Identifier: MPL-2.0

use alloc::{collections::vec_deque::VecDeque, sync::Arc, vec::Vec};
use core::{
    fmt::Debug,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use crate::{
    event::{DrmEventBase, DrmEventType, DrmIoctlEventCtx},
    kms::object::KmsObjectId,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmVblankEvent {
    base: DrmEventBase,
    user_data: u64,
    tv_sec: u32,
    tv_usec: u32,
    sequence: u32,
    crtc_id: u32,
}

impl DrmVblankEvent {
    fn new(event: DrmEventBase, user_data: u64, crtc_id: u32) -> Self {
        Self {
            base: event,
            user_data,
            tv_sec: 0,
            tv_usec: 0,
            sequence: 0,
            crtc_id,
        }
    }

    pub fn complete(&mut self, sequence: u32, tv_sec: u32, tv_usec: u32) {
        self.sequence = sequence;
        self.tv_sec = tv_sec;
        self.tv_usec = tv_usec;
    }
}

#[derive(Debug)]
pub struct DrmPendingVblankEvent {
    target_sequence: u32,
    event: DrmVblankEvent,
    ctx: Arc<dyn DrmIoctlEventCtx>,
}

impl DrmPendingVblankEvent {
    pub fn new_vblank(
        target_sequence: u32,
        user_data: u64,
        crtc_id: KmsObjectId,
        ctx: Arc<dyn DrmIoctlEventCtx>,
    ) -> Self {
        let event = DrmVblankEvent::new(
            DrmEventBase::new(DrmEventType::Vblank, size_of::<DrmVblankEvent>() as u32),
            user_data,
            crtc_id,
        );

        Self {
            target_sequence,
            event,
            ctx,
        }
    }

    pub fn new_flip_complete(
        target_sequence: u32,
        user_data: u64,
        crtc_id: KmsObjectId,
        ctx: Arc<dyn DrmIoctlEventCtx>,
    ) -> Self {
        let event = DrmVblankEvent::new(
            DrmEventBase::new(
                DrmEventType::FlipComplete,
                size_of::<DrmVblankEvent>() as u32,
            ),
            user_data,
            crtc_id,
        );

        Self {
            target_sequence,
            event,
            ctx,
        }
    }

    pub fn send(&mut self, sequence: u32, tv_sec: u32, tv_usec: u32) {
        self.event.complete(sequence, tv_sec, tv_usec);

        let mut bytes = Vec::with_capacity(size_of::<DrmVblankEvent>());
        bytes.extend_from_slice(&self.event.base.type_u32().to_ne_bytes());
        bytes.extend_from_slice(&self.event.base.length().to_ne_bytes());
        bytes.extend_from_slice(&self.event.user_data.to_ne_bytes());
        bytes.extend_from_slice(&tv_sec.to_ne_bytes());
        bytes.extend_from_slice(&tv_usec.to_ne_bytes());
        bytes.extend_from_slice(&sequence.to_ne_bytes());
        bytes.extend_from_slice(&self.event.crtc_id.to_ne_bytes());

        self.ctx.vblank_event_callback(&bytes)
    }
}

#[derive(Debug)]
pub struct DrmVblankState {
    sequence: AtomicU32,
    last_time: Duration,
    pending_events: VecDeque<DrmPendingVblankEvent>,
}

impl DrmVblankState {
    /// Create a new vblank state
    pub fn new() -> Self {
        Self {
            sequence: AtomicU32::new(0),
            last_time: Duration::from_secs(0),
            pending_events: VecDeque::new(),
        }
    }

    pub fn sequence(&self) -> u32 {
        self.sequence.load(Ordering::Relaxed)
    }

    pub fn last_time(&self) -> &Duration {
        &self.last_time
    }

    pub fn increment(&self) -> u32 {
        // return new value.
        self.sequence.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn queue_event(&mut self, event: DrmPendingVblankEvent) {
        self.pending_events.push_back(event);
    }

    pub fn take_pending_events(&mut self, sequence: u32) -> Vec<DrmPendingVblankEvent> {
        let mut ready_events = Vec::new();
        let mut pending_events = VecDeque::new();

        while let Some(event) = self.pending_events.pop_front() {
            if event.target_sequence <= sequence {
                ready_events.push(event);
            } else {
                pending_events.push_back(event);
            }
        }

        self.pending_events = pending_events;
        ready_events
    }

    pub fn update_time(&mut self, time: Duration) {
        self.last_time = time;
    }
}
