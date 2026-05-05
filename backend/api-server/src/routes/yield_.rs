use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::retry::{is_retryable_error, retry_with_backoff, RetryConfig};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/search", get(search))
        .route("/protocols", get(list_protocols))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolType {
    Dex,
    Lending,
    LiquidStaking,
    Vault,
    Farm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    VeryHigh,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProtocolInfo {
    pub id: String,
    pub name: String,
    pub chain_id: u64,
    pub protocol_type: ProtocolType,
    pub tvl_usd: f64,
    pub apy_range: [f64; 2],
    pub risk_level: RiskLevel,
    pub audit_count: u32,
    pub days_active: u32,
}

// Protocol data as const slices for static initialization
struct ProtocolData {
    id: &'static str,
    name: &'static str,
    chain_id: u64,
    protocol_type: ProtocolType,
    tvl_usd: f64,
    apy_range: [f64; 2],
    risk_level: RiskLevel,
    audit_count: u32,
    days_active: u32,
}

const PROTOCOL_DATA: &[ProtocolData] = &[
    ProtocolData {
        id: "aave-v3-base",
        name: "Aave V3",
        chain_id: 8453,
        protocol_type: ProtocolType::Lending,
        tvl_usd: 450_000_000.0,
        apy_range: [0.5, 4.5],
        risk_level: RiskLevel::Low,
        audit_count: 5,
        days_active: 450,
    },
    ProtocolData {
        id: "uniswap-v3-base",
        name: "Uniswap V3",
        chain_id: 8453,
        protocol_type: ProtocolType::Dex,
        tvl_usd: 380_000_000.0,
        apy_range: [0.1, 25.0],
        risk_level: RiskLevel::Medium,
        audit_count: 4,
        days_active: 420,
    },
    ProtocolData {
        id: "aerodrome-base",
        name: "Aerodrome",
        chain_id: 8453,
        protocol_type: ProtocolType::Dex,
        tvl_usd: 220_000_000.0,
        apy_range: [1.0, 45.0],
        risk_level: RiskLevel::Medium,
        audit_count: 2,
        days_active: 180,
    },
    ProtocolData {
        id: "morpho-base",
        name: "Morpho Blue",
        chain_id: 8453,
        protocol_type: ProtocolType::Lending,
        tvl_usd: 180_000_000.0,
        apy_range: [2.0, 8.0],
        risk_level: RiskLevel::Low,
        audit_count: 3,
        days_active: 200,
    },
    ProtocolData {
        id: "coinbase-cbeth",
        name: "Coinbase cbETH",
        chain_id: 8453,
        protocol_type: ProtocolType::LiquidStaking,
        tvl_usd: 2_500_000_000.0,
        apy_range: [3.2, 3.8],
        risk_level: RiskLevel::Low,
        audit_count: 6,
        days_active: 500,
    },
    ProtocolData {
        id: "lido-steth-base",
        name: "Lido stETH",
        chain_id: 8453,
        protocol_type: ProtocolType::LiquidStaking,
        tvl_usd: 1_800_000_000.0,
        apy_range: [3.0, 3.6],
        risk_level: RiskLevel::Low,
        audit_count: 7,
        days_active: 550,
    },
    ProtocolData {
        id: "pendle-base",
        name: "Pendle Finance",
        chain_id: 8453,
        protocol_type: ProtocolType::Vault,
        tvl_usd: 95_000_000.0,
        apy_range: [4.0, 15.0],
        risk_level: RiskLevel::Medium,
        audit_count: 3,
        days_active: 150,
    },
    ProtocolData {
        id: "yearn-base",
        name: "Yearn V3",
        chain_id: 8453,
        protocol_type: ProtocolType::Vault,
        tvl_usd: 45_000_000.0,
        apy_range: [2.5, 12.0],
        risk_level: RiskLevel::Medium,
        audit_count: 4,
        days_active: 300,
    },
];

fn get_protocols() -> Vec<ProtocolInfo> {
    PROTOCOL_DATA
        .iter()
        .map(|p| ProtocolInfo {
            id: p.id.to_string(),
            name: p.name.to_string(),
            chain_id: p.chain_id,
            protocol_type: p.protocol_type.clone(),
            tvl_usd: p.tvl_usd,
            apy_range: p.apy_range,
            risk_level: p.risk_level.clone(),
            audit_count: p.audit_count,
            days_active: p.days_active,
        })
        .collect()
}

/// DeFi Llama API response structure
#[derive(Debug, Clone, Deserialize)]
pub struct DefiLlamaPool {
    pub pool: String,
    pub chain: String,
    pub project: String,
    pub symbol: String,
    pub apy: Option<f64>,
    #[serde(rename = "tvlUsd")]
    pub tvl_usd: Option<f64>,
    #[serde(rename = "apyBase")]
    pub apy_base: Option<f64>,
    #[serde(rename = "apyReward")]
    pub apy_reward: Option<f64>,
    #[serde(rename = "volumeUsd1d")]
    pub volume_24h: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DefiLlamaResponse {
    pub data: Vec<DefiLlamaPool>,
}

/// Yield data cache with TTL
#[derive(Clone, Debug)]
pub struct YieldCache {
    pub data: Arc<RwLock<Vec<YieldOpportunity>>>,
    pub last_updated: Arc<RwLock<Option<chrono::DateTime<chrono::Utc>>>>,
}

impl YieldCache {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(Vec::new())),
            last_updated: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn is_stale(&self) -> bool {
        let last = self.last_updated.read().await;
        match *last {
            Some(time) => (chrono::Utc::now() - time) > chrono::Duration::minutes(5),
            None => true,
        }
    }

    pub async fn update(&self, data: Vec<YieldOpportunity>) {
        let mut cache = self.data.write().await;
        *cache = data;
        let mut last = self.last_updated.write().await;
        *last = Some(chrono::Utc::now());
    }
}

/// Fetch yield data from DeFi Llama API with circuit breaker protection
pub async fn fetch_defi_llama_data(
    client: &reqwest::Client,
    circuit_breaker: &crate::retry::CircuitBreaker,
) -> Result<Vec<YieldOpportunity>, String> {
    let url = "https://yields.llama.fi/pools";

    let response_result = circuit_breaker
        .call(|| async {
            retry_with_backoff(
                RetryConfig::default(),
                || async {
                    client
                        .get(url)
                        .timeout(std::time::Duration::from_secs(10))
                        .send()
                        .await
                        .and_then(|r| r.error_for_status())
                },
                is_retryable_error,
                "defi_llama_fetch",
            )
            .await
        })
        .await;

    let response = match response_result {
        Ok(r) => r,
        Err(None) => {
            return Err("DeFi API unavailable - circuit breaker open".to_string());
        }
        Err(Some(e)) => {
            return Err(format!("Request failed: {}", e));
        }
    };

    let data: DefiLlamaResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let base_chain = "Base";

    let opportunities: Vec<YieldOpportunity> = data
        .data
        .into_iter()
        .filter(|pool| pool.chain == base_chain)
        .filter_map(|pool| {
            let apy = pool.apy.unwrap_or(0.0);
            if apy <= 0.0 {
                return None;
            }

            let risk_level = if apy < 5.0 {
                RiskLevel::Low
            } else if apy < 15.0 {
                RiskLevel::Medium
            } else if apy < 30.0 {
                RiskLevel::High
            } else {
                RiskLevel::VeryHigh
            };

            let protocol_type = map_project_to_type(&pool.project);

            Some(YieldOpportunity {
                id: pool.pool.clone(),
                protocol_id: pool.project.clone(),
                protocol_name: format_project_name(&pool.project),
                chain_id: 8453,
                opportunity_type: protocol_type,
                token_a: Some(TokenInfo {
                    address: "0x0".to_string(),
                    symbol: pool.symbol,
                    name: "".to_string(),
                    decimals: 18,
                    price_usd: None,
                }),
                token_b: None,
                apy,
                apy_breakdown: ApyBreakdown {
                    base_apy: pool.apy_base.unwrap_or(0.0),
                    reward_apy: pool.apy_reward.unwrap_or(0.0),
                    incentive_apy: 0.0,
                    total_apy: apy,
                },
                tvl_usd: pool.tvl_usd.unwrap_or(0.0),
                volume_24h_usd: pool.volume_24h,
                risk_level: risk_level.clone(),
                risk_factors: generate_risk_factors(&risk_level),
                strategy: None,
                lock_days: None,
                smart_contract_address: "0x0".to_string(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            })
        })
        .collect();

    Ok(opportunities)
}

fn map_project_to_type(project: &str) -> ProtocolType {
    match project {
        "aave-v3" | "morpho-blue" | "compound-v3" => ProtocolType::Lending,
        "uniswap-v3" | "sushi-v3" | "aerodrome" | "velodrome" => ProtocolType::Dex,
        "lido" | "rocket-pool" | "coinbase-staked-eth" => ProtocolType::LiquidStaking,
        "pendle" | "yearn" | "convex" | "curve" => ProtocolType::Vault,
        _ => ProtocolType::Farm,
    }
}

fn format_project_name(project: &str) -> String {
    let mapping = [
        ("aave-v3", "Aave V3"),
        ("uniswap-v3", "Uniswap V3"),
        ("aerodrome", "Aerodrome"),
        ("morpho-blue", "Morpho Blue"),
        ("lido", "Lido"),
        ("pendle", "Pendle Finance"),
        ("yearn", "Yearn V3"),
        ("rocket-pool", "Rocket Pool"),
        ("compound-v3", "Compound V3"),
    ];

    mapping
        .iter()
        .find(|(k, _)| *k == project)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| project.to_string())
}

fn generate_risk_factors(level: &RiskLevel) -> Vec<String> {
    match level {
        RiskLevel::Low => vec!["Smart contract risk".to_string()],
        RiskLevel::Medium => vec![
            "Smart contract risk".to_string(),
            "Impermanent loss".to_string(),
        ],
        RiskLevel::High => vec![
            "Smart contract risk".to_string(),
            "Impermanent loss".to_string(),
            "Reward token price volatility".to_string(),
        ],
        RiskLevel::VeryHigh => vec![
            "Smart contract risk".to_string(),
            "Impermanent loss".to_string(),
            "Reward token price volatility".to_string(),
            "High leverage risk".to_string(),
            "Experimental protocol".to_string(),
        ],
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct YieldOpportunity {
    pub id: String,
    pub protocol_id: String,
    pub protocol_name: String,
    pub chain_id: u64,
    pub opportunity_type: ProtocolType,
    pub token_a: Option<TokenInfo>,
    pub token_b: Option<TokenInfo>,
    pub apy: f64,
    pub apy_breakdown: ApyBreakdown,
    pub tvl_usd: f64,
    pub volume_24h_usd: Option<f64>,
    pub risk_level: RiskLevel,
    pub risk_factors: Vec<String>,
    pub strategy: Option<String>,
    pub lock_days: Option<u32>,
    pub smart_contract_address: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenInfo {
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub price_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApyBreakdown {
    pub base_apy: f64,
    pub reward_apy: f64,
    pub incentive_apy: f64,
    pub total_apy: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct YieldSearchResponse {
    pub opportunities: Vec<YieldOpportunity>,
    pub total_count: usize,
    pub best_apy: f64,
    pub average_apy: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchQuery {
    pub chain_id: Option<u64>,
    pub protocol_type: Option<String>,
    pub min_apy: Option<f64>,
    pub max_risk: Option<String>,
    pub token: Option<String>,
    pub limit: Option<usize>,
}

fn generate_opportunities() -> Vec<YieldOpportunity> {
    let mut opps = Vec::new();

    // Aave V3 Lending Markets
    opps.push(YieldOpportunity {
        id: "aave-eth-supply".to_string(),
        protocol_id: "aave-v3-base".to_string(),
        protocol_name: "Aave V3".to_string(),
        chain_id: 8453,
        opportunity_type: ProtocolType::Lending,
        token_a: Some(TokenInfo {
            address: "0x4200000000000000000000000000000000000006".to_string(),
            symbol: "WETH".to_string(),
            name: "Wrapped Ether".to_string(),
            decimals: 18,
            price_usd: Some(3200.0),
        }),
        token_b: None,
        apy: 2.8,
        apy_breakdown: ApyBreakdown {
            base_apy: 2.8,
            reward_apy: 0.0,
            incentive_apy: 0.0,
            total_apy: 2.8,
        },
        tvl_usd: 125_000_000.0,
        volume_24h_usd: Some(8_500_000.0),
        risk_level: RiskLevel::Low,
        risk_factors: vec!["Smart contract risk".to_string(), "Liquidation risk".to_string()],
        strategy: Some("Supply ETH as collateral to earn interest from borrowers".to_string()),
        lock_days: None,
        smart_contract_address: "0xe50fA9b3c56FfB159cB0FCA61F5c910BBc05074E".to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    });

    opps.push(YieldOpportunity {
        id: "aave-usdc-supply".to_string(),
        protocol_id: "aave-v3-base".to_string(),
        protocol_name: "Aave V3".to_string(),
        chain_id: 8453,
        opportunity_type: ProtocolType::Lending,
        token_a: Some(TokenInfo {
            address: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
            symbol: "USDC".to_string(),
            name: "USD Coin".to_string(),
            decimals: 6,
            price_usd: Some(1.0),
        }),
        token_b: None,
        apy: 4.2,
        apy_breakdown: ApyBreakdown {
            base_apy: 4.2,
            reward_apy: 0.0,
            incentive_apy: 0.0,
            total_apy: 4.2,
        },
        tvl_usd: 85_000_000.0,
        volume_24h_usd: Some(12_000_000.0),
        risk_level: RiskLevel::Low,
        risk_factors: vec!["Smart contract risk".to_string(), "Depegging risk".to_string()],
        strategy: Some("Supply USDC stablecoin to earn variable interest rate.".to_string()),
        lock_days: None,
        smart_contract_address: "0xe50fA9b3c56FfB159cB0FCA61F5c910BBc05074E".to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    });

    // Uniswap V3 Pools
    opps.push(YieldOpportunity {
        id: "uni-v3-eth-usdc-03".to_string(),
        protocol_id: "uniswap-v3-base".to_string(),
        protocol_name: "Uniswap V3".to_string(),
        chain_id: 8453,
        opportunity_type: ProtocolType::Dex,
        token_a: Some(TokenInfo {
            address: "0x4200000000000000000000000000000000000006".to_string(),
            symbol: "WETH".to_string(),
            name: "Wrapped Ether".to_string(),
            decimals: 18,
            price_usd: Some(3200.0),
        }),
        token_b: Some(TokenInfo {
            address: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
            symbol: "USDC".to_string(),
            name: "USD Coin".to_string(),
            decimals: 6,
            price_usd: Some(1.0),
        }),
        apy: 18.5,
        apy_breakdown: ApyBreakdown {
            base_apy: 12.5,
            reward_apy: 6.0,
            incentive_apy: 0.0,
            total_apy: 18.5,
        },
        tvl_usd: 45_000_000.0,
        volume_24h_usd: Some(85_000_000.0),
        risk_level: RiskLevel::Medium,
        risk_factors: vec![
            "Impermanent loss".to_string(),
            "Smart contract risk".to_string(),
            "Concentrated liquidity range risk".to_string(),
        ],
        strategy: Some("Provide concentrated liquidity in ETH-USDC 0.3% fee pool with narrow range for higher fees.".to_string()),
        lock_days: None,
        smart_contract_address: "0x33128a8fC17869897dcE68Ed026d694621f6FDfD".to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    });

    // Aerodrome Farms
    opps.push(YieldOpportunity {
        id: "aero-eth-usdc-volatile".to_string(),
        protocol_id: "aerodrome-base".to_string(),
        protocol_name: "Aerodrome".to_string(),
        chain_id: 8453,
        opportunity_type: ProtocolType::Farm,
        token_a: Some(TokenInfo {
            address: "0x4200000000000000000000000000000000000006".to_string(),
            symbol: "WETH".to_string(),
            name: "Wrapped Ether".to_string(),
            decimals: 18,
            price_usd: Some(3200.0),
        }),
        token_b: Some(TokenInfo {
            address: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string(),
            symbol: "USDC".to_string(),
            name: "USD Coin".to_string(),
            decimals: 6,
            price_usd: Some(1.0),
        }),
        apy: 38.2,
        apy_breakdown: ApyBreakdown {
            base_apy: 8.2,
            reward_apy: 25.0,
            incentive_apy: 5.0,
            total_apy: 38.2,
        },
        tvl_usd: 12_500_000.0,
        volume_24h_usd: Some(18_000_000.0),
        risk_level: RiskLevel::High,
        risk_factors: vec![
            "Impermanent loss".to_string(),
            "Reward token price volatility".to_string(),
            "Smart contract risk".to_string(),
            "Voting lock requirements".to_string(),
        ],
        strategy: Some("Stake ETH-USDC LP tokens in Aerodrome gauge to earn trading fees plus AERO emissions.".to_string()),
        lock_days: Some(7),
        smart_contract_address: "0x420DD381b31aEf6683db6B902084cB0FFECe40Da".to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    });

    // Liquid Staking
    opps.push(YieldOpportunity {
        id: "cbeth-stake".to_string(),
        protocol_id: "coinbase-cbeth".to_string(),
        protocol_name: "Coinbase cbETH".to_string(),
        chain_id: 8453,
        opportunity_type: ProtocolType::LiquidStaking,
        token_a: Some(TokenInfo {
            address: "0x4200000000000000000000000000000000000006".to_string(),
            symbol: "ETH".to_string(),
            name: "Ether".to_string(),
            decimals: 18,
            price_usd: Some(3200.0),
        }),
        token_b: None,
        apy: 3.5,
        apy_breakdown: ApyBreakdown {
            base_apy: 3.5,
            reward_apy: 0.0,
            incentive_apy: 0.0,
            total_apy: 3.5,
        },
        tvl_usd: 2_500_000_000.0,
        volume_24h_usd: Some(25_000_000.0),
        risk_level: RiskLevel::Low,
        risk_factors: vec![
            "Consensus layer slashing risk".to_string(),
            "Centralization risk (Coinbase)".to_string(),
            "Withdrawal queue delay".to_string(),
        ],
        strategy: Some("Stake ETH through Coinbase to receive cbETH liquid staking derivative. Earn consensus and execution layer rewards.".to_string()),
        lock_days: None,
        smart_contract_address: "0x2Ae3F1Ec7F1F5012CFEab0185bfc7aa3cf0DEc8".to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    });

    opps.push(YieldOpportunity {
        id: "steth-lido".to_string(),
        protocol_id: "lido-steth-base".to_string(),
        protocol_name: "Lido stETH".to_string(),
        chain_id: 8453,
        opportunity_type: ProtocolType::LiquidStaking,
        token_a: Some(TokenInfo {
            address: "0x4200000000000000000000000000000000000006".to_string(),
            symbol: "ETH".to_string(),
            name: "Ether".to_string(),
            decimals: 18,
            price_usd: Some(3200.0),
        }),
        token_b: None,
        apy: 3.3,
        apy_breakdown: ApyBreakdown {
            base_apy: 3.3,
            reward_apy: 0.0,
            incentive_apy: 0.0,
            total_apy: 3.3,
        },
        tvl_usd: 1_800_000_000.0,
        volume_24h_usd: Some(45_000_000.0),
        risk_level: RiskLevel::Low,
        risk_factors: vec![
            "Consensus layer slashing risk".to_string(),
            "Oracle risk".to_string(),
            "Withdrawal queue delay".to_string(),
        ],
        strategy: Some("Stake ETH with Lido's decentralized validator set to receive stETH. No minimum, no lock-up.".to_string()),
        lock_days: None,
        smart_contract_address: "0x76712280a2F7d86855f89020505915E7B571914f".to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    });

    // Pendle PT strategies
    opps.push(YieldOpportunity {
        id: "pendle-cbeth-pt".to_string(),
        protocol_id: "pendle-base".to_string(),
        protocol_name: "Pendle Finance".to_string(),
        chain_id: 8453,
        opportunity_type: ProtocolType::Vault,
        token_a: Some(TokenInfo {
            address: "0x2Ae3F1Ec7F1F5012CFEab0185bfc7aa3cf0DEc8".to_string(),
            symbol: "cbETH".to_string(),
            name: "Coinbase Wrapped Staked ETH".to_string(),
            decimals: 18,
            price_usd: Some(3450.0),
        }),
        token_b: None,
        apy: 8.7,
        apy_breakdown: ApyBreakdown {
            base_apy: 3.5,
            reward_apy: 0.0,
            incentive_apy: 5.2,
            total_apy: 8.7,
        },
        tvl_usd: 12_000_000.0,
        volume_24h_usd: Some(2_500_000.0),
        risk_level: RiskLevel::Medium,
        risk_factors: vec![
            "Yield token price fluctuation".to_string(),
            "Maturity date risk".to_string(),
            "Smart contract risk".to_string(),
        ],
        strategy: Some("Purchase discounted cbETH Principal Token (PT) on Pendle. Hold to maturity for fixed yield enhancement.".to_string()),
        lock_days: Some(180),
        smart_contract_address: "0x4A61E1DD111C21e7b96B16Fd4E5e202fF689a739".to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    });

    // Morpho Blue Markets
    opps.push(YieldOpportunity {
        id: "morpho-blue-weth-market".to_string(),
        protocol_id: "morpho-base".to_string(),
        protocol_name: "Morpho Blue".to_string(),
        chain_id: 8453,
        opportunity_type: ProtocolType::Lending,
        token_a: Some(TokenInfo {
            address: "0x4200000000000000000000000000000000000006".to_string(),
            symbol: "WETH".to_string(),
            name: "Wrapped Ether".to_string(),
            decimals: 18,
            price_usd: Some(3200.0),
        }),
        token_b: None,
        apy: 5.2,
        apy_breakdown: ApyBreakdown {
            base_apy: 5.2,
            reward_apy: 0.0,
            incentive_apy: 0.0,
            total_apy: 5.2,
        },
        tvl_usd: 35_000_000.0,
        volume_24h_usd: Some(5_000_000.0),
        risk_level: RiskLevel::Low,
        risk_factors: vec![
            "Smart contract risk".to_string(),
            "Oracle risk".to_string(),
            "Isolated market risk".to_string(),
        ],
        strategy: Some("Supply WETH to Morpho Blue's isolated lending market with risk-adjusted interest rates.".to_string()),
        lock_days: None,
        smart_contract_address: "0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb".to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    });

    opps
}

async fn search(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Json<YieldSearchResponse> {
    // Refresh cache if stale
    if state.yield_cache.is_stale().await {
        if let Ok(data) = fetch_defi_llama_data(&state.http, &state.defi_circuit_breaker).await {
            if !data.is_empty() {
                state.yield_cache.update(data).await;
            }
        }
    }

    // Try to get cached data, fallback to static if cache is empty
    let cached = state.yield_cache.data.read().await;
    let all_opps = if !cached.is_empty() {
        cached.clone()
    } else {
        drop(cached);
        match fetch_defi_llama_data(&state.http, &state.defi_circuit_breaker).await {
            Ok(data) if !data.is_empty() => data,
            _ => generate_opportunities(),
        }
    };

    let mut filtered: Vec<YieldOpportunity> = all_opps
        .into_iter()
        .filter(|opp| {
            if let Some(chain_id) = q.chain_id {
                if opp.chain_id != chain_id {
                    return false;
                }
            }
            if let Some(ref ptype) = q.protocol_type {
                let opp_type = serde_json::to_string(&opp.opportunity_type)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string();
                if opp_type.to_lowercase() != ptype.to_lowercase() {
                    return false;
                }
            }
            if let Some(min_apy) = q.min_apy {
                if opp.apy < min_apy {
                    return false;
                }
            }
            if let Some(ref token) = q.token {
                let token_upper = token.to_uppercase();
                let matches = opp
                    .token_a
                    .as_ref()
                    .map(|t| t.symbol == token_upper || t.address.to_lowercase() == token.to_lowercase())
                    .unwrap_or(false)
                    || opp
                        .token_b
                        .as_ref()
                        .map(|t| t.symbol == token_upper || t.address.to_lowercase() == token.to_lowercase())
                        .unwrap_or(false);
                if !matches {
                    return false;
                }
            }
            true
        })
        .collect();

    // Sort by APY descending
    filtered.sort_by(|a, b| b.apy.partial_cmp(&a.apy).unwrap_or(std::cmp::Ordering::Equal));

    let limit = q.limit.unwrap_or(20).min(50);
    let total_count = filtered.len();
    let best_apy = filtered.first().map(|o| o.apy).unwrap_or(0.0);
    let average_apy = if !filtered.is_empty() {
        filtered.iter().map(|o| o.apy).sum::<f64>() / filtered.len() as f64
    } else {
        0.0
    };

    filtered.truncate(limit);

    Json(YieldSearchResponse {
        opportunities: filtered,
        total_count,
        best_apy,
        average_apy,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ProtocolsResponse {
    pub protocols: Vec<ProtocolInfo>,
}

async fn list_protocols(Query(q): Query<SearchQuery>) -> Json<ProtocolsResponse> {
    let protocols = get_protocols();
    let filtered: Vec<ProtocolInfo> = protocols
        .into_iter()
        .filter(|p| {
            if let Some(chain_id) = q.chain_id {
                if p.chain_id != chain_id {
                    return false;
                }
            }
            if let Some(ref ptype) = q.protocol_type {
                let proto_type = serde_json::to_string(&p.protocol_type)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string();
                if proto_type.to_lowercase() != ptype.to_lowercase() {
                    return false;
                }
            }
            true
        })
        .collect();

    Json(ProtocolsResponse { protocols: filtered })
}
