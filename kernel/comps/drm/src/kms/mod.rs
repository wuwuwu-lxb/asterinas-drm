// SPDX-License-Identifier: MPL-2.0

use alloc::vec::Vec;
use core::fmt::Debug;

use ostd::sync::RwLock;

use crate::{
    DrmError,
    kms::object::{DrmKmsObjectStore, KmsObjectId, display::DrmDisplayMode},
};

pub mod object;
pub mod vblank;

/// Provides device-level KMS operations and access to the KMS object store.
///
/// This differs from Linux DRM in one important way.
/// In Linux, KMS objects such as planes and CRTCs typically carry function tables
/// that can reach back into the owning DRM device.
/// Asterinas does not model KMS objects as owning the top-level device,
/// so driver-specific object behavior is currently routed through the device-level
/// KMS operations instead.
///
/// This keeps the core KMS objects as plain data objects with protected runtime state,
/// while the driver retains ownership of hardware-specific behavior.
/// Future refactors may split some of these operations into narrower traits
/// such as `DrmPlaneOps`,
/// but for now they remain attached to the device-side KMS interface.
///
/// Locking rule:
/// callers should acquire the outer KMS object store lock
/// before taking any individual KMS object state lock.
///
pub trait DrmKmsOps: Debug + Send + Sync {
    fn kms_objects(&self) -> &RwLock<DrmKmsObjectStore>;
    fn update_connector_state(&self, conn_id: KmsObjectId) -> Result<(), DrmError>;

    /// Applies a legacy CRTC configuration.
    ///
    /// `None` for `display_mode` disables the CRTC. Enabling a CRTC requires
    /// one mode, one framebuffer, and the connector set driven by that mode.
    /// `x` and `y` are the framebuffer-space origin of the scanout rectangle.
    fn set_crtc(
        &self,
        crtc_id: KmsObjectId,
        fb_id: KmsObjectId,
        x: u32,
        y: u32,
        display_mode: Option<DrmDisplayMode>,
        connector_ids: Vec<KmsObjectId>,
    ) -> Result<(), DrmError>;
}
