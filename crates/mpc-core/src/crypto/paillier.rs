//! Paillier cryptosystem for MtA (Multiplicative-to-Additive) conversion.
//!
//! Key properties:
//! - Homomorphic: Enc(a) * Enc(b) = Enc(a + b) mod N^2
//! - Scalar mult: Enc(a)^b = Enc(a * b) mod N^2
//! - Semantic security under DCRA assumption

use num_bigint::{BigInt, BigUint, RandBigInt, ToBigInt};
use num_integer::Integer;
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};

const PAILLIER_BITS: u64 = 2048;

/// Paillier public key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaillierPublicKey {
    pub n: BigUint,
    n_squared: BigUint,
    g: BigUint, // g = n + 1 (standard optimization)
}

/// Paillier secret key
#[derive(Debug, Clone)]
pub struct PaillierSecretKey {
    pub lambda: BigUint, // lcm(p-1, q-1)
    pub mu: BigUint,     // L(g^lambda mod n^2)^{-1} mod n
    pub p: BigUint,
    pub q: BigUint,
}

/// A Paillier ciphertext
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaillierCiphertext {
    pub c: BigUint,
}

/// Paillier keypair
#[derive(Debug, Clone)]
pub struct PaillierKeypair {
    pub public: PaillierPublicKey,
    pub secret: PaillierSecretKey,
}

impl PaillierKeypair {
    /// Generate a new Paillier keypair with safe primes.
    /// This is computationally expensive (~2-5s on mobile); should only be done during DKG.
    pub fn generate() -> Self {
        let bits = (PAILLIER_BITS / 2) as usize;

        // Generate two safe primes p, q of equal bit length
        // Uses glass_pumpkin's internal OsRng
        let p = glass_pumpkin::safe_prime::new(bits)
            .expect("safe prime generation failed");
        let q = glass_pumpkin::safe_prime::new(bits)
            .expect("safe prime generation failed");

        let n = &p * &q;
        let n_squared = &n * &n;

        // lambda = lcm(p-1, q-1)
        let p_minus_1 = &p - BigUint::one();
        let q_minus_1 = &q - BigUint::one();
        let lambda = lcm_biguint(&p_minus_1, &q_minus_1);

        // g = n + 1 (standard generator)
        let g = &n + BigUint::one();

        // mu = L(g^lambda mod n^2)^{-1} mod n
        // where L(x) = (x - 1) / n
        let g_lambda = mod_pow(&g, &lambda, &n_squared);
        let l_value = l_function(&g_lambda, &n);
        let mu = mod_inverse_biguint(&l_value, &n)
            .expect("mu computation failed — p and q might not be coprime");

        PaillierKeypair {
            public: PaillierPublicKey {
                n: n.clone(),
                n_squared,
                g,
            },
            secret: PaillierSecretKey {
                lambda,
                mu,
                p,
                q,
            },
        }
    }
}

impl PaillierPublicKey {
    /// Encrypt a plaintext message m ∈ Z_n.
    /// Returns ciphertext c = g^m * r^n mod n^2
    pub fn encrypt(&self, m: &BigUint) -> PaillierCiphertext {
        let r = self.random_coprime_to_n();
        self.encrypt_with_randomness(m, &r)
    }

    /// Encrypt with explicit randomness (for ZK proofs).
    pub fn encrypt_with_randomness(&self, m: &BigUint, r: &BigUint) -> PaillierCiphertext {
        // c = g^m * r^n mod n^2
        let g_m = mod_pow(&self.g, m, &self.n_squared);
        let r_n = mod_pow(r, &self.n, &self.n_squared);
        let c = (g_m * r_n) % &self.n_squared;
        PaillierCiphertext { c }
    }

    /// Homomorphic addition: Enc(a) * Enc(b) = Enc(a + b)
    pub fn add(&self, c1: &PaillierCiphertext, c2: &PaillierCiphertext) -> PaillierCiphertext {
        let c = (&c1.c * &c2.c) % &self.n_squared;
        PaillierCiphertext { c }
    }

    /// Scalar multiplication: Enc(a)^b = Enc(a * b)
    pub fn scalar_mul(&self, ct: &PaillierCiphertext, scalar: &BigUint) -> PaillierCiphertext {
        let c = mod_pow(&ct.c, scalar, &self.n_squared);
        PaillierCiphertext { c }
    }

    /// Encrypt a negative value (mod n): Enc(-m) = Enc(n - m)
    pub fn encrypt_negative(&self, m: &BigUint) -> PaillierCiphertext {
        let neg_m = &self.n - (m % &self.n);
        self.encrypt(&neg_m)
    }

    /// Add a plaintext to a ciphertext: Enc(a) * g^b = Enc(a + b)
    pub fn add_plaintext(&self, ct: &PaillierCiphertext, plaintext: &BigUint) -> PaillierCiphertext {
        let g_b = mod_pow(&self.g, plaintext, &self.n_squared);
        let c = (&ct.c * g_b) % &self.n_squared;
        PaillierCiphertext { c }
    }

    fn random_coprime_to_n(&self) -> BigUint {
        let mut rng = rand::thread_rng();
        loop {
            let r = rng.gen_biguint_below(&self.n);
            if r > BigUint::zero() && r.gcd(&self.n) == BigUint::one() {
                return r;
            }
        }
    }
}

