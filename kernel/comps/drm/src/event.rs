// SPDX-License-Identifier: MPL-2.0

use core::fmt::Debug;

#[repr(u32)]
#[derive(Debug)]
pub(crate) enum DrmEventType {
    Vblank = 0x01,
    FlipComplete = 0x02,
    #[expect(dead_code)]
    CrtcSequence = 0x03,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub(crate) struct DrmEventBase {
    type_: u32,
    length: u32,
}

impl DrmEventBase {
    pub(crate) fn new(type_: DrmEventType, length: u32) -> Self {
        Self {
            type_: type_ as u32,
            length,
        }
    }

    pub(crate) fn type_u32(&self) -> u32 {
        self.type_
    }

    pub(crate) fn length(&self) -> u32 {
        self.length
    }
}

/// Callback trait for sending vblank events to userspace
///
/// This allows the vblank subsystem (comps/gpu) to send events
/// without depending on specific types like DrmFile.
/// Users provide closures/callbacks that implement this trait.
pub trait DrmIoctlEventCtx: Debug + Send + Sync {
    fn vblank_event_callback(&self, bytes: &[u8]);
}
