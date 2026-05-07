//! Schnorr zero-knowledge proofs for DKLS23 protocol.
//!
//! Provides non-interactive proofs of knowledge of discrete log (NIZKPK)
//! using Fiat-Shamir transform of the Sigma protocol.
//!
//! Used in:
//! - DKG Round 1: prove knowledge of VSS constant term a_0
//! - Sign Round 1: prove knowledge of ephemeral secret k_i

use k256::{
    elliptic_curve::{
        sec1::ToEncodedPoint,
        Field, PrimeField,
    },
    AffinePoint, ProjectivePoint, Scalar,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Non-interactive Schnorr proof of knowledge of discrete log.
/// Proves: "I know x such that X = x*G" without revealing x.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchnorrProof {
    pub commitment: Vec<u8>,
    pub response: Vec<u8>,
}

impl SchnorrProof {
    /// Generate a Schnorr proof for statement X = x*G.
    /// `domain_tag` provides domain separation (e.g., b"DKG-Round1" or b"Sign-Round1").
    pub fn prove(secret: &Scalar, public_point: &[u8], domain_tag: &[u8]) -> Self {
        let k = Scalar::random(&mut OsRng);
        let r_point = AffinePoint::GENERATOR * k;
        let r_affine: AffinePoint = r_point.into();
        let commitment = r_affine.to_encoded_point(true).as_bytes().to_vec();

        let challenge = Self::compute_challenge(domain_tag, public_point, &commitment);

        // response = k - challenge * secret  (mod q)
        let response = k - challenge * secret;
        let response_bytes = response.to_bytes().to_vec();

        Self {
            commitment,
            response: response_bytes,
        }
    }

    /// Verify a Schnorr proof for statement X = x*G.
    pub fn verify(&self, public_point: &[u8], domain_tag: &[u8]) -> bool {
        let challenge = Self::compute_challenge(domain_tag, public_point, &self.commitment);

        // Parse response scalar
        if self.response.len() != 32 {
            return false;
        }
        let mut resp_bytes = [0u8; 32];
        resp_bytes.copy_from_slice(&self.response);
        let response = match Option::<Scalar>::from(Scalar::from_repr(resp_bytes.into())) {
            Some(s) => s,
            None => return false,
        };

        // Parse the public point X
        let x_point = match parse_point(public_point) {
            Some(p) => p,
            None => return false,
        };

        // Parse commitment R
        let r_point = match parse_point(&self.commitment) {
            Some(p) => p,
            None => return false,
        };

        // Verify: R == response*G + challenge*X
        let lhs = r_point;
        let rhs = ProjectivePoint::from(AffinePoint::GENERATOR) * response
            + ProjectivePoint::from(x_point) * challenge;
        let rhs_affine: AffinePoint = rhs.into();

        lhs == rhs_affine
    }

    fn compute_challenge(domain_tag: &[u8], public_point: &[u8], commitment: &[u8]) -> Scalar {
        let mut hasher = Sha256::new();
        hasher.update(domain_tag);
        hasher.update((public_point.len() as u32).to_le_bytes());
        hasher.update(public_point);
        hasher.update((commitment.len() as u32).to_le_bytes());
        hasher.update(commitment);
        let hash = hasher.finalize();

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&hash);
        // Reduce mod q via rejection — hash output is already uniform in [0, 2^256)
        // and q ≈ 2^256, so bias is negligible. Use from_repr with fallback.
        match Option::<Scalar>::from(Scalar::from_repr(bytes.into())) {
            Some(s) => s,
            None => {
                // Extremely rare case: hash >= q. Rehash with counter.
                let mut counter = 1u32;
                loop {
                    let mut h2 = Sha256::new();
                    h2.update(&hash);
                    h2.update(counter.to_le_bytes());
                    let hash2 = h2.finalize();
                    let mut b2 = [0u8; 32];
                    b2.copy_from_slice(&hash2);
                    if let Some(s) = Option::<Scalar>::from(Scalar::from_repr(b2.into())) {
                        return s;
                    }
                    counter += 1;
                }
            }
        }
    }
}

fn parse_point(bytes: &[u8]) -> Option<AffinePoint> {
    use k256::elliptic_curve::sec1::FromEncodedPoint;
    use k256::EncodedPoint;

    let encoded = EncodedPoint::from_bytes(bytes).ok()?;
    let ct = AffinePoint::from_encoded_point(&encoded);
    if bool::from(ct.is_some()) {
        Some(ct.unwrap())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schnorr_proof_valid() {
        let secret = Scalar::random(&mut OsRng);
        let public = AffinePoint::GENERATOR * secret;
        let public_affine: AffinePoint = public.into();
        let public_bytes = public_affine.to_encoded_point(true).as_bytes().to_vec();

        let proof = SchnorrProof::prove(&secret, &public_bytes, b"test-domain");
        assert!(proof.verify(&public_bytes, b"test-domain"));
    }

    #[test]
    fn test_schnorr_proof_wrong_point() {
        let secret = Scalar::random(&mut OsRng);
        let public = AffinePoint::GENERATOR * secret;
        let public_affine: AffinePoint = public.into();
        let public_bytes = public_affine.to_encoded_point(true).as_bytes().to_vec();

        let other_secret = Scalar::random(&mut OsRng);
        let other_public = AffinePoint::GENERATOR * other_secret;
        let other_affine: AffinePoint = other_public.into();
        let other_bytes = other_affine.to_encoded_point(true).as_bytes().to_vec();

        let proof = SchnorrProof::prove(&secret, &public_bytes, b"test-domain");
        assert!(!proof.verify(&other_bytes, b"test-domain"));
    }

    #[test]
    fn test_schnorr_proof_wrong_domain() {
        let secret = Scalar::random(&mut OsRng);
        let public = AffinePoint::GENERATOR * secret;
        let public_affine: AffinePoint = public.into();
        let public_bytes = public_affine.to_encoded_point(true).as_bytes().to_vec();

        let proof = SchnorrProof::prove(&secret, &public_bytes, b"domain-A");
        assert!(!proof.verify(&public_bytes, b"domain-B"));
    }

    #[test]
    fn test_schnorr_proof_forged_response_fails() {
        let secret = Scalar::random(&mut OsRng);
        let public = AffinePoint::GENERATOR * secret;
        let public_affine: AffinePoint = public.into();
        let public_bytes = public_affine.to_encoded_point(true).as_bytes().to_vec();

        let mut proof = SchnorrProof::prove(&secret, &public_bytes, b"test");
        // Tamper with response
        proof.response[0] ^= 0xFF;
        assert!(!proof.verify(&public_bytes, b"test"));
    }
}
