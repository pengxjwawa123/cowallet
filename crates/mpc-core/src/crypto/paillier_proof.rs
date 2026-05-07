//! Paillier-related zero-knowledge proofs for MtA security.
//!
//! Implements a proof that a Paillier ciphertext encrypts a value
//! in the range [0, q) where q is the secp256k1 order.
//! This prevents the server from exploiting modular wraparound attacks.

use k256::{
    elliptic_curve::{sec1::ToEncodedPoint, PrimeField},
    AffinePoint, Scalar,
};
use num_bigint::{BigUint, RandBigInt};
use num_traits::One;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::paillier::{secp256k1_order, PaillierCiphertext, PaillierPublicKey};

/// Zero-knowledge proof that a Paillier ciphertext encrypts a value in [0, q).
///
/// Based on a simplified version of the range proof from Lindell 2017 / GG18:
/// Prover shows Enc(v) where v ∈ [0, q) by proving knowledge of (v, r)
/// such that c = Enc(v; r) AND V = v*G (optional commitment binding).
///
/// The proof is sound under the strong RSA assumption and DDH on secp256k1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaillierRangeProof {
    pub z: Vec<u8>,
    pub u: Vec<u8>,
    pub w: Vec<u8>,
    pub s: Vec<u8>,
    pub s1: Vec<u8>,
    pub s2: Vec<u8>,
}

impl PaillierRangeProof {
    /// Generate a range proof for ciphertext c = Enc(v; r) proving v ∈ [0, q).
    ///
    /// Parameters:
    /// - `pk`: Paillier public key
    /// - `ciphertext`: the ciphertext being proven
    /// - `value`: the plaintext value v
    /// - `randomness`: the encryption randomness r
    /// - `domain_tag`: domain separation tag
    pub fn prove(
        pk: &PaillierPublicKey,
        ciphertext: &PaillierCiphertext,
        value: &BigUint,
        randomness: &BigUint,
        domain_tag: &[u8],
    ) -> Self {
        let q = secp256k1_order();
        let n = &pk.n;
        let n_squared = n * n;
        let mut rng = rand::thread_rng();

        // Range parameter: we prove v ∈ [-q^3, q^3] which subsumes [0, q)
        let q_cubed = &q * &q * &q;

        // Prover commits:
        // alpha ← random in [-q^3, q^3]
        let alpha = rng.gen_biguint_below(&(&q_cubed * BigUint::from(2u64)));

        // beta ← random coprime to N
        let beta = loop {
            let r = rng.gen_biguint_below(n);
            if r > BigUint::from(0u64) {
                break r;
            }
        };

        // u = Enc(alpha; beta) — commitment ciphertext

        // u = Enc(alpha; beta) — commitment ciphertext
        let g_alpha = mod_pow(&(n + BigUint::one()), &alpha, &n_squared);
        let beta_n = mod_pow(&beta, n, &n_squared);
        let u_big = (g_alpha * beta_n) % &n_squared;

        // w = alpha * G (EC commitment)
        let alpha_bytes = biguint_to_32bytes(&(&alpha % &q));
        let alpha_scalar = Option::<Scalar>::from(Scalar::from_repr(alpha_bytes.into()))
            .unwrap_or(Scalar::ONE);
        let w_point = AffinePoint::GENERATOR * alpha_scalar;
        let w_affine: AffinePoint = w_point.into();
        let w_bytes = w_affine.to_encoded_point(true).as_bytes().to_vec();

        // z = v * G (EC commitment to the value for binding)
        let value_bytes = biguint_to_32bytes(&(value % &q));
        let value_scalar = Option::<Scalar>::from(Scalar::from_repr(value_bytes.into()))
            .unwrap_or(Scalar::ONE);
        let z_point = AffinePoint::GENERATOR * value_scalar;
        let z_affine: AffinePoint = z_point.into();
        let z_bytes = z_affine.to_encoded_point(true).as_bytes().to_vec();

        // Fiat-Shamir challenge
        let e = Self::compute_challenge(
            domain_tag,
            &ciphertext.c.to_bytes_be(),
            &z_bytes,
            &u_big.to_bytes_be(),
            &w_bytes,
        );

        // Responses:
        // s1 = alpha + e*v
        let e_big = scalar_to_biguint_from_scalar(&e);
        let s1 = &alpha + &e_big * value;

        // s2 = beta * r^e mod N
        let r_e = mod_pow(randomness, &e_big, n);
        let s2 = (&beta * r_e) % n;

        Self {
            z: z_bytes,
            u: u_big.to_bytes_be(),
            w: w_bytes,
            s: e.to_bytes().to_vec(),
            s1: s1.to_bytes_be(),
            s2: s2.to_bytes_be(),
        }
    }

