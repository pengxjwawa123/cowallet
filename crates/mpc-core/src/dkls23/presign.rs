use super::{Presignature, ProtocolMessage, SessionConfig};
use crate::errors::{MpcError, Result};
use k256::{
    elliptic_curve::{
        sec1::{FromEncodedPoint, ToEncodedPoint},
        Field, PrimeField,
    },
    AffinePoint, EncodedPoint, Scalar,
};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};

/// Data stored inside a distributed presignature token.
/// Contains the pre-computed nonce material for fast signing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresignData {
    /// This party's ephemeral secret nonce k_i (32 bytes)
    pub k_i: [u8; 32],
    /// This party's ephemeral public nonce R_i = k_i * G (compressed, 33 bytes)
    pub r_i: Vec<u8>,
    /// Aggregate nonce point R = k_i * R_j (compressed, 33 bytes)
    pub aggregate_r: Vec<u8>,
    /// r scalar = R.x mod n (32 bytes)
    pub r_scalar: [u8; 32],
    /// The other party's index (needed to set up signing correctly)
    pub other_party: u16,
}

/// Round 1 message for distributed presign: party broadcasts R_i = k_i * G
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresignRound1Message {
    pub session_id: String,
    pub party_index: u16,
    pub r_public: Vec<u8>,
    pub schnorr_proof: crate::crypto::schnorr::SchnorrProof,
}

pub struct PresignSession {
    config: SessionConfig,
    state: PresignState,
    my_k: Option<Scalar>,
    my_r_public: Option<Vec<u8>>,
    round1_messages: Vec<PresignRound1Message>,
}

enum PresignState {
    Ready,
    Round1Done,
    Complete { presignature: Presignature },
    Failed { error: String },
}

