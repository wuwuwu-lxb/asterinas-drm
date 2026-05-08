// SPDX-License-Identifier: MPL-2.0

#![expect(unused_variables)]

//! Opened File Handle

use core::fmt::Display;

use super::{AccessMode, InodeHandle, StatusFlags, file_table::FdFlags, inode_handle::SeekFrom};
use crate::{
    fs::vfs::{inode::FallocMode, path::Path},
    net::socket::Socket,
    prelude::*,
    process::signal::Pollable,
    util::ioctl::RawIoctl,
    vm::{perms::VmPerms, vmar::MappingHandle, vmo::Vmo},
};

/// The basic operations defined on a file
pub trait FileLike: Pollable + Send + Sync + Any {
    fn read(&self, writer: &mut VmWriter) -> Result<usize> {
        return_errno_with_message!(Errno::EBADF, "the file is not valid for reading");
    }

    fn write(&self, reader: &mut VmReader) -> Result<usize> {
        return_errno_with_message!(Errno::EBADF, "the file is not valid for writing");
    }

    /// Read at the given file offset.
    ///
    /// The file must be seekable to support `read_at`.
    /// Unlike [`read`], `read_at` will not change the file offset.
    ///
    /// [`read`]: FileLike::read
    fn read_at(&self, offset: usize, writer: &mut VmWriter) -> Result<usize> {
        return_errno_with_message!(Errno::ESPIPE, "read_at is not supported");
    }

    /// Write at the given file offset.
    ///
    /// The file must be seekable to support `write_at`.
    /// Unlike [`write`], `write_at` will not change the file offset.
    /// If the file is append-only, the `offset` will be ignored.
    ///
    /// [`write`]: FileLike::write
    fn write_at(&self, offset: usize, reader: &mut VmReader) -> Result<usize> {
        return_errno_with_message!(Errno::ESPIPE, "write_at is not supported");
    }

    fn ioctl(&self, raw_ioctl: RawIoctl) -> Result<i32> {
        // `ENOTTY` means that "The specified operation does not apply to the kind of object that
        // the file descriptor references".
        // Reference: <https://man7.org/linux/man-pages/man2/ioctl.2.html>.
        return_errno_with_message!(Errno::ENOTTY, "ioctl is not supported");
    }

    /// Obtains the mappable object to map this file into the user address space.
    fn mappable(&self) -> Result<MappableObject<'_>> {
        // `ENODEV` means that "The underlying filesystem of the specified file does not support
        // memory mapping".
        // Reference: <https://man7.org/linux/man-pages/man2/mmap.2.html>.
        return_errno_with_message!(Errno::ENODEV, "the file is not mappable");
    }

    fn resize(&self, new_size: usize) -> Result<()> {
        return_errno_with_message!(Errno::EINVAL, "resize is not supported");
    }

    fn status_flags(&self) -> StatusFlags {
        StatusFlags::empty()
    }

    fn set_status_flags(&self, _new_flags: StatusFlags) -> Result<()> {
        return_errno_with_message!(Errno::EINVAL, "set_status_flags is not supported");
    }

    fn access_mode(&self) -> AccessMode {
        AccessMode::O_RDWR
    }

    fn seek(&self, seek_from: SeekFrom) -> Result<usize> {
        return_errno_with_message!(Errno::ESPIPE, "seek is not supported");
    }

    fn fallocate(&self, mode: FallocMode, offset: usize, len: usize) -> Result<()> {
        return_errno_with_message!(Errno::EOPNOTSUPP, "fallocate is not supported");
    }

    fn as_socket(&self) -> Option<&dyn Socket> {
        None
    }

    fn path(&self) -> &Path;

    /// Dumps information to appear in the `fdinfo` file under procfs.
    ///
    /// This method must not break atomic mode because it will be called with the file table's spin
    /// lock held. There are two strategies for implementing this method:
    ///  - If the necessary information can be obtained without breaking atomic mode, the method
    ///    can collect and return the information directly. `Arc<Self>` should be dropped and
    ///    should not appear in the returned `Box<dyn Display>`.
    ///  - Otherwise, if the file can be dropped asynchronously in another process, the method can
    ///    return a `Box<dyn Display>` containing the `Arc<Self>`, so that the information can be
    ///    collected later in its `Display::display()` method, after dropping the file table's spin
    ///    lock.
    fn dump_proc_fdinfo(self: Arc<Self>, fd_flags: FdFlags) -> Box<dyn Display>;
}

