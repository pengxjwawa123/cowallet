use super::protocol::{threshold_sign, verify_signature};
use super::{KeyShare, Presignature, ProtocolMessage, SessionConfig};
use crate::errors::{MpcError, Result};

pub struct SignSession {
    config: SessionConfig,
    shares: Vec<KeyShare>,
    share_indices: Vec<u16>,
    message_hash: [u8; 32],
    state: SignState,
}

enum SignState {
    Ready,
    Complete { signature: EcdsaSignature },
    Failed { error: String },
}

#[derive(Debug, Clone)]
pub struct EcdsaSignature {
    pub r: [u8; 32],
    pub s: [u8; 32],
    pub v: u8,
}

impl EcdsaSignature {
    pub fn to_bytes(&self) -> [u8; 65] {
        let mut sig = [0u8; 65];
        sig[0..32].copy_from_slice(&self.r);
        sig[32..64].copy_from_slice(&self.s);
        sig[64] = self.v;
        sig
    }

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

    pub fn verify(&self, message_hash: &[u8; 32], public_key: &[u8]) -> Result<bool> {
        verify_signature(public_key, message_hash, &self.to_bytes())
    }
}

impl SignSession {
    /// Create a signing session with the participating shares.
    ///
    /// For M2 local mode: pass the key shares directly.
    /// Production will use presignatures over the network.
    pub fn new_local(
        config: SessionConfig,
        share_indices: Vec<u16>,
        shares: Vec<KeyShare>,
        message_hash: [u8; 32],
    ) -> Self {
        Self {
            config,
            shares,
            share_indices,
            message_hash,
            state: SignState::Ready,
        }
    }

    /// Run local (simulated) signing — reconstructs key from shares.
    ///
    /// M2/testnet only. Production uses distributed online signing.
    pub fn sign_local(&mut self) -> Result<EcdsaSignature> {
        let share_refs: Vec<&KeyShare> = self.shares.iter().collect();
        match threshold_sign(&self.share_indices, &share_refs, &self.message_hash) {
            Ok((sig_bytes, v)) => {
                let sig = EcdsaSignature::from_bytes(&sig_bytes)?;
                self.state = SignState::Complete {
                    signature: sig.clone(),
                };
                Ok(sig)
            }
            Err(e) => {
                self.state = SignState::Failed {
                    error: e.to_string(),
                };
                Err(e)
            }
        }
    }

    pub fn finalize(&self) -> Result<EcdsaSignature> {
        match &self.state {
            SignState::Complete { signature } => Ok(signature.clone()),
            SignState::Failed { error } => Err(MpcError::SigningFailed(error.clone())),
            _ => Err(MpcError::SigningFailed("signing not complete".into())),
        }
    }

    /// Stub for future distributed signing — not used in local mode.
    pub fn generate_signature_share(&mut self) -> Result<Vec<ProtocolMessage>> {
        Err(MpcError::SigningFailed(
            "use sign_local() for M2 testnet".into(),
        ))
    }

    pub fn combine(&mut self, _messages: Vec<ProtocolMessage>) -> Result<EcdsaSignature> {
        Err(MpcError::SigningFailed(
            "use sign_local() for M2 testnet".into(),
        ))
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
