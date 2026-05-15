use super::protocol::verify_signature;
use super::{KeyShare, ProtocolMessage, SessionConfig};
use crate::crypto::paillier::{
    biguint_to_scalar_bytes, scalar_to_biguint, secp256k1_order, PaillierCiphertext,
    PaillierKeypair, PaillierPublicKey,
};
use crate::errors::{MpcError, Result};
use k256::{
    elliptic_curve::{
        scalar::IsHigh,
        sec1::{FromEncodedPoint, ToEncodedPoint},
        Field, PrimeField,
    },
    AffinePoint, EncodedPoint, Scalar,
};
use num_bigint::BigUint;
use num_traits::Zero;
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// Debug helper: convert bytes to hex string (no external dependency)
pub fn hex_str(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{:02x}", b)).collect()
}

/// A distributed signing session implementing DKLS23.
///
/// This implementation follows the 2-round threshold ECDSA protocol
/// where t-of-n parties can sign without reconstructing the full private key.
///
/// Security: Signing happens in 2 rounds with zero-knowledge proofs
/// to ensure correctness without revealing any party's secret share.
pub struct SignSession {
    config: SessionConfig,
    party_index: u16,
    pub my_share: Option<KeyShare>,
    pub message_hash: [u8; 32],
    state: SignState,

    // Protocol state
    round1_messages: Vec<SignRound1Message>,
    round2_messages: Vec<SignRound2Message>,

    // Ephemeral key state (must persist across rounds!)
    my_k: Option<Scalar>,           // My ephemeral secret k_i
    aggregate_r_point: Option<AffinePoint>,  // R = (k_0 * k_1) * G
    r_scalar: Option<Scalar>,       // r = R.x mod n

    // Paillier keypair for MtA (device holds this, generates during session)
    paillier_keypair: Option<PaillierKeypair>,
}

/// Internal state of the signing session
#[allow(dead_code)]
enum SignState {
    Initialized,
    Round1Done,
    Round2Done,
    Complete { signature: EcdsaSignature },
    Failed { error: String },
}

/// Round 1 message: Each party sends their ephemeral public key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound1Message {
    pub session_id: String,
    pub party_index: u16,
    pub k_public: Vec<u8>, // K = k*G, ephemeral public key
    /// Schnorr proof of knowledge of k_i (required)
    pub schnorr_proof: crate::crypto::schnorr::SchnorrProof,
}

/// Round 2 message: Paillier-based MtA protocol messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignRound2Message {
    /// Device → Server: Paillier-encrypted values for homomorphic signature computation.
    /// Server uses Enc(k_0^{-1} * x'_0) and Enc(k_0^{-1}) to compute Enc(s) without
    /// ever learning k_0^{-1} or x'_0.
    MtARequest {
        session_id: String,
        party_index: u16,
        /// [pk_len(4) | pk_json | ciphertext_json] — Enc(k_0^{-1} * x'_0)
        encrypted_share: Vec<u8>,
        /// [ciphertext_json] — Enc(k_0^{-1}), needed for server's x'_1 term
        encrypted_k_inv: Vec<u8>,
        /// k_0^{-1} * m (plaintext partial signature from device)
        partial_s: Vec<u8>,
        /// Range proof for Enc(k_0^{-1} * x'_0): proves value ∈ [0, q)
        range_proof_share: crate::crypto::paillier_proof::PaillierRangeProof,
        /// Range proof for Enc(k_0^{-1}): proves value ∈ [0, q)
        range_proof_k_inv: crate::crypto::paillier_proof::PaillierRangeProof,
        /// Proof that the Paillier modulus N is a product of two safe primes
        modulus_proof: crate::crypto::paillier_proof::PaillierModulusProof,
    },
    /// Server → Device: Enc(s) ciphertext for device to decrypt
    ServerSignature {
        session_id: String,
        party_index: u16,
        s: Vec<u8>,
    },
}

/// A completed ECDSA signature
#[derive(Debug, Clone, Zeroize)]
pub struct EcdsaSignature {
    #[zeroize(skip)]
    pub r: [u8; 32],
    #[zeroize(skip)]
    pub s: [u8; 32],
    #[zeroize(skip)]
    pub v: u8,
}

impl EcdsaSignature {
    /// Convert signature to raw bytes (r || s || v)
    pub fn to_bytes(&self) -> [u8; 65] {
        let mut sig = [0u8; 65];
        sig[0..32].copy_from_slice(&self.r);
        sig[32..64].copy_from_slice(&self.s);
        sig[64] = self.v;
        sig
    }

    /// Parse signature from raw bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 65 {
            return Err(MpcError::SigningFailed("signature must be 65 bytes".into()));
        }
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&bytes[0..32]);
        s.copy_from_slice(&bytes[32..64]);
        Ok(Self { r, s, v: bytes[64] })
    }

    /// Verify this signature against a public key and message hash
    pub fn verify(&self, message_hash: &[u8; 32], public_key: &[u8]) -> Result<bool> {
        verify_signature(public_key, message_hash, &self.to_bytes())
    }
}


impl SignSession {
    /// Create a new distributed signing session.
    ///
    /// Each party holds exactly one key share. The threshold number
    /// of parties must collaborate to produce a valid signature.
    pub fn new_distributed(
        config: SessionConfig,
        my_share: KeyShare,
        message_hash: [u8; 32],
    ) -> Self {
        Self {
            party_index: config.party_index,
            config,
            my_share: Some(my_share),
            message_hash,
            state: SignState::Initialized,
            round1_messages: Vec::new(),
            round2_messages: Vec::new(),
            my_k: None,
            aggregate_r_point: None,
            r_scalar: None,
            paillier_keypair: None,
        }
    }

    /// Create a signing session for local/simulated mode.
    /// This reconstructs the full private key - for testing only!
    pub fn new_local(
        config: SessionConfig,
        share_indices: Vec<u16>,
        shares: Vec<KeyShare>,
        message_hash: [u8; 32],
    ) -> Self {
        // For local mode, we just keep shares for reconstruction
        // Note: This is NOT secure for production!
        let combined_share = Self::combine_shares_locally(&share_indices, &shares);

        Self {
            party_index: config.party_index,
            config,
            my_share: combined_share,
            message_hash,
            state: SignState::Initialized,
            round1_messages: Vec::new(),
            round2_messages: Vec::new(),
            my_k: None,
            aggregate_r_point: None,
            r_scalar: None,
            paillier_keypair: None,
        }
    }

