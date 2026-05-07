use alloy_primitives::Address;
use serde::{Deserialize, Serialize};
use std::env;

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
    pub is_l2: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeCurrency {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GasModel {
    Eip1559,
    ArbitrumNitro,
    OpBedrock,
    Legacy,
}

impl ChainConfig {
    pub fn ethereum_mainnet() -> Self {
        let default_rpc = "https://eth.llamarpc.com".to_string();
        let rpc_url = env::var("ETH_MAINNET_RPC_URL").unwrap_or(default_rpc);
        let bundler_url = env::var("BUNDLER_URL_ETH").ok();

        Self {
            chain_id: 1,
            name: "ethereum".into(),
            display_name: "Ethereum".into(),
            rpc_urls: vec![rpc_url],
            block_explorer: "https://etherscan.io".into(),
            native_currency: NativeCurrency {
                name: "Ether".into(),
                symbol: "ETH".into(),
                decimals: 18,
            },
            gas_model: GasModel::Eip1559,
            erc4337_entrypoint: Some(
                "0x0000000071727De22E5E9d8BAf0edAc6f37da032"
                    .parse()
                    .expect("valid EntryPoint v0.7 address"),
            ),
            bundler_url,
            paymaster_url: None,
            is_testnet: false,
            is_l2: false,
        }
    }

    pub fn base_mainnet() -> Self {
        let default_rpc = "https://mainnet.base.org".to_string();
        let rpc_url = env::var("BASE_MAINNET_RPC_URL").unwrap_or(default_rpc);
        let bundler_url = env::var("BUNDLER_URL_BASE").ok();

        Self {
            chain_id: 8453,
            name: "base".into(),
            display_name: "Base".into(),
            rpc_urls: vec![rpc_url],
            block_explorer: "https://basescan.org".into(),
            native_currency: NativeCurrency {
                name: "Ether".into(),
                symbol: "ETH".into(),
                decimals: 18,
            },
            gas_model: GasModel::OpBedrock,
            erc4337_entrypoint: Some(
                "0x0000000071727De22E5E9d8BAf0edAc6f37da032"
                    .parse()
                    .expect("valid EntryPoint v0.7 address"),
            ),
            bundler_url,
            paymaster_url: None,
            is_testnet: false,
            is_l2: true,
        }
    }

    pub fn arbitrum_one() -> Self {
        let default_rpc = "https://arb1.arbitrum.io/rpc".to_string();
        let rpc_url = env::var("ARB_MAINNET_RPC_URL").unwrap_or(default_rpc);
        let bundler_url = env::var("BUNDLER_URL_ARB").ok();

        Self {
            chain_id: 42161,
            name: "arbitrum".into(),
            display_name: "Arbitrum One".into(),
            rpc_urls: vec![rpc_url],
            block_explorer: "https://arbiscan.io".into(),
            native_currency: NativeCurrency {
                name: "Ether".into(),
                symbol: "ETH".into(),
                decimals: 18,
            },
            gas_model: GasModel::ArbitrumNitro,
            erc4337_entrypoint: Some(
                "0x0000000071727De22E5E9d8BAf0edAc6f37da032"
                    .parse()
                    .expect("valid EntryPoint v0.7 address"),
            ),
            bundler_url,
            paymaster_url: None,
            is_testnet: false,
            is_l2: true,
        }
    }

    pub fn optimism_mainnet() -> Self {
        let default_rpc = "https://mainnet.optimism.io".to_string();
        let rpc_url = env::var("OP_MAINNET_RPC_URL").unwrap_or(default_rpc);
        let bundler_url = env::var("BUNDLER_URL_OP").ok();

        Self {
            chain_id: 10,
            name: "optimism".into(),
            display_name: "Optimism".into(),
            rpc_urls: vec![rpc_url],
            block_explorer: "https://optimistic.etherscan.io".into(),
            native_currency: NativeCurrency {
                name: "Ether".into(),
                symbol: "ETH".into(),
                decimals: 18,
            },
            gas_model: GasModel::OpBedrock,
            erc4337_entrypoint: Some(
                "0x0000000071727De22E5E9d8BAf0edAc6f37da032"
                    .parse()
                    .expect("valid EntryPoint v0.7 address"),
            ),
            bundler_url,
            paymaster_url: None,
            is_testnet: false,
            is_l2: true,
        }
    }

    pub fn bnb_chain() -> Self {
        let default_rpc = "https://bsc-dataseed.binance.org".to_string();
        let rpc_url = env::var("BSC_MAINNET_RPC_URL").unwrap_or(default_rpc);
        let bundler_url = env::var("BUNDLER_URL_BSC").ok();

        Self {
            chain_id: 56,
            name: "bsc".into(),
            display_name: "BNB Chain".into(),
            rpc_urls: vec![rpc_url],
            block_explorer: "https://bscscan.com".into(),
            native_currency: NativeCurrency {
                name: "BNB".into(),
                symbol: "BNB".into(),
                decimals: 18,
            },
            gas_model: GasModel::Legacy,
            erc4337_entrypoint: Some(
                "0x0000000071727De22E5E9d8BAf0edAc6f37da032"
                    .parse()
                    .expect("valid EntryPoint v0.7 address"),
            ),
            bundler_url,
            paymaster_url: None,
            is_testnet: false,
            is_l2: false,
        }
    }

    pub fn base_sepolia() -> Self {
        let default_rpc = "https://sepolia.base.org".to_string();
        let rpc_url = env::var("BASE_SEPOLIA_RPC_URL").unwrap_or(default_rpc);
        let bundler_url = env::var("BUNDLER_URL_BASE_SEPOLIA").ok();

        Self {
            chain_id: 84532,
            name: "base-sepolia".into(),
            display_name: "Base Sepolia".into(),
            rpc_urls: vec![rpc_url],
            block_explorer: "https://sepolia.basescan.org".into(),
            native_currency: NativeCurrency {
                name: "Ether".into(),
                symbol: "ETH".into(),
                decimals: 18,
            },
            gas_model: GasModel::OpBedrock,
            erc4337_entrypoint: Some(
                "0x0000000071727De22E5E9d8BAf0edAc6f37da032"
                    .parse()
                    .expect("valid EntryPoint v0.7 address"),
            ),
            bundler_url,
            paymaster_url: None,
            is_testnet: true,
            is_l2: true,
        }
    }

    pub fn ethereum_sepolia() -> Self {
        let default_rpc = "https://rpc.sepolia.org".to_string();
        let rpc_url = env::var("ETH_SEPOLIA_RPC_URL").unwrap_or(default_rpc);
        let bundler_url = env::var("BUNDLER_URL_ETH_SEPOLIA").ok();

        Self {
            chain_id: 11155111,
            name: "sepolia".into(),
            display_name: "Ethereum Sepolia".into(),
            rpc_urls: vec![rpc_url],
            block_explorer: "https://sepolia.etherscan.io".into(),
            native_currency: NativeCurrency {
                name: "Ether".into(),
                symbol: "ETH".into(),
                decimals: 18,
            },
            gas_model: GasModel::Eip1559,
            erc4337_entrypoint: Some(
                "0x0000000071727De22E5E9d8BAf0edAc6f37da032"
                    .parse()
                    .expect("valid EntryPoint v0.7 address"),
            ),
            bundler_url,
            paymaster_url: None,
            is_testnet: true,
            is_l2: false,
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

    /// Return all supported testnet chains.
    pub fn all_testnet() -> Vec<Self> {
        vec![Self::ethereum_sepolia(), Self::base_sepolia()]
    }

    /// Get chain config by chain_id.
    pub fn by_chain_id(chain_id: u64) -> Option<Self> {
        match chain_id {
            1 => Some(Self::ethereum_mainnet()),
            8453 => Some(Self::base_mainnet()),
            42161 => Some(Self::arbitrum_one()),
            10 => Some(Self::optimism_mainnet()),
            56 => Some(Self::bnb_chain()),
            84532 => Some(Self::base_sepolia()),
            11155111 => Some(Self::ethereum_sepolia()),
            _ => None,
        }
    }
}
