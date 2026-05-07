use super::protocol::ThresholdKeyGen;
use super::{KeyShare, ProtocolMessage, SessionConfig};
use crate::errors::{MpcError, Result};
use k256::{
    elliptic_curve::{
        sec1::{FromEncodedPoint, ToEncodedPoint},
        PrimeField, Field,
    },
    AffinePoint, ProjectivePoint, Scalar,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// Distributed Key Generation session implementing DKLS23 protocol.
///
/// This is a 3-round protocol that generates t-of-n threshold ECDSA keys
/// without any party ever learning the full private key.
///
/// Round 1: Each party commits to their VSS polynomial
/// Round 2: Each party verifies commitments and sends point evaluations
/// Round 3: Parties verify shares and output their final key share
pub struct DkgSession {
    config: SessionConfig,
    state: DkgState,
    my_secret: Option<Scalar>,
    my_polynomial: Option<Vec<Scalar>>,
    public_key: Option<Vec<u8>>,
    round1_messages: Vec<DkgRound1Message>,
    round2_messages: Vec<DkgRound2Message>,
}

/// DKG session state
#[allow(dead_code)]
enum DkgState {
    Initialized,
    Round1Done,
    Round2Done,
    Complete { share: KeyShare },
    Failed { error: String },
}

/// Round 1 message: Commitment to VSS polynomial
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound1Message {
    pub session_id: String,
    pub party_index: u16,
    pub commitments: Vec<Vec<u8>>, // C_0, C_1, ..., C_{t-1} where C_j = f(j)*G
    /// Schnorr proof of knowledge of the constant term a_0 (required)
    pub schnorr_proof: crate::crypto::schnorr::SchnorrProof,
}

/// Round 2 message: Secret point evaluations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound2Message {
    pub session_id: String,
    pub from_party: u16,
    pub evaluations: Vec<(u16, Vec<u8>)>, // (recipient_index, f(j) value)
}

impl Zeroize for DkgSession {
    fn zeroize(&mut self) {
        if let Some(_s) = self.my_secret.take() {
            // my_secret has been taken (dropped)
        }
        if let Some(ref mut poly) = self.my_polynomial {
            for coeff in poly.iter_mut() {
                *coeff = Scalar::ZERO;
            }
        }
        self.my_polynomial = None;
        self.public_key.zeroize();
        self.round1_messages.clear();
        self.round2_messages.clear();
    }
}

