use thiserror::Error;

/// Platform keychain abstraction.
///
/// Wraps iOS Keychain / Android Keystore for storing small secrets
/// (API tokens, session keys) that don't require Secure Enclave.
pub trait Keychain: Send + Sync {
    fn store(&self, key: &str, value: &[u8]) -> Result<(), KeychainError>;
    fn load(&self, key: &str) -> Result<Option<Vec<u8>>, KeychainError>;
    fn delete(&self, key: &str) -> Result<(), KeychainError>;
    fn exists(&self, key: &str) -> Result<bool, KeychainError>;
}

#[derive(Debug, Error)]
pub enum KeychainError {
    #[error("keychain not available")]
    NotAvailable,

    #[error("access denied: {0}")]
    AccessDenied(String),

    #[error("item not found: {0}")]
    NotFound(String),

    #[error("keychain error: {0}")]
    Other(String),
}
