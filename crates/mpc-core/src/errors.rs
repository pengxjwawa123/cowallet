use thiserror::Error;

#[derive(Debug, Error)]
pub enum MpcError {
    #[error("DKG failed: {0}")]
    DkgFailed(String),

    #[error("signing failed: {0}")]
    SigningFailed(String),

    #[error("resharing failed: {0}")]
    ResharingFailed(String),

    #[error("invalid message from party {party}: {reason}")]
    InvalidMessage { party: u16, reason: String },

    #[error("protocol aborted: cheating detected from party {party}")]
    CheatingDetected { party: u16 },

    #[error("timeout waiting for party {party} in round {round}")]
    Timeout { party: u16, round: u16 },

    #[error("shard encryption error: {0}")]
    ShardEncryption(String),

    #[error("shard decryption error: {0}")]
    ShardDecryption(String),

    #[error("biometric authentication required")]
    BiometricRequired,

    #[error("transport error: {0}")]
    Transport(String),

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("insufficient parties: need {required}, have {available}")]
    InsufficientParties { required: u16, available: u16 },
}

pub type Result<T> = std::result::Result<T, MpcError>;
