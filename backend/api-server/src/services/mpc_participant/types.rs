use mpc_core::dkls23::{KeyShare, SessionConfig};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The server is always Party 1 in the 2-of-3 scheme.
pub const SERVER_PARTY_INDEX: u16 = 1;

/// Broadcast address used in mpc-core ProtocolMessage.
pub const BROADCAST_PARTY: u16 = 0xFFFF;

/// Session types that the server participant handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MpcSessionType {
    Dkg,
    Keygen,
    Sign,
    Reshare,
}

impl MpcSessionType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "dkg" | "keygen" => Some(Self::Dkg),
            "sign" => Some(Self::Sign),
            "reshare" => Some(Self::Reshare),
            _ => None,
        }
    }
}

/// Internal state of an active server-side session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPhase {
    /// Waiting for client's Round 1 message.
    AwaitingClientRound1,
    /// Server generated Round 1, waiting for client's Round 2.
    AwaitingClientRound2,
    /// DKG complete, shard stored.
    DkgComplete,
    /// Sign: waiting for client's Round 1.
    SignAwaitingRound1,
    /// Sign: server sent Round 1, waiting for client's Round 2.
    SignAwaitingRound2,
    /// Sign complete.
    SignComplete,
    /// Reshare: server sent Round 1, waiting for client's Round 1.
    ReshareAwaitingRound1,
    /// Reshare complete, new shard stored.
    ReshareComplete,
    /// Session failed.
    Failed,
}

/// Metadata about an active session held in memory.
pub struct ActiveSession {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub session_type: MpcSessionType,
    pub phase: SessionPhase,
    pub config: SessionConfig,
    pub created_at: std::time::Instant,
    /// Optional wallet ID for multi-wallet support.
    /// When set, the session uses the key share associated with this wallet.
    pub wallet_id: Option<Uuid>,
}
