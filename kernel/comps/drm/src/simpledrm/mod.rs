// SPDX-License-Identifier: MPL-2.0
use alloc::vec;
use ostd::sync::RwLock;

use crate::{
    DrmConnType, DrmEncoderType, DrmError, DrmPlaneType,
    device::{DrmDevice, DrmDeviceCaps, DrmFeatures},
    kms::{
        DrmKmsOps,
        object::{DrmKmsObjectStore, builder::DrmKmsObjectBuilder, display::DrmDisplayFormat},
    },
};

const SIMPLEDRM_NAME: &'static str = "simpledrm";
const SIMPLEDRM_DESC: &'static str = "DRM driver for simple-framebuffer platform devices";

#[derive(Debug)]
pub(crate) struct SimpleDrmDevice {
    caps: DrmDeviceCaps,
    features: DrmFeatures,
    objects: RwLock<DrmKmsObjectStore>,
}

impl SimpleDrmDevice {
    pub fn new() -> Result<Self, DrmError> {
        let objects = Self::build_kms_objects()?;

        Ok(Self {
            caps: DrmDeviceCaps::default(),
            features: DrmFeatures::GEM | DrmFeatures::MODESET | DrmFeatures::ATOMIC,
            objects: RwLock::new(objects),
        })
    }

    fn build_kms_objects() -> Result<DrmKmsObjectStore, DrmError> {
        let mut builder = DrmKmsObjectBuilder::default();

        // TODO: Derive the exact plane format from the boot framebuffer.
        // see `comps/framebuffer/src/framebuffer.rs`
        let format_types = vec![DrmDisplayFormat::XRGB8888];
        let primary = builder.add_plane(DrmPlaneType::Primary, format_types);
        let crtc = builder.add_crtc(0, primary, None);
        let encoder = builder.add_encoder(DrmEncoderType::VIRTUAL);
        let connector = builder.add_connector(DrmConnType::VIRTUAL);

        builder.plane_attach_crtc(primary, crtc)?;
        builder.encoder_attach_crtc(encoder, crtc)?;
        builder.connector_attach_encoder(connector, encoder)?;

        builder.build()
    }
}

impl DrmKmsOps for SimpleDrmDevice {
    fn kms_objects(&self) -> &RwLock<DrmKmsObjectStore> {
        &self.objects
    }
}

impl DrmDevice for SimpleDrmDevice {
    fn name(&self) -> &str {
        SIMPLEDRM_NAME
    }

    fn desc(&self) -> &str {
        SIMPLEDRM_DESC
    }

    fn features(&self) -> &DrmFeatures {
        &self.features
    }

    fn caps(&self) -> &DrmDeviceCaps {
        &self.caps
    }
}
