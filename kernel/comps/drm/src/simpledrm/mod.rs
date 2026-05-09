// SPDX-License-Identifier: MPL-2.0

use alloc::{sync::Arc, vec, vec::Vec};

use aster_framebuffer::FRAMEBUFFER;
use ostd::{mm::VmWriter, sync::RwLock};

use crate::{
    DrmConnStatus, DrmConnType, DrmConnector, DrmDisplayInfo, DrmDisplayMode, DrmEncoder,
    DrmEncoderType, DrmError, DrmFramebuffer, DrmGemObject, DrmKmsObjectType, DrmPlane,
    DrmPlaneType,
    device::{DrmDevice, DrmDeviceCaps, DrmFeatures},
    gem::{DrmGemOps, DrmIoctlGemCtx, vma_manager::DrmVmaOffsetManager},
    kms::{
        DrmKmsOps,
        object::{
            DrmKmsObjectStore, KmsObjectId,
            builder::DrmKmsObjectBuilder,
            display::{DrmDisplayFormat, SubpixelOrder},
            geometry::RectU32,
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
    vma_manager: DrmVmaOffsetManager,
}

impl SimpleDrmDevice {
    pub fn new() -> Result<Self, DrmError> {
        let objects = Self::build_kms_objects()?;

        Ok(Self {
            caps: DrmDeviceCaps::default(),
            features: DrmFeatures::GEM | DrmFeatures::MODESET | DrmFeatures::ATOMIC,
            objects: RwLock::new(objects),
            vma_manager: DrmVmaOffsetManager::new(),
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

    fn write_firmware_fb(&self, fb: &DrmFramebuffer) -> Result<(), DrmError> {
        let Some(framebuffer) = FRAMEBUFFER.get() else {
            return Err(DrmError::NotFound);
        };

        let width = fb.width() as usize;
        let height = fb.height() as usize;
        let pitch = fb.pitch(0).ok_or(DrmError::NotFound)? as usize;
        let offset = fb.offset(0).ok_or(DrmError::NotFound)? as usize;
        let gem_object = fb.gem_object(0).ok_or(DrmError::NotFound)?;

        let bytes_per_pixel = fb
            .pixel_format()
            .bytes_per_pixel()
            .ok_or(DrmError::Invalid)?;
        let copy_width_bytes = width
            .checked_mul(bytes_per_pixel)
            .ok_or(DrmError::Invalid)?;

        if copy_width_bytes > pitch
            || width > framebuffer.width()
            || height > framebuffer.height()
            || copy_width_bytes > framebuffer.line_size()
        {
            return Err(DrmError::Invalid);
        }

        let mut line_buf = vec![0u8; copy_width_bytes];
        for row in 0..height as usize {
            let src_offset = offset
                .checked_add(row.checked_mul(pitch).ok_or(DrmError::Invalid)?)
                .ok_or(DrmError::Invalid)?;
            let dst_offset = row
                .checked_mul(framebuffer.line_size())
                .ok_or(DrmError::Invalid)?;

            let mut writer = VmWriter::from(line_buf.as_mut_slice()).to_fallible();
            gem_object.read(src_offset, &mut writer)?;
            framebuffer
                .write_bytes_at(dst_offset, &line_buf)
                .map_err(|_| DrmError::Invalid)?;
        }

        Ok(())
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

    fn set_crtc(
        &self,
        crtc_id: KmsObjectId,
        fb_id: KmsObjectId,
        x: u32,
        y: u32,
        display_mode: Option<DrmDisplayMode>,
        connector_ids: Vec<KmsObjectId>,
    ) -> Result<(), DrmError> {
        // TODO: Replace this legacy non-atomic `set_crtc` path with a proper
        // atomic commit flow.
        //
        // We already model object-local atomic state, but the current ioctl
        // path is still not a full atomic transaction. This implementation is
        // therefore only a temporary compatibility bridge for legacy userspace.
        //
        // If display_mode is none, reset the render pipeline.
        let Some(display_mode) = display_mode else {
            let objects = self.objects.write();
            let crtc = objects
                .get_object::<crate::DrmCrtc>(crtc_id)
                .ok_or(DrmError::NotFound)?;
            let primary_plane = objects
                .get_object::<DrmPlane>(crtc.primary_plane_id())
                .ok_or(DrmError::NotFound)?;

            primary_plane.set_fb_id(None);
            primary_plane.set_crtc_id(None);
            crtc.set_enable(false);
            crtc.set_active(false);
            crtc.set_display_mode(None);

            // Simpledrm has only one connector.
            let connector_id = objects
                .collect_object_ids(DrmKmsObjectType::Connector, None)
                .first()
                .cloned()
                .ok_or(DrmError::NotFound)?;
            let connector = objects
                .get_object::<DrmConnector>(connector_id)
                .ok_or(DrmError::NotFound)?;

            if let Some(encoder_id) = connector.snapshot().encoder_id() {
                let encoder = objects
                    .get_object::<DrmEncoder>(encoder_id)
                    .ok_or(DrmError::NotFound)?;

                if encoder.crtc_id() == Some(crtc_id) {
                    connector.set_current_encoder_id(None);
                    encoder.set_crtc_id(None);
                }
            }

            return Ok(());
        };

        let objects = self.objects.write();
        let crtc = objects
            .get_object::<crate::DrmCrtc>(crtc_id)
            .ok_or(DrmError::NotFound)?;
        let fb = objects
            .get_object::<DrmFramebuffer>(fb_id)
            .ok_or(DrmError::NotFound)?;
        let primary_plane = objects
            .get_object::<DrmPlane>(crtc.primary_plane_id())
            .ok_or(DrmError::NotFound)?;

        if fb.width() < display_mode.hdisplay() as u32
            || fb.height() < display_mode.vdisplay() as u32
        {
            return Err(DrmError::Invalid);
        }

        // Simpledrm has only one connector.
        let connector_id = connector_ids.first().ok_or(DrmError::Invalid)?;
        let connector = objects
            .get_object::<DrmConnector>(*connector_id)
            .ok_or(DrmError::NotFound)?;
        let snapshot = connector.snapshot();

        let encoder_id = snapshot
            .encoder_id()
            .or_else(|| {
                objects
                    .collect_object_ids(
                        DrmKmsObjectType::Encoder,
                        Some(connector.possible_encoders()),
                    )
                    .first()
                    .copied()
            })
            .ok_or(DrmError::NotFound)?;

        let encoder = objects
            .get_object::<DrmEncoder>(encoder_id)
            .ok_or(DrmError::NotFound)?;
        if !objects
            .collect_object_ids(DrmKmsObjectType::Crtc, Some(encoder.possible_crtcs()))
            .contains(&crtc_id)
        {
            return Err(DrmError::Invalid);
        }

        let src_rect = RectU32::new(
            x,
            y,
            display_mode.vdisplay() as u32,
            display_mode.hdisplay() as u32,
        );
        let crtc_rect = RectU32::new(
            0,
            0,
            display_mode.vdisplay() as u32,
            display_mode.hdisplay() as u32,
        );

        connector.set_current_encoder_id(Some(encoder_id));
        encoder.set_crtc_id(Some(crtc_id));
        primary_plane.set_src_rect(src_rect);
        primary_plane.set_crtc_rect(crtc_rect);
        primary_plane.set_fb_id(Some(fb_id));
        primary_plane.set_crtc_id(Some(crtc_id));
        crtc.set_enable(true);
        crtc.set_active(true);
        crtc.set_display_mode(Some(display_mode));

        self.write_firmware_fb(fb)?;

        Ok(())
    }
}

impl DrmGemOps for SimpleDrmDevice {
    fn create_dumb(
        &self,
        width: u32,
        height: u32,
        bpp: u32,
        ctx: &dyn DrmIoctlGemCtx,
    ) -> Result<Arc<dyn DrmGemObject>, DrmError> {
        let pitch = width.checked_mul(bpp / 8).ok_or(DrmError::Invalid)?;
        let size = pitch.checked_mul(height).ok_or(DrmError::Invalid)? as usize;
        ctx.create_shmem_gem(size, pitch)
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

    fn vma_manager(&self) -> &DrmVmaOffsetManager {
        &self.vma_manager
    }
}

fn drm_mode_res_mm(resolution_px: u32, dpi: u32) -> u32 {
    (resolution_px * 254) / (dpi * 10)
}