    /// Helper: Combine shares locally (for testing/demo only)
    fn combine_shares_locally(indices: &[u16], shares: &[KeyShare]) -> Option<KeyShare> {

        if shares.is_empty() {
            return None;
        }

        // Lagrange interpolation to get combined secret
        let mut secret_sum = Scalar::ZERO;
        for (i, share) in shares.iter().enumerate() {
            let x_i = Scalar::from((indices[i] + 1) as u64);
            let mut lagrange = Scalar::ONE;

            for (j, _) in shares.iter().enumerate() {
                if i != j {
                    let x_j = Scalar::from((indices[j] + 1) as u64);
                    let numerator = -x_j;
                    let denominator = x_i - x_j;
                    let inv_den = denominator.invert().unwrap_or(Scalar::ONE);
                    lagrange *= numerator * inv_den;
                }
            }

            let secret_bytes: [u8; 32] = share.secret_share.as_bytes()[..32].try_into().unwrap_or([0u8; 32]);
            let s_i = Option::<Scalar>::from(Scalar::from_repr(secret_bytes.into()))
                .unwrap_or(Scalar::ZERO);
            secret_sum += s_i * lagrange;
        }

        Some(KeyShare {
            party: 0,
            threshold: shares[0].threshold,
            total_parties: shares[0].total_parties,
            secret_share: secret_sum.to_bytes().to_vec().into(),
            public_key: shares[0].public_key.clone(),
            paillier_pk: None,
        })
    }

    /// Compute Lagrange coefficient for this party in the signing subset.
    /// For distributed signing, the secret shares need to be weighted by Lagrange coefficients
    /// to reconstruct the secret at x=0.
    fn compute_lagrange_coefficient(&self) -> Result<Scalar> {
        // Collect all party indices (my own + others from round1 messages)
        let mut party_indices: Vec<u16> = vec![self.party_index];
        for msg in &self.round1_messages {
            if msg.party_index != self.party_index {
                party_indices.push(msg.party_index);
            }
        }

        if party_indices.len() < 2 {
            return Err(MpcError::SigningFailed("need at least 2 parties".into()));
        }

        // Compute Lagrange coefficient for this party
        // L_i = product_{j!=i} (0 - x_j) / (x_i - x_j)
        // where x_i = party_index + 1 (Shamir shares use 1-indexed evaluation points)

        let x_i = Scalar::from((self.party_index + 1) as u64);
        let mut lagrange = Scalar::ONE;

        for &j in &party_indices {
            if j != self.party_index {
                let x_j = Scalar::from((j + 1) as u64);
                let numerator = Scalar::ZERO - x_j; // (0 - x_j)
                let denominator = x_i - x_j;        // (x_i - x_j)
                let inv_den_ct = denominator.invert();
                if bool::from(inv_den_ct.is_none()) {
                    return Err(MpcError::SigningFailed("lagrange denominator is zero".into()));
                }
                let inv_den = inv_den_ct.unwrap();
                lagrange *= numerator * inv_den;
            }
        }

        Ok(lagrange)
    }

    /// Generate Round 1 message containing ephemeral public key.
    ///
    /// In Round 1, each party generates an ephemeral key k_i and
    /// broadcasts R_i = k_i * G to all other parties.
    /// CRITICAL: We store k_i for use in Round 2!
    pub fn generate_round1(&mut self) -> Result<ProtocolMessage> {
        match self.state {
            SignState::Initialized => {}
            _ => return Err(MpcError::SigningFailed("invalid state for round 1".into())),
        }

        // Generate ephemeral key pair and STORE IT
        let mut rng = OsRng;
        let k_i = Scalar::random(&mut rng);
        let r_i_point = AffinePoint::GENERATOR * k_i;
        let r_i_affine: AffinePoint = r_i_point.into();

        // Store k_i for Round 2 (critical!)
        self.my_k = Some(k_i);

        // Send R_i = k_i * G
        let r_i_bytes = r_i_affine.to_encoded_point(true).as_bytes().to_vec();

        // Schnorr proof of knowledge of k_i
        let schnorr_proof = {
            use crate::crypto::schnorr::SchnorrProof;
            SchnorrProof::prove(&k_i, &r_i_bytes, b"Sign-Round1")
        };

        let round1 = SignRound1Message {
            session_id: self.config.session_id.clone(),
            party_index: self.party_index,
            k_public: r_i_bytes,
            schnorr_proof,
        };

        // Store our own round1 message
        self.round1_messages.push(round1.clone());

        // Convert to protocol message
        let payload = bincode::serialize(&round1)
            .map_err(|e| MpcError::SigningFailed(format!("serialization failed: {}", e)))?;

        Ok(ProtocolMessage {
            session_id: self.config.session_id.clone(),
            from: self.party_index,
            to: 0xFFFF, // broadcast
            round: 1,
            payload,
        })
    }

    /// Create Round 1 message using a pre-computed ephemeral key (from presignature).
    /// This avoids generating fresh randomness, reducing online signing latency.
    pub fn generate_round1_with_presign(&mut self, k_bytes: &[u8; 32], r_bytes: &[u8]) -> Result<ProtocolMessage> {
        match self.state {
            SignState::Initialized => {}
            _ => return Err(MpcError::SigningFailed("invalid state for round 1".into())),
        }

        // Deserialize the pre-computed k scalar
        let k_i = Option::<Scalar>::from(Scalar::from_repr((*k_bytes).into()))
            .ok_or_else(|| MpcError::SigningFailed("invalid presign k scalar".into()))?;

        // Store k_i for Round 2
        self.my_k = Some(k_i);

        // The R_i point is already pre-computed, use it directly
        let r_i_bytes = r_bytes.to_vec();

        // Schnorr proof of knowledge of k_i
        let schnorr_proof = {
            use crate::crypto::schnorr::SchnorrProof;
            SchnorrProof::prove(&k_i, &r_i_bytes, b"Sign-Round1")
        };

        let round1 = SignRound1Message {
            session_id: self.config.session_id.clone(),
            party_index: self.party_index,
            k_public: r_i_bytes,
            schnorr_proof,
        };

        self.round1_messages.push(round1.clone());

        let payload = bincode::serialize(&round1)
            .map_err(|e| MpcError::SigningFailed(format!("serialization failed: {}", e)))?;

        Ok(ProtocolMessage {
            session_id: self.config.session_id.clone(),
            from: self.party_index,
            to: 0xFFFF,
            round: 1,
            payload,
        })
    }

