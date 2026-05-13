use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use alloy_primitives::{Address, Bytes, U256};

use crate::{errors::ApiError, state::AppState};
use chain_evm::userop::{
    build_transfer_userop, estimate_userop_gas, request_paymaster_sponsorship, submit_to_bundler,
    UserOperation,
};

/// Request to build a UserOperation for a transfer
#[derive(Debug, Deserialize)]
pub struct BuildUserOpRequest {
    /// Recipient address
    pub to: Address,
    /// Amount to send (in wei for ETH, or token decimals for ERC-20)
    pub value: U256,
    /// Optional ERC-20 token address (if None, native ETH transfer)
    #[serde(default)]
    pub token: Option<Address>,
    /// Chain ID (default: 1 for Ethereum mainnet)
    #[serde(default = "default_chain_id")]
    pub chain_id: u64,
    /// Smart account sender address
    pub sender: Address,
    /// Nonce for the UserOperation
    pub nonce: U256,
    /// EntryPoint contract address
    #[serde(default = "default_entry_point")]
    pub entry_point: Address,
}

fn default_chain_id() -> u64 {
    1
}

fn default_entry_point() -> Address {
    // ERC-4337 v0.6 EntryPoint
    "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789"
        .parse()
        .unwrap()
}

/// Response containing the unsigned UserOperation and its hash
#[derive(Debug, Serialize)]
pub struct BuildUserOpResponse {
    pub userop: UserOperation,
    pub user_op_hash: String,
    pub sponsored: bool,
}

/// Request to submit a signed UserOperation
#[derive(Debug, Deserialize)]
pub struct SubmitUserOpRequest {
    pub userop: UserOperation,
    /// 65-byte ECDSA signature from MPC signing
    pub signature: String,
    /// EntryPoint contract address
    #[serde(default = "default_entry_point")]
    pub entry_point: Address,
}

/// Response after submitting a UserOperation
#[derive(Debug, Serialize)]
pub struct SubmitUserOpResponse {
    pub user_op_hash: String,
    pub status: String,
}

/// POST /api/v1/userop/build
///
/// Build a UserOperation for a transfer, estimate gas, and optionally request paymaster sponsorship.
async fn build_userop_handler(
    State(state): State<AppState>,
    Json(req): Json<BuildUserOpRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if bundler is configured
    let bundler_url = state
        .bundler_url
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("ERC-4337 bundler not configured"))?;

    // Build the UserOperation
    let mut userop = build_transfer_userop(req.sender, req.nonce, req.to, req.value, req.token)
        .map_err(|e| ApiError::validation_failed(format!("failed to build UserOp: {}", e)))?;

    // Estimate gas
    let (pre_verification_gas, verification_gas_limit, call_gas_limit) =
        estimate_userop_gas(&userop, req.entry_point, bundler_url)
            .await
            .map_err(|e| {
                ApiError::internal(format!("gas estimation failed: {}", e))
            })?;

    userop.pre_verification_gas = pre_verification_gas;
    userop.verification_gas_limit = verification_gas_limit;
    userop.call_gas_limit = call_gas_limit;

    // Request paymaster sponsorship if configured
    let sponsored = if let Some(paymaster_url) = &state.paymaster_url {
        match request_paymaster_sponsorship(&mut userop, paymaster_url, req.entry_point, req.chain_id)
            .await
        {
            Ok(_) => {
                tracing::info!("Paymaster sponsorship approved");
                true
            }
            Err(e) => {
                tracing::warn!("Paymaster sponsorship failed: {} — user will pay gas", e);
                false
            }
        }
    } else {
        false
    };

    // Calculate the UserOp hash for signing
    let user_op_hash = userop.hash(req.entry_point, req.chain_id);

    Ok((
        StatusCode::OK,
        Json(BuildUserOpResponse {
            userop,
            user_op_hash: format!("0x{}", hex::encode(user_op_hash.as_slice())),
            sponsored,
        }),
    ))
}

/// POST /api/v1/userop/submit
///
/// Attach signature to a UserOperation and submit it to the bundler.
async fn submit_userop_handler(
    State(state): State<AppState>,
    Json(req): Json<SubmitUserOpRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if bundler is configured
    let bundler_url = state
        .bundler_url
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("ERC-4337 bundler not configured"))?;

    // Parse and attach signature
    let sig_hex = req.signature.strip_prefix("0x").unwrap_or(&req.signature);
    let sig_bytes = hex::decode(sig_hex)
        .map_err(|e| ApiError::validation_failed(format!("invalid signature hex: {}", e)))?;

    if sig_bytes.len() != 65 {
        return Err(ApiError::validation_failed(format!(
            "signature must be 65 bytes, got {}",
            sig_bytes.len()
        )));
    }

    let mut userop = req.userop;
    userop.signature = Bytes::from(sig_bytes);

    // Submit to bundler
    let user_op_hash = submit_to_bundler(&userop, req.entry_point, bundler_url)
        .await
        .map_err(|e| ApiError::internal(format!("bundler submission failed: {}", e)))?;

    Ok((
        StatusCode::OK,
        Json(SubmitUserOpResponse {
            user_op_hash,
            status: "submitted".to_string(),
        }),
    ))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/build", post(build_userop_handler))
        .route("/submit", post(submit_userop_handler))
}
