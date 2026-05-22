// SPDX-License-Identifier: MPL-2.0

use alloc::vec;

use crate::kms::object::{
    DrmKmsObjectType,
    property::{DrmProperty, DrmPropertyEnum, DrmPropertyFlags, DrmPropertySpec},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrmConnectorProps {
    #[expect(dead_code)]
    Edid,
    #[expect(dead_code)]
    Path,
    #[expect(dead_code)]
    HdrOutputMetadata,
    CrtcId,
    #[expect(dead_code)]
    Dpms,
    #[expect(dead_code)]
    LinkStatus,
    #[expect(dead_code)]
    NonDesktop,
    #[expect(dead_code)]
    Tile,
}

impl DrmPropertySpec for DrmConnectorProps {
    fn name(&self) -> &'static str {
        match self {
            Self::Edid => "EDID",
            Self::Path => "PATH",
            Self::HdrOutputMetadata => "HDR_OUTPUT_METADATA",
            Self::CrtcId => "CRTC_ID",
            Self::Dpms => "DPMS",
            Self::LinkStatus => "link-status",
            Self::NonDesktop => "non-desktop",
            Self::Tile => "TILE",
        }
    }

    fn build(&self) -> DrmProperty {
        match self {
            Self::Edid => DrmProperty::create_blob(self.name(), DrmPropertyFlags::IMMUTABLE),
            Self::Path => DrmProperty::create_blob(self.name(), DrmPropertyFlags::IMMUTABLE),
            Self::HdrOutputMetadata => {
                DrmProperty::create_blob(self.name(), DrmPropertyFlags::ATOMIC)
            }
            Self::CrtcId => DrmProperty::create_object(
                self.name(),
                DrmPropertyFlags::ATOMIC,
                DrmKmsObjectType::Crtc,
            ),
            Self::Dpms => DrmProperty::create_enum(
                self.name(),
                DrmPropertyFlags::empty(),
                vec![
                    DrmPropertyEnum::new(0, "On"),
                    DrmPropertyEnum::new(1, "Standby"),
                    DrmPropertyEnum::new(2, "Suspend"),
                    DrmPropertyEnum::new(3, "Off"),
                ],
            ),
            Self::LinkStatus => DrmProperty::create_enum(
                self.name(),
                DrmPropertyFlags::empty(),
                vec![
                    DrmPropertyEnum::new(0, "Good"),
                    DrmPropertyEnum::new(1, "Bad"),
                ],
            ),
            Self::NonDesktop => DrmProperty::create_bool(self.name(), DrmPropertyFlags::IMMUTABLE),
            Self::Tile => DrmProperty::create_blob(self.name(), DrmPropertyFlags::IMMUTABLE),
        }
    }
}
