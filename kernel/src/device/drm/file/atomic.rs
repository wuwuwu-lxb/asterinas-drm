// SPDX-License-Identifier: MPL-2.0

use core::sync::atomic::Ordering;

use aster_drm::{DrmAtomicFlags, DrmAtomicObjectRequest};

use crate::{
    device::drm::{
        file::{DrmFile, copy_array_from_user, user_array_ptr_at},
        ioctl::*,
    },
    prelude::*,
};

impl DrmFile {
    pub(super) fn ioctl_mode_atomic(&self, cmd: DrmIoctlModeAtomic) -> Result<i32> {
        if !self.caps.has_atomic.load(Ordering::Relaxed) {
            return_errno!(Errno::EINVAL);
        }

        let args = cmd.read()?;
        if args.reserved != 0 {
            return_errno!(Errno::EINVAL);
        }

        cmd.with_data_ptr(|args_ptr| {
            let object_prop_counts =
                copy_array_from_user::<u32>(args_ptr.vm(), args.count_props_ptr, args.count_objs)?;
            let object_ids =
                copy_array_from_user::<u32>(args_ptr.vm(), args.objs_ptr, args.count_objs)?;

            let mut prop_index = 0u32;
            let mut atomic_requests = Vec::new();
            for (object_id, prop_count) in object_ids.iter().zip(object_prop_counts) {
                let mut atomic_request = DrmAtomicObjectRequest::new(*object_id);

                let prop_id_ptr = user_array_ptr_at::<u32>(args.props_ptr, prop_index as usize)?;
                let prop_value_ptr =
                    user_array_ptr_at::<u64>(args.prop_values_ptr, prop_index as usize)?;
                let prop_ids = copy_array_from_user::<u32>(args_ptr.vm(), prop_id_ptr, prop_count)?;
                let prop_values =
                    copy_array_from_user::<u64>(args_ptr.vm(), prop_value_ptr, prop_count)?;

                for (prop_id, prop_value) in prop_ids.iter().zip(prop_values) {
                    atomic_request.add_property(*prop_id, prop_value);
                }

                atomic_requests.push(atomic_request);

                prop_index = prop_index.checked_add(prop_count).ok_or(Errno::EOVERFLOW)?;
            }

            let flags = DrmAtomicFlags::from_bits(args.flags).ok_or(Errno::EINVAL)?;
            self.device().atomic_commit_request(
                atomic_requests,
                flags,
                args.user_data,
                self.events.clone(),
            )?;

            Ok(())
        })?;

        Ok(0)
    }
}