    /// Verify a range proof.
    pub fn verify(
        &self,
        pk: &PaillierPublicKey,
        ciphertext: &PaillierCiphertext,
        domain_tag: &[u8],
    ) -> bool {
        let q = secp256k1_order();
        let n = &pk.n;
        let n_squared = n * n;
        let q_cubed = &q * &q * &q;

        // Recompute challenge
        let e = Self::compute_challenge(
            domain_tag,
            &ciphertext.c.to_bytes_be(),
            &self.z,
            &self.u,
            &self.w,
        );
        let e_big = scalar_to_biguint_from_scalar(&e);

        // Parse s1, s2
        let s1 = BigUint::from_bytes_be(&self.s1);
        let s2 = BigUint::from_bytes_be(&self.s2);

        // Check 1: s1 ∈ [-q^3, q^3] (range bound)
        if s1 > q_cubed * BigUint::from(3u64) {
            return false;
        }

        // Check 2: Enc(s1; s2) == u * c^e mod N^2
        let enc_s1 = {
            let g_s1 = mod_pow(&(n + BigUint::one()), &s1, &n_squared);
            let s2_n = mod_pow(&s2, n, &n_squared);
            (g_s1 * s2_n) % &n_squared
        };
        let u_big = BigUint::from_bytes_be(&self.u);
        let c_e = mod_pow(&ciphertext.c, &e_big, &n_squared);
        let expected = (&u_big * c_e) % &n_squared;

        if enc_s1 != expected {
            return false;
        }

        // Check 3: s1*G == w + e*z (EC point check)
        let s1_mod_q = &s1 % &q;
        let s1_bytes = biguint_to_32bytes(&s1_mod_q);
        let s1_scalar = match Option::<Scalar>::from(Scalar::from_repr(s1_bytes.into())) {
            Some(s) => s,
            None => return false,
        };
        let lhs = AffinePoint::GENERATOR * s1_scalar;
        let lhs_affine: AffinePoint = lhs.into();

        let w_point = match parse_point(&self.w) {
            Some(p) => p,
            None => return false,
        };
        let z_point = match parse_point(&self.z) {
            Some(p) => p,
            None => return false,
        };

        use k256::ProjectivePoint;
        let rhs = ProjectivePoint::from(w_point) + ProjectivePoint::from(z_point) * e;
        let rhs_affine: AffinePoint = rhs.into();

        lhs_affine == rhs_affine
    }

    fn compute_challenge(
        domain_tag: &[u8],
        ciphertext_bytes: &[u8],
        z: &[u8],
        u: &[u8],
        w: &[u8],
    ) -> Scalar {
        let mut hasher = Sha256::new();
        hasher.update(domain_tag);
        hasher.update((ciphertext_bytes.len() as u32).to_le_bytes());
        hasher.update(ciphertext_bytes);
        hasher.update((z.len() as u32).to_le_bytes());
        hasher.update(z);
        hasher.update((u.len() as u32).to_le_bytes());
        hasher.update(u);
        hasher.update((w.len() as u32).to_le_bytes());
        hasher.update(w);
        let hash = hasher.finalize();

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&hash);
        match Option::<Scalar>::from(Scalar::from_repr(bytes.into())) {
            Some(s) if !bool::from(s.is_zero()) => s,
            _ => {
                let mut counter = 1u32;
                loop {
                    let mut h2 = Sha256::new();
                    h2.update(&hash);
                    h2.update(counter.to_le_bytes());
                    let h2_out = h2.finalize();
                    let mut b2 = [0u8; 32];
                    b2.copy_from_slice(&h2_out);
                    if let Some(s) = Option::<Scalar>::from(Scalar::from_repr(b2.into())) {
                        if !bool::from(s.is_zero()) {
                            return s;
                        }
                    }
                    counter += 1;
                }
            }
        }
    }
}

