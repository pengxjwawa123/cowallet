//! Zero-Knowledge Proofs for DKL23
//!
//! Implements:
//! - DLogProof: Proof of knowledge of discrete log
//! - EncProof: Proof of correct encryption

use super::curve::{Point, Scalar};
use super::hashes::*;
use zeroize::Zeroize;

/// Proof of knowledge of discrete logarithm (Sigma protocol)
///
/// Proves knowledge of x such that X = x * G
/// without revealing x.
#[derive(Debug, Clone, Zeroize)]
#[zeroize(drop)]
pub struct DLogProof {
    /// Commitment (first message)
    pub commitment: Point,
    /// Response (second message)
    pub response: Scalar,
}

impl DLogProof {
    /// Prove knowledge of discrete log: X = x * G
    pub fn prove(secret: Scalar, public: &Point) -> Self {
        // Random k
        let k = Scalar::random_nonzero();
        let g = Point::generator();
        let commitment = g.clone() * k.clone();

        // Fiat-Shamir challenge: H(G, X, R)
        let g_bytes = g.to_bytes_uncompressed();
        let x_bytes = public.to_bytes_uncompressed();
        let r_bytes = commitment.to_bytes_uncompressed();
        let challenge = hash_to_scalar(b"DKL23-DLog", &[&g_bytes, &x_bytes, &r_bytes]);

        // Response: z = k + challenge * secret
        let response = k + challenge.clone() * secret;

        Self {
            commitment,
            response,
        }
    }

    /// Verify a DLog proof
    pub fn verify(&self, public: &Point) -> bool {
        let g = Point::generator();

        // Recompute challenge
        let g_bytes = g.to_bytes_uncompressed();
        let x_bytes = public.to_bytes_uncompressed();
        let r_bytes = self.commitment.to_bytes_uncompressed();
        let challenge = hash_to_scalar(b"DKL23-DLog", &[&g_bytes, &x_bytes, &r_bytes]);

        // Check: z * G == R + challenge * X
        let left = g.clone() * self.response.clone();
        let right = self.commitment.clone() + (*public).clone() * challenge.clone();

        left == right
    }
}

/// Proof of correct ElGamal encryption
///
/// Proves that (C1, C2) = (r * G, r * H + m * G)
/// without revealing r or m.
#[derive(Debug, Clone, Zeroize)]
#[zeroize(drop)]
pub struct EncProof {
    /// Commitment for r
    pub a: Point,
    /// Commitment for m
    pub b: Point,
    /// Response for r
    pub z_r: Scalar,
    /// Response for m
    pub z_m: Scalar,
}

impl EncProof {
    /// Prove correct encryption: C = Enc(pk, (m, r))
    ///
    /// C1 = r * G
    /// C2 = r * pk + m * G
    pub fn prove(pk: &Point, m: Scalar, r: Scalar, c1: &Point, c2: &Point) -> Self {
        let g = Point::generator();

        // Random a_r, a_m
        let a_r = Scalar::random_nonzero();
        let a_m = Scalar::random_nonzero();

        // A = a_r * G, B = a_r * pk + a_m * G
        let a = g.clone() * a_r.clone();
        let b = (*pk).clone() * a_r.clone() + g.clone() * a_m.clone();

        // Fiat-Shamir challenge
        let pk_bytes = pk.to_bytes_uncompressed();
        let c1_bytes = c1.to_bytes_uncompressed();
        let c2_bytes = c2.to_bytes_uncompressed();
        let a_bytes = a.to_bytes_uncompressed();
        let b_bytes = b.to_bytes_uncompressed();

        let challenge = hash_to_scalar(
            b"DKL23-Enc",
            &[&pk_bytes, &c1_bytes, &c2_bytes, &a_bytes, &b_bytes],
        );

        // z_r = a_r + challenge * r
        // z_m = a_m + challenge * m
        let z_r = a_r + challenge.clone() * r;
        let z_m = a_m + challenge * m;

        Self { a, b, z_r, z_m }
    }

    /// Verify EncProof
    pub fn verify(&self, pk: &Point, c1: &Point, c2: &Point) -> bool {
        let g = Point::generator();

        // Recompute challenge
        let pk_bytes = pk.to_bytes_uncompressed();
        let c1_bytes = c1.to_bytes_uncompressed();
        let c2_bytes = c2.to_bytes_uncompressed();
        let a_bytes = self.a.to_bytes_uncompressed();
        let b_bytes = self.b.to_bytes_uncompressed();

        let challenge = hash_to_scalar(
            b"DKL23-Enc",
            &[&pk_bytes, &c1_bytes, &c2_bytes, &a_bytes, &b_bytes],
        );

        // Check 1: z_r * G == A + challenge * C1
        let left1 = g.clone() * self.z_r.clone();
        let right1 = self.a.clone() + (*c1).clone() * challenge.clone();

        // Check 2: z_r * pk + z_m * G == B + challenge * C2
        let left2 = (*pk).clone() * self.z_r.clone() + g.clone() * self.z_m.clone();
        let right2 = self.b.clone() + (*c2).clone() * challenge;

        left1 == right1 && left2 == right2
    }
}

/// Proof that a committed value is in a range (0..2^L)
///
/// Used to prevent malleability attacks.
#[derive(Debug, Clone, Zeroize)]
#[zeroize(drop)]
pub struct RangeProof {
    // Simple range proof using multiple bits
    pub bits: Vec<(DLogProof, Point)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlog_proof() {
        let (x, x_g) = Point::random();
        let proof = DLogProof::prove(x, &x_g);
        assert!(proof.verify(&x_g));
    }

    #[test]
    fn test_dlog_proof_wrong_key() {
        let (x, x_g) = Point::random();
        let (_, y_g) = Point::random();
        let proof = DLogProof::prove(x, &x_g);
        assert!(!proof.verify(&y_g));
    }

    #[test]
    fn test_enc_proof() {
        // ElGamal keypair
        let (_sk, pk) = Point::random();

        // Message and randomness
        let m = Scalar::random_nonzero();
        let r = Scalar::random_nonzero();

        // Encrypt
        let g = Point::generator();
        let c1 = g.clone() * r.clone();
        let c2 = pk.clone() * r.clone() + g.clone() * m.clone();

        let proof = EncProof::prove(&pk, m, r, &c1, &c2);
        assert!(proof.verify(&pk, &c1, &c2));
    }

    #[test]
    fn test_enc_proof_wrong() {
        let (_, pk) = Point::random();
        let m = Scalar::random_nonzero();
        let r = Scalar::random_nonzero();
        let g = Point::generator();
        let c1 = g.clone() * r.clone();
        let c2 = pk.clone() * r.clone() + g.clone() * m.clone();

        // Create proof with wrong message
        let m2 = Scalar::random_nonzero();
        let proof = EncProof::prove(&pk, m2, r, &c1, &c2);
        assert!(!proof.verify(&pk, &c1, &c2));
    }
}
