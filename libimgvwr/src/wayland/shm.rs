//! Wayland SHM pool backed by a `memfd`.
//!
//! [`ShmPool`] owns the file descriptor and the memory mapping. Call
//! [`ShmPool::fd`] to obtain the [`BorrowedFd`] needed by
//! `wl_shm::create_pool`.

use std::{
    io,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
};

use memmap2::MmapMut;
use rustix::fs::{MemfdFlags, ftruncate, memfd_create};

/// An anonymous shared-memory pool suitable for use as a Wayland SHM buffer.
pub struct ShmPool {
    fd: OwnedFd,
    mmap: MmapMut,
    /// Current size of the pool in bytes.
    pub size: usize,
}

impl ShmPool {
    /// Allocate a new pool of `size` bytes using `memfd_create`.
    pub fn create(size: usize) -> io::Result<ShmPool> {
        let fd: OwnedFd =
            memfd_create(c"imgvwr-shm", MemfdFlags::CLOEXEC).map_err(io::Error::from)?;
        ftruncate(&fd, size as u64).map_err(io::Error::from)?;
        // SAFETY: the fd is valid and the mapping covers the full file.
        let mmap = unsafe { MmapMut::map_mut(&fd) }?;
        Ok(ShmPool { fd, mmap, size })
    }

    /// Grow or shrink the pool to `new_size` bytes.
    ///
    /// The existing mapping is replaced; any outstanding `wl_buffer` objects
    /// pointing into the old mapping must be destroyed by the caller before
    /// calling this.
    pub fn resize(&mut self, new_size: usize) -> io::Result<()> {
        ftruncate(&self.fd, new_size as u64).map_err(io::Error::from)?;
        // SAFETY: fd is valid; new size matches the ftruncate above.
        let new_mmap = unsafe { MmapMut::map_mut(&self.fd) }?;
        self.mmap = new_mmap;
        self.size = new_size;
        Ok(())
    }

    /// Return a mutable view of the entire pool memory.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.mmap
    }

    /// Borrow the underlying file descriptor for passing to
    /// `wl_shm::create_pool`.
    pub fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}
