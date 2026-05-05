//! Security enhancements for MPC operations.
//!
//! This module provides memory protection mechanisms to prevent sensitive
//! cryptographic material from being swapped to disk or leaked.

pub mod memory;

pub use memory::{mlock, munlock, mlock_guard, SecureVec};