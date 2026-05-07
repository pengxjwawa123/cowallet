//! Multiplicative-to-Additive (MtA) protocol using Paillier homomorphism.
//!
//! Converts a multiplicative sharing of a value into an additive sharing:
//!   Given: Alice holds `a`, Bob holds `b`
//!   Want: Alice gets `alpha`, Bob gets `beta` such that alpha + beta = a * b (mod q)
//!
//! Protocol (Lindell 2017):
//!   1. Alice encrypts `a` under her Paillier key: c_a = Enc(pk_A, a)
//!   2. Alice sends c_a to Bob (with range proof)
//!   3. Bob computes: c_beta = b ⊙ c_a ⊕ Enc(pk_A, -beta')
//!      where beta' is a random mask, and sets beta = beta'
//!   4. Bob sends c_beta to Alice
//!   5. Alice decrypts: alpha = Dec(sk_A, c_beta) = a*b - beta' (mod n)
//!      Reduced mod q: alpha = a*b - beta (mod q)
//!   Result: alpha + beta = a*b (mod q)

use super::paillier::{
    biguint_to_scalar_bytes, scalar_to_biguint, secp256k1_order, PaillierCiphertext,
    PaillierKeypair, PaillierPublicKey,
};
use num_bigint::{BigUint, RandBigInt};
use num_traits::Zero;
use serde::{Deserialize, Serialize};

/// Message from Alice (holds secret `a`) to Bob: encrypted value of `a`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtAFirstMessage {
    pub c_a: PaillierCiphertext,
}

/// Message from Bob back to Alice: homomorphically computed ciphertext.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtASecondMessage {
    pub c_beta: PaillierCiphertext,
}

/// Alice's state in the MtA protocol.
pub struct MtAAlice {
    a: [u8; 32],
    keypair: PaillierKeypair,
}

/// Bob's output from the MtA protocol.
pub struct MtABobOutput {
    pub beta: [u8; 32], // Bob's additive share
}

/// Alice's output from the MtA protocol.
pub struct MtAAliceOutput {
    pub alpha: [u8; 32], // Alice's additive share
}

impl MtAAlice {
    /// Create a new MtA session for Alice.
    /// `a` is Alice's secret scalar (32 bytes, big-endian).
    /// `keypair` is Alice's Paillier keypair.
    pub fn new(a: [u8; 32], keypair: PaillierKeypair) -> Self {
        Self { a, keypair }
    }

    /// Step 1: Alice encrypts her value and sends to Bob.
    pub fn generate_first_message(&self) -> MtAFirstMessage {
        let a_int = scalar_to_biguint(&self.a);
        let c_a = self.keypair.public.encrypt(&a_int);
        MtAFirstMessage { c_a }
    }

    /// Step 3: Alice decrypts Bob's response to get her additive share.
    ///
    /// Decrypted value is (a*b - beta) mod N.
    /// Since a*b < q^2 and beta < q, and N >> q^2, the value is either:
    ///   - positive: a*b - beta (when a*b >= beta)
    ///   - wrapped: N - (beta - a*b) (when beta > a*b)
    /// We reduce mod q to get alpha such that alpha + beta = a*b (mod q).
    pub fn process_second_message(&self, msg: &MtASecondMessage) -> MtAAliceOutput {
        let q = secp256k1_order();
        let n = &self.keypair.public.n;

        // Decrypt: plaintext = (a*b - beta) mod N
        let plaintext = self.keypair.secret.decrypt(&self.keypair.public, &msg.c_beta);

        // If plaintext > N/2, it represents a negative value (N - |val|)
        // Convert to signed interpretation then reduce mod q
        let half_n = n / BigUint::from(2u64);
        let alpha_mod_q = if plaintext > half_n {
            // Negative: the actual value is -(N - plaintext)
            let neg_val = (n - &plaintext) % &q;
            if neg_val.is_zero() {
                BigUint::zero()
            } else {
                &q - &neg_val
            }
        } else {
            plaintext % &q
        };

        let alpha = biguint_to_scalar_bytes(&alpha_mod_q, &q);
        MtAAliceOutput { alpha }
    }

    /// Get Alice's Paillier public key (to send to Bob during setup).
    pub fn public_key(&self) -> &PaillierPublicKey {
        &self.keypair.public
    }
}

