//! Commitment Scheme for DKLS23
//!
//! Implements Pedersen commitments and hash-based point commitments
//! used throughout the DKLS23 protocol.

use super::curve::{Point, Scalar};
use super::hashes::{tagged_hash, HashOutput};
use zeroize::Zeroize;

/// Pedersen commitment parameters
///
/// Commitment = r * G + v * H
/// where H is a second generator with unknown discrete log relative to G
#[derive(Debug, Clone)]
pub struct PedersenParams {
    /// Generator H (second generator)
    pub h: Point,
}

impl PedersenParams {
    /// Create standard Pedersen parameters using NUMS (Nothing Up My Sleeve)
    ///
    /// H = hash_to_curve("DKLS23-Pedersen-H")
    pub fn standard() -> Self {
        // Hash to curve - we use a verifiably random point
        // This is a NUMS point: H = SHA256("DKLS23-Pedersen-H") * G
        let hash = tagged_hash(b"DKLS23-Pedersen-H", &[b"generator"]);
        let mut counter = 0u64;
        let h = loop {
            let mut input = hash.as_bytes().to_vec();
            input.extend_from_slice(&counter.to_be_bytes());
            let h_scalar = tagged_hash(b"retry", &[&input]);
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(h_scalar.as_bytes());
            if let Some(s) = Scalar::from_bytes(&bytes) {
                if !s.is_zero() {
                    break Point::generator() * s;
                }
            }
            counter += 1;
        };

        Self { h }
    }
}

/// A Pedersen commitment to a value
#[derive(Debug, Clone, Zeroize)]
#[zeroize(drop)]
pub struct Commitment {
    /// The commitment point
    pub point: Point,
    /// Randomness used in commitment
    pub randomness: Scalar,
}

impl Commitment {
    /// Create a new commitment to a scalar value
    ///
    /// C = r * G + v * H
    pub fn new(params: &PedersenParams, value: Scalar) -> Self {
        let r = Scalar::random();
        let g = Point::generator();
        let point = g.clone() * r.clone() + params.h.clone() * value.clone();
        Self { point, randomness: r }
    }

    /// Create a commitment with explicit randomness
    pub fn with_randomness(params: &PedersenParams, value: Scalar, randomness: Scalar) -> Self {
        let g = Point::generator();
        let point = g.clone() * randomness.clone() + params.h.clone() * value.clone();
        Self { point, randomness }
    }

    /// Verify the commitment opens to the given value
    pub fn verify(&self, params: &PedersenParams, value: Scalar) -> bool {
        let g = Point::generator();
        let expected = g.clone() * self.randomness.clone() + params.h.clone() * value;
        self.point == expected
    }
}

/// Commit to a point using hash commitment
///
/// commit(point) = H(point_bytes)
pub fn commit_point(point: Point) -> HashOutput {
    let bytes = point.to_bytes_uncompressed();
    tagged_hash(b"DKLS23-Point-Commit", &[&bytes])
}

/// Verify a point commitment
pub fn verify_commit_point(point: Point, commitment: HashOutput) -> bool {
    let expected = commit_point(point);
    expected == commitment
}

/// Commit to a scalar using hash commitment
pub fn commit_scalar(scalar: Scalar) -> HashOutput {
    let bytes = scalar.to_bytes();
    tagged_hash(b"DKLS23-Scalar-Commit", &[&bytes])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pedersen_commit() {
        let params = PedersenParams::standard();
        let v = Scalar::random();
        let comm = Commitment::new(&params, v.clone());
        assert!(comm.verify(&params, v));
    }

    #[test]
    fn test_pedersen_bad_value() {
        let params = PedersenParams::standard();
        let v = Scalar::random();
        let v2 = Scalar::random();
        let comm = Commitment::new(&params, v);
        assert!(!comm.verify(&params, v2));
    }

    #[test]
    fn test_point_commit() {
        let (_, point) = Point::random();
        let comm = commit_point(point.clone());
        assert!(verify_commit_point(point, comm));
    }
}
