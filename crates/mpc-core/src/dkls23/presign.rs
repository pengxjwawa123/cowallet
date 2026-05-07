use super::{KeyShare, Presignature, ProtocolMessage, SessionConfig};
use crate::errors::{MpcError, Result};
use rand::RngCore;

pub struct PresignSession {
    #[allow(dead_code)]
    config: SessionConfig,
    share_indices: Vec<u16>,
    shares: Vec<KeyShare>,
    state: PresignState,
}

enum PresignState {
    Ready,
    Complete { presignature: Presignature },
    Failed { error: String },
}

impl PresignSession {
    pub fn new_local(
        config: SessionConfig,
        share_indices: Vec<u16>,
        shares: Vec<KeyShare>,
    ) -> Self {
        Self {
            config,
            share_indices,
            shares,
            state: PresignState::Ready,
        }
    }

    /// Generate a presignature token for M2 local mode.
    ///
    /// In local mode the presignature just stores the share indices and
    /// serialized shares so sign_local can consume them. Production will
    /// compute real OT-based presignatures.
    pub fn run_local(&mut self) -> Result<Presignature> {
        if self.shares.len() < self.shares[0].threshold as usize {
            let err = format!(
                "need {} shares, got {}",
                self.shares[0].threshold,
                self.shares.len()
            );
            self.state = PresignState::Failed { error: err.clone() };
            return Err(MpcError::SigningFailed(err));
        }

        let mut id = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut id);

        // Encode share indices into the presignature for later consumption.
        // In local mode we don't need the actual shares in the token —
        // they'll be passed directly to SignSession::new_local.
        let data_bytes = serde_json::to_vec(&self.share_indices)
            .map_err(|e| MpcError::SigningFailed(e.to_string()))?;

        let presig = Presignature {
            id,
            data: data_bytes.into(),
        };
        self.state = PresignState::Complete {
            presignature: presig.clone(),
        };
        Ok(presig)
    }

    pub fn finalize(&self) -> Result<Presignature> {
        match &self.state {
            PresignState::Complete { presignature } => Ok(presignature.clone()),
            PresignState::Failed { error } => Err(MpcError::SigningFailed(error.clone())),
            _ => Err(MpcError::SigningFailed("presigning not complete".into())),
        }
    }

    /// Stub for future distributed rounds.
    pub fn generate_round1(&mut self) -> Result<Vec<ProtocolMessage>> {
        Err(MpcError::SigningFailed(
            "use run_local() for M2 testnet".into(),
        ))
    }

    pub fn process_round1(
        &mut self,
        _messages: Vec<ProtocolMessage>,
    ) -> Result<Vec<ProtocolMessage>> {
        Err(MpcError::SigningFailed(
            "use run_local() for M2 testnet".into(),
        ))
    }
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
    fn test_presign_local() {
        let shares = dkg_shares();
        let config = SessionConfig {
            session_id: "presign-001".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        };
        let mut session = PresignSession::new_local(
            config,
            vec![0, 1],
            vec![shares[0].clone(), shares[1].clone()],
        );
        let presig = session.run_local().unwrap();
        assert_eq!(presig.id.len(), 32);
        assert!(!presig.data.is_empty());
    }

    #[test]
    fn test_presignature_store() {
        let shares = dkg_shares();
        let config = SessionConfig {
            session_id: "ps".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        };
        let mut session = PresignSession::new_local(
            config,
            vec![0, 2],
            vec![shares[0].clone(), shares[2].clone()],
        );
        let presig = session.run_local().unwrap();

        let mut store = PresignatureStore::new();
        assert!(store.is_empty());

        store.add(presig);
        assert_eq!(store.count(), 1);

        let taken = store.take().unwrap();
        assert!(store.is_empty());
        assert_eq!(taken.id.len(), 32);
    }

    #[test]
    fn test_presign_insufficient_shares_fails() {
        let shares = dkg_shares();
        let config = SessionConfig {
            session_id: "ps-fail".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        };
        let mut session = PresignSession::new_local(config, vec![0], vec![shares[0].clone()]);
        assert!(session.run_local().is_err());
    }
}
