// SPDX-License-Identifier: MPL-2.0

use alloc::vec::Vec;
use core::sync::atomic::Ordering;

use aster_drm::{
    DrmConnector, DrmCrtc, DrmEncoder, DrmKmsObject, DrmKmsObjectProp, DrmKmsObjectType,
    DrmModeModeInfo, DrmPlane, DrmProperty, DrmPropertyBlob, DrmPropertyFlags, DrmPropertyKind,
};
use ostd::mm::VmIo;

use super::DrmFile;
use crate::{
    device::drm::{file::copy_array_to_user, ioctl::*},
    prelude::*,
};

impl DrmFile {
    fn visible_property_values(
        &self,
        properties: &DrmKmsObjectProp,
    ) -> Result<(Vec<u32>, Vec<u64>)> {
        let objects = self.device().kms_objects().read();
        let mut prop_ids = Vec::new();
        let mut prop_values = Vec::new();

        for (id, value) in properties.entries() {
            let property = objects.get_object::<DrmProperty>(id).ok_or(Errno::ENOENT)?;
            if property.flags().contains(DrmPropertyFlags::ATOMIC)
                && !self.caps.has_atomic.load(Ordering::Relaxed)
            {
                continue;
            }

            prop_ids.push(id);
            prop_values.push(value);
        }

        Ok((prop_ids, prop_values))
    }

    pub(super) fn ioctl_mode_get_resources(&self, cmd: DrmIoctlModeGetResources) -> Result<i32> {
        let mut args: DrmModeGetResources = cmd.read()?;

        cmd.with_data_ptr(|args_ptr| {
            let dev = self.device();
            let (crtc_ids, encoder_ids, connector_ids, framebuffer_ids) = {
                let objects = dev.kms_objects().read();
                (
                    objects.collect_object_ids(DrmKmsObjectType::Crtc, None),
                    objects.collect_object_ids(DrmKmsObjectType::Encoder, None),
                    objects.collect_object_ids(DrmKmsObjectType::Connector, None),
                    objects.collect_object_ids(DrmKmsObjectType::Framebuffer, None),
                )
            };

            copy_array_to_user(args_ptr.vm(), args.crtc_id_ptr, args.count_crtcs, &crtc_ids)?;
            copy_array_to_user(
                args_ptr.vm(),
                args.encoder_id_ptr,
                args.count_encoders,
                &encoder_ids,
            )?;
            copy_array_to_user(
                args_ptr.vm(),
                args.connector_id_ptr,
                args.count_connectors,
                &connector_ids,
            )?;
            copy_array_to_user(
                args_ptr.vm(),
                args.fb_id_ptr,
                args.count_fbs,
                &framebuffer_ids,
            )?;

            args.count_crtcs = crtc_ids.len() as u32;
            args.count_encoders = encoder_ids.len() as u32;
            args.count_connectors = connector_ids.len() as u32;
            args.count_fbs = framebuffer_ids.len() as u32;

            args.min_width = dev.caps().min_fb_width_px();
            args.max_width = dev.caps().max_fb_width_px();
            args.min_height = dev.caps().min_fb_height_px();
            args.max_height = dev.caps().max_fb_height_px();

            args_ptr.write(&args)?;

            Ok(())
        })?;

        Ok(0)
    }