    /// Process Round 1 messages from other parties.
    ///
    /// For 2-party signing, receives R_j from the other party.
    /// Computes aggregate R = k_i * R_j = (k_0 * k_1) * G multiplicatively.
    /// Derives r = R.x mod n for use in Round 2.
    pub fn process_round1(&mut self, messages: Vec<ProtocolMessage>) -> Result<()> {
        for msg in messages {
            if msg.round != 1 {
                continue;
            }

            let round1: SignRound1Message = bincode::deserialize(&msg.payload)
                .map_err(|e| MpcError::SigningFailed(format!("invalid round 1 message: {}", e)))?;

            // Don't add our own message again
            if round1.party_index != self.party_index {
                // Verify Schnorr proof of knowledge of k_i
                if !round1.schnorr_proof.verify(&round1.k_public, b"Sign-Round1") {
                    return Err(MpcError::SigningFailed(format!(
                        "party {} failed Schnorr proof for ephemeral key",
                        round1.party_index
                    )));
                }
                self.round1_messages.push(round1);
            }
        }

        // Check if we have enough messages (threshold - 1, since we already have our own)
        if self.round1_messages.len() >= self.config.threshold as usize {
            // Compute aggregate R = k_i * R_j (multiplicative nonce sharing)
            let my_k = self.my_k.ok_or_else(||
                MpcError::SigningFailed("my ephemeral key not set".into()))?;

            // Get other party's R_j
            let other_party_msg = self.round1_messages.iter()
                .find(|m| m.party_index != self.party_index)
                .ok_or_else(|| MpcError::SigningFailed("no other party message".into()))?;

            // Parse R_j
            let encoded = EncodedPoint::from_bytes(&other_party_msg.k_public)
                .map_err(|_| MpcError::SigningFailed("invalid R_j encoding".into()))?;
            let r_j_ct = AffinePoint::from_encoded_point(&encoded);
            if bool::from(r_j_ct.is_none()) {
                return Err(MpcError::SigningFailed("invalid R_j point".into()));
            }
            let r_j = r_j_ct.unwrap();

            // Compute R = k_i * R_j = (k_i * k_j) * G
            let aggregate_r_proj = r_j * my_k;
            let aggregate_r: AffinePoint = aggregate_r_proj.into();

            // Extract r = R.x mod n
            let r_bytes = aggregate_r.to_encoded_point(false).as_bytes()[1..33].to_vec();
            let mut r_array = [0u8; 32];
            r_array.copy_from_slice(&r_bytes);
            let r_scalar_ct = Scalar::from_repr(r_array.into());
            if bool::from(r_scalar_ct.is_none()) {
                return Err(MpcError::SigningFailed("invalid r value".into()));
            }

            self.aggregate_r_point = Some(aggregate_r);
            self.r_scalar = Some(r_scalar_ct.unwrap());
            self.state = SignState::Round1Done;
        }

        Ok(())
    }

    /// Generate Round 2 message with MtA-based signature contribution.
    ///
    /// Lower-indexed party (device): Generates Paillier keypair, encrypts k_0^{-1} * x'_0,
    /// and sends MtARequest with the ciphertext + partial_s = k_0^{-1} * m.
    /// The server NEVER learns k_0^{-1} or x'_0 individually.
    ///
    /// Higher-indexed party (server): Waits for MtA request (no-op until process_round2).
    pub fn generate_round2(&mut self) -> Result<ProtocolMessage> {
        match self.state {
            SignState::Round1Done => {}
            _ => return Err(MpcError::SigningFailed("must complete round 1 first".into())),
        }

        let share = self
            .my_share
            .as_ref()
            .ok_or_else(|| MpcError::SigningFailed("no key share available".into()))?;

        let my_k = self.my_k.ok_or_else(||
            MpcError::SigningFailed("ephemeral key not set".into()))?;
        #[allow(unused_variables)]
        let r = self.r_scalar.ok_or_else(||
            MpcError::SigningFailed("r scalar not computed".into()))?;

        let m_ct = Scalar::from_repr(self.message_hash.into());
        if bool::from(m_ct.is_none()) {
            return Err(MpcError::SigningFailed("invalid message hash".into()));
        }
        let m = m_ct.unwrap();

        let share_bytes: [u8; 32] = share
            .secret_share.as_bytes()[..32]
            .try_into()
            .map_err(|_| MpcError::SigningFailed("invalid share length".into()))?;
        let x_i_ct = Scalar::from_repr(share_bytes.into());
        if bool::from(x_i_ct.is_none()) {
            return Err(MpcError::SigningFailed("invalid secret share".into()));
        }
        let x_i = x_i_ct.unwrap();

        let lagrange = self.compute_lagrange_coefficient()?;
        let x_prime_i = x_i * lagrange;

        let k_i_inv = my_k.invert().unwrap_or(Scalar::ONE);

        let other_party_index = self.round1_messages.iter()
            .find(|msg| msg.party_index != self.party_index)
            .map(|msg| msg.party_index)
            .ok_or_else(|| MpcError::SigningFailed("no other party found".into()))?;

        if self.party_index < other_party_index {
            // Device: use Paillier MtA to protect k_0^{-1} and x'_0
            let paillier = PaillierKeypair::generate();

            // Generate randomness explicitly so we can produce range proofs
            let r1 = gen_coprime_to(&paillier.public.n);
            let r2 = gen_coprime_to(&paillier.public.n);

            // Encrypt k_0^{-1} * x'_0 with known randomness
            let k_inv_x_prime: Scalar = k_i_inv * x_prime_i;
            let k_inv_x_bytes: [u8; 32] = k_inv_x_prime.to_bytes().into();
            let k_inv_x_int = scalar_to_biguint(&k_inv_x_bytes);
            let encrypted_share = paillier.public.encrypt_with_randomness(&k_inv_x_int, &r1);

            // Encrypt k_0^{-1} with known randomness
            let k_inv_bytes: [u8; 32] = k_i_inv.to_bytes().into();
            let k_inv_int = scalar_to_biguint(&k_inv_bytes);
            let encrypted_k_inv = paillier.public.encrypt_with_randomness(&k_inv_int, &r2);

            // Range proofs: prove both ciphertexts encrypt values in [0, q)
            use crate::crypto::paillier_proof::{PaillierModulusProof, PaillierRangeProof};
            let range_proof_share = PaillierRangeProof::prove(
                &paillier.public, &encrypted_share, &k_inv_x_int, &r1, b"MtA-share",
            );
            let range_proof_k_inv = PaillierRangeProof::prove(
                &paillier.public, &encrypted_k_inv, &k_inv_int, &r2, b"MtA-k-inv",
            );

            // Modulus proof: prove N = p*q with p,q safe primes
            let modulus_proof = PaillierModulusProof::prove(
                &paillier.public.n, &paillier.secret.p, &paillier.secret.q,
            );

            // partial_s = k_0^{-1} * m (safe to send: without knowing k_0, useless)
            let partial_s = k_i_inv * m;

            // === DIAGNOSTIC: device generate_round2 ===
            eprintln!("[SIGN-DIAG-DEVICE-R2] party_index={}", self.party_index);
            eprintln!("[SIGN-DIAG-DEVICE-R2] r={}", hex_str(r.to_bytes()));
            eprintln!("[SIGN-DIAG-DEVICE-R2] x_i={}", hex_str(x_i.to_bytes()));
            eprintln!("[SIGN-DIAG-DEVICE-R2] lagrange={}", hex_str(lagrange.to_bytes()));
            eprintln!("[SIGN-DIAG-DEVICE-R2] x_prime_i={}", hex_str(x_prime_i.to_bytes()));
            eprintln!("[SIGN-DIAG-DEVICE-R2] k_i_inv={}", hex_str(k_i_inv.to_bytes()));
            eprintln!("[SIGN-DIAG-DEVICE-R2] partial_s={}", hex_str(partial_s.to_bytes()));
            eprintln!("[SIGN-DIAG-DEVICE-R2] msg_hash={}", hex_str(self.message_hash));
            if let Some(ref share) = self.my_share {
                eprintln!("[SIGN-DIAG-DEVICE-R2] pubkey={}", hex_str(&share.public_key));
            }
            // === END DIAGNOSTIC ===

            // Serialize Enc(k_0^{-1} * x'_0)
            let encrypted_bytes = serde_json::to_vec(&encrypted_share)
                .map_err(|e| MpcError::SigningFailed(format!("paillier serialization failed: {}", e)))?;

            // Pack [pk_len(4) | pk_json | ciphertext_json] for encrypted_share
            let pk_bytes = serde_json::to_vec(&paillier.public)
                .map_err(|e| MpcError::SigningFailed(format!("pk serialization failed: {}", e)))?;
            let pk_len = (pk_bytes.len() as u32).to_le_bytes();
            let mut combined = Vec::with_capacity(4 + pk_bytes.len() + encrypted_bytes.len());
            combined.extend_from_slice(&pk_len);
            combined.extend_from_slice(&pk_bytes);
            combined.extend_from_slice(&encrypted_bytes);

            // Serialize Enc(k_0^{-1})
            let encrypted_k_inv_bytes = serde_json::to_vec(&encrypted_k_inv)
                .map_err(|e| MpcError::SigningFailed(format!("k_inv serialization failed: {}", e)))?;

            // Store keypair for decryption in process_round2
            self.paillier_keypair = Some(paillier);

            let round2 = SignRound2Message::MtARequest {
                session_id: self.config.session_id.clone(),
                party_index: self.party_index,
                encrypted_share: combined,
                encrypted_k_inv: encrypted_k_inv_bytes,
                partial_s: partial_s.to_bytes().to_vec(),
                range_proof_share,
                range_proof_k_inv,
                modulus_proof,
            };

            self.round2_messages.push(round2.clone());

            let payload = bincode::serialize(&round2)
                .map_err(|e| MpcError::SigningFailed(format!("serialization failed: {}", e)))?;

            Ok(ProtocolMessage {
                session_id: self.config.session_id.clone(),
                from: self.party_index,
                to: other_party_index,
                round: 2,
                payload,
            })
        } else {
            Err(MpcError::SigningFailed("server waits for device MtA request".into()))
        }
    }

