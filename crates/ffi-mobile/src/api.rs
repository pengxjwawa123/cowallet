/// FFI-safe API surface for flutter_rust_bridge.
///
/// Design rules:
/// 1. Only primitive types, String, Vec<u8>, and simple structs cross the boundary
/// 2. Secret material stays in Rust (state.rs) — Dart gets addresses and public keys
/// 3. All async Rust work uses the shared tokio runtime
/// 4. Errors are returned as Result<T, String> for simple FFI mapping
use mpc_core::dkls23::protocol::ThresholdKeyGen;
use mpc_core::dkls23::SessionConfig;

use crate::state;

// ---------------------------------------------------------------------------
// Types returned to Dart
// ---------------------------------------------------------------------------

pub struct FfiWalletInfo {
    pub address: String,
    pub public_key: Vec<u8>,
}

pub struct FfiBalance {
    pub wei: String,
    pub formatted: String,
    pub decimals: u8,
}

pub struct FfiTxResult {
    pub tx_hash: String,
}

pub struct FfiGasEstimate {
    pub gas_limit: u64,
    pub max_fee_per_gas: String,
    pub max_priority_fee_per_gas: String,
    pub estimated_cost_wei: String,
}

pub struct FfiKeyStatus {
    pub has_device_shard: bool,
    pub has_server_shard: bool,
    pub has_backup_shard: bool,
    pub address: String,
}

// ---------------------------------------------------------------------------
// Wallet operations
// ---------------------------------------------------------------------------

/// Generate a new MPC wallet using local-mode TSS (2-of-3).
/// Returns the derived Ethereum address and public key.
/// The 3 key shares are stored in Rust memory — NEVER sent to Dart.
pub fn generate_wallet() -> Result<FfiWalletInfo, String> {
    let config = SessionConfig {
        session_id: uuid::Uuid::new_v4().to_string(),
        threshold: 2,
        total_parties: 3,
        party_index: 0,
    };

    let keygen = ThresholdKeyGen::new(config);
    let shares = keygen.generate_local().map_err(|e| e.to_string())?;

    let address = shares[0].eth_address();
    let address_hex = format!(
        "0x{}",
        address.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    let public_key = shares[0].public_key.clone();

    state::store_shares(shares);

    Ok(FfiWalletInfo {
        address: address_hex,
        public_key,
    })
}

/// Check if wallet shares are loaded in memory.
pub fn has_wallet() -> bool {
    state::has_shares()
}

/// Get the key status for the UI (which shards are present).
pub fn get_key_status() -> FfiKeyStatus {
    FfiKeyStatus {
        has_device_shard: state::get_share(0).is_some(),
        has_server_shard: state::get_share(1).is_some(),
        has_backup_shard: state::get_share(2).is_some(),
        address: state::get_share(0)
            .map(|s| {
                let addr = s.eth_address();
                format!(
                    "0x{}",
                    addr.iter().map(|b| format!("{b:02x}")).collect::<String>()
                )
            })
            .unwrap_or_default(),
    }
}

/// Clear all shares from memory (for wallet deletion / reset).
pub fn clear_wallet() {
    state::clear_shares();
}

// ---------------------------------------------------------------------------
// Signing — called from Dart after biometric auth succeeds on the Dart side
// ---------------------------------------------------------------------------

/// Sign a 32-byte message hash using the device (0) and server (1) shards.
/// Returns the 65-byte signature (r || s || v).
pub fn sign_hash(msg_hash: Vec<u8>) -> Result<Vec<u8>, String> {
    if msg_hash.len() != 32 {
        return Err("msg_hash must be 32 bytes".into());
    }

    let share0 = state::get_share(0).ok_or("device shard not loaded")?;
    let share1 = state::get_share(1).ok_or("server shard not loaded")?;

    let indices = vec![share0.party, share1.party];

    let msg_arr: [u8; 32] = msg_hash.try_into().map_err(|_| "msg_hash must be 32 bytes")?;

    let (sig_bytes, recovery_id) = mpc_core::dkls23::protocol::threshold_sign(
        &indices,
        &[&share0, &share1],
        &msg_arr,
    )
    .map_err(|e| e.to_string())?;

    let mut result = sig_bytes;
    result.push(recovery_id);
    Ok(result)
}

// ---------------------------------------------------------------------------
// Balance queries (async, uses tokio)
// ---------------------------------------------------------------------------

/// Query ETH balance for an address on Base Sepolia.
/// Returns balance in wei as a decimal string.
pub async fn query_eth_balance(address: String, rpc_url: String) -> Result<FfiBalance, String> {
    // Placeholder — chain-evm::tokens::query_native_balance is a stub.
    // When implemented, this will call the real function.
    let _ = (address, rpc_url);
    Err("query_native_balance not yet implemented in chain-evm".into())
}

/// Query ERC-20 token balance.
pub async fn query_token_balance(
    owner: String,
    token_contract: String,
    rpc_url: String,
) -> Result<FfiBalance, String> {
    let _ = (owner, token_contract, rpc_url);
    Err("query_balance not yet implemented in chain-evm".into())
}

// ---------------------------------------------------------------------------
// Transaction building
// ---------------------------------------------------------------------------

/// Estimate gas for a transaction.
pub async fn estimate_gas(
    _from: String,
    _to: String,
    _value_wei: String,
    _data: Option<Vec<u8>>,
    _rpc_url: String,
) -> Result<FfiGasEstimate, String> {
    Err("gas estimation not yet implemented".into())
}

/// Build, sign, and broadcast an ETH transfer.
/// Biometric auth must happen on the Dart side BEFORE calling this.
pub async fn send_eth(
    _to: String,
    _value_wei: String,
    _rpc_url: String,
) -> Result<FfiTxResult, String> {
    Err("send_eth not yet implemented — use Dart-side signing for now".into())
}