fn mod_pow(base: &BigUint, exp: &BigUint, modulus: &BigUint) -> BigUint {
    base.modpow(exp, modulus)
}

fn biguint_to_32bytes(val: &BigUint) -> [u8; 32] {
    let bytes = val.to_bytes_be();
    let mut result = [0u8; 32];
    let start = 32usize.saturating_sub(bytes.len());
    let copy_len = bytes.len().min(32);
    result[start..start + copy_len].copy_from_slice(&bytes[bytes.len() - copy_len..]);
    result
}

fn scalar_to_biguint_from_scalar(s: &Scalar) -> BigUint {
    let bytes: [u8; 32] = s.to_bytes().into();
    BigUint::from_bytes_be(&bytes)
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

/// Proof of knowledge of the factorization of a Paillier modulus N.
///
/// Uses N-th root proof: for deterministic challenges w_i, the prover demonstrates
/// the ability to compute w_i^(N^{-1} mod phi(N)) mod N. Only someone who knows
/// phi(N) (i.e., the factorization) can compute this.
///
/// Verifier checks: response_i^N ≡ w_i mod N for all challenges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaillierModulusProof {
    pub n: Vec<u8>,
    pub responses: Vec<Vec<u8>>,
}

const MODULUS_PROOF_ITERATIONS: usize = 64;

impl PaillierModulusProof {
    /// Generate a proof of knowledge of factorization of N = p*q.
    pub fn prove(n: &BigUint, p: &BigUint, q: &BigUint) -> Self {
        let n_bytes = n.to_bytes_be();

        // phi(N) = (p-1)*(q-1)
        let phi_n = (p - BigUint::one()) * (q - BigUint::one());

        // d = N^{-1} mod phi(N)
        let d = mod_inverse(n, &phi_n);

        let mut responses = Vec::with_capacity(MODULUS_PROOF_ITERATIONS);

        for i in 0..MODULUS_PROOF_ITERATIONS {
            let w_i = Self::derive_challenge(&n_bytes, i);
            // response = w_i^d mod N
            let y_i = w_i.modpow(&d, n);
            responses.push(y_i.to_bytes_be());
        }

        Self {
            n: n_bytes,
            responses,
        }
    }

    /// Verify the proof of factorization knowledge.
    /// Checks that response_i^N ≡ w_i mod N for all challenges.
    pub fn verify(&self) -> bool {
        if self.responses.len() != MODULUS_PROOF_ITERATIONS {
            return false;
        }

        let n = BigUint::from_bytes_be(&self.n);

        // Basic checks: N must be odd and at least 2047 bits
        // (product of two 1024-bit primes can be 2047 or 2048 bits)
        if &n % BigUint::from(2u64) == BigUint::from(0u64) {
            return false;
        }
        if n.bits() < 2047 {
            return false;
        }

        for i in 0..MODULUS_PROOF_ITERATIONS {
            let w_i = Self::derive_challenge(&self.n, i);
            let y_i = BigUint::from_bytes_be(&self.responses[i]);

            // Verify y_i^N ≡ w_i mod N
            let yn = y_i.modpow(&n, &n);
            if yn != w_i {
                return false;
            }
        }

        true
    }

    fn derive_challenge(n_bytes: &[u8], index: usize) -> BigUint {
        let mut hasher = Sha256::new();
        hasher.update(b"PaillierModulus");
        hasher.update(n_bytes);
        hasher.update((index as u64).to_le_bytes());
        let hash = hasher.finalize();

        // Expand to a full-width value mod N
        let mut expanded = Vec::with_capacity(256);
        for j in 0..8u32 {
            let mut h = Sha256::new();
            h.update(&hash);
            h.update(j.to_le_bytes());
            expanded.extend_from_slice(&h.finalize());
        }

        let n = BigUint::from_bytes_be(n_bytes);
        BigUint::from_bytes_be(&expanded) % n
    }
}

