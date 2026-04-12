// SPDX-License-Identifier: MPL-2.0

use core::mem::offset_of;

use aster_util::safe_ptr::SafePtr;
use ostd_pod::FromZeros;

use crate::transport::{ConfigManager, VirtioTransport};

bitflags::bitflags! {
    pub(super) struct GpuFeatures: u64 {
        /// Display: 2D mode via EDID
        const VIRTIO_GPU_F_EDID = 1 << 0;
        /// Resource UUID
        const VIRTIO_GPU_F_RESOURCE_UUID = 1 << 1;
        /// Blob resources
        const VIRTIO_GPU_F_RESOURCE_BLOB = 1 << 2;
        /// Context init
        const VIRTIO_GPU_F_CONTEXT_INIT = 1 << 3;
    }
}

/// Virtio GPU configuration structure
#[derive(Debug, Pod, Clone, Copy)]
#[repr(C)]
pub(super) struct VirtioGpuConfig {
    /// Number of scanouts supported
    pub num_scanouts: u32,
    /// Number of capability sets
    pub num_capsets: u32,
}

impl VirtioGpuConfig {
    pub(super) fn new_manager(transport: &dyn VirtioTransport) -> ConfigManager<Self> {
        let safe_ptr = transport
            .device_config_mem()
            .map(|mem| SafePtr::new(mem, 0));
        let bar_space = transport.device_config_bar();
        ConfigManager::new(safe_ptr, bar_space)
    }
}

impl ConfigManager<VirtioGpuConfig> {
    pub(super) fn read_config(&self) -> VirtioGpuConfig {
        let mut config = VirtioGpuConfig::new_zeroed();
        config.num_scanouts = self
            .read_once::<u32>(offset_of!(VirtioGpuConfig, num_scanouts))
            .unwrap();
        config.num_capsets = self
            .read_once::<u32>(offset_of!(VirtioGpuConfig, num_capsets))
            .unwrap();
        config
    }
}