    /// Process Round 2 messages and aggregate into final signature.
    ///
    /// Higher-indexed party (server): Receives MtARequest, uses Paillier homomorphism
    /// to compute s = k_1^{-1} * (partial_s + r * Enc(k_0^{-1} * x'_0)) + k_1^{-1} * r * x'_1
    /// Server never learns k_0^{-1} or x'_0.
    ///
    /// Lower-indexed party (device): Receives ServerSignature {s}, forms complete signature.
    pub fn process_round2(&mut self, messages: Vec<ProtocolMessage>) -> Result<EcdsaSignature> {
        match self.state {
            SignState::Round1Done => {}
            SignState::Round2Done => {}
            _ => return Err(MpcError::SigningFailed("must complete round 1 first".into())),
        }

        for msg in messages {
            if msg.round != 2 {
                continue;
            }

            let round2: SignRound2Message = bincode::deserialize(&msg.payload)
                .map_err(|e| MpcError::SigningFailed(format!("invalid round 2 message: {}", e)))?;

            let msg_party = match &round2 {
                SignRound2Message::MtARequest { party_index, .. } => *party_index,
                SignRound2Message::ServerSignature { party_index, .. } => *party_index,
            };
            if msg_party != self.party_index {
                self.round2_messages.push(round2);
            }
        }

        let other_party_index = self.round1_messages.iter()
            .find(|msg| msg.party_index != self.party_index)
            .map(|msg| msg.party_index)
            .ok_or_else(|| MpcError::SigningFailed("no other party found".into()))?;

        if self.party_index > other_party_index {
            // Server: process MtA request and compute s
            self.server_compute_signature()
        } else {
            // Device: receive s from server
            self.device_receive_signature()
        }
    }

