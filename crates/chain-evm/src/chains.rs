use alloy_primitives::Address;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub name: String,
    pub display_name: String,
    pub rpc_urls: Vec<String>,
    pub block_explorer: String,
    pub native_currency: NativeCurrency,
    pub gas_model: GasModel,
    pub erc4337_entrypoint: Option<Address>,
    pub bundler_url: Option<String>,
    pub paymaster_url: Option<String>,
    pub is_testnet: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeCurrency {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum GasModel {
    Eip1559,
    ArbitrumNitro,
    OpBedrock,
    Legacy,
}

impl ChainConfig {
    pub fn ethereum_mainnet() -> Self {
        Self {
            chain_id: 1,
            name: "ethereum".into(),
            display_name: "Ethereum".into(),
            rpc_urls: vec!["https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY".into()],
            block_explorer: "https://etherscan.io".into(),
            native_currency: NativeCurrency {
                name: "Ether".into(),
                symbol: "ETH".into(),
                decimals: 18,
            },
            gas_model: GasModel::Eip1559,
            erc4337_entrypoint: Some(
                "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789"
                    .parse()
                    .expect("valid EntryPoint address"),
            ),
            bundler_url: Some("https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY".into()),
            paymaster_url: None,
            is_testnet: false,
        }
    }

    pub fn base_mainnet() -> Self {
        Self {
            chain_id: 8453,
            name: "base".into(),
            display_name: "Base".into(),
            rpc_urls: vec!["https://base-mainnet.g.alchemy.com/v2/YOUR_KEY".into()],
            block_explorer: "https://basescan.org".into(),
            native_currency: NativeCurrency {
                name: "Ether".into(),
                symbol: "ETH".into(),
                decimals: 18,
            },
            gas_model: GasModel::OpBedrock,
            erc4337_entrypoint: Some(
                "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789"
                    .parse()
                    .expect("valid EntryPoint address"),
            ),
            bundler_url: Some("https://base-mainnet.g.alchemy.com/v2/YOUR_KEY".into()),
            paymaster_url: None,
            is_testnet: false,
        }
    }

    pub fn arbitrum_one() -> Self {
        Self {
            chain_id: 42161,
            name: "arbitrum".into(),
            display_name: "Arbitrum One".into(),
            rpc_urls: vec!["https://arb-mainnet.g.alchemy.com/v2/YOUR_KEY".into()],
            block_explorer: "https://arbiscan.io".into(),
            native_currency: NativeCurrency {
                name: "Ether".into(),
                symbol: "ETH".into(),
                decimals: 18,
            },
            gas_model: GasModel::ArbitrumNitro,
            erc4337_entrypoint: Some(
                "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789"
                    .parse()
                    .expect("valid EntryPoint address"),
            ),
            bundler_url: Some("https://arb-mainnet.g.alchemy.com/v2/YOUR_KEY".into()),
            paymaster_url: None,
            is_testnet: false,
        }
    }

    pub fn optimism_mainnet() -> Self {
        Self {
            chain_id: 10,
            name: "optimism".into(),
            display_name: "Optimism".into(),
            rpc_urls: vec!["https://opt-mainnet.g.alchemy.com/v2/YOUR_KEY".into()],
            block_explorer: "https://optimistic.etherscan.io".into(),
            native_currency: NativeCurrency {
                name: "Ether".into(),
                symbol: "ETH".into(),
                decimals: 18,
            },
            gas_model: GasModel::OpBedrock,
            erc4337_entrypoint: Some(
                "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789"
                    .parse()
                    .expect("valid EntryPoint address"),
            ),
            bundler_url: Some("https://opt-mainnet.g.alchemy.com/v2/YOUR_KEY".into()),
            paymaster_url: None,
            is_testnet: false,
        }
    }

    pub fn bnb_chain() -> Self {
        Self {
            chain_id: 56,
            name: "bsc".into(),
            display_name: "BNB Chain".into(),
            rpc_urls: vec!["https://bsc-dataseed.binance.org".into()],
            block_explorer: "https://bscscan.com".into(),
            native_currency: NativeCurrency {
                name: "BNB".into(),
                symbol: "BNB".into(),
                decimals: 18,
            },
            gas_model: GasModel::Legacy,
            erc4337_entrypoint: Some(
                "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789"
                    .parse()
                    .expect("valid EntryPoint address"),
            ),
            bundler_url: Some("https://bsc-dataseed.binance.org".into()),
            paymaster_url: None,
            is_testnet: false,
        }
    }

    pub fn base_sepolia() -> Self {
        Self {
            chain_id: 84532,
            name: "base-sepolia".into(),
            display_name: "Base Sepolia".into(),
            rpc_urls: vec!["https://base-sepolia.g.alchemy.com/v2/YOUR_KEY".into()],
            block_explorer: "https://sepolia.basescan.org".into(),
            native_currency: NativeCurrency {
                name: "Ether".into(),
                symbol: "ETH".into(),
                decimals: 18,
            },
            gas_model: GasModel::OpBedrock,
            erc4337_entrypoint: Some(
                "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789"
                    .parse()
                    .expect("valid EntryPoint address"),
            ),
            bundler_url: Some("https://base-sepolia.g.alchemy.com/v2/YOUR_KEY".into()),
            paymaster_url: None,
            is_testnet: true,
        }
    }

    /// Return all supported mainnet chains.
    pub fn all_mainnet() -> Vec<Self> {
        vec![
            Self::ethereum_mainnet(),
            Self::base_mainnet(),
            Self::arbitrum_one(),
            Self::optimism_mainnet(),
            Self::bnb_chain(),
        ]
    }
}