    pub(super) fn ioctl_mode_get_crtc(&self, cmd: DrmIoctlModeGetCrtc) -> Result<i32> {
        let mut args: DrmModeCrtc = cmd.read()?;
        let (gamma_size, x, y, fb_id, mode) = {
            let objects = self.device().kms_objects().read();

            let crtc = objects
                .get_object::<DrmCrtc>(args.crtc_id)
                .ok_or(Errno::ENOENT)?;
            let crtc_snapshot = crtc.snapshot();

            let primary_plane = objects
                .get_object::<DrmPlane>(crtc.primary_plane_id())
                .ok_or(Errno::ENOENT)?;
            let primary_plane_snapshot = primary_plane.snapshot();

            (
                crtc.gamma_size_px(),
                primary_plane_snapshot.crtc_rect().x(),
                primary_plane_snapshot.crtc_rect().y(),
                primary_plane_snapshot.fb_id().unwrap_or(0),
                crtc_snapshot
                    .enable()
                    .then(|| crtc_snapshot.display_mode())
                    .flatten(),
            )
        };

        args.gamma_size = gamma_size;
        args.x = x;
        args.y = y;
        args.fb_id = fb_id;
        if let Some(mode) = mode {
            args.mode = mode.into();
            args.mode_valid = 1;
        } else {
            args.mode_valid = 0;
        }

        cmd.write(&args)?;
        Ok(0)
    }

    pub(super) fn ioctl_mode_get_encoder(&self, cmd: DrmIoctlModeGetEncoder) -> Result<i32> {
        let mut args: DrmModeGetEncoder = cmd.read()?;
        let (crtc_id, encoder_type, possible_crtcs) = {
            let objects = self.device().kms_objects().read();
            let encoder = objects
                .get_object::<DrmEncoder>(args.encoder_id)
                .ok_or(Errno::ENOENT)?;
            (
                encoder.crtc_id().unwrap_or(0),
                encoder.type_() as u32,
                encoder.possible_crtcs(),
            )
        };

        args.crtc_id = crtc_id;
        args.encoder_type = encoder_type;
        args.possible_crtcs = possible_crtcs;
        args.possible_clones = 0; // TODO: see DrmEncoder

        cmd.write(&args)?;
        Ok(0)
    }

    pub(super) fn ioctl_mode_get_connector(&self, cmd: DrmIoctlModeGetConnector) -> Result<i32> {
        let mut args: DrmModeGetConnector = cmd.read()?;

        // Linux treats `GETCONNECTOR` with `count_modes == 0` as a forced probe
        // request. Only the DRM master is allowed to refresh the connector
        // state in this path; non-master callers fall back to a read-only query.
        if args.count_modes == 0 && self.is_master() {
            self.device().update_connector_state(args.connector_id)?;
        }

        cmd.with_data_ptr(|args_ptr| {
            let (
                encoder_ids,
                mode_info,
                prop_ids,
                prop_values,
                encoder_id,
                connector_type,
                connector_type_id,
                connection,
                mm_width,
                mm_height,
                subpixel,
            ) = {
                let objects = self.device().kms_objects().read();
                let connector = objects
                    .get_object::<DrmConnector>(args.connector_id)
                    .ok_or(Errno::ENOENT)?;
                let snapshot = connector.snapshot();

                let encoder_ids = objects.collect_object_ids(
                    DrmKmsObjectType::Encoder,
                    Some(connector.possible_encoders()),
                );
                let mode_info: Vec<DrmModeModeInfo> = snapshot
                    .display_modes()
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect();
                let (prop_ids, prop_values) =
                    self.visible_property_values(connector.properties())?;

                (
                    encoder_ids,
                    mode_info,
                    prop_ids,
                    prop_values,
                    snapshot.encoder_id().unwrap_or(0),
                    connector.type_() as u32,
                    connector.type_index(),
                    snapshot.status() as u32,
                    snapshot.mm_width(),
                    snapshot.mm_height(),
                    snapshot.subpixel(),
                )
            };

            copy_array_to_user(
                args_ptr.vm(),
                args.encoders_ptr,
                args.count_encoders,
                &encoder_ids,
            )?;
            copy_array_to_user(args_ptr.vm(), args.modes_ptr, args.count_modes, &mode_info)?;
            copy_array_to_user(args_ptr.vm(), args.props_ptr, args.count_props, &prop_ids)?;
            copy_array_to_user(
                args_ptr.vm(),
                args.prop_values_ptr,
                args.count_props,
                &prop_values,
            )?;

            args.count_encoders = encoder_ids.len() as u32;
            args.count_modes = mode_info.len() as u32;
            args.count_props = prop_ids.len() as u32;

            args.encoder_id = encoder_id;
            args.connector_type = connector_type;
            args.connector_type_id = connector_type_id;
            args.connection = connection;
            args.mm_width = mm_width;
            args.mm_height = mm_height;
            args.subpixel = subpixel;

            args.pad = 0;

            cmd.write(&args)?;
            Ok(())
        })?;

        Ok(0)
    }

