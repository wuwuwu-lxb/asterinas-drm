// SPDX-License-Identifier: MPL-2.0

use alloc::vec;

use crate::kms::object::{
    DrmKmsObjectType,
    plane::DrmPlaneType,
    property::{DrmProperty, DrmPropertyEnum, DrmPropertyFlags, DrmPropertySpec},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrmPlaneProps {
    Type,
    InFormats,
    #[expect(dead_code)]
    SrcX,
    #[expect(dead_code)]
    SrcY,
    #[expect(dead_code)]
    SrcW,
    #[expect(dead_code)]
    SrcH,
    #[expect(dead_code)]
    CrtcX,
    #[expect(dead_code)]
    CrtcY,
    #[expect(dead_code)]
    CrtcW,
    #[expect(dead_code)]
    CrtcH,
    #[expect(dead_code)]
    FbId,
    #[expect(dead_code)]
    CrtcId,
}

impl DrmPropertySpec for DrmPlaneProps {
    fn name(&self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::InFormats => "IN_FORMATS",
            Self::SrcX => "SRC_X",
            Self::SrcY => "SRC_Y",
            Self::SrcW => "SRC_W",
            Self::SrcH => "SRC_H",
            Self::CrtcX => "CRTC_X",
            Self::CrtcY => "CRTC_Y",
            Self::CrtcW => "CRTC_W",
            Self::CrtcH => "CRTC_H",
            Self::FbId => "FB_ID",
            Self::CrtcId => "CRTC_ID",
        }
    }

    fn build(&self) -> DrmProperty {
        match self {
            Self::Type => DrmProperty::create_enum(
                self.name(),
                DrmPropertyFlags::ATOMIC,
                vec![
                    DrmPropertyEnum::new(DrmPlaneType::Primary as u64, "Primary"),
                    DrmPropertyEnum::new(DrmPlaneType::Overlay as u64, "Overlay"),
                    DrmPropertyEnum::new(DrmPlaneType::Cursor as u64, "Cursor"),
                ],
            ),
            Self::InFormats => DrmProperty::create_blob(
                self.name(),
                DrmPropertyFlags::ATOMIC | DrmPropertyFlags::IMMUTABLE,
            ),
            Self::SrcX
            | Self::SrcY
            | Self::SrcW
            | Self::SrcH
            | Self::CrtcX
            | Self::CrtcY
            | Self::CrtcW
            | Self::CrtcH => {
                DrmProperty::create_range(self.name(), DrmPropertyFlags::ATOMIC, 0, u32::MAX as u64)
            }
            Self::FbId | Self::CrtcId => {
                let object_type = match self {
                    Self::FbId => DrmKmsObjectType::Framebuffer,
                    Self::CrtcId => DrmKmsObjectType::Crtc,
                    _ => unreachable!(),
                };
                DrmProperty::create_object(self.name(), DrmPropertyFlags::ATOMIC, object_type)
            }
        }
    }
}
