//! Linux-specific optimizations for maximum performance
//!
//! Uses modern Linux kernel features:
//! - `madvise(MADV_SEQUENTIAL)` - Hint for sequential access patterns
//! - `madvise(MADV_WILLNEED)` - Pre-fault pages for faster access
//! - `fallocate()` - Pre-allocate disk space to avoid fragmentation
//! - `fadvise(POSIX_FADV_SEQUENTIAL)` - Hint for file access patterns

use std::fs::File;
use std::num::NonZeroU64;

/// Apply madvise hints to memory-mapped region for sequential reading
#[cfg(target_os = "linux")]
pub fn madvise_sequential(addr: *const u8, len: usize) {
    use rustix::mm::{Advice, madvise};

    // SAFETY: addr and len come from a valid mmap region
    unsafe {
        let ptr = addr as *mut std::ffi::c_void;
        // Sequential access pattern - kernel can read-ahead aggressively
        let _ = madvise(ptr, len, Advice::Sequential);
        // Tell kernel we'll need this data soon
        let _ = madvise(ptr, len, Advice::WillNeed);
    }
}

#[cfg(not(target_os = "linux"))]
pub fn madvise_sequential(_addr: *const u8, _len: usize) {
    // No-op on non-Linux platforms
}

/// Pre-allocate disk space for a file to avoid fragmentation
#[cfg(target_os = "linux")]
pub fn preallocate_file(file: &File, size: u64) -> std::io::Result<()> {
    use rustix::fs::{FallocateFlags, fallocate};

    if size > 0 {
        // Pre-allocate space without zeroing (faster)
        let _ = fallocate(file, FallocateFlags::empty(), 0, size);
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn preallocate_file(_file: &File, _size: u64) -> std::io::Result<()> {
    Ok(())
}

/// Advise kernel about file access pattern
#[cfg(target_os = "linux")]
pub fn fadvise_sequential(file: &File, len: u64) {
    use rustix::fs::{Advice, fadvise};

    // Tell kernel we'll read sequentially
    let _ = fadvise(file, 0, NonZeroU64::new(len), Advice::Sequential);
    // And that we'll need the data soon
    let _ = fadvise(file, 0, NonZeroU64::new(len), Advice::WillNeed);
}

#[cfg(not(target_os = "linux"))]
pub fn fadvise_sequential(_file: &File, _len: u64) {
    // No-op on non-Linux platforms
}

/// Advise kernel we're done with file data (can be evicted from cache)
#[cfg(target_os = "linux")]
pub fn fadvise_dontneed(file: &File, offset: u64, len: u64) {
    use rustix::fs::{Advice, fadvise};

    let _ = fadvise(file, offset, NonZeroU64::new(len), Advice::DontNeed);
}

#[cfg(not(target_os = "linux"))]
pub fn fadvise_dontneed(_file: &File, _offset: u64, _len: u64) {
    // No-op on non-Linux platforms
}

/// Sync file data to disk efficiently using fdatasync
#[cfg(target_os = "linux")]
pub fn sync_file_data(file: &File) {
    use rustix::fs::fdatasync;

    // Write out dirty data without metadata
    let _ = fdatasync(file);
}

#[cfg(not(target_os = "linux"))]
pub fn sync_file_data(_file: &File) {
    // No-op on non-Linux platforms
}