impl PaillierSecretKey {
    /// Decrypt a ciphertext. Returns plaintext m ∈ Z_n.
    pub fn decrypt(&self, pk: &PaillierPublicKey, ct: &PaillierCiphertext) -> BigUint {
        // m = L(c^lambda mod n^2) * mu mod n
        let c_lambda = mod_pow(&ct.c, &self.lambda, &pk.n_squared);
        let l_value = l_function(&c_lambda, &pk.n);
        (l_value * &self.mu) % &pk.n
    }
}

/// Convert a secp256k1 scalar (32 bytes, big-endian) to BigUint
pub fn scalar_to_biguint(scalar_bytes: &[u8; 32]) -> BigUint {
    BigUint::from_bytes_be(scalar_bytes)
}

/// Convert a BigUint back to a 32-byte scalar (reduced mod q if needed).
/// secp256k1 order: q = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
pub fn biguint_to_scalar_bytes(val: &BigUint, n: &BigUint) -> [u8; 32] {
    let reduced = val % n;
    let bytes = reduced.to_bytes_be();
    let mut result = [0u8; 32];
    let start = 32usize.saturating_sub(bytes.len());
    result[start..].copy_from_slice(&bytes[..bytes.len().min(32)]);
    result
}

/// secp256k1 curve order as BigUint
pub fn secp256k1_order() -> BigUint {
    BigUint::from_bytes_be(&[
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE,
        0xBA, 0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B,
        0xBF, 0xD2, 0x5E, 0x8C, 0xD0, 0x36, 0x41, 0x41,
    ])
}

// --- Internal helpers ---

/// L(x) = (x - 1) / n
fn l_function(x: &BigUint, n: &BigUint) -> BigUint {
    (x - BigUint::one()) / n
}

/// Modular exponentiation: base^exp mod modulus
fn mod_pow(base: &BigUint, exp: &BigUint, modulus: &BigUint) -> BigUint {
    base.modpow(exp, modulus)
}

/// LCM of two BigUints
fn lcm_biguint(a: &BigUint, b: &BigUint) -> BigUint {
    a * b / a.gcd(b)
}

/// Modular inverse using extended Euclidean algorithm
fn mod_inverse_biguint(a: &BigUint, m: &BigUint) -> Option<BigUint> {
    let a_int = a.to_bigint().unwrap();
    let m_int = m.to_bigint().unwrap();

    let (gcd, x, _) = extended_gcd(&a_int, &m_int);
    if gcd != BigInt::one() {
        return None;
    }

    let result = ((x % &m_int) + &m_int) % &m_int;
    Some(result.to_biguint().unwrap())
}

fn extended_gcd(a: &BigInt, b: &BigInt) -> (BigInt, BigInt, BigInt) {
    if a.is_zero() {
        return (b.clone(), BigInt::zero(), BigInt::one());
    }
    let (gcd, x1, y1) = extended_gcd(&(b % a), a);
    let x = y1 - (b / a) * &x1;
    let y = x1;
    (gcd, x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paillier_encrypt_decrypt() {
        let keypair = PaillierKeypair::generate();
        let m = BigUint::from(42u64);
        let ct = keypair.public.encrypt(&m);
        let decrypted = keypair.secret.decrypt(&keypair.public, &ct);
        assert_eq!(decrypted, m);
    }

    #[test]
    fn test_paillier_homomorphic_add() {
        let keypair = PaillierKeypair::generate();
        let m1 = BigUint::from(100u64);
        let m2 = BigUint::from(200u64);

        let c1 = keypair.public.encrypt(&m1);
        let c2 = keypair.public.encrypt(&m2);
        let c_sum = keypair.public.add(&c1, &c2);

        let decrypted = keypair.secret.decrypt(&keypair.public, &c_sum);
        assert_eq!(decrypted, BigUint::from(300u64));
    }

    #[test]
    fn test_paillier_scalar_mul() {
        let keypair = PaillierKeypair::generate();
        let m = BigUint::from(7u64);
        let scalar = BigUint::from(6u64);

        let ct = keypair.public.encrypt(&m);
        let ct_mul = keypair.public.scalar_mul(&ct, &scalar);

        let decrypted = keypair.secret.decrypt(&keypair.public, &ct_mul);
        assert_eq!(decrypted, BigUint::from(42u64));
    }

    #[test]
    fn test_paillier_with_scalar_values() {
        let keypair = PaillierKeypair::generate();
        let q = secp256k1_order();

        // Simulate encrypting a secp256k1 scalar
        let scalar_val = BigUint::from(123456789u64);
        let ct = keypair.public.encrypt(&scalar_val);
        let decrypted = keypair.secret.decrypt(&keypair.public, &ct);
        assert_eq!(decrypted, scalar_val);

        // Scalar multiply within curve order
        let multiplier = BigUint::from(987654321u64);
        let expected = (&scalar_val * &multiplier) % &q;
        let ct_mul = keypair.public.scalar_mul(&ct, &multiplier);
        let decrypted_mul = keypair.secret.decrypt(&keypair.public, &ct_mul) % &q;
        assert_eq!(decrypted_mul, expected);
    }
}
