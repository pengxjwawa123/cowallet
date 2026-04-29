use super::protocol::ThresholdKeyGen;
use super::{KeyShare, ProtocolMessage, SessionConfig};
use crate::errors::{MpcError, Result};

pub struct DkgSession {
    config: SessionConfig,
    state: DkgState,
}

enum DkgState {
    AwaitingStart,
    Complete { shares: Vec<KeyShare> },
    Failed { error: String },
}

impl DkgSession {
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            state: DkgState::AwaitingStart,
        }
    }

    /// Run local (simulated) DKG producing shares for all parties.
    ///
    /// For M2/testnet only — the full key exists transiently.
    /// Production will use synedrion's distributed DKG rounds.
    pub fn run_local(&mut self) -> Result<Vec<KeyShare>> {
        let kg = ThresholdKeyGen::new(self.config.clone());
        match kg.generate_local() {
            Ok(shares) => {
                self.state = DkgState::Complete {
                    shares: shares.clone(),
                };
                Ok(shares)
            }
            Err(e) => {
                self.state = DkgState::Failed {
                    error: e.to_string(),
                };
                Err(e)
            }
        }
    }

    /// Extract this party's key share after DKG completes.
    pub fn finalize(&self) -> Result<KeyShare> {
        match &self.state {
            DkgState::Complete { shares } => {
                let idx = self.config.party_index as usize;
                shares
                    .get(idx)
                    .cloned()
                    .ok_or_else(|| MpcError::DkgFailed(format!("no share for party {idx}")))
            }
            DkgState::Failed { error } => Err(MpcError::DkgFailed(error.clone())),
            _ => Err(MpcError::DkgFailed("DKG not yet complete".into())),
        }
    }

    /// Stub for future distributed rounds — not used in local mode.
    pub fn generate_round1(&mut self) -> Result<Vec<ProtocolMessage>> {
        Err(MpcError::DkgFailed("use run_local() for M2 testnet".into()))
    }

    pub fn process_round1(
        &mut self,
        _messages: Vec<ProtocolMessage>,
    ) -> Result<Vec<ProtocolMessage>> {
        Err(MpcError::DkgFailed("use run_local() for M2 testnet".into()))
    }

    pub fn process_round2(
        &mut self,
        _messages: Vec<ProtocolMessage>,
    ) -> Result<Vec<ProtocolMessage>> {
        Err(MpcError::DkgFailed("use run_local() for M2 testnet".into()))
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
        assert!(matches!(session.state, DkgState::AwaitingStart));
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
