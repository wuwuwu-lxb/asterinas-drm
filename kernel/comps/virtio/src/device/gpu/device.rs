// SPDX-License-Identifier: MPL-2.0

use alloc::{boxed::Box, sync::Arc};
use core::hint::spin_loop;

use log::info;
use ostd::{
    arch::trap::TrapFrame,
    mm::{CachePolicy, VmIo},
    sync::SpinLock,
};

use super::{config::VirtioGpuConfig, commands::*, DEVICE_NAME};
use crate::{
    device::{VirtioDeviceError, gpu::config::GpuFeatures},
    queue::VirtQueue,
    transport::{ConfigManager, VirtioTransport},
};

/// The virtio GPU device.
pub struct GpuDevice {
    config_manager: ConfigManager<VirtioGpuConfig>,
    transport: SpinLock<Box<dyn VirtioTransport>>,
    /// Control queue for sending commands
    control_queue: SpinLock<VirtQueue>,
}

impl GpuDevice {
    pub(crate) fn negotiate_features(features: u64) -> u64 {
        let mut features = GpuFeatures::from_bits_truncate(features);
        // Filter features based on what we support
        // For now, we support basic 2D mode
        features.remove(GpuFeatures::VIRTIO_GPU_F_RESOURCE_UUID);
        features.remove(GpuFeatures::VIRTIO_GPU_F_RESOURCE_BLOB);
        features.remove(GpuFeatures::VIRTIO_GPU_F_CONTEXT_INIT);
        features.bits()
    }

    pub(crate) fn init(mut transport: Box<dyn VirtioTransport>) -> Result<(), VirtioDeviceError> {
        let config_manager = VirtioGpuConfig::new_manager(transport.as_ref());
        info!("virtio_gpu_config = {:?}", config_manager.read_config());

        // Control queue is at index 0 for virtio-gpu
        const CONTROL_QUEUE_INDEX: u16 = 0;
        let control_queue =
            SpinLock::new(VirtQueue::new(CONTROL_QUEUE_INDEX, 64, transport.as_mut()).unwrap());

        let device = Arc::new(Self {
            config_manager,
            transport: SpinLock::new(transport),
            control_queue,
        });

        // Register IRQ callback for control queue
        let mut transport = device.transport.lock();
        let handle_gpu_ctrl = {
            let device = device.clone();
            move |_: &TrapFrame| device.handle_ctrl_irq()
        };
        transport
            .register_queue_callback(CONTROL_QUEUE_INDEX, Box::new(handle_gpu_ctrl), false)
            .unwrap();
        transport.finish_init();
        drop(transport);

        info!("[Virtio-GPU] device initialized");

        Ok(())
    }

    fn handle_ctrl_irq(&self) {
        let mut control_queue = self.control_queue.disable_irq().lock();

        while let Ok((desc, len)) = control_queue.pop_used() {
            // Process response - for now just mark as consumed
            info!("[Virtio-GPU] control queue response processed, len = {}", len);
        }
    }
}
