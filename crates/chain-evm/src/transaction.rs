use alloy_consensus::SignableTransaction;
use alloy_consensus::TxEip1559;
use alloy_primitives::{Address, B256, Bytes, TxKind, U256};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::signer::MpcSigner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRequest {
    pub to: Address,
    pub value: U256,
    pub data: Vec<u8>,
    pub chain_id: u64,
    pub gas_limit: Option<u64>,
    pub nonce: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasEstimate {
    pub gas_limit: u64,
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
    pub l1_data_fee: Option<u128>,
    pub estimated_cost_wei: U256,
    pub estimated_cost_usd: Option<f64>,
}

/// Build an unsigned EIP-1559 transaction from request + gas params.
pub fn build_unsigned_eip1559(tx: &TransactionRequest, gas: &GasEstimate, nonce: u64) -> TxEip1559 {
    TxEip1559 {
        chain_id: tx.chain_id,
        nonce,
        gas_limit: gas.gas_limit,
        max_fee_per_gas: gas.max_fee_per_gas,
        max_priority_fee_per_gas: gas.max_priority_fee_per_gas,
        to: TxKind::Call(tx.to),
        value: tx.value,
        access_list: Default::default(),
        input: Bytes::copy_from_slice(&tx.data),
    }
}

/// Build and sign an EIP-1559 transaction, returning the RLP-encoded bytes.
pub fn sign_eip1559_tx(
    tx: &TransactionRequest,
    gas: &GasEstimate,
    nonce: u64,
    signer: &MpcSigner,
) -> Result<(Vec<u8>, B256), TransactionError> {
    let unsigned = build_unsigned_eip1559(tx, gas, nonce);
    let sig_hash = unsigned.signature_hash();

    let alloy_sig = signer
        .sign_hash_inner(&sig_hash)
        .map_err(|e| TransactionError::SigningFailed(e.to_string()))?;

    let signed = unsigned.into_signed(alloy_sig);
    let tx_hash = *signed.hash();

    let mut encoded = Vec::new();
    signed.eip2718_encode(&mut encoded);

    Ok((encoded, tx_hash))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationResult {
    pub success: bool,
    pub return_data: Vec<u8>,
    pub gas_used: u64,
    pub state_changes: Vec<StateChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChange {
    pub address: Address,
    pub token: Option<String>,
    pub balance_change: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("gas estimation failed: {0}")]
    GasEstimation(String),

    #[error("signing failed: {0}")]
    SigningFailed(String),

    #[error("simulation failed: {0}")]
    SimulationFailed(String),

    #[error("nonce too low")]
    NonceTooLow,

    #[error("insufficient funds")]
    InsufficientFunds,

    #[error("RPC error: {0}")]
    Rpc(String),
}

/// Query the next nonce for an address via eth_getTransactionCount.
pub async fn get_nonce(
    client: &Client,
    rpc_url: &str,
    address: Address,
) -> Result<u64, TransactionError> {
    let addr_hex = format!("0x{}", hex::encode(address.as_slice()));

    let rpc_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_getTransactionCount",
        "params": [addr_hex, "latest"],
        "id": 1
    });

    let resp = client
        .post(rpc_url)
        .json(&rpc_body)
        .send()
        .await
        .map_err(|e| TransactionError::Rpc(e.to_string()))?;

    let rpc_resp: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| TransactionError::Rpc(e.to_string()))?;

    if let Some(err) = rpc_resp.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown RPC error");
        return Err(TransactionError::Rpc(msg.to_string()));
    }

    let nonce_hex = rpc_resp
        .get("result")
        .and_then(|r| r.as_str())
        .ok_or_else(|| TransactionError::Rpc("no result in response".into()))?;

    let nonce = u64::from_str_radix(nonce_hex.strip_prefix("0x").unwrap_or(nonce_hex), 16)
        .map_err(|e| TransactionError::Rpc(format!("failed to parse nonce: {}", e)))?;

    Ok(nonce)
}

/// Broadcast a signed transaction via eth_sendRawTransaction.
pub async fn broadcast_tx(
    client: &Client,
    rpc_url: &str,
    signed_tx: &[u8],
) -> Result<B256, TransactionError> {
    let tx_hex = format!("0x{}", hex::encode(signed_tx));

    let rpc_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_sendRawTransaction",
        "params": [tx_hex],
        "id": 1
    });

    let resp = client
        .post(rpc_url)
        .json(&rpc_body)
        .send()
        .await
        .map_err(|e| TransactionError::Rpc(e.to_string()))?;

    let rpc_resp: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| TransactionError::Rpc(e.to_string()))?;

    if let Some(err) = rpc_resp.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown RPC error");
        return Err(TransactionError::Rpc(msg.to_string()));
    }

    let tx_hash_hex = rpc_resp
        .get("result")
        .and_then(|r| r.as_str())
        .ok_or_else(|| TransactionError::Rpc("no result in response".into()))?;

    let tx_hash_bytes = hex::decode(tx_hash_hex.strip_prefix("0x").unwrap_or(tx_hash_hex))
        .map_err(|e| TransactionError::Rpc(format!("failed to parse tx hash: {}", e)))?;

    let tx_hash = B256::from_slice(&tx_hash_bytes);

    Ok(tx_hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signer::MpcSigner;
    use mpc_core::dkls23::{SessionConfig, dkg::DkgSession};

    fn test_signer() -> MpcSigner {
        let config = SessionConfig {
            session_id: "tx-test".into(),
            threshold: 2,
            total_parties: 3,
            party_index: 0,
        };
        let mut dkg = DkgSession::new(config);
        let shares = dkg.run_local().unwrap();
        let eth_addr = shares[0].eth_address();

        MpcSigner::from_shares(
            Address::from_slice(&eth_addr),
            84532,
            vec![0, 1],
            vec![shares[0].clone(), shares[1].clone()],
        )
    }

    #[test]
    fn test_build_unsigned_eip1559() {
        let tx_req = TransactionRequest {
            to: Address::ZERO,
            value: U256::from(1_000_000_000_000_000_000u128), // 1 ETH
            data: vec![],
            chain_id: 84532,
            gas_limit: None,
            nonce: None,
        };
        let gas = GasEstimate {
            gas_limit: 21000,
            max_fee_per_gas: 1_000_000_000,
            max_priority_fee_per_gas: 100_000_000,
            l1_data_fee: None,
            estimated_cost_wei: U256::ZERO,
            estimated_cost_usd: None,
        };
        let unsigned = build_unsigned_eip1559(&tx_req, &gas, 0);
        assert_eq!(unsigned.chain_id, 84532);
        assert_eq!(unsigned.gas_limit, 21000);
        assert_eq!(unsigned.nonce, 0);
    }

    #[test]
    fn test_sign_eip1559_tx() {
        let signer = test_signer();
        let tx_req = TransactionRequest {
            to: Address::ZERO,
            value: U256::from(1_000_000_000_000_000_000u128),
            data: vec![],
            chain_id: 84532,
            gas_limit: None,
            nonce: None,
        };
        let gas = GasEstimate {
            gas_limit: 21000,
            max_fee_per_gas: 1_000_000_000,
            max_priority_fee_per_gas: 100_000_000,
            l1_data_fee: None,
            estimated_cost_wei: U256::ZERO,
            estimated_cost_usd: None,
        };
        let (encoded, tx_hash) = sign_eip1559_tx(&tx_req, &gas, 0, &signer).unwrap();

        // EIP-1559 type prefix
        assert_eq!(encoded[0], 0x02);
        assert!(encoded.len() > 1);
        assert_ne!(tx_hash, B256::ZERO);
    }
}
