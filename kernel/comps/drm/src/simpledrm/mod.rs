// SPDX-License-Identifier: MPL-2.0

use alloc::vec;

use aster_framebuffer::FRAMEBUFFER;
use ostd::sync::RwLock;

use crate::{
    DrmConnStatus, DrmConnType, DrmConnector, DrmDisplayInfo, DrmDisplayMode, DrmEncoderType,
    DrmError, DrmKmsObjectType, DrmPlaneType,
    device::{DrmDevice, DrmDeviceCaps, DrmFeatures},
    kms::{
        DrmKmsOps,
        object::{
            DrmKmsObjectStore, KmsObjectId,
            builder::DrmKmsObjectBuilder,
            display::{DrmDisplayFormat, SubpixelOrder},
        },
    },
};

const SIMPLEDRM_NAME: &'static str = "simpledrm";
const SIMPLEDRM_DESC: &'static str = "DRM driver for simple-framebuffer platform devices";
const SIMPLEDRM_DPI: u32 = 96;
const SIMPLEDRM_VFRESH: u32 = 60;

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

    fn update_connector_state(&self, conn_id: KmsObjectId) -> Result<(), DrmError> {
        let objects = self.objects.read();
        let connector = objects
            .get_object::<DrmConnector>(conn_id)
            .ok_or(DrmError::NotFound)?;

        // update simpledrm connector current encoder id.
        let snapshot = connector.snapshot();
        let encoder_id = if snapshot.encoder_id().is_some() {
            snapshot.encoder_id()
        } else {
            objects
                .collect_object_ids(
                    DrmKmsObjectType::Encoder,
                    Some(connector.possible_encoders()),
                )
                .first()
                .copied()
        };
        connector.set_current_encoder_id(encoder_id);

        let Some(framebuffer) = FRAMEBUFFER.get() else {
            return connector.set_display_state(
                DrmConnStatus::Disconnected,
                vec![],
                DrmDisplayInfo::default(),
                None,
            );
        };

        let width = framebuffer.width().min(u16::MAX as usize) as u16;
        let height = framebuffer.height().min(u16::MAX as usize) as u16;
        let display_mode = DrmDisplayMode::new(width, height, SIMPLEDRM_VFRESH);
        let display_info = DrmDisplayInfo::new(
            drm_mode_res_mm(width as u32, SIMPLEDRM_DPI),
            drm_mode_res_mm(height as u32, SIMPLEDRM_DPI),
            SubpixelOrder::Unknown,
        );

        // `simpledrm` only has the boot framebuffer's pixel geometry here, so
        // it relies on the shared physical-size fallback path.
        connector.set_display_state(
            DrmConnStatus::Connected,
            vec![display_mode],
            display_info,
            encoder_id,
        )
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

fn drm_mode_res_mm(resolution_px: u32, dpi: u32) -> u32 {
    (resolution_px * 254) / (dpi * 10)
}