    pub(super) fn ioctl_mode_get_property(&self, cmd: DrmIoctlModeGetProperty) -> Result<i32> {
        let mut args: DrmModeGetProperty = cmd.read()?;

        cmd.with_data_ptr(|args_ptr| {
            let (property_name, property_flags, values, enum_entries) = {
                let objects = self.device().kms_objects().read();
                let property = objects
                    .get_object::<DrmProperty>(args.prop_id)
                    .ok_or(Errno::ENOENT)?;

                let values = match property.kind() {
                    DrmPropertyKind::Range { min, max } => vec![*min, *max],
                    DrmPropertyKind::SignedRange { min, max } => {
                        vec![*min as u64, *max as u64]
                    }
                    DrmPropertyKind::Enum(entries) | DrmPropertyKind::Bitmask(entries) => {
                        entries.iter().map(|entry| entry.value).collect()
                    }
                    DrmPropertyKind::Object(object_type) => vec![*object_type as u64],
                    DrmPropertyKind::Plain | DrmPropertyKind::Blob => vec![],
                };
                let enum_entries = match property.kind() {
                    DrmPropertyKind::Enum(entries) | DrmPropertyKind::Bitmask(entries) => {
                        entries.iter().copied().collect()
                    }
                    _ => Vec::new(),
                };

                (
                    property.name_to_u8(),
                    property.flags().bits(),
                    values,
                    enum_entries,
                )
            };

            copy_array_to_user(args_ptr.vm(), args.values_ptr, args.count_values, &values)?;
            copy_array_to_user(
                args_ptr.vm(),
                args.enum_blob_ptr,
                args.count_enum_blobs,
                &enum_entries,
            )?;

            args.flags = property_flags;
            args.name = property_name;
            args.count_values = values.len() as u32;
            args.count_enum_blobs = enum_entries.len() as u32;

            args_ptr.write(&args)?;
            Ok(())
        })?;

        Ok(0)
    }

    pub(super) fn ioctl_mode_get_blob(&self, cmd: DrmIoctlModeGetPropBlob) -> Result<i32> {
        let mut args: DrmModeGetBlob = cmd.read()?;

        cmd.with_data_ptr(|args_ptr| {
            let blob = {
                let objects = self.device().kms_objects().read();
                objects
                    .get_object::<DrmPropertyBlob>(args.blob_id)
                    .ok_or(Errno::ENOENT)?
                    .clone()
            };

            let data = blob.data();
            if args.data != 0 && args.length != 0 && !data.is_empty() {
                let write_len = core::cmp::min(args.length as usize, data.len());
                args_ptr
                    .vm()
                    .write_bytes(args.data as usize, &data[..write_len])?;
            }

            args.length = blob.length() as u32;
            args_ptr.write(&args)?;
            Ok(())
        })?;

        Ok(0)
    }

    pub(super) fn ioctl_mode_get_plane_resources(
        &self,
        cmd: DrmIoctlModeGetPlaneResources,
    ) -> Result<i32> {
        let mut args: DrmModeGetPlaneRes = cmd.read()?;

        cmd.with_data_ptr(|args_ptr| {
            let plane_ids = {
                let objects = self.device().kms_objects().read();
                objects.collect_object_ids(DrmKmsObjectType::Plane, None)
            };

            copy_array_to_user(
                args_ptr.vm(),
                args.plane_id_ptr,
                args.count_planes,
                &plane_ids,
            )?;

            args.count_planes = plane_ids.len() as u32;

            args_ptr.write(&args)?;
            Ok(())
        })?;

        Ok(0)
    }

