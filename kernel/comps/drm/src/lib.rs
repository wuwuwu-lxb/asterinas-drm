// SPDX-License-Identifier: MPL-2.0

#![no_std]
#![deny(unsafe_code)]

extern crate alloc;
#[macro_use]
extern crate ostd_pod;

// Set this crate's log prefix for `ostd::log`.
macro_rules! __log_prefix {
    () => {
        "drm: "
    };
}

mod device;
mod kms;
mod simpledrm;

use alloc::{sync::Arc, vec::Vec};

use aster_framebuffer::FRAMEBUFFER;
use component::{ComponentInitError, init_component};
pub use device::{DrmDevice, DrmDeviceCaps, DrmFeatures};
pub use kms::{
    DrmKmsOps,
    object::{
        DrmKmsObject, DrmKmsObjectStore, DrmKmsObjectType,
        builder::DrmKmsObjectBuilder,
        connector::{DrmConnState, DrmConnStatus, DrmConnType, DrmConnector, DrmConnectorSnapshot},
        crtc::{DrmCrtc, DrmCrtcSnapshot, DrmCrtcState},
        display::{DrmDisplayInfo, DrmDisplayMode, DrmModeModeInfo},
        encoder::{DrmEncoder, DrmEncoderState, DrmEncoderType},
        plane::{DrmPlane, DrmPlaneState, DrmPlaneType},
    },
};
use ostd::sync::Mutex;
use spin::Once;

use crate::simpledrm::SimpleDrmDevice;

/// Error type for GPU device registry operations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    AlreadyRegistered,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrmError {
    /// Generic invalid argument or state
    Invalid,
    /// Object not found (CRTC / FB / GEM handle / connector, etc.)
    NotFound,
    /// Operation not supported by this driver / device
    NotSupported,
    /// Resource temporarily unavailable (busy, in use)
    Busy,
    /// Permission or access violation
    PermissionDenied,
    /// Memory allocation or mapping failure
    NoMemory,
}

impl From<Error> for ComponentInitError {
    fn from(error: Error) -> Self {
        match error {
            Error::AlreadyRegistered => {
                ostd::warn!("The device already registered")
            }
            _ => {}
        }
        ComponentInitError::Unknown
    }
}

pub fn register_drm_device(device: Arc<dyn DrmDevice>) -> Result<(), Error> {
    let component = COMPONENT
        .get()
        .expect("aster-drm component not initialized");

    component.drm_devices.lock().push(device);

    Ok(())
}

pub fn registered_drm_devices() -> Vec<Arc<dyn DrmDevice>> {
    let component = COMPONENT
        .get()
        .expect("aster-drm component not initialized");

    component.drm_devices.lock().clone()
}

pub fn unregister_drm_device(device: &Arc<dyn DrmDevice>) -> Result<Arc<dyn DrmDevice>, Error> {
    let component = COMPONENT
        .get()
        .expect("aster-drm component not initialized");

    let mut devices = component.drm_devices.lock();
    if let Some(pos) = devices.iter().position(|d| Arc::ptr_eq(d, device)) {
        Ok(devices.remove(pos))
    } else {
        Err(Error::NotFound)
    }
}

static COMPONENT: Once<Component> = Once::new();

#[init_component]
fn component_init() -> Result<(), ComponentInitError> {
    let component = Component::init()?;
    COMPONENT.call_once(|| component);

    if FRAMEBUFFER.get().is_some() {
        match SimpleDrmDevice::new() {
            Ok(device) => register_drm_device(Arc::new(device))?,
            Err(error) => {
                ostd::warn!("[kernel] DRM: failed to initialize simpledrm: {:?}", error);
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct Component {
    drm_devices: Mutex<Vec<Arc<dyn DrmDevice>>>,
}

impl Component {
    fn init() -> Result<Self, ComponentInitError> {
        Ok(Self {
            drm_devices: Mutex::new(Vec::new()),
        })
    }
}
