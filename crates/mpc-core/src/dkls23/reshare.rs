use super::{KeyShare, ProtocolMessage, SessionConfig};
use crate::errors::{MpcError, Result};

/// Proactive key resharing protocol.
///
/// Generates new shares of the same underlying key without reconstructing it.
/// After resharing, old shares become useless — even if an attacker captured
/// a share before the refresh, it cannot be combined with new shares.
///
/// Should be triggered:
/// - Every 30 days (automatic, via worker crate)
/// - When a party is suspected compromised
/// - When recovering to a new device
#[allow(dead_code)]
pub struct ReshareSession {
    config: SessionConfig,
    old_share: KeyShare,
    state: ReshareState,
}

#[allow(dead_code)]
enum ReshareState {
    AwaitingRound1,
    AwaitingRound2 { round1_data: Vec<u8> },
    Complete { new_share: KeyShare },
    Failed { error: String },
}

impl ReshareSession {
    /// Start a resharing session.
    ///
    /// At least `threshold` parties with valid old shares must participate.
    /// The result is a set of new shares for the same public key, but the
    /// old shares are no longer compatible.
    pub fn new(config: SessionConfig, old_share: KeyShare) -> Self {
        Self {
            config,
            old_share,
            state: ReshareState::AwaitingRound1,
        }
    }

    /// Generate round 1 resharing messages.
    pub fn generate_round1(&mut self) -> Result<Vec<ProtocolMessage>> {
        // TODO: Implement DKLS23 resharing round 1
        // 1. Generate new random polynomial with same free term
        // 2. Compute new VSS commitments
        // 3. Send shares of new polynomial to each party
        Err(MpcError::ResharingFailed("not yet implemented".into()))
    }

    /// Process round 1 and generate round 2 messages.
    pub fn process_round1(
        &mut self,
        _messages: Vec<ProtocolMessage>,
    ) -> Result<Vec<ProtocolMessage>> {
        // TODO: Implement DKLS23 resharing round 2
        // 1. Verify new VSS commitments
        // 2. Combine received shares into new local share
        // 3. Verify public key is unchanged
        Err(MpcError::ResharingFailed("not yet implemented".into()))
    }

    /// Finalize resharing and get the new key share.
    ///
    /// The caller MUST securely erase the old share after this succeeds.
    pub fn finalize(self) -> Result<KeyShare> {
        match self.state {
            ReshareState::Complete { new_share } => Ok(new_share),
            ReshareState::Failed { error } => Err(MpcError::ResharingFailed(error)),
            _ => Err(MpcError::ResharingFailed("resharing not complete".into())),
        }
    }
}
