// SPDX-License-Identifier: MPL-2.0

use alloc::vec::Vec;
use core::{
    mem,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

use aster_drm::{DrmDevice, DrmFeatures, DrmGemObject};
use hashbrown::HashMap;
use ostd::mm::VmIo;

use crate::{
    device::drm::{
        DrmMinorType, file::kms::DrmFileEvent, has_current_sys_admin, ioctl::*, minor::DrmMinor,
    },
    events::IoEvents,
    fs::{
        file::{FileIo, Mappable, StatusFlags},
        vfs::inode::InodeIo,
    },
    prelude::*,
    process::{
        Process,
        signal::{PollHandle, Pollable, Poller},
    },
    util::ioctl::RawIoctl,
};

mod atomic;
mod gem;
mod kms;

#[derive(Debug, Default)]
struct DrmFileCaps {
    /// True when the client has asked us to expose stereo 3D mode flags.
    has_stereo: AtomicBool,
    /// True if client understands CRTC primary planes and cursor planes
    /// in the plane list. Automatically set when atomic is set.
    has_universal_planes: AtomicBool,
    /// True if client understands atomic properties.
    has_atomic: AtomicBool,
    /// True, if client can handle picture aspect ratios, and has requested
    /// to pass this information along with the mode.
    has_aspect_ratio: AtomicBool,
    /// True if client understands writeback connectors.
    has_writeback_connectors: AtomicBool,
    /// This client is capable of handling the cursor plane with the
    /// restrictions imposed on it by the virtualized drivers.
    has_virtualized_cursor_plane: AtomicBool,
}

#[derive(Debug, Default)]
struct DrmFileAuthState {
    /// Tracks the current owner process for this file's master-management checks.
    ///
    /// For files that have never been master, this owner can follow the current
    /// ioctl caller (e.g., after fd passing). Once the file has been master,
    /// ownership is frozen to preserve "same process can reacquire master"
    /// semantics.
    owner_process_pid: u32,
    /// Indicates whether this file has ever successfully become DRM master.
    ///
    /// This is sticky after the first successful `SET_MASTER` and is used to
    /// gate non-root master reacquisition to the same owner process.
    was_master: bool,
    /// Tracks legacy primary-node authentication state for this file.
    ///
    /// Currently does not implement legacy auth ioctls
    /// (`DRM_IOCTL_GET_MAGIC`/`DRM_IOCTL_AUTH_MAGIC`) to transition this flag.
    /// `is_authenticated()` also treats current master as authenticated.
    authenticated: bool,
}

/// Represents an open DRM file descriptor exposed to userspace.
///
/// `DrmFile` is created on each successful `open()` of a DRM device node
/// (e.g. `/dev/dri/cardX`, `/dev/dri/renderDX`). It serves as the **per-open
/// execution context** for all userspace interactions with the DRM subsystem.
///
/// Responsibilities:
/// - Dispatching ioctl requests issued from userspace.
/// - Enforcing access restrictions and semantics defined by the associated
///   DRM minor (primary, render, control, etc.).
///
/// `DrmFile` does not own device-wide state. Instead, it holds a reference to
/// the `DrmMinor` through which it was opened, and all operations are ultimately
/// routed to the underlying `DrmDevice` shared by all minors of the same device.
///
/// Each `DrmFile` instance is independent and represents a single userspace
/// file descriptor.
///
#[derive(Debug)]
pub(super) struct DrmFile {
    file_id: u32,
    minor: Arc<DrmMinor>,
    caps: DrmFileCaps,
    auth_state: Mutex<DrmFileAuthState>,
    blob_ids: Mutex<Vec<u32>>,
    framebuffer_ids: Mutex<Vec<u32>>,

    next_gem_handle: AtomicU32,
    gem_table: Mutex<HashMap<u32, Arc<dyn DrmGemObject>>>,

    events: Arc<DrmFileEvent>,
}

impl DrmFile {
    pub(super) fn new(file_id: u32, minor: Arc<DrmMinor>) -> Self {
        let owner_process_pid = Process::current().map_or(0, |process| process.pid());
        let is_master = minor.is_master(file_id);

        let auth_state = DrmFileAuthState {
            owner_process_pid,
            was_master: is_master,
            authenticated: is_master,
        };

        Self {
            file_id,
            minor,
            caps: DrmFileCaps::default(),
            auth_state: Mutex::new(auth_state),
            blob_ids: Mutex::new(Vec::new()),
            framebuffer_ids: Mutex::new(Vec::new()),
            next_gem_handle: AtomicU32::new(1),
            gem_table: Mutex::new(HashMap::new()),
            events: Arc::new(DrmFileEvent::new()),
        }
    }

    pub(super) fn is_master(&self) -> bool {
        self.minor.is_master(self.file_id)
    }

    pub(super) fn minor_type(&self) -> DrmMinorType {
        self.minor.type_()
    }

    pub(super) fn is_authenticated(&self) -> bool {
        self.is_master() || self.auth_state.lock().authenticated
    }

    pub(super) fn has_feature(&self, feature: DrmFeatures) -> bool {
        self.device().has_feature(feature)
    }

    /// Keep tracking the ioctl caller while this file has never been master,
    /// so fd passing can update ownership. After the file has been master once,
    /// keep owner pid stable to enforce same-owner master reacquisition semantics.
    fn update_owner_process(&self) {
        let mut auth_state = self.auth_state.lock();

        if auth_state.was_master {
            return;
        }

        if let Some(process) = Process::current() {
            auth_state.owner_process_pid = process.pid();
        }
    }

    fn master_check_perm(&self) -> bool {
        if has_current_sys_admin() {
            return true;
        }

        let auth_state = self.auth_state.lock();

        let is_same_process =
            Process::current().is_some_and(|process| process.pid() == auth_state.owner_process_pid);
        let is_previous_master = auth_state.was_master;

        is_previous_master && is_same_process
    }

    fn device(&self) -> &Arc<dyn DrmDevice> {
        self.minor.device()
    }
}

impl Drop for DrmFile {
    fn drop(&mut self) {
        self.minor.drop_master(self.file_id);

        for gem_object in self
            .gem_table
            .get_mut()
            .drain()
            .map(|(_, gem_object)| gem_object)
        {
            gem_object.vma_node().revoke(self.file_id);
        }

        let blob_ids: Vec<u32> = self.blob_ids.get_mut().drain(..).collect();
        let framebuffer_ids: Vec<u32> = self.framebuffer_ids.get_mut().drain(..).collect();
        let mut objects = self.device().kms_objects().write();
        for blob_id in blob_ids {
            let _ = objects.remove_blob(blob_id);
        }
        for framebuffer_id in framebuffer_ids {
            let plane_ids = objects.collect_object_ids(aster_drm::DrmKmsObjectType::Plane, None);
            for plane_id in plane_ids {
                let Some(plane) = objects.get_object::<aster_drm::DrmPlane>(plane_id) else {
                    continue;
                };
                if plane.snapshot().fb_id() == Some(framebuffer_id) {
                    plane.set_fb_id(None);
                }
            }
            let _ = objects.remove_framebuffer(framebuffer_id);
        }
    }
}

impl Pollable for DrmFile {
    fn poll(&self, mask: IoEvents, poller: Option<&mut PollHandle>) -> IoEvents {
        self.events.pollee().poll_with(mask, poller, || {
            let mut events = IoEvents::OUT;
            if !self.events.queue().lock().is_empty() {
                events |= IoEvents::IN;
            }
            events
        })
    }
}

impl InodeIo for DrmFile {
    fn read_at(
        &self,
        _offset: usize,
        writer: &mut VmWriter,
        status_flags: StatusFlags,
    ) -> Result<usize> {
        let nonblocking = status_flags.contains(StatusFlags::O_NONBLOCK);

        if nonblocking && self.events.queue().lock().is_empty() {
            return_errno!(Errno::EAGAIN);
        }

        if !nonblocking {
            loop {
                let mut poller = Poller::new(None);
                let events = self.events.pollee().poll_with(
                    IoEvents::IN,
                    Some(poller.as_handle_mut()),
                    || {
                        if self.events.queue().lock().is_empty() {
                            IoEvents::empty()
                        } else {
                            IoEvents::IN
                        }
                    },
                );
                if events.contains(IoEvents::IN) {
                    break;
                }

                poller.wait()?;
            }
        }

        let mut queue = self.events.queue().lock();

        let mut total_written = 0usize;
        while let Some(event) = queue.front() {
            if event.len() > writer.avail() {
                if total_written == 0 {
                    // Linux DRM requires user buffer to fit the next full event.
                    return_errno!(Errno::EINVAL);
                }
                break;
            }

            let Some(event) = queue.pop_front() else {
                break;
            };
            writer.write_fallible(&mut event.as_slice().into())?;
            total_written += event.len();
        }

        if queue.is_empty() {
            self.events.pollee().invalidate();
        }

        Ok(total_written)
    }

    fn write_at(
        &self,
        _offset: usize,
        _reader: &mut VmReader,
        _status_flags: StatusFlags,
    ) -> Result<usize> {
        return_errno_with_message!(Errno::EINVAL, "drm: write not supported");
    }
}

impl FileIo for DrmFile {
    fn check_seekable(&self) -> Result<()> {
        Ok(())
    }

    fn is_offset_aware(&self) -> bool {
        true
    }

    fn mappable(&self) -> Result<&dyn Mappable> {
        Ok(self as &dyn Mappable)
    }

    fn ioctl(&self, raw_ioctl: RawIoctl) -> Result<i32> {
        self.update_owner_process();

        dispatch_drm_ioctl!(
            self,
            match raw_ioctl {
                cmd @ DrmIoctlVersion => {
                    let mut args: DrmVersion = cmd.read()?;

                    let dev = self.device();
                    let name = dev.name();
                    let name_len = name.len();
                    let desc = dev.desc();
                    let desc_len = desc.len();
                    // These fields are legacy in modern DRM userspace flows.
                    // Keep reporting them to preserve `DRM_IOCTL_VERSION` ABI compatibility.
                    let date = "0";
                    let date_len = date.len();
                    let major = 0;
                    let minor = 0;
                    let patch_level = 0;

                    cmd.with_data_ptr(|args_ptr| {
                        // Linux `drm_copy_field` semantics:
                        // copy each field independently with truncation,
                        // then always report the full source length.
                        if args.name_len != 0 {
                            let write_len = core::cmp::min(args.name_len, name_len);
                            args_ptr
                                .vm()
                                .write_bytes(args.name, &name.as_bytes()[..write_len])?;
                        }

                        if args.desc_len != 0 {
                            let write_len = core::cmp::min(args.desc_len, desc_len);
                            args_ptr
                                .vm()
                                .write_bytes(args.desc, &desc.as_bytes()[..write_len])?;
                        }

                        if args.date_len != 0 {
                            let write_len = core::cmp::min(args.date_len, date_len);
                            args_ptr
                                .vm()
                                .write_bytes(args.date, &date.as_bytes()[..write_len])?;
                        }

                        args.name_len = name_len;
                        args.desc_len = desc_len;
                        args.date_len = date_len;
                        args.version_major = major;
                        args.version_minor = minor;
                        args.version_patchlevel = patch_level;

                        args_ptr.write(&args)?;
                        Ok(())
                    })?;

                    Ok(0)
                }
                cmd @ DrmIoctlGetCap => {
                    use DrmGetCapability::*;

                    let mut args: DrmGetCap = cmd.read()?;
                    let cap = DrmGetCapability::try_from(args.capability)?;
                    let dev = self.device();

                    let value = match cap {
                        TimestampMonotonic => 1,
                        Prime => (DrmPrimeValue::IMPORT | DrmPrimeValue::EXPORT).bits(),
                        SyncObj => self.has_feature(DrmFeatures::SYNCOBJ) as u64,
                        SyncObjTimeline => self.has_feature(DrmFeatures::SYNCOBJ_TIMELINE) as u64,
                        _ => {
                            if !self.has_feature(DrmFeatures::MODESET) {
                                return_errno!(Errno::EOPNOTSUPP);
                            }

                            match cap {
                                DumbBuffer => dev.caps().has_dumb_buffer() as u64,
                                VblankHighCrtc => 1,
                                DumbPreferredDepth => dev.caps().preferred_color_depth_px() as u64,
                                DumbPreferShadow => dev.caps().prefer_shadow_buffer() as u64,
                                AsyncPageFlip => dev.caps().has_async_page_flip() as u64,
                                PageFlipTarget => dev.caps().has_flip_target() as u64,
                                CursorWidth => dev.caps().cursor_width_px() as u64,
                                CursorHeight => dev.caps().cursor_height_px() as u64,
                                Addfb2Modifiers => dev.caps().has_fb_modifiers() as u64,
                                CrtcInVblankEvent => 1,
                                AtomicAsyncPageFlip => {
                                    (self.has_feature(DrmFeatures::ATOMIC)
                                        && dev.caps().has_async_page_flip())
                                        as u64
                                }
                                _ => 0,
                            }
                        }
                    };

                    args.value = value;

                    cmd.write(&args)?;
                    Ok(0)
                }
                cmd @ DrmIoctlSetClientCap => {
                    use DrmSetCapability::*;
                    let args: DrmSetClientCap = cmd.read()?;

                    match DrmSetCapability::try_from(args.capability)? {
                        Stereo3D => match args.value {
                            0 | 1 => {
                                self.caps
                                    .has_stereo
                                    .store(args.value == 1, Ordering::Relaxed);
                            }
                            _ => return_errno!(Errno::EINVAL),
                        },
                        UniversalPlane => {
                            match args.value {
                                0 | 1 => {
                                    self.caps
                                        .has_universal_planes
                                        .store(args.value == 1, Ordering::Relaxed);
                                }
                                _ => return_errno!(Errno::EINVAL),
                            };
                        }
                        Atomic => {
                            if !self.has_feature(DrmFeatures::ATOMIC) {
                                return_errno!(Errno::EOPNOTSUPP);
                            }

                            match args.value {
                                0..=2 => {
                                    let v = args.value;

                                    self.caps.has_atomic.store(v >= 1, Ordering::Relaxed);
                                    self.caps
                                        .has_universal_planes
                                        .store(v >= 1, Ordering::Relaxed);
                                    self.caps.has_aspect_ratio.store(v == 2, Ordering::Relaxed);
                                }
                                _ => return_errno!(Errno::EINVAL),
                            }
                        }
                        AspectRatio => {
                            match args.value {
                                0 | 1 => {
                                    self.caps
                                        .has_aspect_ratio
                                        .store(args.value == 1, Ordering::Relaxed);
                                }
                                _ => return_errno!(Errno::EINVAL),
                            };
                        }
                        WritebackConnectors => {
                            if !self.caps.has_atomic.load(Ordering::Relaxed) {
                                return_errno!(Errno::EINVAL);
                            }

                            match args.value {
                                0 | 1 => {
                                    self.caps
                                        .has_writeback_connectors
                                        .store(args.value == 1, Ordering::Relaxed);
                                }
                                _ => return_errno!(Errno::EINVAL),
                            };
                        }
                        CursorPlaneHostport => {
                            if !self.has_feature(DrmFeatures::CURSOR_HOTSPOT)
                                && self.caps.has_atomic.load(Ordering::Relaxed)
                            {
                                return_errno!(Errno::EOPNOTSUPP);
                            }

                            match args.value {
                                0 | 1 => {
                                    self.caps
                                        .has_virtualized_cursor_plane
                                        .store(args.value == 1, Ordering::Relaxed);
                                }
                                _ => return_errno!(Errno::EINVAL),
                            };
                        }
                    }
                    Ok(0)
                }
                DrmIoctlSetMaster => {
                    if !self.master_check_perm() {
                        return_errno!(Errno::EACCES)
                    }

                    self.minor.set_master(self.file_id)?;
                    let mut auth_state = self.auth_state.lock();
                    auth_state.was_master = true;
                    auth_state.authenticated = true;
                    Ok(0)
                }
                DrmIoctlDropMaster => {
                    if !self.master_check_perm() {
                        return_errno!(Errno::EACCES);
                    }
                    if !self.is_master() {
                        return_errno!(Errno::EINVAL);
                    }

                    self.minor.drop_master(self.file_id);
                    Ok(0)
                }
                cmd @ DrmIoctlWaitVblank => self.ioctl_wait_vblank(cmd),
                cmd @ DrmIoctlModeGetResources => self.ioctl_mode_get_resources(cmd),
                cmd @ DrmIoctlModeGetCrtc => self.ioctl_mode_get_crtc(cmd),
                cmd @ DrmIoctlModeSetCrtc => self.ioctl_mode_set_crtc(cmd),
                cmd @ DrmIoctlModeGetEncoder => self.ioctl_mode_get_encoder(cmd),
                cmd @ DrmIoctlModeGetConnector => self.ioctl_mode_get_connector(cmd),
                cmd @ DrmIoctlModeGetProperty => self.ioctl_mode_get_property(cmd),
                cmd @ DrmIoctlModeGetPropBlob => self.ioctl_mode_get_blob(cmd),
                cmd @ DrmIoctlModeAddFB => self.ioctl_mode_add_fb(cmd),
                cmd @ DrmIoctlModeRmFB => self.ioctl_mode_rm_fb(cmd),
                cmd @ DrmIoctlModePageFlip => self.ioctl_mode_page_flip(cmd),
                cmd @ DrmIoctlModeDirtyFb => self.ioctl_mode_dirty_fb(cmd),
                cmd @ DrmIoctlModeCreateDumb => self.ioctl_mode_create_dumb(cmd),
                cmd @ DrmIoctlModeMapDumb => self.ioctl_mode_map_dumb(cmd),
                cmd @ DrmIoctlModeDestroyDumb => self.ioctl_mode_destroy_dumb(cmd),
                cmd @ DrmIoctlModeGetPlaneResources => self.ioctl_mode_get_plane_resources(cmd),
                cmd @ DrmIoctlModeGetPlane => self.ioctl_mode_get_plane(cmd),
                cmd @ DrmIoctlModeAddFB2 => self.ioctl_mode_add_fb2(cmd),
                cmd @ DrmIoctlModeObjectGetProps => self.ioctl_mode_get_object_props(cmd),
                cmd @ DrmIoctlModeAtomic => self.ioctl_mode_atomic(cmd),
                cmd @ DrmIoctlModeCreatePropBlob => self.ioctl_mode_create_blob(cmd),
                cmd @ DrmIoctlModeDestroyPropBlob => self.ioctl_mode_destroy_blob(cmd),
                _ => {
                    ostd::debug!(
                        "the ioctl command {:#x} is unknown for framebuffer devices",
                        raw_ioctl.cmd()
                    );
                    return_errno_with_message!(Errno::ENOTTY, "the ioctl command is unknown");
                }
            }
        )
    }
}

fn copy_array_to_user<T: Pod>(
    vm: &impl VmIo,
    user_ptr: u64,
    user_capacity: u32,
    values: &[T],
) -> Result<()> {
    if user_ptr == 0 || user_capacity == 0 || values.is_empty() {
        return Ok(());
    }

    let total = u32::try_from(values.len())
        .map_err(|_| Error::with_message(Errno::EOVERFLOW, "array too large"))?;
    let copied = core::cmp::min(user_capacity, total);
    if copied != 0 {
        vm.write_slice(user_ptr as usize, &values[..copied as usize])?;
    }
    Ok(())
}

fn copy_array_from_user<T: Pod>(vm: &impl VmIo, user_ptr: u64, count: u32) -> Result<Vec<T>> {
    let count = count as usize;
    let mut values = Vec::with_capacity(count);

    for index in 0..count {
        let offset = index
            .checked_mul(mem::size_of::<T>())
            .ok_or(Errno::EOVERFLOW)?;
        let address = (user_ptr as usize)
            .checked_add(offset)
            .ok_or(Errno::EOVERFLOW)?;

        values.push(vm.read_val::<T>(address)?);
    }

    Ok(values)
}

fn user_array_ptr_at<T>(user_ptr: u64, index: usize) -> Result<u64> {
    let offset = index
        .checked_mul(core::mem::size_of::<T>())
        .ok_or(Errno::EOVERFLOW)?;
    let address = (user_ptr as usize)
        .checked_add(offset)
        .ok_or(Errno::EOVERFLOW)?;

    Ok(address as u64)
}