    pub(super) fn ioctl_mode_get_plane(&self, cmd: DrmIoctlModeGetPlane) -> Result<i32> {
        let mut args: DrmModeGetPlane = cmd.read()?;

        cmd.with_data_ptr(|args_ptr| {
            let (crtc_id, fb_id, possible_crtcs, format_types) = {
                let objects = self.device().kms_objects().read();
                let plane = objects
                    .get_object::<DrmPlane>(args.plane_id)
                    .ok_or(Errno::ENOENT)?;
                let snapshot = plane.snapshot();
                let format_types: Vec<u32> = plane
                    .format_types()
                    .iter()
                    .copied()
                    .map(|f| f as u32)
                    .collect();

                (
                    snapshot.crtc_id().unwrap_or(0),
                    snapshot.fb_id().unwrap_or(0),
                    plane.possible_crtcs(),
                    format_types,
                )
            };

            copy_array_to_user(
                args_ptr.vm(),
                args.format_type_ptr,
                args.count_format_types,
                &format_types,
            )?;

            args.crtc_id = crtc_id;
            args.fb_id = fb_id;
            args.possible_crtcs = possible_crtcs;
            args.gamma_size = 0;
            args.count_format_types = format_types.len() as u32;

            args_ptr.write(&args)?;
            Ok(())
        })?;

        Ok(0)
    }

    pub(super) fn ioctl_mode_get_object_props(
        &self,
        cmd: DrmIoctlModeObjectGetProps,
    ) -> Result<i32> {
        let mut args: DrmModeObjectGetProps = cmd.read()?;

        cmd.with_data_ptr(|args_ptr| {
            let (prop_ids, prop_values) = {
                let objects = self.device().kms_objects().read();
                let object_type =
                    DrmKmsObjectType::try_from(args.obj_type).map_err(|_| Errno::EINVAL)?;
                let properties = objects.get_object_props(args.obj_id, object_type)?;
                self.visible_property_values(properties)?
            };

            copy_array_to_user(args_ptr.vm(), args.props_ptr, args.count_props, &prop_ids)?;
            copy_array_to_user(
                args_ptr.vm(),
                args.prop_values_ptr,
                args.count_props,
                &prop_values,
            )?;

            args.count_props = prop_ids.len() as u32;
            args_ptr.write(&args)?;
            Ok(())
        })?;

        Ok(0)
    }

    pub(super) fn ioctl_mode_create_blob(&self, cmd: DrmIoctlModeCreatePropBlob) -> Result<i32> {
        let mut args: DrmModeCreateBlob = cmd.read()?;

        cmd.with_data_ptr(|args_ptr| {
            let mut data = vec![0; args.length as usize];
            if args.length != 0 {
                args_ptr.vm().read_bytes(args.data as usize, &mut data)?;
            }

            let blob_id = {
                let mut objects = self.device().kms_objects().write();
                objects.add_object(DrmKmsObject::Blob(DrmPropertyBlob::new(data)))?
            };

            self.blob_ids.lock().push(blob_id);

            args.blob_id = blob_id;
            args_ptr.write(&args)?;
            Ok(())
        })?;

        Ok(0)
    }

    pub(super) fn ioctl_mode_destroy_blob(&self, cmd: DrmIoctlModeDestroyPropBlob) -> Result<i32> {
        let args: DrmModeDestroyBlob = cmd.read()?;

        let mut blob_ids = self.blob_ids.lock();
        let Some(index) = blob_ids.iter().position(|id| *id == args.blob_id) else {
            return_errno!(Errno::EPERM);
        };

        let mut objects = self.device().kms_objects().write();
        objects.remove_blob(args.blob_id).ok_or(Errno::ENOENT)?;

        blob_ids.remove(index);

        Ok(0)
    }
}