impl PresignSession {
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            state: PresignState::Ready,
            my_k: None,
            my_r_public: None,
            round1_messages: Vec::new(),
        }
    }

    /// Round 1: Generate ephemeral nonce k_i and broadcast R_i = k_i * G.
    pub fn generate_round1(&mut self) -> Result<Vec<ProtocolMessage>> {
        match self.state {
            PresignState::Ready => {}
            _ => return Err(MpcError::SigningFailed("invalid state for presign round 1".into())),
        }

        self.state = PresignState::Round1Done;

        let k_i = Scalar::random(&mut OsRng);
        let r_i_point = AffinePoint::GENERATOR * k_i;
        let r_i_affine: AffinePoint = r_i_point.into();
        let r_i_bytes = r_i_affine.to_encoded_point(true).as_bytes().to_vec();

        self.my_k = Some(k_i);
        self.my_r_public = Some(r_i_bytes.clone());

        let schnorr_proof = {
            use crate::crypto::schnorr::SchnorrProof;
            SchnorrProof::prove(&k_i, &r_i_bytes, b"Presign-Round1")
        };

        let round1 = PresignRound1Message {
            session_id: self.config.session_id.clone(),
            party_index: self.config.party_index,
            r_public: r_i_bytes,
            schnorr_proof,
        };

        self.round1_messages.push(round1.clone());

        let payload = bincode::serialize(&round1)
            .map_err(|e| MpcError::SigningFailed(format!("serialization failed: {}", e)))?;

        Ok(vec![ProtocolMessage {
            session_id: self.config.session_id.clone(),
            from: self.config.party_index,
            to: 0xFFFF,
            round: 1,
            payload,
        }])
    }

    /// Process Round 1 messages from other parties, compute aggregate R, produce presignature.
    pub fn process_round1(
        &mut self,
        messages: Vec<ProtocolMessage>,
    ) -> Result<Vec<ProtocolMessage>> {
        for msg in messages {
            if msg.round != 1 {
                continue;
            }
            let round1: PresignRound1Message = bincode::deserialize(&msg.payload)
                .map_err(|e| MpcError::SigningFailed(format!("invalid presign round1: {}", e)))?;

            if round1.party_index != self.config.party_index {
                if !round1.schnorr_proof.verify(&round1.r_public, b"Presign-Round1") {
                    return Err(MpcError::SigningFailed(format!(
                        "party {} failed Schnorr proof for presign nonce",
                        round1.party_index
                    )));
                }
                self.round1_messages.push(round1);
            }
        }

        // Need at least 2 messages (own + one other) for 2-of-3
        if self.round1_messages.len() < 2 {
            self.state = PresignState::Round1Done;
            return Ok(Vec::new());
        }

        let my_k = self.my_k.ok_or_else(||
            MpcError::SigningFailed("ephemeral key not set".into()))?;

        let other_msg = self.round1_messages.iter()
            .find(|m| m.party_index != self.config.party_index)
            .ok_or_else(|| MpcError::SigningFailed("no other party message".into()))?;

        let other_party = other_msg.party_index;

        // Parse R_j
        let encoded = EncodedPoint::from_bytes(&other_msg.r_public)
            .map_err(|_| MpcError::SigningFailed("invalid R_j encoding".into()))?;
        let r_j_ct = AffinePoint::from_encoded_point(&encoded);
        if bool::from(r_j_ct.is_none()) {
            return Err(MpcError::SigningFailed("invalid R_j point".into()));
        }
        let r_j = r_j_ct.unwrap();

        // Compute aggregate R = k_i * R_j = (k_i * k_j) * G
        let aggregate_r_proj = r_j * my_k;
        let aggregate_r: AffinePoint = aggregate_r_proj.into();
        let aggregate_r_bytes = aggregate_r.to_encoded_point(true).as_bytes().to_vec();

        // Extract r = R.x mod n
        let r_uncompressed = aggregate_r.to_encoded_point(false);
        let r_x_bytes = &r_uncompressed.as_bytes()[1..33];
        let mut r_array = [0u8; 32];
        r_array.copy_from_slice(r_x_bytes);

        let r_scalar_ct = Scalar::from_repr(r_array.into());
        if bool::from(r_scalar_ct.is_none()) {
            return Err(MpcError::SigningFailed("invalid r scalar from presign".into()));
        }

        // Build presign data
        let k_bytes: [u8; 32] = my_k.to_bytes().into();
        let presign_data = PresignData {
            k_i: k_bytes,
            r_i: self.my_r_public.clone().unwrap_or_default(),
            aggregate_r: aggregate_r_bytes,
            r_scalar: r_array,
            other_party,
        };

        let data_bytes = bincode::serialize(&presign_data)
            .map_err(|e| MpcError::SigningFailed(format!("presign data serialization: {}", e)))?;

        let mut id = [0u8; 32];
        OsRng.fill_bytes(&mut id);

        let presig = Presignature {
            id,
            data: data_bytes.into(),
        };

        self.state = PresignState::Complete {
            presignature: presig.clone(),
        };

        Ok(Vec::new())
    }

    pub fn finalize(&self) -> Result<Presignature> {
        match &self.state {
            PresignState::Complete { presignature } => Ok(presignature.clone()),
            PresignState::Failed { error } => Err(MpcError::SigningFailed(error.clone())),
            _ => Err(MpcError::SigningFailed("presigning not complete".into())),
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self.state, PresignState::Complete { .. })
    }
}

/// Extract presign data from a distributed presignature token.
pub fn decode_presign_data(presig: &Presignature) -> Result<PresignData> {
    bincode::deserialize(presig.data.as_bytes())
        .map_err(|e| MpcError::SigningFailed(format!("invalid presign data: {}", e)))
}

pub struct PresignatureStore {
    presignatures: Vec<Presignature>,
}

impl PresignatureStore {
    pub fn new() -> Self {
        Self {
            presignatures: Vec::new(),
        }
    }

    pub fn add(&mut self, presig: Presignature) {
        self.presignatures.push(presig);
    }

    pub fn take(&mut self) -> Option<Presignature> {
        self.presignatures.pop()
    }

    pub fn count(&self) -> usize {
        self.presignatures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.presignatures.is_empty()
    }
}

