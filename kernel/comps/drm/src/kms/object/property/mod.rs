// SPDX-License-Identifier: MPL-2.0

use alloc::vec::Vec;
use core::fmt::Debug;

use hashbrown::HashMap;

use crate::kms::object::{DrmKmsObject, DrmKmsObjectCast, DrmKmsObjectType, KmsObjectId};

pub mod blob;

pub const DRM_PROP_NAME_LEN: usize = 32;
pub type KmsObjectPropValue = u64;

/// Stores the property attachments of a KMS object as `property_id -> value`.
///
/// In modern atomic DRM semantics, it should be treated primarily as the
/// userspace-facing property attachment table. Immutable or static properties
/// may rely directly on the stored value, while mutable atomic properties are
/// expected to derive their current value from the typed KMS object state.
#[derive(Debug, Default, Clone)]
pub struct DrmKmsObjectProp(HashMap<KmsObjectId, KmsObjectPropValue>);

impl DrmKmsObjectProp {
    pub fn add_property(&mut self, id: KmsObjectId, value: KmsObjectPropValue) {
        self.0.insert(id, value);
    }

    pub fn entries(&self) -> Vec<(KmsObjectId, KmsObjectPropValue)> {
        self.0.iter().map(|(id, value)| (*id, *value)).collect()
    }

    pub fn ids(&self) -> Vec<KmsObjectId> {
        self.0.keys().copied().collect()
    }

    pub fn values(&self) -> Vec<KmsObjectPropValue> {
        self.0.values().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

bitflags::bitflags! {
    pub struct DrmPropertyFlags: u32 {
        const PENDING    = 1 << 0; // deprecated
        const RANGE      = 1 << 1;
        const IMMUTABLE  = 1 << 2;
        const ENUM       = 1 << 3;
        const BLOB       = 1 << 4;
        const BITMASK    = 1 << 5;
        const OBJECT     = 1 << 6;
        const SIGNED_RANGE = 1 << 7;

        const ATOMIC     = 0x8000_0000;
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod)]
pub struct DrmPropertyEnum {
    pub value: u64,
    pub name: [u8; DRM_PROP_NAME_LEN],
}

impl DrmPropertyEnum {
    pub fn new(value: u64, name: &'static str) -> Self {
        Self {
            value,
            name: str_to_u8(name),
        }
    }
}

#[derive(Debug, Clone)]
pub enum DrmPropertyKind {
    Plain,
    Range { min: u64, max: u64 },
    SignedRange { min: i64, max: i64 },
    Enum(Vec<DrmPropertyEnum>),
    Bitmask(Vec<DrmPropertyEnum>),
    Blob,
    Object(DrmKmsObjectType),
}

/// Describes a DRM property definition attached to a KMS object.
///
/// In the atomic DRM model, a property is the userspace-visible configuration
/// entry point: it defines the property's name, type, and constraints, and is
/// used to address state updates through `(object_id, property_id, value)`.
/// But a property does not, by itself, act as the kernel's single source of truth
/// for mutable state. Instead, mutable property values such as `CRTC_ID`,
/// `FB_ID`, or `SRC_X` are expected to be carried by the typed KMS object
/// state, while immutable or static properties may rely on their attached
/// value directly.
#[derive(Debug, Clone)]
pub struct DrmProperty {
    name: &'static str,
    flags: DrmPropertyFlags,
    kind: DrmPropertyKind,
}

impl DrmProperty {
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn name_to_u8(&self) -> [u8; DRM_PROP_NAME_LEN] {
        str_to_u8(self.name)
    }

    pub fn flags(&self) -> &DrmPropertyFlags {
        &self.flags
    }

    pub fn kind(&self) -> &DrmPropertyKind {
        &self.kind
    }

    pub fn create(name: &'static str, flags: DrmPropertyFlags) -> Self {
        Self {
            name,
            flags,
            kind: DrmPropertyKind::Plain,
        }
    }

    pub fn create_blob(name: &'static str, flags: DrmPropertyFlags) -> Self {
        Self {
            name,
            flags: flags | DrmPropertyFlags::BLOB,
            kind: DrmPropertyKind::Blob,
        }
    }

    pub fn create_object(
        name: &'static str,
        flags: DrmPropertyFlags,
        object_type: DrmKmsObjectType,
    ) -> Self {
        Self {
            name,
            flags: flags | DrmPropertyFlags::OBJECT,
            kind: DrmPropertyKind::Object(object_type),
        }
    }

    pub fn create_bool(name: &'static str, flags: DrmPropertyFlags) -> Self {
        Self::create_range(name, flags, 0, 1)
    }

    pub fn create_range(name: &'static str, flags: DrmPropertyFlags, min: u64, max: u64) -> Self {
        Self {
            name,
            flags: flags | DrmPropertyFlags::RANGE,
            kind: DrmPropertyKind::Range { min, max },
        }
    }

    pub fn create_enum(
        name: &'static str,
        flags: DrmPropertyFlags,
        enums: Vec<DrmPropertyEnum>,
    ) -> Self {
        Self {
            name,
            flags: flags | DrmPropertyFlags::ENUM,
            kind: DrmPropertyKind::Enum(enums),
        }
    }
}

impl DrmKmsObjectCast for DrmProperty {
    fn cast(obj: &DrmKmsObject) -> Option<&Self> {
        if let DrmKmsObject::Property(p) = obj {
            Some(p)
        } else {
            None
        }
    }
}

/// Describes a property definition that can be attached to a KMS object.
///
/// The KMS object builder uses this trait to create standard or driver-defined
/// properties in a uniform way, and to match properties by their stable
/// userspace-visible name during initialization.
pub trait DrmPropertySpec: Debug {
    /// Provides the stable userspace-visible property name.
    fn name(&self) -> &'static str;
    /// Materializes the final `DrmProperty` definition that will be
    /// registered in the KMS object store.
    fn build(&self) -> DrmProperty;
}

fn str_to_u8(s: &str) -> [u8; DRM_PROP_NAME_LEN] {
    let mut buf = [0u8; DRM_PROP_NAME_LEN];

    let bytes = s.as_bytes();
    let len = bytes.len().min(DRM_PROP_NAME_LEN - 1);

    buf[..len].copy_from_slice(&bytes[..len]);
    buf
}
