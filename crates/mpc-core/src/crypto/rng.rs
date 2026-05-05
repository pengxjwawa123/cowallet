//! Cryptographically Secure Random Number Generation
//!
//! Wrapper around OsRng with DKLS23-specific randomness

use rand_core::{OsRng, RngCore};

/// Get a cryptographically secure random number generator
pub fn get_rng() -> OsRng {
    OsRng
}

/// Generate random bytes
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

/// Generate a random u8 in range
pub fn random_u8() -> u8 {
    let mut bytes = [0u8; 1];
    OsRng.fill_bytes(&mut bytes);
    bytes[0]
}

/// Generate a random bool
pub fn random_bool() -> bool {
    random_u8() % 2 == 0
}

/// Generate a random u32
pub fn random_u32() -> u32 {
    let mut bytes = [0u8; 4];
    OsRng.fill_bytes(&mut bytes);
    u32::from_be_bytes(bytes)
}

/// Generate a random u64
pub fn random_u64() -> u64 {
    let mut bytes = [0u8; 8];
    OsRng.fill_bytes(&mut bytes);
    u64::from_be_bytes(bytes)
}

/// Generate a random usize in range [0, max) using rejection sampling
pub fn random_range(max: usize) -> usize {
    if max == 0 {
        return 0;
    }
    let bytes_needed = std::mem::size_of::<usize>();
    let mask = if max.is_power_of_two() {
        max - 1
    } else {
        max.next_power_of_two() - 1
    };

    loop {
        let bytes = random_bytes::<8>();
        let mut val = 0usize;
        for i in 0..bytes_needed.min(8) {
            val |= (bytes[i] as usize) << (i * 8);
        }
        val &= mask;
        if val < max {
            return val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_bytes() {
        let a = random_bytes::<32>();
        let b = random_bytes::<32>();
        // Extremely unlikely to be equal
        assert_ne!(&a[..], &b[..]);
    }

    #[test]
    fn test_random_range() {
        for _ in 0..100 {
            let v = random_range(10);
            assert!(v < 10);
        }
    }
}
