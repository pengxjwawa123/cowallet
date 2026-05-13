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
        let default_rpc = "https://1rpc.io/eth".to_string();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ethereum_mainnet_config() {
        let chain = ChainConfig::ethereum_mainnet();
        assert_eq!(chain.chain_id, 1);
        assert_eq!(chain.name, "ethereum");
        assert_eq!(chain.native_currency.symbol, "ETH");
        assert_eq!(chain.native_currency.decimals, 18);
        assert_eq!(chain.gas_model, GasModel::Eip1559);
        assert!(!chain.is_testnet);
        assert!(!chain.is_l2);
        assert!(chain.erc4337_entrypoint.is_some());
    }

    #[test]
    fn test_base_mainnet_config() {
        let chain = ChainConfig::base_mainnet();
        assert_eq!(chain.chain_id, 8453);
        assert_eq!(chain.name, "base");
        assert_eq!(chain.native_currency.symbol, "ETH");
        assert_eq!(chain.gas_model, GasModel::OpBedrock);
        assert!(!chain.is_testnet);
        assert!(chain.is_l2);
    }

    #[test]
    fn test_arbitrum_one_config() {
        let chain = ChainConfig::arbitrum_one();
        assert_eq!(chain.chain_id, 42161);
        assert_eq!(chain.name, "arbitrum");
        assert_eq!(chain.native_currency.symbol, "ETH");
        assert_eq!(chain.gas_model, GasModel::ArbitrumNitro);
        assert!(!chain.is_testnet);
        assert!(chain.is_l2);
    }

    #[test]
    fn test_optimism_mainnet_config() {
        let chain = ChainConfig::optimism_mainnet();
        assert_eq!(chain.chain_id, 10);
        assert_eq!(chain.name, "optimism");
        assert_eq!(chain.native_currency.symbol, "ETH");
        assert_eq!(chain.gas_model, GasModel::OpBedrock);
        assert!(!chain.is_testnet);
        assert!(chain.is_l2);
    }

    #[test]
    fn test_bnb_chain_config() {
        let chain = ChainConfig::bnb_chain();
        assert_eq!(chain.chain_id, 56);
        assert_eq!(chain.name, "bsc");
        assert_eq!(chain.native_currency.symbol, "BNB");
        assert_eq!(chain.native_currency.decimals, 18);
        assert_eq!(chain.gas_model, GasModel::Legacy);
        assert!(!chain.is_testnet);
        assert!(!chain.is_l2);
    }

    #[test]
    fn test_polygon_via_chain_id() {
        // Polygon is not yet implemented, should return None
        let chain = ChainConfig::by_chain_id(137);
        assert!(chain.is_none());
    }

    #[test]
    fn test_by_chain_id_all_supported() {
        let supported_chains = vec![1, 8453, 42161, 10, 56, 84532, 11155111];

        for chain_id in supported_chains {
            let chain = ChainConfig::by_chain_id(chain_id);
            assert!(chain.is_some(), "chain_id {} should be supported", chain_id);
            assert_eq!(chain.unwrap().chain_id, chain_id);
        }
    }

    #[test]
    fn test_by_chain_id_unsupported() {
        let unsupported_chains = vec![137, 43114, 250, 100];

        for chain_id in unsupported_chains {
            let chain = ChainConfig::by_chain_id(chain_id);
            assert!(chain.is_none(), "chain_id {} should not be supported", chain_id);
        }
    }

    #[test]
    fn test_all_mainnet_chains() {
        let mainnets = ChainConfig::all_mainnet();
        assert_eq!(mainnets.len(), 5);

        for chain in mainnets {
            assert!(!chain.is_testnet);
            assert!(chain.chain_id > 0);
        }
    }

    #[test]
    fn test_all_testnet_chains() {
        let testnets = ChainConfig::all_testnet();
        assert_eq!(testnets.len(), 2);

        for chain in testnets {
            assert!(chain.is_testnet);
        }
    }

    #[test]
    fn test_rpc_urls_not_empty() {
        let chains = ChainConfig::all_mainnet();

        for chain in chains {
            assert!(!chain.rpc_urls.is_empty(), "chain {} should have RPC URLs", chain.name);
            assert!(chain.rpc_urls[0].starts_with("http"));
        }
    }

    #[test]
    fn test_entrypoint_addresses_consistent() {
        let expected_entrypoint = "0x0000000071727De22E5E9d8BAf0edAc6f37da032"
            .parse::<Address>()
            .unwrap();

        let chains = vec![
            ChainConfig::ethereum_mainnet(),
            ChainConfig::base_mainnet(),
            ChainConfig::arbitrum_one(),
            ChainConfig::optimism_mainnet(),
            ChainConfig::bnb_chain(),
        ];

        for chain in chains {
            assert_eq!(
                chain.erc4337_entrypoint,
                Some(expected_entrypoint),
                "chain {} should have consistent EntryPoint v0.7 address",
                chain.name
            );
        }
    }
}
