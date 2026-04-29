use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
use alloy_contract::Contract;
use alloy_network::Ethereum;
use serde::{Deserialize, Serialize};

/// Known ERC-20 token metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub address: Address,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub chain_id: u64,
    pub logo_uri: Option<String>,
}

/// A user's token balance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBalance {
    pub token: TokenInfo,
    pub balance: U256,
    pub balance_formatted: String,
    pub value_usd: Option<f64>,
}

/// Well-known token addresses by chain.
pub fn usdc_address(chain_id: u64) -> Option<Address> {
    match chain_id {
        1 => Some(
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
                .parse()
                .unwrap(),
        ),
        8453 => Some(
            "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
                .parse()
                .unwrap(),
        ),
        42161 => Some(
            "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"
                .parse()
                .unwrap(),
        ),
        10 => Some(
            "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85"
                .parse()
                .unwrap(),
        ),
        56 => Some(
            "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d"
                .parse()
                .unwrap(),
        ),
        _ => None,
    }
}

/// Query ERC-20 balance for a given token and owner.
pub async fn query_balance(
    token_address: Address,
    owner: Address,
    rpc_url: &str,
) -> Result<U256, TokenError> {
    let provider = alloy_provider::RootProvider::<Ethereum>::connect(rpc_url)
        .await
        .map_err(|e| TokenError::Rpc(e.to_string()))?;

    // Standard ERC20 ABI - balanceOf(address owner)
    let contract = Contract::new(token_address, ERC20_ABI.clone(), &provider);

    // Call balanceOf on the contract
    contract
        .call_raw::<(Address,), U256>("balanceOf", (owner,))
        .await
        .map_err(|e| TokenError::Rpc(e.to_string()))
}

/// Query native token (ETH/BNB) balance.
pub async fn query_native_balance(owner: Address, rpc_url: &str) -> Result<U256, TokenError> {
    let provider = alloy_provider::RootProvider::<Ethereum>::connect(rpc_url)
        .await
        .map_err(|e| TokenError::Rpc(e.to_string()))?;

    // Call eth_getBalance
    provider
        .get_balance(owner, Default::default())
        .await
        .map_err(|e| TokenError::Rpc(e.to_string()))
}

// Minimal ERC20 ABI for balanceOf
lazy_static::lazy_static! {
    static ref ERC20_ABI: alloy_json_abi::JsonAbi = serde_json::from_str(
        r#"[
            {
                "type": "function",
                "name": "balanceOf",
                "inputs": [{"name": "account", "type": "address"}],
                "outputs": [{"name": "", "type": "uint256"}],
                "stateMutability": "view"
            }
        ]"#
    ).unwrap();
}

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("unknown token: {0}")]
    UnknownToken(String),

    #[error("contract call failed: {0}")]
    ContractCallFailed(String),
}
