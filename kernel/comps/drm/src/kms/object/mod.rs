// SPDX-License-Identifier: MPL-2.0

use alloc::vec::Vec;
use core::{
    fmt::Debug,
    sync::atomic::{AtomicU32, Ordering},
};

use hashbrown::HashMap;

use crate::{
    DrmError,
    kms::object::{connector::DrmConnector, crtc::DrmCrtc, encoder::DrmEncoder, plane::DrmPlane},
};

pub mod builder;
pub mod connector;
pub mod crtc;
pub mod display;
pub mod encoder;
pub mod geometry;
pub mod plane;

pub type KmsObjectId = u32;
pub type KmsObjectIndex = usize;

pub trait DrmKmsObjectCast: Debug {
    fn cast(obj: &DrmKmsObject) -> Option<&Self>;
}

#[derive(Debug)]
pub enum DrmKmsObject {
    Plane(DrmPlane),
    Crtc(DrmCrtc),
    Encoder(DrmEncoder),
    Connector(DrmConnector),
}

#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DrmKmsObjectType {
    Any = 0,
    Crtc = 0xCCCC_CCCC,
    Connector = 0xC0C0_C0C0,
    Encoder = 0xE0E0_E0E0,
    Mode = 0xDEDE_DEDE,
    Property = 0xB0B0_B0B0,
    Framebuffer = 0xFBFB_FBFB,
    Blob = 0xBBBB_BBBB,
    Plane = 0xEEEE_EEEE,
}

impl TryFrom<u32> for DrmKmsObjectType {
    type Error = DrmError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Any),
            0xCCCC_CCCC => Ok(Self::Crtc),
            0xC0C0_C0C0 => Ok(Self::Connector),
            0xE0E0_E0E0 => Ok(Self::Encoder),
            0xDEDE_DEDE => Ok(Self::Mode),
            0xB0B0_B0B0 => Ok(Self::Property),
            0xFBFB_FBFB => Ok(Self::Framebuffer),
            0xBBBB_BBBB => Ok(Self::Blob),
            0xEEEE_EEEE => Ok(Self::Plane),
            _ => Err(DrmError::Invalid),
        }
    }
}

/// Stores finalized KMS objects and their global object IDs.
///
/// Drivers typically construct a store through `DrmKmsObjectBuilder`
/// instead of registering topology manually.
/// The builder validates the static topology, creates the final KMS objects,
/// and inserts them into this store.
///
/// Locking rule:
/// callers should acquire the outer store lock
/// before taking any individual KMS object state lock.
///
#[derive(Debug)]
pub struct DrmKmsObjectStore {
    plane_ids: Vec<KmsObjectId>,
    crtc_ids: Vec<KmsObjectId>,
    encoder_ids: Vec<KmsObjectId>,
    connector_ids: Vec<KmsObjectId>,
    object_by_id: HashMap<KmsObjectId, DrmKmsObject>,
    next_object_id: AtomicU32,
}

impl DrmKmsObjectStore {
    pub fn new() -> Self {
        Self {
            plane_ids: Vec::new(),
            crtc_ids: Vec::new(),
            encoder_ids: Vec::new(),
            connector_ids: Vec::new(),
            object_by_id: HashMap::new(),
            next_object_id: AtomicU32::new(1),
        }
    }

    pub fn alloc_object_id(&mut self) -> KmsObjectId {
        self.next_object_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn collect_object_ids(
        &self,
        type_: DrmKmsObjectType,
        mask: Option<u32>,
    ) -> Vec<KmsObjectId> {
        let res: &[KmsObjectId] = match type_ {
            DrmKmsObjectType::Crtc => &self.crtc_ids,
            DrmKmsObjectType::Connector => &self.connector_ids,
            DrmKmsObjectType::Encoder => &self.encoder_ids,
            DrmKmsObjectType::Plane => &self.plane_ids,
            _ => &[],
        };

        res.iter()
            .enumerate()
            .filter(move |(i, _)| match mask {
                None => true,
                Some(m) => (m & (1 << i)) != 0,
            })
            .map(|(_, id)| *id)
            .collect()
    }

    pub fn add_object(&mut self, object: DrmKmsObject) -> Result<KmsObjectId, DrmError> {
        let id = self.alloc_object_id();

        match &object {
            DrmKmsObject::Plane(_) => self.plane_ids.push(id),
            DrmKmsObject::Crtc(_) => self.crtc_ids.push(id),
            DrmKmsObject::Encoder(_) => self.encoder_ids.push(id),
            DrmKmsObject::Connector(_) => self.connector_ids.push(id),
        }

        self.object_by_id.insert(id, object);
        Ok(id)
    }

    pub fn get_object_id_from_index(
        &self,
        index: KmsObjectIndex,
        type_: DrmKmsObjectType,
    ) -> Option<KmsObjectId> {
        match type_ {
            DrmKmsObjectType::Crtc => self.crtc_ids.get(index).copied(),
            DrmKmsObjectType::Connector => self.connector_ids.get(index).copied(),
            DrmKmsObjectType::Encoder => self.encoder_ids.get(index).copied(),
            DrmKmsObjectType::Plane => self.plane_ids.get(index).copied(),
            _ => None,
        }
    }

    pub fn get_object<T: DrmKmsObjectCast>(&self, id: KmsObjectId) -> Option<&T> {
        let obj = self.object_by_id.get(&id)?;
        T::cast(obj)
    }
}