/// Bob's side of the MtA protocol.
pub struct MtABob;

impl MtABob {
    /// Step 2: Bob receives Alice's encrypted `a`, multiplies by his `b`,
    /// adds random mask, and returns his additive share.
    ///
    /// Bob picks beta ∈ Z_q as his output share, then computes:
    ///   c_result = (b ⊙ c_a) ⊕ Enc(pk, N - beta)
    /// This gives Enc(a*b - beta mod N). Alice decrypts and reduces mod q.
    pub fn process_first_message(
        pk: &PaillierPublicKey,
        b: &[u8; 32],
        msg: &MtAFirstMessage,
    ) -> (MtASecondMessage, MtABobOutput) {
        let q = secp256k1_order();
        let n = &pk.n;
        let mut rng = rand::thread_rng();

        let b_int = scalar_to_biguint(b);

        // Bob's output share: random beta ∈ [0, q)
        let beta_val = rng.gen_biguint_below(&q);

        // Homomorphic computation: c_ab = Enc(a * b)
        let c_ab = pk.scalar_mul(&msg.c_a, &b_int);

        // Enc(-beta mod N) = Enc(N - beta) since Paillier plaintext is in Z_N
        let neg_beta = n - &beta_val;
        let c_neg_beta = pk.encrypt(&neg_beta);

        // c_result = Enc(a*b - beta mod N)
        let c_result = pk.add(&c_ab, &c_neg_beta);

        let beta = biguint_to_scalar_bytes(&beta_val, &q);

        (MtASecondMessage { c_beta: c_result }, MtABobOutput { beta })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mta_correctness() {
        // Alice holds a, Bob holds b
        // After MtA: alpha + beta = a * b (mod q)
        let q = secp256k1_order();

        let a_val = BigUint::from(12345u64);
        let b_val = BigUint::from(67890u64);

        let mut a_bytes = [0u8; 32];
        let a_be = a_val.to_bytes_be();
        a_bytes[32 - a_be.len()..].copy_from_slice(&a_be);

        let mut b_bytes = [0u8; 32];
        let b_be = b_val.to_bytes_be();
        b_bytes[32 - b_be.len()..].copy_from_slice(&b_be);

        // Alice generates Paillier keypair
        let keypair = PaillierKeypair::generate();

        // Alice creates MtA instance
        let alice = MtAAlice::new(a_bytes, keypair.clone());

        // Step 1: Alice -> Bob
        let first_msg = alice.generate_first_message();

        // Step 2: Bob processes and returns his share
        let (second_msg, bob_output) = MtABob::process_first_message(
            &keypair.public,
            &b_bytes,
            &first_msg,
        );

        // Step 3: Alice processes Bob's response
        let alice_output = alice.process_second_message(&second_msg);

        // Verify: alpha + beta = a * b (mod q)
        let alpha = scalar_to_biguint(&alice_output.alpha);
        let beta = scalar_to_biguint(&bob_output.beta);
        let expected = (&a_val * &b_val) % &q;
        let sum = (&alpha + &beta) % &q;

        assert_eq!(sum, expected, "MtA failed: alpha + beta != a*b mod q");
    }

    #[test]
    fn test_mta_with_large_scalars() {
        let q = secp256k1_order();
        let mut rng = rand::thread_rng();

        // Random scalars in the curve order range
        let a_val = rng.gen_biguint_below(&q);
        let b_val = rng.gen_biguint_below(&q);

        let a_bytes = biguint_to_scalar_bytes(&a_val, &q);
        let b_bytes = biguint_to_scalar_bytes(&b_val, &q);

        let keypair = PaillierKeypair::generate();
        let alice = MtAAlice::new(a_bytes, keypair.clone());

        let first_msg = alice.generate_first_message();
        let (second_msg, bob_output) = MtABob::process_first_message(
            &keypair.public,
            &b_bytes,
            &first_msg,
        );
        let alice_output = alice.process_second_message(&second_msg);

        let alpha = scalar_to_biguint(&alice_output.alpha);
        let beta = scalar_to_biguint(&bob_output.beta);
        let expected = (&a_val * &b_val) % &q;
        let sum = (&alpha + &beta) % &q;

        assert_eq!(sum, expected, "MtA with large scalars failed");
    }
}
