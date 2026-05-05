//! Hash Functions and Tagging for DKLS23
//!
//! Implements tagged hashing per DKLS23 specification.

use sha2::{Digest, Sha256};

/// Output of a hash function (32 bytes for SHA-256)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashOutput(pub [u8; 32]);

impl HashOutput {
    /// Create a new HashOutput from bytes
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the underlying bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to a Vec<u8>
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl AsRef<[u8]> for HashOutput {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Tagged hash as specified in DKLS23 protocol
///
/// H_tag(tag, data) = SHA256(SHA256(tag) || SHA256(tag) || data)
///
/// This provides domain separation for different uses of hashes.
pub fn tagged_hash(tag: &[u8], data_parts: &[&[u8]]) -> HashOutput {
    let mut hasher = Sha256::new();

    // First, hash the tag twice
    let tag_hash = Sha256::digest(tag);
    hasher.update(tag_hash);
    hasher.update(tag_hash);

    // Then hash all data parts
    for part in data_parts {
        hasher.update(part);
    }

    let result = hasher.finalize();
    let mut output = [0u8; 32];
    output.copy_from_slice(&result);
    HashOutput(output)
}

/// Simple SHA256 hash
pub fn sha256(data: &[u8]) -> HashOutput {
    let result = Sha256::digest(data);
    let mut output = [0u8; 32];
    output.copy_from_slice(&result);
    HashOutput(output)
}

/// Hash to scalar (mod secp256k1 order)
pub fn hash_to_scalar(tag: &[u8], data_parts: &[&[u8]]) -> crate::crypto::curve::Scalar {
    use crate::crypto::curve::Scalar;

    let hash = tagged_hash(tag, data_parts);
    // Reduce mod n (secp256k1 order)
    // For simplicity, we use rejection sampling: if hash >= n, hash again
    let mut counter = 0u64;
    loop {
        let mut input = hash.as_bytes().to_vec();
        input.extend_from_slice(&counter.to_be_bytes());
        let candidate = Sha256::digest(&input);
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&candidate);
        if let Some(scalar) = Scalar::from_bytes(&bytes) {
            if !scalar.is_zero() {
                return scalar;
            }
        }
        counter += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tagged_hash_deterministic() {
        let tag = b"test-tag";
        let data = b"hello world";

        let h1 = tagged_hash(tag, &[data]);
        let h2 = tagged_hash(tag, &[data]);

        assert_eq!(h1, h2);
    }

    #[test]
    fn test_tagged_hash_domain_separation() {
        let tag1 = b"tag1";
        let tag2 = b"tag2";
        let data = b"hello world";

        let h1 = tagged_hash(tag1, &[data]);
        let h2 = tagged_hash(tag2, &[data]);

        assert_ne!(h1, h2);
    }

    #[test]
    fn test_hash_to_scalar() {
        let tag = b"DKLS23-test";
        let data = b"test data";
        let s = hash_to_scalar(tag, &[data]);
        assert!(!s.is_zero());
    }
}
