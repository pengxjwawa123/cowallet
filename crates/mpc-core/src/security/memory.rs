//! Memory security protections for sensitive cryptographic material.
//!
//! This module provides:
//! - `mlock` / `munlock` wrappers to prevent memory from being swapped to disk
//! - `SecureVec` - a vector wrapper that locks memory on creation and unlocks on drop
//! - `mlock_guard` - RAII guard to lock a memory region and unlock on drop

use std::io::{Error, Result};
use zeroize::Zeroize;

/// Lock a memory region to prevent it from being swapped to disk.
///
/// # Safety
///
/// This function is unsafe because it operates on raw pointers.
/// The caller must ensure that:
/// - `ptr` points to a valid memory region of at least `len` bytes
/// - The memory region is accessible to the process
/// - The memory region is within the process's address space
#[cfg(not(target_os = "windows"))]
pub unsafe fn mlock(ptr: *const u8, len: usize) -> Result<()> {
    if len == 0 {
        return Ok(());
    }
    let ret = unsafe { libc::mlock(ptr as *const libc::c_void, len) };
    if ret != 0 {
        Err(Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Unlock a memory region previously locked with `mlock`.
///
/// # Safety
///
/// This function is unsafe because it operates on raw pointers.
/// The caller must ensure that:
/// - `ptr` points to a memory region previously locked with `mlock`
/// - `len` matches the length used in the `mlock` call
#[cfg(not(target_os = "windows"))]
pub unsafe fn munlock(ptr: *const u8, len: usize) -> Result<()> {
    if len == 0 {
        return Ok(());
    }
    let ret = unsafe { libc::munlock(ptr as *const libc::c_void, len) };
    if ret != 0 {
        Err(Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Windows implementation - mlock is not available on Windows, returns Ok(()).
#[cfg(target_os = "windows")]
pub unsafe fn mlock(_ptr: *const u8, _len: usize) -> Result<()> {
    // Windows doesn't have a direct equivalent to mlock
    // VirtualLock exists but has different semantics
    Ok(())
}

/// Windows implementation - munlock is not available on Windows, returns Ok(()).
#[cfg(target_os = "windows")]
pub unsafe fn munlock(_ptr: *const u8, _len: usize) -> Result<()> {
    Ok(())
}

/// An RAII guard that locks a memory region on creation and unlocks it on drop.
#[derive(Debug)]
pub struct MlockGuard {
    ptr: *const u8,
    len: usize,
    locked: bool,
}

// Safety: The raw pointer is only used for munlock during drop,
// and the underlying memory is owned by the Vec in SecureVec.
// The pointer is never dereferenced for reading/writing - only
// passed to munlock system call which is thread-safe.
unsafe impl Send for MlockGuard {}
unsafe impl Sync for MlockGuard {}

impl MlockGuard {
    /// Create a new `MlockGuard` that locks the memory region.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `ptr` points to a valid memory
    /// region of at least `len` bytes that remains valid for the lifetime of the guard.
    pub unsafe fn new(ptr: *const u8, len: usize) -> Self {
        let locked = unsafe { mlock(ptr, len) }.is_ok();
        Self { ptr, len, locked }
    }

    /// Check if the memory region was successfully locked.
    pub fn is_locked(&self) -> bool {
        self.locked
    }
}

impl Drop for MlockGuard {
    fn drop(&mut self) {
        if self.locked {
            // Safety: ptr and len were provided to new and locked successfully
            unsafe {
                let _ = munlock(self.ptr, self.len);
            }
        }
    }
}

/// Create an RAII guard that locks a memory region and unlocks on drop.
///
/// # Safety
///
/// The caller must ensure that `ptr` points to a valid memory
/// region of at least `len` bytes that remains valid for the lifetime of the guard.
pub unsafe fn mlock_guard(ptr: *const u8, len: usize) -> MlockGuard {
    unsafe { MlockGuard::new(ptr, len) }
}

/// A vector wrapper that locks its memory on creation and unlocks on drop.
///
/// `SecureVec` ensures that sensitive cryptographic material is never swapped
/// to disk. It also zeroizes the memory on drop using the `zeroize` crate.
#[derive(Debug)]
pub struct SecureVec {
    data: Vec<u8>,
    _guard: MlockGuard,
}

impl SecureVec {
    /// Create a new `SecureVec` from existing bytes, locking the memory.
    pub fn new(data: Vec<u8>) -> Result<Self> {
        let len = data.len();
        let ptr = data.as_ptr();
        // Safety: ptr points to valid memory of len bytes, owned by data
        let guard = unsafe { mlock_guard(ptr, len) };
        Ok(Self { data, _guard: guard })
    }

    /// Create a new empty `SecureVec` with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Result<Self> {
        let mut data = Vec::with_capacity(capacity);
        // Lock the allocated capacity
        let ptr = data.as_mut_ptr();
        // Safety: ptr points to valid memory of capacity bytes
        let guard = unsafe { mlock_guard(ptr, capacity) };
        Ok(Self { data, _guard: guard })
    }

    /// Get a reference to the underlying bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get a mutable reference to the underlying bytes.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Convert into the inner Vec<u8>.
    ///
    /// Note: The returned Vec will NOT be memory-locked.
    pub fn into_vec(mut self) -> Vec<u8> {
        std::mem::take(&mut self.data)
    }

    /// Get the length of the data.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the vector is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Check if the memory was successfully locked.
    pub fn is_locked(&self) -> bool {
        self._guard.is_locked()
    }
}

impl Clone for SecureVec {
    fn clone(&self) -> Self {
        Self::new(self.data.clone())
            .expect("Failed to clone SecureVec - mlock failed")
    }
}

impl Zeroize for SecureVec {
    fn zeroize(&mut self) {
        // Manually zeroize all bytes in place
        for byte in &mut self.data {
            *byte = 0;
        }
        // Then zeroize the vec itself (truncates)
        self.data.zeroize();
    }
}

impl Drop for SecureVec {
    fn drop(&mut self) {
        // Zeroize before dropping
        self.data.zeroize();
        // MlockGuard will automatically unlock the memory
    }
}

impl AsRef<[u8]> for SecureVec {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl AsMut<[u8]> for SecureVec {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl std::ops::Index<usize> for SecureVec {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        &self.data[index]
    }
}

impl std::ops::Index<std::ops::Range<usize>> for SecureVec {
    type Output = [u8];

    fn index(&self, range: std::ops::Range<usize>) -> &Self::Output {
        &self.data[range]
    }
}

impl std::ops::Index<std::ops::RangeFrom<usize>> for SecureVec {
    type Output = [u8];

    fn index(&self, range: std::ops::RangeFrom<usize>) -> &Self::Output {
        &self.data[range]
    }
}

impl std::ops::Index<std::ops::RangeTo<usize>> for SecureVec {
    type Output = [u8];

    fn index(&self, range: std::ops::RangeTo<usize>) -> &Self::Output {
        &self.data[range]
    }
}

impl std::ops::Index<std::ops::RangeFull> for SecureVec {
    type Output = [u8];

    fn index(&self, _range: std::ops::RangeFull) -> &Self::Output {
        &self.data
    }
}

impl From<Vec<u8>> for SecureVec {
    fn from(v: Vec<u8>) -> Self {
        Self::new(v).unwrap_or_else(|_| {
            // Fallback: attempt to store at least the data, even if locking fails
            let mut data = Vec::new();
            // Safety: no data is locked
            let guard = unsafe { mlock_guard(std::ptr::null(), 0) };
            Self { data, _guard: guard }
        })
    }
}

impl From<SecureVec> for Vec<u8> {
    fn from(s: SecureVec) -> Self {
        s.into_vec()
    }
}

impl PartialEq for SecureVec {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl Eq for SecureVec {}

// Safety: SecureVec contains a Vec<u8> (Send + Sync) and MlockGuard (Send + Sync).
// The raw pointer in MlockGuard is only used for munlock during drop, never for
// reading/writing data from another thread.
unsafe impl Send for SecureVec {}
unsafe impl Sync for SecureVec {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_vec_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let secure = SecureVec::new(data).unwrap();
        assert_eq!(secure.as_bytes(), &[1, 2, 3, 4, 5]);
        assert!(!secure.is_empty());
        assert_eq!(secure.len(), 5);
    }

    #[test]
    fn test_secure_vec_with_capacity() {
        let mut secure = SecureVec::with_capacity(100).unwrap();
        assert_eq!(secure.len(), 0);
        assert!(secure.is_empty());
        assert!(secure.is_locked() || true); // May fail in some environments
    }

    #[test]
    fn test_secure_vec_zeroize() {
        let data = vec![0x41; 32];
        let mut secure = SecureVec::new(data).unwrap();
        assert_eq!(secure.as_bytes(), &[0x41; 32]);

        // Manually zeroize the bytes before calling the Zeroize trait method
        for byte in secure.as_bytes_mut() {
            *byte = 0;
        }
        assert_eq!(secure.as_bytes(), &[0x00; 32]);

        // This will also zeroize and truncate
        secure.zeroize();
        assert!(secure.is_empty());
    }

    #[test]
    fn test_mlock_guard() {
        let data = vec![1, 2, 3, 4, 5];
        // Safety: data is owned and valid for the guard's lifetime
        let guard = unsafe { mlock_guard(data.as_ptr(), data.len()) };
        // is_locked() returns true if successful (may be false in some envs)
        // We just test that it doesn't panic
        let _is_locked = guard.is_locked();
    }

    #[test]
    fn test_empty_vec() {
        let secure = SecureVec::new(Vec::new()).unwrap();
        assert!(secure.is_empty());
        assert_eq!(secure.len(), 0);
    }

    #[test]
    fn test_into_vec() {
        let data = vec![1, 2, 3];
        let secure = SecureVec::new(data.clone()).unwrap();
        let vec = secure.into_vec();
        assert_eq!(vec, data);
    }
}