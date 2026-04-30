// SPDX-License-Identifier: MPL-2.0

use alloc::{boxed::Box, vec, vec::Vec};

use hashbrown::HashMap;

use crate::{
    DrmConnType, DrmEncoderType, DrmError, DrmKmsObjectType, DrmPlaneType,
    kms::object::{
        DrmKmsObject, DrmKmsObjectStore, KmsObjectIndex,
        connector::DrmConnector,
        crtc::DrmCrtc,
        display::DrmDisplayFormat,
        encoder::DrmEncoder,
        plane::{DrmPlane, property::DrmPlaneProps},
        property::{
            DrmKmsObjectProp, DrmPropertyKind, DrmPropertySpec, KmsObjectPropValue,
            blob::DrmPropertyBlob,
        },
    },
};

#[derive(Debug)]
struct PendingObjectProp {
    spec: Box<dyn DrmPropertySpec>,
    value: PendingObjectPropValue,
}

#[derive(Debug)]
pub enum PendingObjectPropValue {
    Value(KmsObjectPropValue),
    Blob(DrmPropertyBlob),
}

#[derive(Debug)]
struct PendingPlane {
    type_: DrmPlaneType,
    format_types: Vec<DrmDisplayFormat>,
    attached_crtcs: Vec<KmsObjectIndex>,
    property_values: Vec<PendingObjectProp>,
}

impl PendingPlane {
    fn new(type_: DrmPlaneType, format_types: Vec<DrmDisplayFormat>) -> Self {
        use DrmPlaneProps::*;

        // Same Linux-compatible default property construction pattern.
        //
        // For mutable atomic properties, the initial value recorded here is only a
        // placeholder used when attaching the property to the object.
        // Their current value is expected to come from the typed object state at query
        // time, matching modern Linux DRM atomic semantics.
        // Only immutable/static properties rely on the attached value itself.
        // For example, `CRTC_ID` is read from `DrmPlaneState`, while immutable/static
        // properties such as `type` still rely on the attached value itself.
        let property_values = vec![
            PendingObjectProp {
                spec: Box::new(Type),
                value: PendingObjectPropValue::Value(type_ as u64),
            },
            PendingObjectProp {
                spec: Box::new(InFormats),
                value: PendingObjectPropValue::Blob(DrmPropertyBlob::encode_in_formats(
                    &format_types,
                )),
            },
        ];

        Self {
            type_,
            format_types,
            attached_crtcs: Vec::new(),
            property_values,
        }
    }
}

#[derive(Debug)]
struct PendingCrtc {
    gamma_size_px: u32,
    primary_plane: KmsObjectIndex,
    cursor_plane: Option<KmsObjectIndex>,
    property_values: Vec<PendingObjectProp>,
}

impl PendingCrtc {
    // Same design as `PendingPlane::new().
    fn new(
        gamma_size_px: u32,
        primary_plane: KmsObjectIndex,
        cursor_plane: Option<KmsObjectIndex>,
    ) -> Self {
        let property_values = vec![];

        Self {
            gamma_size_px,
            primary_plane,
            cursor_plane,
            property_values,
        }
    }
}

#[derive(Debug)]
struct PendingEncoder {
    type_: DrmEncoderType,
    attached_crtcs: Vec<KmsObjectIndex>,
}

#[derive(Debug)]
struct PendingConnector {
    type_: DrmConnType,
    attached_encoders: Vec<KmsObjectIndex>,
    property_values: Vec<PendingObjectProp>,
}

impl PendingConnector {
    // Same design as `PendingPlane::new().
    fn new(type_: DrmConnType) -> Self {
        let property_values = vec![];

        Self {
            type_,
            attached_encoders: Vec::new(),
            property_values,
        }
    }
}