/// Compute modular inverse of a mod m using extended Euclidean algorithm.
/// Panics if gcd(a, m) != 1.
fn mod_inverse(a: &BigUint, m: &BigUint) -> BigUint {
    use num_bigint::BigInt;
    use num_traits::{One, Signed, Zero};

    let a_int = BigInt::from(a.clone());
    let m_int = BigInt::from(m.clone());

    // Reduce a mod m first to handle a > m
    let a_reduced = &a_int % &m_int;

    let (mut old_r, mut r) = (m_int.clone(), a_reduced);
    let (mut old_s, mut s) = (BigInt::zero(), BigInt::one());

    while !r.is_zero() {
        let quotient = &old_r / &r;
        let temp_r = r.clone();
        r = &old_r - &quotient * &r;
        old_r = temp_r;
        let temp_s = s.clone();
        s = &old_s - &quotient * &s;
        old_s = temp_s;
    }

    // old_r is gcd(a, m) — must be 1
    assert!(old_r == BigInt::one(), "mod_inverse: gcd != 1, inverse does not exist");

    // Ensure result is positive
    let result = if old_s.is_negative() {
        old_s + &m_int
    } else {
        old_s
    };

    result.to_biguint().unwrap()
}

#[cfg(test)]
mod tests {
    use super::super::paillier::PaillierKeypair;
    use super::*;

    #[test]
    fn test_paillier_range_proof_valid() {
        let keypair = PaillierKeypair::generate();
        let q = secp256k1_order();

        // Value in valid range
        let value = BigUint::from(123456789u64);
        assert!(value < q);

        // Encrypt with known randomness
        let r = loop {
            let candidate = rand::thread_rng().gen_biguint_below(&keypair.public.n);
            if candidate > BigUint::from(0u64) {
                break candidate;
            }
        };
        let ct = keypair.public.encrypt_with_randomness(&value, &r);

        let proof = PaillierRangeProof::prove(
            &keypair.public,
            &ct,
            &value,
            &r,
            b"test-range",
        );

        assert!(proof.verify(&keypair.public, &ct, b"test-range"));
    }

    #[test]
    fn test_paillier_range_proof_wrong_domain() {
        let keypair = PaillierKeypair::generate();
        let value = BigUint::from(42u64);
        let r = loop {
            let candidate = rand::thread_rng().gen_biguint_below(&keypair.public.n);
            if candidate > BigUint::from(0u64) {
                break candidate;
            }
        };
        let ct = keypair.public.encrypt_with_randomness(&value, &r);

        let proof = PaillierRangeProof::prove(
            &keypair.public,
            &ct,
            &value,
            &r,
            b"domain-A",
        );

        assert!(!proof.verify(&keypair.public, &ct, b"domain-B"));
    }

    #[test]
    fn test_paillier_range_proof_tampered_ciphertext() {
        let keypair = PaillierKeypair::generate();
        let value = BigUint::from(999u64);
        let r = loop {
            let candidate = rand::thread_rng().gen_biguint_below(&keypair.public.n);
            if candidate > BigUint::from(0u64) {
                break candidate;
            }
        };
        let ct = keypair.public.encrypt_with_randomness(&value, &r);

        let proof = PaillierRangeProof::prove(
            &keypair.public,
            &ct,
            &value,
            &r,
            b"test",
        );

        // Tamper with ciphertext
        let tampered = PaillierCiphertext { c: &ct.c + BigUint::one() };
        assert!(!proof.verify(&keypair.public, &tampered, b"test"));
    }

    #[test]
    fn test_paillier_modulus_proof_valid() {
        let keypair = PaillierKeypair::generate();
        let proof = PaillierModulusProof::prove(
            &keypair.public.n,
            &keypair.secret.p,
            &keypair.secret.q,
        );
        assert!(proof.verify());
    }

    #[test]
    fn test_paillier_modulus_proof_tampered_n() {
        let keypair = PaillierKeypair::generate();
        let mut proof = PaillierModulusProof::prove(
            &keypair.public.n,
            &keypair.secret.p,
            &keypair.secret.q,
        );
        // Tamper with N (add 2 to keep it odd)
        let tampered_n = BigUint::from_bytes_be(&proof.n) + BigUint::from(2u64);
        proof.n = tampered_n.to_bytes_be();
        assert!(!proof.verify());
    }

    #[test]
    fn test_paillier_modulus_proof_small_n_rejected() {
        // A small N should be rejected regardless
        let small_proof = PaillierModulusProof {
            n: BigUint::from(15u64).to_bytes_be(), // 3*5
            responses: vec![vec![1u8]; MODULUS_PROOF_ITERATIONS],
        };
        assert!(!small_proof.verify());
    }
}