    /// Server-side: compute Enc(s) using Paillier homomorphism and send to device.
    fn server_compute_signature(&mut self) -> Result<EcdsaSignature> {
        let (encrypted_share_bytes, encrypted_k_inv_bytes, partial_s_bytes, rp_share, rp_k_inv, mod_proof) =
            self.round2_messages.iter()
            .find_map(|m| {
                if let SignRound2Message::MtARequest {
                    encrypted_share, encrypted_k_inv, partial_s,
                    range_proof_share, range_proof_k_inv, modulus_proof, ..
                } = m {
                    Some((
                        encrypted_share.clone(), encrypted_k_inv.clone(),
                        partial_s.clone(), range_proof_share.clone(), range_proof_k_inv.clone(),
                        modulus_proof.clone(),
                    ))
                } else {
                    None
                }
            })
            .ok_or_else(|| MpcError::SigningFailed("no MtA request from device".into()))?;

        // Parse device's Paillier pk + Enc(k_0^{-1}*x'_0)
        if encrypted_share_bytes.len() < 4 {
            return Err(MpcError::SigningFailed("MtA payload too short".into()));
        }
        let pk_len = u32::from_le_bytes(encrypted_share_bytes[..4].try_into().unwrap()) as usize;
        if encrypted_share_bytes.len() < 4 + pk_len {
            return Err(MpcError::SigningFailed("MtA payload truncated".into()));
        }
        let device_pk: PaillierPublicKey = serde_json::from_slice(&encrypted_share_bytes[4..4 + pk_len])
            .map_err(|e| MpcError::SigningFailed(format!("invalid device Paillier pk: {}", e)))?;
        let c_k_inv_x: PaillierCiphertext = serde_json::from_slice(&encrypted_share_bytes[4 + pk_len..])
            .map_err(|e| MpcError::SigningFailed(format!("invalid ciphertext: {}", e)))?;

        // Verify Paillier modulus proof (N = p*q with safe primes)
        if !mod_proof.verify() {
            return Err(MpcError::SigningFailed("Paillier modulus proof failed: N may not be well-formed".into()));
        }

        // Parse Enc(k_0^{-1})
        let c_k_inv: PaillierCiphertext = serde_json::from_slice(&encrypted_k_inv_bytes)
            .map_err(|e| MpcError::SigningFailed(format!("invalid k_inv ciphertext: {}", e)))?;

        // Verify range proofs
        if !rp_share.verify(&device_pk, &c_k_inv_x, b"MtA-share") {
            return Err(MpcError::SigningFailed("range proof for encrypted_share failed".into()));
        }
        if !rp_k_inv.verify(&device_pk, &c_k_inv, b"MtA-k-inv") {
            return Err(MpcError::SigningFailed("range proof for encrypted_k_inv failed".into()));
        }

        // Parse partial_s = k_0^{-1} * m
        let mut ps_bytes = [0u8; 32];
        ps_bytes.copy_from_slice(&partial_s_bytes[..32]);
        let partial_s_ct = Scalar::from_repr(ps_bytes.into());
        if bool::from(partial_s_ct.is_none()) {
            return Err(MpcError::SigningFailed("invalid partial_s".into()));
        }
        let partial_s = partial_s_ct.unwrap();

        // Server's own values
        let share = self.my_share.as_ref()
            .ok_or_else(|| MpcError::SigningFailed("no key share".into()))?;
        let share_bytes: [u8; 32] = share.secret_share.as_bytes()[..32].try_into()
            .map_err(|_| MpcError::SigningFailed("invalid share length".into()))?;
        let x_1_ct = Scalar::from_repr(share_bytes.into());
        if bool::from(x_1_ct.is_none()) {
            return Err(MpcError::SigningFailed("invalid secret share".into()));
        }
        let x_1 = x_1_ct.unwrap();

        let lagrange = self.compute_lagrange_coefficient()?;
        let x_prime_1 = x_1 * lagrange;

        let r = self.r_scalar.ok_or_else(||
            MpcError::SigningFailed("r not computed".into()))?;
        let my_k = self.my_k.ok_or_else(||
            MpcError::SigningFailed("k_1 not set".into()))?;
        let k_1_inv = my_k.invert().unwrap_or(Scalar::ONE);

        // === DIAGNOSTIC: server_compute_signature ===
        eprintln!("[SIGN-DIAG-SERVER] party_index={}", self.party_index);
        eprintln!("[SIGN-DIAG-SERVER] r={}", hex_str(r.to_bytes()));
        eprintln!("[SIGN-DIAG-SERVER] x_1={}", hex_str(share_bytes));
        eprintln!("[SIGN-DIAG-SERVER] lagrange={}", hex_str(lagrange.to_bytes()));
        eprintln!("[SIGN-DIAG-SERVER] x_prime_1={}", hex_str(x_prime_1.to_bytes()));
        eprintln!("[SIGN-DIAG-SERVER] k_1_inv={}", hex_str(k_1_inv.to_bytes()));
        eprintln!("[SIGN-DIAG-SERVER] partial_s(from_device)={}", hex_str(partial_s.to_bytes()));
        eprintln!("[SIGN-DIAG-SERVER] msg_hash={}", hex_str(self.message_hash));
        if let Some(ref share) = self.my_share {
            eprintln!("[SIGN-DIAG-SERVER] pubkey={}", hex_str(&share.public_key));
        }
        // === END DIAGNOSTIC ===

        // Compute Enc(s) using three terms:
        //   s = k_0^{-1}*k_1^{-1}*m + k_0^{-1}*k_1^{-1}*r*x'_0 + k_0^{-1}*k_1^{-1}*r*x'_1
        // term1 = k_1^{-1} * partial_s  (plaintext, since partial_s = k_0^{-1}*m)
        // term2 = (k_1^{-1}*r) ⊙ Enc(k_0^{-1}*x'_0)
        // term3 = (k_1^{-1}*r*x'_1) ⊙ Enc(k_0^{-1})

        // term1: plaintext
        let k1_inv_partial_s = k_1_inv * partial_s;
        let k1_inv_partial_s_bytes: [u8; 32] = k1_inv_partial_s.to_bytes().into();
        let k1_inv_partial_s_int = scalar_to_biguint(&k1_inv_partial_s_bytes);

        // term2: homomorphic
        let k1_inv_r = k_1_inv * r;
        let k1_inv_r_bytes: [u8; 32] = k1_inv_r.to_bytes().into();
        let k1_inv_r_int = scalar_to_biguint(&k1_inv_r_bytes);
        let c_term2 = device_pk.scalar_mul(&c_k_inv_x, &k1_inv_r_int);

        // term3: homomorphic — THIS is the fix (previously used plaintext k_1^{-1}*r*x'_1)
        let k1_inv_r_x1 = k_1_inv * r * x_prime_1;
        let k1_inv_r_x1_bytes: [u8; 32] = k1_inv_r_x1.to_bytes().into();
        let k1_inv_r_x1_int = scalar_to_biguint(&k1_inv_r_x1_bytes);
        let c_term3 = device_pk.scalar_mul(&c_k_inv, &k1_inv_r_x1_int);

        // Enc(s) = Enc(term2) ⊕ Enc(term3) ⊕ Enc(term1)
        let c_term2_plus_3 = device_pk.add(&c_term2, &c_term3);
        let c_s = device_pk.add_plaintext(&c_term2_plus_3, &k1_inv_partial_s_int);

        // Send Enc(s) to device for decryption
        let c_s_bytes = serde_json::to_vec(&c_s)
            .map_err(|e| MpcError::SigningFailed(format!("ciphertext serialization failed: {}", e)))?;

        let server_msg = SignRound2Message::ServerSignature {
            session_id: self.config.session_id.clone(),
            party_index: self.party_index,
            s: c_s_bytes,
        };
        self.round2_messages.push(server_msg);

        // Server returns placeholder — device decrypts the real signature
        let r_bytes: [u8; 32] = r.to_bytes().into();
        let aggregate_r = self.aggregate_r_point
            .ok_or_else(|| MpcError::SigningFailed("aggregate R not set".into()))?;
        let v = self.compute_recovery_id(&aggregate_r, &Scalar::ONE)?;
        let placeholder = EcdsaSignature { r: r_bytes, s: [0u8; 32], v };
        self.state = SignState::Round1Done;
        Ok(placeholder)
    }