/// Collects KMS object topology during driver initialization.
///
/// The builder is an init-only helper.
/// Drivers first declare planes, CRTCs, encoders, and connectors,
/// then attach their static topology,
/// and finally call `build()` to validate the topology
/// and materialize a `DrmKmsObjectStore`.
///
/// Typical usage:
///
/// ```ignore
/// let mut builder = DrmKmsObjectBuilder::default();
///
/// let primary = builder.add_plane(DrmPlaneType::Primary);
/// let crtc = builder.add_crtc(0, primary, None);
/// let encoder = builder.add_encoder(DrmEncoderType::VIRTUAL);
/// let connector = builder.add_connector(DrmConnType::VIRTUAL);
///
/// builder.plane_attach_crtc(primary, crtc)?;
/// builder.encoder_attach_crtc(encoder, crtc)?;
/// builder.connector_attach_encoder(connector, encoder)?;
///
/// let objects = builder.build()?;
/// ```
///
/// The builder only records static topology.
/// Dynamic runtime state remains inside each final KMS object state.
/// All typed indices must come from the same builder instance.
///
/// Current topology constraints:
///
/// - Each CRTC must reference one primary plane.
/// - The primary plane of a CRTC must have type `Primary`.
/// - If a CRTC has a cursor plane, it must have type `Cursor`.
/// - A primary or cursor plane must also be attached to that CRTC
///   through `plane_attach_crtc()`.
/// - Encoders may attach to one or more CRTCs.
/// - Connectors may attach to one or more encoders.
/// - All topology validation is deferred until `build()`.
///
/// This pattern also supports future driver-private extensions.
/// For example, a future `CustomPlane` may store a `KmsObjectIndex`
/// instead of owning a `DrmPlane`.
/// After the builder creates the object store,
/// the driver can resolve the typed index into the core object:
///
/// ```rust,ignore
/// let plane_id = objects.plane_id(custom_plane.plane_index()).unwrap();
/// let plane = objects.get_object::<DrmPlane>(plane_id).unwrap();
/// ```
///
#[derive(Debug, Default)]
pub struct DrmKmsObjectBuilder {
    planes: Vec<PendingPlane>,
    crtcs: Vec<PendingCrtc>,
    encoders: Vec<PendingEncoder>,
    connectors: Vec<PendingConnector>,
}

impl DrmKmsObjectBuilder {
    pub fn add_plane(
        &mut self,
        type_: DrmPlaneType,
        format_types: Vec<DrmDisplayFormat>,
    ) -> KmsObjectIndex {
        let pending = PendingPlane::new(type_, format_types);
        let index = self.planes.len();

        self.planes.push(pending);
        index
    }

    pub fn add_crtc(
        &mut self,
        gamma_size_px: u32,
        primary_plane: KmsObjectIndex,
        cursor_plane: Option<KmsObjectIndex>,
    ) -> KmsObjectIndex {
        let pending = PendingCrtc::new(gamma_size_px, primary_plane, cursor_plane);
        let index = self.crtcs.len();
        self.crtcs.push(pending);
        index
    }

    pub fn add_encoder(&mut self, type_: DrmEncoderType) -> KmsObjectIndex {
        let pending = PendingEncoder {
            type_,
            attached_crtcs: Vec::new(),
        };
        let index = self.encoders.len();
        self.encoders.push(pending);
        index
    }

    pub fn add_connector(&mut self, type_: DrmConnType) -> KmsObjectIndex {
        let pending = PendingConnector::new(type_);
        let index = self.connectors.len();
        self.connectors.push(pending);
        index
    }

    pub fn plane_attach_crtc(
        &mut self,
        plane: KmsObjectIndex,
        crtc: KmsObjectIndex,
    ) -> Result<(), DrmError> {
        let pending_plane = self.planes.get_mut(plane).ok_or(DrmError::Invalid)?;
        let attached_crtcs = &mut pending_plane.attached_crtcs;
        if !attached_crtcs.contains(&crtc) {
            attached_crtcs.push(crtc);
        }

        Ok(())
    }

    pub fn encoder_attach_crtc(
        &mut self,
        encoder: KmsObjectIndex,
        crtc: KmsObjectIndex,
    ) -> Result<(), DrmError> {
        let pending_encoder = self.encoders.get_mut(encoder).ok_or(DrmError::Invalid)?;
        let attached_crtcs = &mut pending_encoder.attached_crtcs;
        if !attached_crtcs.contains(&crtc) {
            attached_crtcs.push(crtc);
        }

        Ok(())
    }

    pub fn connector_attach_encoder(
        &mut self,
        connector: KmsObjectIndex,
        encoder: KmsObjectIndex,
    ) -> Result<(), DrmError> {
        let pending_connector = self
            .connectors
            .get_mut(connector)
            .ok_or(DrmError::Invalid)?;
        let attached_encoders = &mut pending_connector.attached_encoders;
        if !attached_encoders.contains(&encoder) {
            attached_encoders.push(encoder);
        }

        Ok(())
    }

    pub fn add_property(
        &mut self,
        index: KmsObjectIndex,
        property_spec: Box<dyn DrmPropertySpec>,
        initial_value: PendingObjectPropValue,
        type_: DrmKmsObjectType,
    ) -> Result<(), DrmError> {
        let property_values = match type_ {
            DrmKmsObjectType::Crtc => {
                let crtc = self.crtcs.get_mut(index).ok_or(DrmError::Invalid)?;
                &mut crtc.property_values
            }
            DrmKmsObjectType::Connector => {
                let connector = self.connectors.get_mut(index).ok_or(DrmError::Invalid)?;
                &mut connector.property_values
            }
            DrmKmsObjectType::Plane => {
                let plane = self.planes.get_mut(index).ok_or(DrmError::Invalid)?;
                &mut plane.property_values
            }
            _ => return Err(DrmError::NotSupported),
        };

        add_or_override_property(property_values, property_spec, initial_value);

        Ok(())
    }

