// SPDX-License-Identifier: MPL-2.0

use crate::kms::object::property::{DrmProperty, DrmPropertyFlags, DrmPropertySpec};

#[expect(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrmCrtcProps {
    Active,
    ModeId,
}

impl DrmPropertySpec for DrmCrtcProps {
    fn name(&self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::ModeId => "MODE_ID",
        }
    }

    fn build(&self) -> DrmProperty {
        match self {
            Self::Active => DrmProperty::create_bool(self.name(), DrmPropertyFlags::ATOMIC),
            Self::ModeId => DrmProperty::create_blob(self.name(), DrmPropertyFlags::ATOMIC),
        }
    }
}
