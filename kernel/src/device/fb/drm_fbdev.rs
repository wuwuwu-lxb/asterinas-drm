// SPDX-License-Identifier: MPL-2.0

use alloc::sync::Arc;
use core::time::Duration;

use aster_drm::{
    DRM_FORMAT_MAX_PLANES, DrmConnStatus, DrmConnector, DrmDevice, DrmDisplayFormat,
    DrmDisplayMode, DrmFeatures, DrmFramebuffer, DrmGemObject, DrmIoctlGemCtx, DrmKmsObject,
    DrmKmsObjectType,
};
use aster_framebuffer::{ColorMapEntry, MAX_CMAP_SIZE};
use ostd::sync::WaitQueue;

use crate::{
    device::drm::gem::DrmGemShmemObject, prelude::*, thread::kernel_thread::ThreadOptions,
};

const FBDEV_REFRESH_INTERVAL_MS: u64 = 33;

#[derive(Debug)]
pub(super) struct DrmFbdevBackend {
    pub(super) device: Arc<dyn DrmDevice>,
    pub(super) gem: Arc<dyn DrmGemObject>,
    pub(super) fb_id: u32,
    pub(super) crtc_id: u32,
    pub(super) connector_id: u32,
    pub(super) mode: DrmDisplayMode,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) pitch_bytes: u32,
    pub(super) size_bytes: usize,
    pub(super) width_mm: u32,
    pub(super) height_mm: u32,
    pub(super) cmap: Mutex<Vec<ColorMapEntry>>,
}

#[derive(Debug)]
struct FbGemCtx;

impl DrmIoctlGemCtx for FbGemCtx {
    fn create_shmem_gem(
        &self,
        size: usize,
        pitch: u32,
    ) -> core::result::Result<Arc<dyn DrmGemObject>, aster_drm::DrmError> {
        Ok(Arc::new(DrmGemShmemObject::new(size, pitch)?))
    }
}

impl DrmFbdevBackend {
    pub(super) fn new() -> Result<Self> {
        let (device, crtc_id, connector_id, mode, width_mm, height_mm) =
            Self::choose_kms_target()?;
        let width = u32::from(mode.hdisplay());
        let height = u32::from(mode.vdisplay());

        let gem = device.create_dumb(width, height, 32, &FbGemCtx)?;
        let pitch_bytes = gem.pitch();
        let size_bytes = gem.size();
        let mut gems: [Option<Arc<dyn DrmGemObject>>; DRM_FORMAT_MAX_PLANES] =
            [None, None, None, None];
        gems[0] = Some(gem.clone());

        let framebuffer = DrmFramebuffer::new(
            width,
            height,
            DrmDisplayFormat::XRGB8888,
            0,
            [pitch_bytes, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            gems,
        )?;

        let fb_id = device
            .kms_objects()
            .write()
            .add_object(DrmKmsObject::Framebuffer(framebuffer))?;
        if let Err(error) =
            device.set_crtc(crtc_id, fb_id, 0, 0, Some(mode), vec![connector_id])
        {
            device.kms_objects().write().remove_framebuffer(fb_id);
            return Err(error.into());
        }

        device.dirty_fb(fb_id)?;

        Ok(Self {
            device,
            gem,
            fb_id,
            crtc_id,
            connector_id,
            mode,
            width,
            height,
            pitch_bytes,
            size_bytes,
            width_mm,
            height_mm,
            cmap: Mutex::new(vec![
                ColorMapEntry {
                    red: 0,
                    green: 0,
                    blue: 0,
                    transp: 0,
                };
                MAX_CMAP_SIZE
            ]),
        })
    }

    fn choose_kms_target() -> Result<(Arc<dyn DrmDevice>, u32, u32, DrmDisplayMode, u32, u32)> {
        for device in aster_drm::registered_drm_devices() {
            if !device.has_feature(DrmFeatures::MODESET) || !device.caps().has_dumb_buffer() {
                continue;
            }

            let crtc_id = {
                let objects = device.kms_objects().read();
                objects
                    .collect_object_ids(DrmKmsObjectType::Crtc, None)
                    .first()
                    .copied()
            };
            let Some(crtc_id) = crtc_id else {
                continue;
            };

            let connector_ids = {
                let objects = device.kms_objects().read();
                objects.collect_object_ids(DrmKmsObjectType::Connector, None)
            };

            for connector_id in connector_ids {
                if device.update_connector_state(connector_id).is_err() {
                    continue;
                }

                let connector_snapshot = {
                    let objects = device.kms_objects().read();
                    let Some(connector) = objects.get_object::<DrmConnector>(connector_id) else {
                        continue;
                    };
                    connector.snapshot()
                };

                if !matches!(connector_snapshot.status(), DrmConnStatus::Connected) {
                    continue;
                }

                let Some(mode) = connector_snapshot.display_modes().first().copied() else {
                    continue;
                };

                return Ok((
                    device,
                    crtc_id,
                    connector_id,
                    mode,
                    connector_snapshot.mm_width(),
                    connector_snapshot.mm_height(),
                ));
            }
        }

        return_errno_with_message!(Errno::ENODEV, "no DRM KMS target is available");
    }

    pub(super) fn restore_scanout(&self) -> Result<()> {
        self.device.set_crtc(
            self.crtc_id,
            self.fb_id,
            0,
            0,
            Some(self.mode),
            vec![self.connector_id],
        )?;
        Ok(())
    }

    pub(super) fn dirty(&self) -> Result<()> {
        self.device.dirty_fb(self.fb_id)?;
        Ok(())
    }

    pub(super) fn spawn_refresh_thread(self: &Arc<Self>) {
        let backend = self.clone();
        let task_fn = move || {
            let wait_queue = WaitQueue::new();
            let interval = Duration::from_millis(FBDEV_REFRESH_INTERVAL_MS);

            loop {
                let _ = wait_queue.wait_until_or_timeout(|| -> Option<()> { None }, &interval);
                let _ = backend.dirty();
            }
        };

        ThreadOptions::new(task_fn).spawn();
    }
}
