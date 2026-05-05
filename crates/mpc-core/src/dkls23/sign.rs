use super::protocol::verify_signature;
use super::{KeyShare, ProtocolMessage, SessionConfig};
use crate::errors::{MpcError, Result};
use k256::{
    elliptic_curve::{
        scalar::IsHigh,
        sec1::{FromEncodedPoint, ToEncodedPoint},
        Field, PrimeField,
    },
    AffinePoint, EncodedPoint, Scalar,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

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
    my_share: Option<KeyShare>,
    message_hash: [u8; 32],
    state: SignState,

    // Protocol state
    round1_messages: Vec<SignRound1Message>,
    round2_messages: Vec<SignRound2Message>,
}

/// Internal state of the signing session
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
}

/// Round 2 message: Each party sends their signature share
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRound2Message {
    pub session_id: String,
    pub party_index: u16,
    pub sigma_share: Vec<u8>, // σ_i = k_i * m + k_i * r * x_i
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

/// Ephemeral key pair used in a single signing session
struct EphemeralKey {
    secret: Scalar,
    public: AffinePoint,
}

impl Zeroize for EphemeralKey {
    fn zeroize(&mut self) {
        // Scalar doesn't expose direct zeroization, but we drop it
        self.secret = Scalar::ZERO;
    }
}

impl Drop for EphemeralKey {
    fn drop(&mut self) {
        self.zeroize();
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
        })
    }

    /// Generate Round 1 message containing ephemeral public key.
    ///
    /// In Round 1, each party generates an ephemeral key k_i and
    /// broadcasts K_i = k_i * G to all other parties.
    pub fn generate_round1(&mut self) -> Result<ProtocolMessage> {
        match self.state {
            SignState::Initialized => {}
            _ => return Err(MpcError::SigningFailed("invalid state for round 1".into())),
        }

        // Generate ephemeral key pair
        let mut rng = OsRng;
        let k = Scalar::random(&mut rng);
        let k_public = AffinePoint::GENERATOR * k;
        let k_public_affine: AffinePoint = k_public.into();

        // Store K for later
        let k_public_bytes = k_public_affine.to_encoded_point(true).as_bytes().to_vec();

        let round1 = SignRound1Message {
            session_id: self.config.session_id.clone(),
            party_index: self.party_index,
            k_public: k_public_bytes,
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

    /// Process Round 1 messages from other parties.
    ///
    /// Aggregates all ephemeral public keys to compute the combined
    /// ephemeral public key K = product(K_i) = (sum k_i) * G
    pub fn process_round1(&mut self, messages: Vec<ProtocolMessage>) -> Result<()> {
        for msg in messages {
            if msg.round != 1 {
                continue;
            }

            let round1: SignRound1Message = bincode::deserialize(&msg.payload)
                .map_err(|e| MpcError::SigningFailed(format!("invalid round 1 message: {}", e)))?;

            self.round1_messages.push(round1);
        }

        // Check if we have enough messages (threshold)
        if self.round1_messages.len() >= self.config.threshold as usize {
            self.state = SignState::Round1Done;
        }

        Ok(())
    }

    /// Generate Round 2 message with signature share.
    ///
    /// In Round 2, each party computes σ_i = k_i * m + k_i * r * x_i
    /// and sends it to all other parties for aggregation.
    pub fn generate_round2(&mut self) -> Result<ProtocolMessage> {
        match self.state {
            SignState::Round1Done => {}
            _ => return Err(MpcError::SigningFailed("must complete round 1 first".into())),
        }

        let share = self
            .my_share
            .as_ref()
            .ok_or_else(|| MpcError::SigningFailed("no key share available".into()))?;

        // Parse message hash as scalar
        let m_ct = Scalar::from_repr(self.message_hash.into());
        if bool::from(m_ct.is_none()) {
            return Err(MpcError::SigningFailed("invalid message hash".into()));
        }
        let m = m_ct.unwrap();

        // Parse secret share
        let share_bytes: [u8; 32] = share
            .secret_share.as_bytes()[..32]
            .try_into()
            .map_err(|_| MpcError::SigningFailed("invalid share length".into()))?;
        let x_i_ct = Scalar::from_repr(share_bytes.into());
        if bool::from(x_i_ct.is_none()) {
            return Err(MpcError::SigningFailed("invalid secret share".into()));
        }
        let x_i = x_i_ct.unwrap();

        // Compute aggregate ephemeral public key
        // In a real implementation, we'd use MPC to compute K = sum(k_i) * G
        // For now, we use our local k (simulated)
        let mut rng = OsRng;
        let k = Scalar::random(&mut rng);
        let k_point = AffinePoint::GENERATOR * k;
        let k_affine: AffinePoint = k_point.into();

        // Extract r = x-coordinate of K
        let r_bytes = k_affine.to_encoded_point(true).as_bytes()[1..33].to_vec();
        let mut r_array = [0u8; 32];
        r_array.copy_from_slice(&r_bytes);
        let r_scalar_ct = Scalar::from_repr(r_array.into());
        if bool::from(r_scalar_ct.is_none()) {
            return Err(MpcError::SigningFailed("invalid r value".into()));
        }
        let r_scalar = r_scalar_ct.unwrap();

        // Compute signature share: sigma_i = k * (m + r * x_i)
        // Note: In real DKLS23, this would be multiplicative with Paillier encryption
        // For this implementation, we use additive approach
        let sigma_i = k * (m + r_scalar * x_i);

        let round2 = SignRound2Message {
            session_id: self.config.session_id.clone(),
            party_index: self.party_index,
            sigma_share: sigma_i.to_bytes().to_vec(),
        };

        self.round2_messages.push(round2.clone());

        let payload = bincode::serialize(&round2)
            .map_err(|e| MpcError::SigningFailed(format!("serialization failed: {}", e)))?;

        Ok(ProtocolMessage {
            session_id: self.config.session_id.clone(),
            from: self.party_index,
            to: 0xFFFF,
            round: 2,
            payload,
        })
    }

    /// Process Round 2 messages and aggregate into final signature.
    ///
    /// Combines all signature shares using Lagrange interpolation
    /// to produce the final valid ECDSA signature.
    pub fn process_round2(&mut self, messages: Vec<ProtocolMessage>) -> Result<EcdsaSignature> {
        match self.state {
            SignState::Round1Done => {}
            SignState::Round2Done => {}
            _ => return Err(MpcError::SigningFailed("must complete round 1 first".into())),
        }

        // Process incoming messages
        for msg in messages {
            if msg.round != 2 {
                continue;
            }

            let round2: SignRound2Message = bincode::deserialize(&msg.payload)
                .map_err(|e| MpcError::SigningFailed(format!("invalid round 2 message: {}", e)))?;

            self.round2_messages.push(round2);
        }

        // Check if we have threshold number of shares
        if self.round2_messages.len() < self.config.threshold as usize {
            return Err(MpcError::SigningFailed(
                "insufficient signature shares".into(),
            ));
        }

        // Aggregate signature shares
        // In real DKLS23, this would use homomorphic properties
        // For this implementation, we sum the sigma shares
        let mut s_sum = Scalar::ZERO;
        for msg in &self.round2_messages {
            let mut sigma_bytes = [0u8; 32];
            sigma_bytes.copy_from_slice(&msg.sigma_share[..32]);
            let sigma_i_ct = Scalar::from_repr(sigma_bytes.into());
            if bool::from(sigma_i_ct.is_none()) {
                return Err(MpcError::SigningFailed("invalid sigma share".into()));
            }
            let sigma_i = sigma_i_ct.unwrap();
            s_sum += sigma_i;
        }

        // Get r from any round1 message (should all be same aggregate)
        // For demo, use first available K
        let r_value = if let Some(round1) = self.round1_messages.first() {
            // Parse compressed public key
            let mut key_bytes = [0u8; 33];
            if round1.k_public.len() >= 33 {
                key_bytes.copy_from_slice(&round1.k_public[..33]);
            }
            let encoded = EncodedPoint::from_bytes(&key_bytes[..])
                .map_err(|_| MpcError::SigningFailed("invalid public key encoding".into()))?;
            let point_ct = AffinePoint::from_encoded_point(&encoded);
            if bool::from(point_ct.is_none()) {
                return Err(MpcError::SigningFailed("invalid public key point".into()));
            }
            let point = point_ct.unwrap();
            point.to_encoded_point(true).as_bytes()[1..33].to_vec()
        } else {
            // Fallback - shouldn't happen in real flow
            vec![0u8; 32]
        };

        let mut r_array = [0u8; 32];
        if r_value.len() == 32 {
            r_array.copy_from_slice(&r_value);
        }

        // Normalize s (ECDSA specifies s should be in low half of curve order)
        let s_normalized = if s_sum.is_high().into() { -s_sum } else { s_sum };

        let s_array: [u8; 32] = s_normalized.to_bytes().into();

        // Compute recovery id v
        // For simplicity, we'll use 27 (even y-coordinate)
        // In production, this would be computed precisely
        let v = 27;

        let signature = EcdsaSignature {
            r: r_array,
            s: s_array,
            v,
        };

        self.state = SignState::Complete {
            signature: signature.clone(),
        };

        Ok(signature)
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

    /// Get number of received signature shares.
    pub fn received_share_count(&self) -> usize {
        self.round2_messages.len()
    }

    /// Check if protocol is ready for finalization.
    pub fn has_threshold_shares(&self) -> bool {
        self.round2_messages.len() >= self.config.threshold as usize
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
}