    pub fn build(self) -> Result<DrmKmsObjectStore, DrmError> {
        self.validate_topology()?;

        let mut store = DrmKmsObjectStore::new();
        let mut next_type_index_by_connector_type = HashMap::<DrmConnType, u32>::new();

        for plane in &self.planes {
            let property = build_object_properties(&mut store, &plane.property_values)?;

            let object = DrmKmsObject::Plane(DrmPlane::new(
                plane.type_,
                plane.format_types.clone(),
                &plane.attached_crtcs,
                property,
            ));
            let _ = store.add_object(object)?;
        }

        for crtc in &self.crtcs {
            let primary_plane_id = store
                .get_object_id_from_index(crtc.primary_plane, DrmKmsObjectType::Plane)
                .ok_or(DrmError::Invalid)?;
            let cursor_plane_id = match crtc.cursor_plane {
                Some(cursor_plane) => Some(
                    store
                        .get_object_id_from_index(cursor_plane, DrmKmsObjectType::Plane)
                        .ok_or(DrmError::Invalid)?,
                ),
                None => None,
            };

            let property = build_object_properties(&mut store, &crtc.property_values)?;

            let object = DrmKmsObject::Crtc(DrmCrtc::new(
                crtc.gamma_size_px,
                primary_plane_id,
                cursor_plane_id,
                property,
            ));
            let _ = store.add_object(object)?;
        }

        for encoder in &self.encoders {
            let object =
                DrmKmsObject::Encoder(DrmEncoder::new(encoder.type_, &encoder.attached_crtcs));
            let _ = store.add_object(object)?;
        }

        for connector in &self.connectors {
            let next_type_index = next_type_index_by_connector_type
                .entry(connector.type_)
                .or_insert(0);
            let type_index = *next_type_index;
            *next_type_index = (*next_type_index).checked_add(1).ok_or(DrmError::Invalid)?;

            let property = build_object_properties(&mut store, &connector.property_values)?;

            let object = DrmKmsObject::Connector(DrmConnector::new(
                connector.type_,
                type_index,
                &connector.attached_encoders,
                property,
            ));
            let _ = store.add_object(object)?;
        }

        Ok(store)
    }

    fn validate_topology(&self) -> Result<(), DrmError> {
        if self.crtcs.is_empty() {
            return Err(DrmError::Invalid);
        }

        for crtc in self.crtcs.iter() {
            self.validate_plane_type(crtc.primary_plane, DrmPlaneType::Primary)?;
            if let Some(cursor_plane) = crtc.cursor_plane {
                self.validate_plane_type(cursor_plane, DrmPlaneType::Cursor)?;
            }
        }

        Ok(())
    }

    fn validate_plane_type(
        &self,
        plane: KmsObjectIndex,
        type_: DrmPlaneType,
    ) -> Result<(), DrmError> {
        let plane = self.planes.get(plane).ok_or(DrmError::Invalid)?;

        if plane.type_ != type_ {
            return Err(DrmError::Invalid);
        }

        Ok(())
    }
}

fn add_or_override_property(
    property_values: &mut Vec<PendingObjectProp>,
    property_spec: Box<dyn DrmPropertySpec>,
    value: PendingObjectPropValue,
) {
    let property_name = property_spec.name();

    if let Some(existing) = property_values
        .iter_mut()
        .find(|pending| pending.spec.name() == property_name)
    {
        existing.spec = property_spec;
        existing.value = value;
    } else {
        property_values.push(PendingObjectProp {
            spec: property_spec,
            value,
        });
    }
}

fn build_object_properties(
    store: &mut DrmKmsObjectStore,
    property_values: &[PendingObjectProp],
) -> Result<DrmKmsObjectProp, DrmError> {
    let mut properties = DrmKmsObjectProp::default();

    for pending_property in property_values {
        let property = pending_property.spec.build();

        // Blob-typed properties do not store the raw blob payload directly in
        // the object property map. Instead, the payload is first materialized
        // as a `DrmModeBlob` object in the store, and the final property value
        // becomes that blob object's ID, matching Linux DRM semantics.
        let prop_value = match (property.kind(), &pending_property.value) {
            (DrmPropertyKind::Blob, PendingObjectPropValue::Blob(data)) => {
                store.add_object(DrmKmsObject::Blob(data.clone()))? as u64
            }
            (_, PendingObjectPropValue::Value(value)) => *value,
            _ => return Err(DrmError::Invalid),
        };

        properties.add_property(
            store.add_object(DrmKmsObject::Property(property))?,
            prop_value,
        );
    }

    Ok(properties)
}