impl dyn FileLike {
    pub fn downcast_ref<T: FileLike>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref::<T>()
    }

    pub fn read_bytes(&self, buf: &mut [u8]) -> Result<usize> {
        let mut writer = VmWriter::from(buf).to_fallible();
        self.read(&mut writer)
    }

    pub fn write_bytes(&self, buf: &[u8]) -> Result<usize> {
        let mut reader = VmReader::from(buf).to_fallible();
        self.write(&mut reader)
    }

    pub fn read_bytes_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        let mut writer = VmWriter::from(buf).to_fallible();
        self.read_at(offset, &mut writer)
    }

    #[expect(dead_code)]
    pub fn write_bytes_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        let mut reader = VmReader::from(buf).to_fallible();
        self.write_at(offset, &mut reader)
    }

    pub fn as_socket_or_err(&self) -> Result<&dyn Socket> {
        self.as_socket()
            .ok_or_else(|| Error::with_message(Errno::ENOTSOCK, "the file is not a socket"))
    }

    pub fn as_inode_handle_or_err(&self) -> Result<&InodeHandle> {
        self.downcast_ref().ok_or_else(|| {
            Error::with_message(Errno::EINVAL, "the file is not related to an inode")
        })
    }
}

/// An object that may be memory mapped into the user address space.
pub enum MappableObject<'a> {
    /// A VMO (i.e., page cache).
    Vmo(Arc<Vmo>),
    /// A device mapping.
    Device(&'a dyn Mappable),
}

/// A trait that describes memory mapping behavior for special files (in `mmap`).
pub trait Mappable {
    /// Fills the memory region to map with `handle`.
    ///
    /// `offset` specifies the file offset.
    fn map(&self, offset: usize, handle: MappingHandle) -> Result<Box<dyn MappedObject>>;

    /// Returns a reference to the VMO for reverse mappings, if any.
    ///
    /// Note that this must return the same VMO as [`MappedObject::vmo_for_rmap`] returns.
    fn vmo_for_rmap(&self) -> Option<&Arc<Vmo>> {
        None
    }
}

/// A trait that describes memory mapping behavior for special files (after `mmap`).
pub trait MappedObject: Send + Sync + Debug {
    /// Duplicates the memory mapping.
    fn dup(&self) -> Box<dyn MappedObject>;

    /// Splits the memory mapping.
    ///
    /// `offset` specifies the memory address offset within the mapping, which must be greater than
    /// zero and smaller than the mapping size.
    fn split_at(self: Box<Self>, offset: usize) -> (Box<dyn MappedObject>, Box<dyn MappedObject>);

    /// Handles the page fault.
    ///
    /// `offset` specifies the memory address offset within the mapping, which must be smaller than
    /// the mapping size.
    fn handle_page_fault(
        &self,
        _offset: usize,
        _required_perms: VmPerms,
        _handle: MappingHandle,
    ) -> Result<()> {
        return_errno_with_message!(
            Errno::EFAULT,
            "device memory page faults cannot be resolved"
        );
    }

    /// Returns a reference to the VMO for reverse mappings, if any.
    fn vmo_for_rmap(&self) -> Option<&Arc<Vmo>> {
        None
    }

    /// Returns the file offset for reverse mapping.
    ///
    /// This method must return `Some` if [`Self::vmo_for_rmap`] returns `Some`.
    fn offset_for_rmap(&self) -> Option<usize> {
        None
    }
}
