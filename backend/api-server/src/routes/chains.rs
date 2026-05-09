use axum::{routing::get, Json, Router};
use serde::Serialize;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(get_supported_chains))
}

#[derive(Serialize)]
struct ChainInfo {
    chain_id: u64,
    name: &'static str,
    display_name: &'static str,
    symbol: &'static str,
    is_testnet: bool,
    is_l2: bool,
}

#[derive(Serialize)]
struct ChainsResponse {
    chains: Vec<ChainInfo>,
}

async fn get_supported_chains() -> Json<ChainsResponse> {
    let chains = vec![
        ChainInfo {
            chain_id: 1,
            name: "ethereum",
            display_name: "Ethereum",
            symbol: "ETH",
            is_testnet: false,
            is_l2: false,
        },
        ChainInfo {
            chain_id: 8453,
            name: "base",
            display_name: "Base",
            symbol: "ETH",
            is_testnet: false,
            is_l2: true,
        },
        ChainInfo {
            chain_id: 42161,
            name: "arbitrum",
            display_name: "Arbitrum One",
            symbol: "ETH",
            is_testnet: false,
            is_l2: true,
        },
        ChainInfo {
            chain_id: 10,
            name: "optimism",
            display_name: "Optimism",
            symbol: "ETH",
            is_testnet: false,
            is_l2: true,
        },
        ChainInfo {
            chain_id: 56,
            name: "bsc",
            display_name: "BNB Chain",
            symbol: "BNB",
            is_testnet: false,
            is_l2: false,
        },
        ChainInfo {
            chain_id: 137,
            name: "polygon",
            display_name: "Polygon",
            symbol: "POL",
            is_testnet: false,
            is_l2: false,
        },
        ChainInfo {
            chain_id: 84532,
            name: "base-sepolia",
            display_name: "Base Sepolia",
            symbol: "ETH",
            is_testnet: true,
            is_l2: true,
        },
        ChainInfo {
            chain_id: 11155111,
            name: "sepolia",
            display_name: "Ethereum Sepolia",
            symbol: "ETH",
            is_testnet: true,
            is_l2: false,
        },
    ];

    Json(ChainsResponse { chains })
}