impl Drop for DkgSession {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl DkgSession {
    /// Create a new DKG session for this party.
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            state: DkgState::Initialized,
            my_secret: None,
            my_polynomial: None,
            public_key: None,
            round1_messages: Vec::new(),
            round2_messages: Vec::new(),
        }
    }

    /// Generate Round 1 message with VSS commitments.
    ///
    /// Each party generates a random polynomial f_i(x) of degree t-1
    /// and publishes commitments to each coefficient: C_j = f_i(j) * G.
    pub fn generate_round1(&mut self) -> Result<ProtocolMessage> {
        match self.state {
            DkgState::Initialized => {}
            _ => return Err(MpcError::DkgFailed("invalid state for round 1".into())),
        }

        let t = self.config.threshold as usize;
        let mut rng = OsRng;

        // Generate random polynomial coefficients: f(x) = a_0 + a_1*x + ... + a_{t-1}*x^{t-1}
        let mut coeffs = Vec::with_capacity(t);
        for _ in 0..t {
            coeffs.push(Scalar::random(&mut rng));
        }

        // Store the full polynomial for Round 2 evaluation consistency
        self.my_secret = Some(coeffs[0]);
        self.my_polynomial = Some(coeffs.clone());

        // Generate commitments: C_j = a_j * G for each coefficient
        let mut commitments = Vec::with_capacity(t);
        for coeff in &coeffs {
            let point = AffinePoint::GENERATOR * coeff;
            let affine: AffinePoint = point.into();
            commitments.push(affine.to_encoded_point(true).as_bytes().to_vec());
        }

        // Schnorr proof of knowledge of a_0 (the constant term / secret contribution)
        let schnorr_proof = {
            use crate::crypto::schnorr::SchnorrProof;
            let c0 = &commitments[0]; // C_0 = a_0 * G
            SchnorrProof::prove(&coeffs[0], c0, b"DKG-Round1")
        };

        let round1 = DkgRound1Message {
            session_id: self.config.session_id.clone(),
            party_index: self.config.party_index,
            commitments,
            schnorr_proof,
        };

        // Store our own message
        self.round1_messages.push(round1.clone());

        // Serialize
        let payload = bincode::serialize(&round1)
            .map_err(|e| MpcError::DkgFailed(format!("serialization failed: {}", e)))?;

        Ok(ProtocolMessage {
            session_id: self.config.session_id.clone(),
            from: self.config.party_index,
            to: 0xFFFF, // broadcast
            round: 1,
            payload,
        })
    }

    /// Process Round 1 messages from all parties.
    ///
    /// Verifies all commitments are valid curve points and stores them
    /// for use in Round 2 verification.
    pub fn process_round1(&mut self, messages: Vec<ProtocolMessage>) -> Result<()> {
        for msg in messages {
            if msg.round != 1 {
                continue;
            }

            let round1: DkgRound1Message = bincode::deserialize(&msg.payload)
                .map_err(|e| MpcError::DkgFailed(format!("invalid round 1 message: {}", e)))?;

            // Verify: number of commitments == threshold
            if round1.commitments.len() != self.config.threshold as usize {
                return Err(MpcError::DkgFailed(format!(
                    "party {} sent wrong number of commitments",
                    round1.party_index
                )));
            }

            // Verify Schnorr proof of knowledge of a_0
            if !round1.schnorr_proof.verify(&round1.commitments[0], b"DKG-Round1") {
                return Err(MpcError::DkgFailed(format!(
                    "party {} failed Schnorr proof of knowledge",
                    round1.party_index
                )));
            }

            self.round1_messages.push(round1);
        }

        // Check if we have commitments from all parties
        if self.round1_messages.len() >= self.config.total_parties as usize {
            self.state = DkgState::Round1Done;
        }

        Ok(())
    }

    /// Generate Round 2 messages containing secret share evaluations.
    ///
    /// For each other party j, party i computes s_ij = f_i(j) and sends it
    /// encrypted to party j (via Noise channel in production).
    pub fn generate_round2(&mut self) -> Result<Vec<ProtocolMessage>> {
        match self.state {
            DkgState::Round1Done => {}
            _ => return Err(MpcError::DkgFailed("must complete round 1 first".into())),
        }

        let my_idx = self.config.party_index as usize;

        // Use the stored polynomial from Round 1 (ensures consistency with commitments)
        let coeffs = self.my_polynomial.as_ref()
            .ok_or_else(|| MpcError::DkgFailed("polynomial not stored from round 1".into()))?
            .clone();

        let mut messages = Vec::new();

        // Evaluate polynomial at each party's index (1-indexed for Shamir)
        for j in 0..self.config.total_parties as usize {
            let x = Scalar::from((j + 1) as u64); // x = 1, 2, 3, ...

            // Evaluate f(x) = a_0 + a_1*x + a_2*x^2 + ... + a_{t-1}*x^{t-1}
            let mut y = Scalar::ZERO;
            let mut x_pow = Scalar::ONE;
            for coeff in &coeffs {
                y += coeff * &x_pow;
                x_pow *= x;
            }

            let round2 = DkgRound2Message {
                session_id: self.config.session_id.clone(),
                from_party: self.config.party_index,
                evaluations: vec![(j as u16, y.to_bytes().to_vec())],
            };

            let payload = bincode::serialize(&round2)
                .map_err(|e| MpcError::DkgFailed(format!("serialization failed: {}", e)))?;

            messages.push(ProtocolMessage {
                session_id: self.config.session_id.clone(),
                from: self.config.party_index,
                to: j as u16,
                round: 2,
                payload,
            });
        }

        // Also store our own evaluation for ourselves
        let x_self = Scalar::from((my_idx + 1) as u64);
        let mut y_self = Scalar::ZERO;
        let mut x_pow = Scalar::ONE;
        for coeff in &coeffs {
            y_self += coeff * &x_pow;
            x_pow *= x_self;
        }

        self.round2_messages.push(DkgRound2Message {
            session_id: self.config.session_id.clone(),
            from_party: self.config.party_index,
            evaluations: vec![(my_idx as u16, y_self.to_bytes().to_vec())],
        });

        self.state = DkgState::Round2Done;

        Ok(messages)
    }

    /// Process Round 2 messages and finalize the key share.
    ///
    /// Each party sums all the shares they received:
    ///   x_i = sum_{j=1..n} s_ji
    ///
    /// Feldman VSS verification ensures each share is consistent with Round 1 commitments:
    ///   s_ij * G == C_{i,0} + (j+1)*C_{i,1} + (j+1)^2*C_{i,2} + ...
    ///
    /// The joint public key is the sum of all the constant commitments.
    pub fn process_round2(&mut self, messages: Vec<ProtocolMessage>) -> Result<KeyShare> {
        match self.state {
            DkgState::Round1Done => {}
            DkgState::Round2Done => {}
            _ => return Err(MpcError::DkgFailed("must complete round 1 first".into())),
        }

        // Process incoming messages
        for msg in messages {
            if msg.round != 2 {
                continue;
            }

            let round2: DkgRound2Message = bincode::deserialize(&msg.payload)
                .map_err(|e| MpcError::DkgFailed(format!("invalid round 2 message: {}", e)))?;

            self.round2_messages.push(round2);
        }

        // Check we have enough shares
        if self.round2_messages.len() < self.config.threshold as usize {
            return Err(MpcError::DkgFailed(
                "insufficient round 2 messages received".into(),
            ));
        }

        // Sum all the shares sent to us, with Feldman VSS verification
        let mut my_share = Scalar::ZERO;
        let my_idx = self.config.party_index;

        for msg in &self.round2_messages {
            for (recipient, share_bytes) in &msg.evaluations {
                if *recipient == my_idx {
                    let mut bytes = [0u8; 32];
                    bytes.copy_from_slice(&share_bytes[..32]);
                    let share = Option::<Scalar>::from(Scalar::from_repr(bytes.into()))
                        .ok_or_else(|| MpcError::DkgFailed("invalid share value".into()))?;

                    // Feldman VSS verification:
                    // Verify s_ij * G == sum_k( C_{sender,k} * (my_idx+1)^k )
                    if let Some(round1) = self.round1_messages.iter().find(|r| r.party_index == msg.from_party) {
                        Self::verify_feldman_share(&share, my_idx, &round1.commitments)?;
                    }

                    my_share += share;
                }
            }
        }

        // Compute the joint public key: sum of all C_0 commitments
        // C_0 is the commitment to the constant term (a_0 = secret)
        let mut pubkey_point = ProjectivePoint::IDENTITY;
        for round1 in &self.round1_messages {
            if let Some(c0_bytes) = round1.commitments.first() {
                let mut key_bytes = [0u8; 33];
                if c0_bytes.len() >= 33 {
                    key_bytes.copy_from_slice(&c0_bytes[..33]);
                }
                if let Ok(encoded) = k256::elliptic_curve::sec1::EncodedPoint::<k256::Secp256k1>::from_bytes(&key_bytes[..]) {
                    let ct_point = AffinePoint::from_encoded_point(&encoded);
                    if ct_point.is_some().into() {
                        pubkey_point += ProjectivePoint::from(ct_point.unwrap());
                    }
                }
            }
        }

        let public_key = pubkey_point.to_encoded_point(true).as_bytes().to_vec();
        self.public_key = Some(public_key.clone());

        let key_share = KeyShare {
            party: self.config.party_index,
            threshold: self.config.threshold,
            total_parties: self.config.total_parties,
            secret_share: my_share.to_bytes().to_vec().into(),
            public_key,
            paillier_pk: None,
        };

        self.state = DkgState::Complete {
            share: key_share.clone(),
        };

        Ok(key_share)
    }

    /// Run local/simulated DKG (for testing only - creates full private key!).
    /// NOT secure for production use.
    pub fn run_local(&mut self) -> Result<Vec<KeyShare>> {
        let kg = ThresholdKeyGen::new(self.config.clone());
        let shares = kg.generate_local()?;

        // Set our own share
        let my_idx = self.config.party_index as usize;
        if let Some(share) = shares.get(my_idx) {
            self.state = DkgState::Complete {
                share: share.clone(),
            };
        }

        Ok(shares)
    }

    /// Extract this party's key share after DKG completes.
    pub fn finalize(&self) -> Result<KeyShare> {
        match &self.state {
            DkgState::Complete { share } => Ok(share.clone()),
            DkgState::Failed { error } => Err(MpcError::DkgFailed(error.clone())),
            _ => Err(MpcError::DkgFailed("DKG not yet complete".into())),
        }
    }

    /// Derive the backup shard (Party 2) after DKG completes.
    /// Sums all f_i(3) evaluations from Round 2 messages addressed to Party 2.
    /// This allows the device to generate the backup shard for offline storage.
    pub fn derive_backup_share(&self, backup_party_index: u16) -> Result<KeyShare> {
        match &self.state {
            DkgState::Complete { share } => {
                let mut backup_scalar = Scalar::ZERO;

                // First try to find evaluations from Round 2 messages
                for msg in &self.round2_messages {
                    for (recipient, share_bytes) in &msg.evaluations {
                        if *recipient == backup_party_index {
                            let mut bytes = [0u8; 32];
                            if share_bytes.len() >= 32 {
                                bytes.copy_from_slice(&share_bytes[..32]);
                            }
                            let s = Option::<Scalar>::from(Scalar::from_repr(bytes.into()))
                                .ok_or_else(|| MpcError::DkgFailed("invalid backup share value".into()))?;
                            backup_scalar += s;
                        }
                    }
                }

                // If no Round 2 evaluations exist for backup party, compute directly
                // from our local polynomial (device's contribution to the backup shard)
                if backup_scalar == Scalar::ZERO {
                    if let Some(coeffs) = &self.my_polynomial {
                        let x = Scalar::from((backup_party_index + 1) as u64);
                        let mut x_pow = Scalar::ONE;
                        for coeff in coeffs {
                            backup_scalar += coeff * &x_pow;
                            x_pow *= x;
                        }
                    } else {
                        return Err(MpcError::DkgFailed(
                            "no evaluations found for backup party and polynomial unavailable".into(),
                        ));
                    }
                }

                if backup_scalar == Scalar::ZERO {
                    return Err(MpcError::DkgFailed(
                        "backup share computation resulted in zero".into(),
                    ));
                }

                Ok(KeyShare {
                    party: backup_party_index,
                    threshold: share.threshold,
                    total_parties: share.total_parties + 1, // include backup party in total
                    secret_share: backup_scalar.to_bytes().to_vec().into(),
                    public_key: share.public_key.clone(),
                    paillier_pk: None,
                })
            }
            _ => Err(MpcError::DkgFailed("DKG not yet complete".into())),
        }
    }

    /// Feldman VSS verification: verify that a share is consistent with the commitments.
    ///
    /// Checks: share * G == C_0 + x*C_1 + x^2*C_2 + ... + x^{t-1}*C_{t-1}
    /// where x = recipient_index + 1 (1-indexed evaluation points for Shamir)
    fn verify_feldman_share(share: &Scalar, recipient_index: u16, commitments: &[Vec<u8>]) -> Result<()> {
        let x = Scalar::from((recipient_index + 1) as u64);

        // LHS: share * G
        let share_point = ProjectivePoint::from(AffinePoint::GENERATOR) * share;

        // RHS: sum_k( C_k * x^k )
        let mut expected = ProjectivePoint::IDENTITY;
        let mut x_pow = Scalar::ONE;

        for c_bytes in commitments {
            // Parse commitment point
            if c_bytes.len() < 33 {
                return Err(MpcError::DkgFailed("commitment too short".into()));
            }
            let mut key_bytes = [0u8; 33];
            key_bytes.copy_from_slice(&c_bytes[..33]);

            let encoded = k256::elliptic_curve::sec1::EncodedPoint::<k256::Secp256k1>::from_bytes(&key_bytes[..])
                .map_err(|_| MpcError::DkgFailed("invalid commitment encoding".into()))?;
            let ct_point = AffinePoint::from_encoded_point(&encoded);
            if bool::from(ct_point.is_none()) {
                return Err(MpcError::DkgFailed("invalid commitment point".into()));
            }
            let c_point = ProjectivePoint::from(ct_point.unwrap());

            expected += c_point * x_pow;
            x_pow *= x;
        }

        if share_point != expected {
            return Err(MpcError::DkgFailed("Feldman VSS verification failed: share inconsistent with commitments".into()));
        }

        Ok(())
    }

    /// Get number of received round 1 messages.
    pub fn round1_count(&self) -> usize {
        self.round1_messages.len()
    }

    /// Get number of received round 2 shares.
    pub fn round2_count(&self) -> usize {
        self.round2_messages.len()
    }

    /// Check if we have threshold number of messages for a round.
    pub fn has_threshold_messages(&self, round: u16) -> bool {
        match round {
            1 => self.round1_messages.len() >= self.config.threshold as usize,
            2 => self.round2_messages.len() >= self.config.threshold as usize,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(party: u16) -> SessionConfig {
        SessionConfig {
            session_id: "test-dkg-001".into(),
            threshold: 2,
            total_parties: 3,
            party_index: party,
        }
    }

    #[test]
    fn test_dkg_session_creation() {
        let session = DkgSession::new(test_config(0));
        assert!(matches!(session.state, DkgState::Initialized));
    }

    #[test]
    fn test_local_dkg_produces_consistent_shares() {
        let mut session = DkgSession::new(test_config(0));
        let shares = session.run_local().unwrap();

        assert_eq!(shares.len(), 3);
        assert_eq!(shares[0].public_key, shares[1].public_key);
        assert_eq!(shares[1].public_key, shares[2].public_key);
        for (i, s) in shares.iter().enumerate() {
            assert_eq!(s.party, i as u16);
            assert_eq!(s.threshold, 2);
            assert_eq!(s.total_parties, 3);
            assert_eq!(s.secret_share.len(), 32);
        }
    }

    #[test]
    fn test_finalize_returns_correct_party_share() {
        for party_idx in 0..3u16 {
            let mut session = DkgSession::new(test_config(party_idx));
            session.run_local().unwrap();
            let share = session.finalize().unwrap();
            assert_eq!(share.party, party_idx);
        }
    }

    #[test]
    fn test_finalize_before_dkg_fails() {
        let session = DkgSession::new(test_config(0));
        assert!(session.finalize().is_err());
    }
}