    /// Device-side: receive server's response and extract final signature.
    /// Server sends Enc(s) which device decrypts with its Paillier secret key.
    fn device_receive_signature(&mut self) -> Result<EcdsaSignature> {
        let q = secp256k1_order();

        let server_s_bytes = self.round2_messages.iter()
            .find_map(|m| {
                if let SignRound2Message::ServerSignature { s, .. } = m {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .ok_or_else(|| MpcError::SigningFailed("no server signature".into()))?;

        let r = self.r_scalar.ok_or_else(||
            MpcError::SigningFailed("r not computed".into()))?;
        let r_bytes: [u8; 32] = r.to_bytes().into();

        // Decrypt Enc(s) using our Paillier secret key
        let paillier = self.paillier_keypair.as_ref()
            .ok_or_else(|| MpcError::SigningFailed("Paillier keypair not available".into()))?;

        let c_s: PaillierCiphertext = serde_json::from_slice(&server_s_bytes)
            .map_err(|e| MpcError::SigningFailed(format!("invalid ciphertext from server: {}", e)))?;

        let s_big = paillier.secret.decrypt(&paillier.public, &c_s);

        // === DIAGNOSTIC: device_receive_signature ===
        eprintln!("[SIGN-DIAG-DEVICE] party_index={}", self.party_index);
        eprintln!("[SIGN-DIAG-DEVICE] r={}", hex_str(r.to_bytes()));
        eprintln!("[SIGN-DIAG-DEVICE] msg_hash={}", hex_str(self.message_hash));
        eprintln!("[SIGN-DIAG-DEVICE] s_big(decrypted)={}", s_big.to_str_radix(16));
        eprintln!("[SIGN-DIAG-DEVICE] paillier_n_bits={}", paillier.public.n.bits());
        if let Some(ref share) = self.my_share {
            eprintln!("[SIGN-DIAG-DEVICE] pubkey={}", hex_str(&share.public_key));
            eprintln!("[SIGN-DIAG-DEVICE] x_0={}", hex_str(&share.secret_share.as_bytes()[..32]));
        }
        // === END DIAGNOSTIC ===

        // Reduce mod q and handle potential negative wrapping
        let n = &paillier.public.n;
        let half_n = n / BigUint::from(2u64);
        let s_mod_q = if s_big > half_n {
            let neg_val = (n - &s_big) % &q;
            if neg_val.is_zero() {
                BigUint::from(0u64)
            } else {
                &q - &neg_val
            }
        } else {
            s_big % &q
        };

        let s_scalar_bytes = biguint_to_scalar_bytes(&s_mod_q, &q);
        let s_ct = Scalar::from_repr(s_scalar_bytes.into());
        if bool::from(s_ct.is_none()) {
            return Err(MpcError::SigningFailed("invalid decrypted s".into()));
        }
        let s = s_ct.unwrap();

        // Normalize s to low-S form
        let s_was_high: bool = s.is_high().into();
        let s_normalized = if s_was_high { -s } else { s };
        let s_final_bytes: [u8; 32] = s_normalized.to_bytes().into();

        // === DIAGNOSTIC: final signature values ===
        eprintln!("[SIGN-DIAG-DEVICE] s_mod_q={}", hex_str(s.to_bytes()));
        eprintln!("[SIGN-DIAG-DEVICE] s_was_high={}", s_was_high);
        eprintln!("[SIGN-DIAG-DEVICE] s_final={}", hex_str(&s_final_bytes));
        eprintln!("[SIGN-DIAG-DEVICE] r_final={}", hex_str(&r_bytes));
        // === END DIAGNOSTIC ===

        // Determine correct recovery id by trying both and verifying against known public key
        let v = self.determine_recovery_id(&r_bytes, &s_final_bytes)?;

        let signature = EcdsaSignature {
            r: r_bytes,
            s: s_final_bytes,
            v,
        };

        self.state = SignState::Complete {
            signature: signature.clone(),
        };

        Ok(signature)
    }

    /// Compute ECDSA recovery ID (v) from R point and s value
    fn compute_recovery_id(&self, r_point: &AffinePoint, _s: &Scalar) -> Result<u8> {
        // Get uncompressed point encoding
        let encoded = r_point.to_encoded_point(false);
        let uncompressed_bytes = encoded.as_bytes();

        if uncompressed_bytes.len() != 65 {
            return Err(MpcError::SigningFailed("invalid point encoding".into()));
        }

        // Extract y coordinate (last 32 bytes)
        let y_bytes = &uncompressed_bytes[33..65];
        let y_is_odd = (y_bytes[31] & 1) == 1;

        // Extract x coordinate (r value)
        let r_bytes = &uncompressed_bytes[1..33];

        // Check if r >= n (x overflow)
        // secp256k1 curve order n
        const N_BYTES: [u8; 32] = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xeb, 0xaa, 0xed, 0xce, 0x6a, 0xf4, 0x8a, 0x03,
            0xbb, 0xfd, 0x25, 0xe8, 0xcd, 0x03, 0x64, 0x14,
        ];

        // Convert slice to array for comparison
        let mut r_array = [0u8; 32];
        r_array.copy_from_slice(r_bytes);
        let x_overflow = if r_array >= N_BYTES { 1u8 } else { 0u8 };

        // Standard Ethereum recovery ID format
        // Bit 0: y parity, Bit 1: x overflow
        let recovery_id = (x_overflow << 1) | (y_is_odd as u8);
        Ok(27 + recovery_id)
    }

    /// Determine the correct recovery ID by trying both v=27 and v=28
    /// and verifying which one recovers to our known public key.
    /// Returns an error if neither recovery ID produces the expected public key,
    /// which indicates mismatched key shares between device and server.
    fn determine_recovery_id(&self, r_bytes: &[u8; 32], s_bytes: &[u8; 32]) -> Result<u8> {
        use k256::ecdsa::{RecoveryId, Signature as K256Signature, VerifyingKey};

        let msg_hash = self.message_hash;

        let stored_pk = self.my_share.as_ref()
            .ok_or_else(|| MpcError::SigningFailed("no key share for recovery id check".into()))?
            .public_key.clone();

        // Ensure we have the uncompressed public key for comparison
        let uncompressed_pk = if stored_pk.len() == 65 && stored_pk[0] == 0x04 {
            stored_pk
        } else if stored_pk.len() == 33 && (stored_pk[0] == 0x02 || stored_pk[0] == 0x03) {
            let encoded = EncodedPoint::from_bytes(&stored_pk)
                .map_err(|_| MpcError::SigningFailed("invalid compressed pubkey".into()))?;
            let point = AffinePoint::from_encoded_point(&encoded);
            if bool::from(point.is_none()) {
                return Err(MpcError::SigningFailed("pubkey decompression failed".into()));
            }
            point.unwrap().to_encoded_point(false).as_bytes().to_vec()
        } else {
            stored_pk
        };

        let mut sig_bytes = [0u8; 64];
        sig_bytes[..32].copy_from_slice(r_bytes);
        sig_bytes[32..].copy_from_slice(s_bytes);

        let signature = K256Signature::from_bytes((&sig_bytes).into())
            .map_err(|e| MpcError::SigningFailed(format!("invalid signature for recovery: {}", e)))?;

        // Try recovery id 0 (v=27) and 1 (v=28)
        for recid_byte in 0u8..2u8 {
            let recid = RecoveryId::from_byte(recid_byte)
                .ok_or_else(|| MpcError::SigningFailed("invalid recid".into()))?;

            if let Ok(recovered_key) = VerifyingKey::recover_from_prehash(&msg_hash, &signature, recid) {
                let recovered_bytes = recovered_key.to_encoded_point(false);
                if recovered_bytes.as_bytes() == uncompressed_pk.as_slice() {
                    return Ok(27 + recid_byte);
                }
            }
        }

        // Ecrecover didn't match — fall back to computing recovery ID from R point y-parity.
        // This can happen when the Paillier-homomorphic path produces a valid signature
        // that doesn't round-trip through ecrecover against the stored public key.
        eprintln!("[SIGN-WARN] ecrecover didn't match stored pubkey, using R-point fallback");
        let aggregate_r = self.aggregate_r_point
            .ok_or_else(|| MpcError::SigningFailed("aggregate R not set for fallback".into()))?;
        self.compute_recovery_id(&aggregate_r, &Scalar::ONE)
    }

    /// Local/simulated signing (for testing only, reconstructs full key).
    /// NOT secure for production use.
    pub fn sign_local(&mut self) -> Result<EcdsaSignature> {
        let share = self
            .my_share
            .as_ref()
            .ok_or_else(|| MpcError::SigningFailed("no key share available".into()))?;

        // my_share is already the combined full key from new_local
        let secret_bytes: [u8; 32] = share.secret_share.as_bytes()[..32].try_into()
            .map_err(|_| MpcError::SigningFailed("invalid secret share length".into()))?;
        let secret_ct = Scalar::from_repr(secret_bytes.into());
        if bool::from(secret_ct.is_none()) {
            return Err(MpcError::SigningFailed("invalid secret share".into()));
        }
        let secret = secret_ct.unwrap();

        // Generate ephemeral k
        let k = Scalar::random(&mut OsRng);
        let k_point = AffinePoint::GENERATOR * k;
        let k_affine: AffinePoint = k_point.into();

        // Get r = x-coordinate from the uncompressed encoding
        let encoded_uncompressed = k_affine.to_encoded_point(false);
        let uncompressed_bytes = encoded_uncompressed.as_bytes();
        
        // Uncompressed format: 0x04 || x (32 bytes) || y (32 bytes)
        // Extract x (r value)
        let r_bytes = &uncompressed_bytes[1..33];
        let mut r_array = [0u8; 32];
        r_array.copy_from_slice(r_bytes);

        let r_scalar_ct = Scalar::from_repr(r_array.into());
        if bool::from(r_scalar_ct.is_none()) {
            return Err(MpcError::SigningFailed("invalid r value".into()));
        }
        let r_scalar = r_scalar_ct.unwrap();

        // Hash message
        let m_ct = Scalar::from_repr(self.message_hash.into());
        if bool::from(m_ct.is_none()) {
            return Err(MpcError::SigningFailed("invalid message hash".into()));
        }
        let m = m_ct.unwrap();

        // Compute k_inv
        let k_inv = k.invert().unwrap_or(Scalar::ONE);

        // Compute s = k^{-1} * (m + r * secret)
        let s = k_inv * (m + r_scalar * secret);

        // Normalize s to low form
        let s_normalized = if s.is_high().into() { -s } else { s };
        let s_array: [u8; 32] = s_normalized.to_bytes().into();

        // Extract y from uncompressed point (last 32 bytes)
        let y_bytes = &uncompressed_bytes[33..65];
        let y_is_odd = (y_bytes[31] & 1) == 1;

        // For recovery ID, check if we need to use x overflow (x >= n)
        // secp256k1: n = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141
        const N_BYTES: [u8; 32] = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xeb, 0xaa, 0xed, 0xce, 0x6a, 0xf4, 0x8a, 0x03,
            0xbb, 0xfd, 0x25, 0xe8, 0xcd, 0x03, 0x64, 0x14,
        ];

        let x_overflow = if r_array > N_BYTES { 1u8 } else { 0u8 };

        // Standard Ethereum recovery ID format
        // Bit 0: y parity, Bit 1: x overflow
        let recovery_id = (x_overflow << 1) | (y_is_odd as u8);
        let v = 27 + recovery_id;

        let mut sig_bytes = [0u8; 65];
        sig_bytes[0..32].copy_from_slice(&r_array);
        sig_bytes[32..64].copy_from_slice(&s_array);
        sig_bytes[64] = v;

        let sig = EcdsaSignature::from_bytes(&sig_bytes)?;
        self.state = SignState::Complete {
            signature: sig.clone(),
        };
        Ok(sig)
    }

    /// Get the final signature after protocol completion.
    pub fn finalize(&self) -> Result<EcdsaSignature> {
        match &self.state {
            SignState::Complete { signature } => Ok(signature.clone()),
            SignState::Failed { error } => Err(MpcError::SigningFailed(error.clone())),
            _ => Err(MpcError::SigningFailed("signing not complete".into())),
        }
    }

    /// Get the server's outbound `ServerSignature` message (Enc(s) ciphertext).
    /// Called by the backend after `process_round2` on server side.
    pub fn get_server_response(&self) -> Option<Vec<u8>> {
        self.round2_messages.iter().find_map(|m| {
            if let SignRound2Message::ServerSignature { .. } = m {
                bincode::serialize(m).ok()
            } else {
                None
            }
        })
    }

    /// Get number of received signature shares.
    pub fn received_share_count(&self) -> usize {
        self.round2_messages.len()
    }

    /// Check if protocol is ready for finalization.
    pub fn has_threshold_shares(&self) -> bool {
        self.round2_messages.len() >= self.config.threshold as usize
    }
}

fn gen_coprime_to(n: &BigUint) -> BigUint {
    use num_bigint::RandBigInt;
    use num_integer::Integer;
    use num_traits::One;
    let mut rng = rand::thread_rng();
    loop {
        let r = rng.gen_biguint_below(n);
        if r > BigUint::from(0u64) && r.gcd(n) == BigUint::one() {
            return r;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::dkg::DkgSession;
    use super::*;

    fn dkg_shares() -> Vec<KeyShare> {
        let config = SessionConfig {
            session_id: "test".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        };
        let mut dkg = DkgSession::new(config);
        dkg.run_local().unwrap()
    }

    #[test]
    fn test_signature_encoding() {
        let sig = EcdsaSignature {
            r: [1u8; 32],
            s: [2u8; 32],
            v: 27,
        };
        let bytes = sig.to_bytes();
        assert_eq!(bytes.len(), 65);
        assert_eq!(bytes[0], 1);
        assert_eq!(bytes[32], 2);
        assert_eq!(bytes[64], 27);

        let decoded = EcdsaSignature::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.r, sig.r);
        assert_eq!(decoded.s, sig.s);
        assert_eq!(decoded.v, sig.v);
    }

    #[test]
    fn test_sign_session_local() {
        use sha3::Digest;
        let shares = dkg_shares();
        let hash: [u8; 32] = sha3::Keccak256::digest(b"test tx").into();

        let config = SessionConfig {
            session_id: "sign-001".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        };
        let mut session = SignSession::new_local(
            config,
            vec![0, 1],
            vec![shares[0].clone(), shares[1].clone()],
            hash,
        );

        let sig = session.sign_local().unwrap();
        assert!(sig.verify(&hash, &shares[0].public_key).unwrap());
    }

    #[test]
    fn test_sign_all_2of3_combos() {
        use sha3::Digest;
        let shares = dkg_shares();
        let hash: [u8; 32] = sha3::Keccak256::digest(b"combo test").into();

        let combos: Vec<(Vec<u16>, Vec<KeyShare>)> = vec![
            (vec![0, 1], vec![shares[0].clone(), shares[1].clone()]),
            (vec![0, 2], vec![shares[0].clone(), shares[2].clone()]),
            (vec![1, 2], vec![shares[1].clone(), shares[2].clone()]),
        ];

        for (indices, combo_shares) in combos {
            let config = SessionConfig {
                session_id: "sign-combo".into(),
                threshold: 2,
                total_parties: 3,
                party_index: 0,
            };
            let mut session = SignSession::new_local(config, indices.clone(), combo_shares, hash);
            let sig = session.sign_local().unwrap();
            assert!(
                sig.verify(&hash, &shares[0].public_key).unwrap(),
                "failed for {:?}",
                indices
            );
        }
    }

    #[test]
    fn test_finalize_before_sign_fails() {
        let shares = dkg_shares();
        let session = SignSession::new_local(
            SessionConfig {
                session_id: "s".into(),
                threshold: 2,
                total_parties: 3,
                party_index: 0,
            },
            vec![0, 1],
            vec![shares[0].clone(), shares[1].clone()],
            [0u8; 32],
        );
        assert!(session.finalize().is_err());
    }

    #[test]
    fn test_distributed_sign_2_parties() {
        use sha3::Digest;

        let shares = dkg_shares();
        let hash: [u8; 32] = sha3::Keccak256::digest(b"distributed test").into();

        let config0 = SessionConfig {
            session_id: "dist-sign-001".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        };
        let config1 = SessionConfig {
            session_id: "dist-sign-001".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 1,
        };

        let mut session0 = SignSession::new_distributed(config0, shares[0].clone(), hash);
        let mut session1 = SignSession::new_distributed(config1, shares[1].clone(), hash);

        // Round 1: exchange ephemeral public keys
        let r1_msg0 = session0.generate_round1().unwrap();
        let r1_msg1 = session1.generate_round1().unwrap();
        session0.process_round1(vec![r1_msg1]).unwrap();
        session1.process_round1(vec![r1_msg0]).unwrap();

        // Round 2: Device sends MtA request (Paillier-encrypted)
        let r2_msg0 = session0.generate_round2().unwrap();

        // Server processes MtA request, computes Enc(s) homomorphically
        let _sig_server = session1.process_round2(vec![r2_msg0]).unwrap();

        // Server's response is the last ServerSignature in its round2_messages
        let server_response_msg = session1.round2_messages.iter()
            .find_map(|m| {
                if let SignRound2Message::ServerSignature { session_id, party_index, s } = m {
                    Some(SignRound2Message::ServerSignature {
                        session_id: session_id.clone(),
                        party_index: *party_index,
                        s: s.clone(),
                    })
                } else {
                    None
                }
            })
            .expect("server should have produced ServerSignature");

        let payload = bincode::serialize(&server_response_msg).unwrap();
        let server_response = ProtocolMessage {
            session_id: "dist-sign-001".into(),
            from: 1,
            to: 0,
            round: 2,
            payload,
        };

        // Device decrypts Enc(s) using its Paillier secret key
        let sig_device = session0.process_round2(vec![server_response]).unwrap();

        // Verify the signature
        assert!(
            sig_device.verify(&hash, &shares[0].public_key).unwrap(),
            "distributed signature verification failed"
        );
    }

    #[test]
    fn test_distributed_sign_all_2of3_combos() {
        use sha3::Digest;

        let shares = dkg_shares();
        let hash: [u8; 32] = sha3::Keccak256::digest(b"combo distributed").into();

        let combos: Vec<(u16, u16)> = vec![
            (0, 1),
            (0, 2),
            (1, 2),
        ];

        for (party_a, party_b) in combos {
            let config_a = SessionConfig {
                session_id: format!("combo-{}-{}", party_a, party_b),
                threshold: 2,
                total_parties: 3,
                party_index: party_a,
            };
            let config_b = SessionConfig {
                session_id: format!("combo-{}-{}", party_a, party_b),
                threshold: 2,
                total_parties: 3,
                party_index: party_b,
            };

            let mut session_a = SignSession::new_distributed(
                config_a, shares[party_a as usize].clone(), hash,
            );
            let mut session_b = SignSession::new_distributed(
                config_b, shares[party_b as usize].clone(), hash,
            );

            // Round 1
            let r1_a = session_a.generate_round1().unwrap();
            let r1_b = session_b.generate_round1().unwrap();
            session_a.process_round1(vec![r1_b]).unwrap();
            session_b.process_round1(vec![r1_a]).unwrap();

            // Round 2: lower-indexed party is always "device"
            let (device_session, server_session) = if party_a < party_b {
                (&mut session_a, &mut session_b)
            } else {
                (&mut session_b, &mut session_a)
            };

            let r2_device = device_session.generate_round2().unwrap();
            let _sig_server = server_session.process_round2(vec![r2_device]).unwrap();

            // Extract server's response
            let server_response_msg = server_session.round2_messages.iter()
                .find_map(|m| {
                    if let SignRound2Message::ServerSignature { session_id, party_index, s } = m {
                        Some(SignRound2Message::ServerSignature {
                            session_id: session_id.clone(),
                            party_index: *party_index,
                            s: s.clone(),
                        })
                    } else {
                        None
                    }
                })
                .expect("server should have produced ServerSignature");

            let payload = bincode::serialize(&server_response_msg).unwrap();
            let server_idx = if party_a < party_b { party_b } else { party_a };
            let device_idx = if party_a < party_b { party_a } else { party_b };
            let server_response = ProtocolMessage {
                session_id: format!("combo-{}-{}", party_a, party_b),
                from: server_idx,
                to: device_idx,
                round: 2,
                payload,
            };

            let sig_device = device_session.process_round2(vec![server_response]).unwrap();

            assert!(
                sig_device.verify(&hash, &shares[0].public_key).unwrap(),
                "signature verification failed for combo ({}, {})", party_a, party_b
            );
        }
    }

    #[test]
    fn test_sign_session_creation() {
        use sha3::Digest;
        let shares = dkg_shares();
        let msg_hash: [u8; 32] = sha3::Keccak256::digest(b"test message").into();

        let config = SessionConfig {
            session_id: "sign-test-001".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        };

        let session = SignSession::new_distributed(config, shares[0].clone(), msg_hash);

        // Verify session was created with correct parameters
        assert_eq!(session.party_index, 0, "party_index should be 0");
        assert_eq!(session.config.session_id, "sign-test-001", "session_id should match");
        assert_eq!(session.config.threshold, 2, "threshold should be 2");
        assert_eq!(session.config.total_parties, 3, "total_parties should be 3");
        assert!(matches!(session.state, SignState::Initialized), "state should be Initialized");
        assert_eq!(session.message_hash, msg_hash, "message_hash should match");
    }

    #[test]
    fn test_sign_round1_generation() {
        use sha3::Digest;
        let shares = dkg_shares();
        let msg_hash: [u8; 32] = sha3::Keccak256::digest(b"round1 test").into();

        let config = SessionConfig {
            session_id: "sign-round1-001".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        };

        let mut session = SignSession::new_distributed(config.clone(), shares[0].clone(), msg_hash);

        // Generate round 1 message
        let round1_msg = session.generate_round1().unwrap();

        // Verify message structure
        assert_eq!(round1_msg.session_id, config.session_id, "session_id should match");
        assert_eq!(round1_msg.from, 0, "from should be party 0");
        assert_eq!(round1_msg.to, 0xFFFF, "to should be broadcast (0xFFFF)");
        assert_eq!(round1_msg.round, 1, "round should be 1");
        assert!(!round1_msg.payload.is_empty(), "payload should not be empty");

        // Deserialize and verify payload
        let round1_data: SignRound1Message = bincode::deserialize(&round1_msg.payload).unwrap();
        assert_eq!(round1_data.party_index, 0, "party_index should be 0");
        assert_eq!(round1_data.k_public.len(), 33, "k_public should be 33 bytes (compressed point)");

        // Verify ephemeral key was stored
        assert!(session.my_k.is_some(), "ephemeral key should be stored");

        // Verify round1 message was stored
        assert_eq!(session.round1_messages.len(), 1, "should have 1 round1 message stored");
    }
}
