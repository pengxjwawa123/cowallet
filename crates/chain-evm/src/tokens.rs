use alloy_primitives::{Address, U256};
use alloy_provider::Provider;
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

    // ERC20 balanceOf selector: keccak256("balanceOf(address)") = 0x70a08231
    // Encode the owner address as the parameter
    let data: Vec<u8> = {
        let mut buf = vec![0x70u8, 0xa0u8, 0x82u8, 0x31u8]; // balanceOf selector
        buf.extend_from_slice(&[0u8; 12]);
        buf.extend_from_slice(owner.as_slice());
        buf
    };

    // Make the raw call
    use alloy_rpc_types::TransactionRequest;
    let tx = TransactionRequest {
        to: Some(token_address.into()),
        input: alloy_primitives::Bytes::from(data).into(),
        ..Default::default()
    };

    let result = provider
        .call(tx)
        .await
        .map_err(|e| TokenError::Rpc(e.to_string()))?;

    // Decode result as U256
    if result.len() < 32 {
        return Err(TokenError::ContractCallFailed(
            "Invalid response length".to_string(),
        ));
    }

    Ok(U256::from_be_slice(&result[..32]))
}

/// Query native token (ETH/BNB) balance.
pub async fn query_native_balance(owner: Address, rpc_url: &str) -> Result<U256, TokenError> {
    let provider = alloy_provider::RootProvider::<Ethereum>::connect(rpc_url)
        .await
        .map_err(|e| TokenError::Rpc(e.to_string()))?;

    // Call eth_getBalance
    provider
        .get_balance(owner)
        .await
        .map_err(|e| TokenError::Rpc(e.to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usdc_address_ethereum() {
        let addr = usdc_address(1);
        assert!(addr.is_some());
        let expected: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
            .parse()
            .unwrap();
        assert_eq!(addr.unwrap(), expected);
    }

    #[test]
    fn test_usdc_address_base() {
        let addr = usdc_address(8453);
        assert!(addr.is_some());
        let expected: Address = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
            .parse()
            .unwrap();
        assert_eq!(addr.unwrap(), expected);
    }

    #[test]
    fn test_usdc_address_arbitrum() {
        let addr = usdc_address(42161);
        assert!(addr.is_some());
        let expected: Address = "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"
            .parse()
            .unwrap();
        assert_eq!(addr.unwrap(), expected);
    }

    #[test]
    fn test_usdc_address_optimism() {
        let addr = usdc_address(10);
        assert!(addr.is_some());
        let expected: Address = "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85"
            .parse()
            .unwrap();
        assert_eq!(addr.unwrap(), expected);
    }

    #[test]
    fn test_usdc_address_bsc() {
        let addr = usdc_address(56);
        assert!(addr.is_some());
        let expected: Address = "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d"
            .parse()
            .unwrap();
        assert_eq!(addr.unwrap(), expected);
    }

    #[test]
    fn test_usdc_address_unsupported_chain() {
        let addr = usdc_address(137); // Polygon
        assert!(addr.is_none());

        let addr = usdc_address(43114); // Avalanche
        assert!(addr.is_none());
    }

    #[test]
    fn test_usdc_addresses_all_chains() {
        let supported_chains = vec![1, 8453, 42161, 10, 56];

        for chain_id in supported_chains {
            let addr = usdc_address(chain_id);
            assert!(addr.is_some(), "USDC should exist on chain {}", chain_id);
            assert_ne!(addr.unwrap(), Address::ZERO);
        }
    }

    #[test]
    fn test_token_info_creation() {
        let token = TokenInfo {
            address: Address::ZERO,
            symbol: "TEST".into(),
            name: "Test Token".into(),
            decimals: 18,
            chain_id: 1,
            logo_uri: Some("https://example.com/logo.png".into()),
        };

        assert_eq!(token.symbol, "TEST");
        assert_eq!(token.decimals, 18);
        assert!(token.logo_uri.is_some());
    }

    #[test]
    fn test_token_balance_creation() {
        let token = TokenInfo {
            address: Address::ZERO,
            symbol: "USDC".into(),
            name: "USD Coin".into(),
            decimals: 6,
            chain_id: 1,
            logo_uri: None,
        };

        let balance = TokenBalance {
            token: token.clone(),
            balance: U256::from(1_000_000u64), // 1 USDC
            balance_formatted: "1.0".into(),
            value_usd: Some(1.0),
        };

        assert_eq!(balance.balance, U256::from(1_000_000u64));
        assert_eq!(balance.value_usd, Some(1.0));
    }
}
