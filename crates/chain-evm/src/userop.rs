use alloy_primitives::{Address, Bytes, U256};
use serde::{Deserialize, Serialize};

/// ERC-4337 UserOperation for account abstraction.
///
/// Sent to a bundler which submits it to the EntryPoint contract.
/// Enables gas sponsorship (paymaster), batched operations,
/// and on-chain social recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserOperation {
    pub sender: Address,
    pub nonce: U256,
    pub init_code: Bytes,
    pub call_data: Bytes,
    pub call_gas_limit: U256,
    pub verification_gas_limit: U256,
    pub pre_verification_gas: U256,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub paymaster_and_data: Bytes,
    pub signature: Bytes,
}

/// Build a UserOperation for a simple ETH/token transfer.
pub fn build_transfer_userop(
    _sender: Address,
    _to: Address,
    _value: U256,
    _token: Option<Address>,
) -> Result<UserOperation, UserOpError> {
    // TODO: Encode call_data for:
    // - Native transfer: direct call with value
    // - ERC-20: encode transfer(to, amount)
    Err(UserOpError::NotImplemented)
}

/// Build init_code for deploying a new smart account.
pub fn build_account_init_code(_owner_pubkey: &[u8], _salt: U256) -> Result<Bytes, UserOpError> {
    // TODO: Encode SimpleAccountFactory.createAccount(owner, salt)
    Err(UserOpError::NotImplemented)
}

/// Submit a signed UserOperation to a bundler.
pub async fn submit_to_bundler(
    _userop: &UserOperation,
    _bundler_url: &str,
) -> Result<String, UserOpError> {
    // TODO: eth_sendUserOperation JSON-RPC to bundler
    Err(UserOpError::NotImplemented)
}

#[derive(Debug, thiserror::Error)]
pub enum UserOpError {
    #[error("not yet implemented")]
    NotImplemented,

    #[error("bundler rejected: {0}")]
    BundlerRejected(String),

    #[error("paymaster error: {0}")]
    PaymasterError(String),

    #[error("account not deployed")]
    AccountNotDeployed,
}