impl Default for PresignatureStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::dkg::DkgSession;
    use super::super::sign::SignSession;
    use super::super::KeyShare;
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

    fn presign_pair() -> (PresignData, PresignData) {
        let config0 = SessionConfig {
            session_id: "presign-001".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        };
        let config1 = SessionConfig {
            session_id: "presign-001".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 1,
        };

        let mut s0 = PresignSession::new(config0);
        let mut s1 = PresignSession::new(config1);

        let msgs0 = s0.generate_round1().unwrap();
        let msgs1 = s1.generate_round1().unwrap();
        s0.process_round1(msgs1).unwrap();
        s1.process_round1(msgs0).unwrap();

        let p0 = s0.finalize().unwrap();
        let p1 = s1.finalize().unwrap();
        (decode_presign_data(&p0).unwrap(), decode_presign_data(&p1).unwrap())
    }

    #[test]
    fn test_presign_round1_exchange() {
        let (data0, data1) = presign_pair();

        assert_eq!(data0.aggregate_r, data1.aggregate_r, "aggregate R must match");
        assert_eq!(data0.r_scalar, data1.r_scalar, "r scalar must match");
        assert_ne!(data0.k_i, data1.k_i, "k_i must differ between parties");
    }

    #[test]
    fn test_presignature_store() {
        let mut store = PresignatureStore::new();
        assert!(store.is_empty());

        for _ in 0..3 {
            let config0 = SessionConfig {
                session_id: "ps".into(),
                threshold: 2,
                total_parties: 3,
                party_index: 0,
            };
            let config1 = SessionConfig {
                session_id: "ps".into(),
                threshold: 2,
                total_parties: 3,
                party_index: 1,
            };
            let mut s0 = PresignSession::new(config0);
            let mut s1 = PresignSession::new(config1);
            let msgs0 = s0.generate_round1().unwrap();
            let msgs1 = s1.generate_round1().unwrap();
            s0.process_round1(msgs1).unwrap();
            store.add(s0.finalize().unwrap());
        }

        assert_eq!(store.count(), 3);
        let taken = store.take().unwrap();
        assert_eq!(store.count(), 2);
        assert_eq!(taken.id.len(), 32);
    }

    #[test]
    fn test_presign_invalid_state() {
        let config = SessionConfig {
            session_id: "ps-fail".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        };
        let mut session = PresignSession::new(config);
        session.generate_round1().unwrap();
        // Cannot generate round1 again
        assert!(session.generate_round1().is_err());
    }

    #[test]
    fn test_presign_then_sign() {
        use sha3::Digest;

        let shares = dkg_shares();
        let hash: [u8; 32] = sha3::Keccak256::digest(b"presign-then-sign test").into();

        let (data0, data1) = presign_pair();

        let sign_config0 = SessionConfig {
            session_id: "sign-with-presign-001".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        };
        let sign_config1 = SessionConfig {
            session_id: "sign-with-presign-001".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 1,
        };

        let mut sign0 = SignSession::new_distributed(sign_config0, shares[0].clone(), hash);
        let mut sign1 = SignSession::new_distributed(sign_config1, shares[1].clone(), hash);

        let r1_msg0 = sign0.generate_round1_with_presign(&data0.k_i, &data0.r_i).unwrap();
        let r1_msg1 = sign1.generate_round1_with_presign(&data1.k_i, &data1.r_i).unwrap();

        sign0.process_round1(vec![r1_msg1]).unwrap();
        sign1.process_round1(vec![r1_msg0]).unwrap();

        let r2_msg0 = sign0.generate_round2().unwrap();
        let _sig_server = sign1.process_round2(vec![r2_msg0]).unwrap();

        let payload = sign1.get_server_response()
            .expect("server should produce ServerSignature");

        let server_response = ProtocolMessage {
            session_id: "sign-with-presign-001".into(),
            from: 1,
            to: 0,
            round: 2,
            payload,
        };

        let sig = sign0.process_round2(vec![server_response]).unwrap();

        assert!(
            sig.verify(&hash, &shares[0].public_key).unwrap(),
            "presign-then-sign signature verification failed"
        );
    }
}
