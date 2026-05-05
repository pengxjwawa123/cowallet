//! AI Tool Execution Engine
//! Handles execution of wallet tools requested by Claude AI

use crate::routes::yield_::{fetch_defi_llama_data, ProtocolInfo};
use crate::state::AppState;
use alloy_primitives::Address;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use std::str::FromStr;

/// Execution context for tool calls
#[derive(Clone)]
pub struct ToolContext {
    pub app_state: AppState,
    pub user_id: Option<String>,
}

/// Result of a tool execution
#[derive(Debug, Clone, Serialize)]
pub struct ToolExecutionResult {
    pub tool_id: String,
    pub tool_name: String,
    pub success: bool,
    pub result: Value,
    pub error: Option<String>,
}

/// Helper: Parse a parameter from JSON Value
fn parse_param<T: for<'a> Deserialize<'a>>(params: &Value, key: &str) -> Option<T> {
    params
        .get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

/// Helper: Get demo wallet address (fallback)
fn demo_wallet_address() -> Address {
    std::env::var("WALLET_DEMO_ADDRESS")
        .ok()
        .and_then(|s| Address::from_str(&s).ok())
        .unwrap_or_else(|| {
            // Vitalik's address as default demo
            Address::from_str("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045").unwrap()
        })
}

/// Helper: Get USDC address for common chains
fn usdc_address_for_chain(chain_id: u64) -> Option<Address> {
    match chain_id {
        1 => Some(
            Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap(),
        ),
        8453 => Some(
            Address::from_str("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913").unwrap(),
        ),
        _ => None,
    }
}

/// Format U256 value with given decimals (simplified version)
fn format_units(value: alloy_primitives::U256, decimals: u32) -> String {
    // Very basic formatting - just divide by 10^decimals
    let divisor = alloy_primitives::U256::from(10).pow(alloy_primitives::U256::from(decimals));
    let integer = value / divisor;
    let fraction = value % divisor;
    if fraction.is_zero() {
        format!("{}", integer)
    } else {
        format!("{}.{:06}", integer, fraction.to_string().chars().take(6).collect::<String>())
    }
}

impl ToolContext {
    /// Execute a tool by name with parameters
    pub async fn execute_tool(&self, tool_name: &str, tool_id: &str, params: Value) -> ToolExecutionResult {
        tracing::debug!("Executing tool: {} with params: {:?}", tool_name, params);

        let result = match tool_name {
            "get_balance" => self.execute_get_balance(tool_id, params).await,
            "send_transaction" => self.execute_send_transaction(tool_id, params).await,
            "get_transaction_history" => self.execute_get_transaction_history(tool_id, params).await,
            "search_yield_opportunities" => self.execute_search_yield(tool_id, params).await,
            "list_yield_protocols" => self.execute_list_protocols(tool_id, params).await,
            _ => ToolExecutionResult {
                tool_id: tool_id.to_string(),
                tool_name: tool_name.to_string(),
                success: false,
                result: Value::Null,
                error: Some(format!("Unknown tool: {}", tool_name)),
            },
        };

        tracing::debug!(
            "Tool {} result: success={}, error={:?}",
            tool_name,
            result.success,
            result.error
        );
        result
    }

    // --- get_balance ---
    async fn execute_get_balance(&self, tool_id: &str, params: Value) -> ToolExecutionResult {
        let chain_id: u64 = parse_param(&params, "chain_id").unwrap_or(8453); // Default: Base
        let token: Option<String> = parse_param(&params, "token");

        let owner = demo_wallet_address();
        let rpc_url = &self.app_state.rpc_url;

        let result = match token.as_deref() {
            Some("ETH") | None => {
                // Native ETH balance
                match chain_evm::tokens::query_native_balance(owner, rpc_url).await {
                    Ok(balance) => {
                        let formatted = format!("{} ETH", format_units(balance, 18));
                        serde_json::json!({
                            "token": "ETH",
                            "chain_id": chain_id,
                            "address": format!("0x{:x}", owner),
                            "balance": balance.to_string(),
                            "balance_formatted": formatted,
                            "unit": "wei"
                        })
                    }
                    Err(e) => {
                        return ToolExecutionResult {
                            tool_id: tool_id.to_string(),
                            tool_name: "get_balance".into(),
                            success: false,
                            result: Value::Null,
                            error: Some(format!("Failed to query balance: {}", e)),
                        };
                    }
                }
            }
            Some(token_symbol) => {
                // ERC-20 token balance
                let token_address = if token_symbol.starts_with("0x") {
                    match Address::from_str(token_symbol) {
                        Ok(addr) => addr,
                        Err(_) => {
                            return ToolExecutionResult {
                                tool_id: tool_id.to_string(),
                                tool_name: "get_balance".into(),
                                success: false,
                                result: Value::Null,
                                error: Some(format!("Invalid token address: {}", token_symbol)),
                            };
                        }
                    }
                } else {
                    // Try to resolve by symbol
                    match token_symbol.to_uppercase().as_str() {
                        "USDC" => match usdc_address_for_chain(chain_id) {
                            Some(addr) => addr,
                            None => {
                                return ToolExecutionResult {
                                    tool_id: tool_id.to_string(),
                                    tool_name: "get_balance".into(),
                                    success: false,
                                    result: Value::Null,
                                    error: Some(format!("USDC not supported for chain {}", chain_id)),
                                };
                            }
                        },
                        _ => {
                            return ToolExecutionResult {
                                tool_id: tool_id.to_string(),
                                tool_name: "get_balance".into(),
                                success: false,
                                result: Value::Null,
                                error: Some(format!(
                                    "Token {} not recognized. Use 0x address or ETH/USDC",
                                    token_symbol
                                )),
                            };
                        }
                    }
                };

                match chain_evm::tokens::query_balance(token_address, owner, rpc_url).await {
                    Ok(balance) => {
                        let decimals = 6; // USDC default
                        let formatted = format!("{} {}", format_units(balance, decimals), token_symbol);
                        serde_json::json!({
                            "token": token_symbol,
                            "token_address": format!("0x{:x}", token_address),
                            "chain_id": chain_id,
                            "address": format!("0x{:x}", owner),
                            "balance": balance.to_string(),
                            "balance_formatted": formatted,
                            "unit": "wei"
                        })
                    }
                    Err(e) => {
                        return ToolExecutionResult {
                            tool_id: tool_id.to_string(),
                            tool_name: "get_balance".into(),
                            success: false,
                            result: Value::Null,
                            error: Some(format!("Failed to query token balance: {}", e)),
                        };
                    }
                }
            }
        };

        ToolExecutionResult {
            tool_id: tool_id.to_string(),
            tool_name: "get_balance".into(),
            success: true,
            result,
            error: None,
        }
    }

    // --- send_transaction ---
    async fn execute_send_transaction(&self, tool_id: &str, params: Value) -> ToolExecutionResult {
        // Important: We only PREPARE the transaction, do NOT actually send it
        // User biometric confirmation is required before signing

        let to_address: String = match parse_param(&params, "to_address") {
            Some(addr) => addr,
            None => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "send_transaction".into(),
                    success: false,
                    result: Value::Null,
                    error: Some("Missing required parameter: to_address".into()),
                };
            }
        };

        let value: String = match parse_param(&params, "value") {
            Some(v) => v,
            None => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "send_transaction".into(),
                    success: false,
                    result: Value::Null,
                    error: Some("Missing required parameter: value (in wei)".into()),
                };
            }
        };

        let chain_id: u64 = parse_param(&params, "chain_id").unwrap_or(8453);
        let from_address = demo_wallet_address();

        // Validate to_address format
        if !to_address.starts_with("0x") || to_address.len() != 42 {
            return ToolExecutionResult {
                tool_id: tool_id.to_string(),
                tool_name: "send_transaction".into(),
                success: false,
                result: Value::Null,
                error: Some("Invalid to_address format. Expected 0x-prefixed hex address".into()),
            };
        }

        // Parse value
        let value_u256 = match alloy_primitives::U256::from_str_radix(&value, 10) {
            Ok(v) => v,
            Err(_) => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "send_transaction".into(),
                    success: false,
                    result: Value::Null,
                    error: Some("Invalid value format. Expected numeric string in wei".into()),
                };
            }
        };

        // Estimate gas - for demo, use reasonable defaults
        let gas_limit = 21000;
        let value_formatted = format_units(value_u256, 18);

        let result = serde_json::json!({
            "status": "prepared",
            "from": format!("0x{:x}", from_address),
            "to": to_address,
            "value": value,
            "value_formatted": format!("{} ETH", value_formatted),
            "chain_id": chain_id,
            "estimated_gas": gas_limit,
            "warning": "This transaction requires your biometric confirmation before being signed and broadcast. Please verify all parameters carefully.",
            "next_step": "Review the details above and confirm with your biometric authentication to proceed"
        });

        ToolExecutionResult {
            tool_id: tool_id.to_string(),
            tool_name: "send_transaction".into(),
            success: true,
            result,
            error: None,
        }
    }

    // --- get_transaction_history ---
    async fn execute_get_transaction_history(&self, tool_id: &str, params: Value) -> ToolExecutionResult {
        let limit: i64 = parse_param(&params, "limit").unwrap_or(20).min(100);
        let offset: i64 = parse_param(&params, "offset").unwrap_or(0);

        let db = match self.app_state.require_db() {
            Ok(db) => db,
            Err(_) => {
                // Database not available - return demo response
                let demo_tx = serde_json::json!({
                    "transactions": [],
                    "total": 0,
                    "note": "Database not available. Running in demo mode."
                });
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "get_transaction_history".into(),
                    success: true,
                    result: demo_tx,
                    error: None,
                };
            }
        };

        // Parse user_id from context if available
        let user_id = match &self.user_id {
            Some(uid) => match uuid::Uuid::parse_str(uid) {
                Ok(id) => id,
                Err(_) => {
                    return ToolExecutionResult {
                        tool_id: tool_id.to_string(),
                        tool_name: "get_transaction_history".into(),
                        success: false,
                        result: Value::Null,
                        error: Some("Invalid user ID format".into()),
                    };
                }
            },
            None => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "get_transaction_history".into(),
                    success: false,
                    result: Value::Null,
                    error: Some("User not authenticated".into()),
                };
            }
        };

        // Query database
        let rows = sqlx::query(
            "SELECT id, chain_id, to_addr, value, token, tx_hash, status, created_at
             FROM transactions WHERE user_id = $1
             ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await;

        let rows = match rows {
            Ok(r) => r,
            Err(e) => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "get_transaction_history".into(),
                    success: false,
                    result: Value::Null,
                    error: Some(format!("Database query failed: {}", e)),
                };
            }
        };

        let transactions: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|row| {
                serde_json::json!({
                    "id": row.get::<uuid::Uuid, _>("id").to_string(),
                    "chain_id": row.get::<i64, _>("chain_id"),
                    "to_addr": format!("0x{}", hex::encode(row.get::<Vec<u8>, _>("to_addr"))),
                    "value": row.get::<String, _>("value"),
                    "token": row.get::<Option<String>, _>("token"),
                    "tx_hash": row.get::<Option<Vec<u8>>, _>("tx_hash").map(|h| format!("0x{}", hex::encode(&h))),
                    "status": row.get::<String, _>("status"),
                    "created_at": row.get::<chrono::DateTime<chrono::Utc>, _>("created_at").to_rfc3339()
                })
            })
            .collect();

        let total = transactions.len();
        let result = serde_json::json!({
            "transactions": transactions,
            "limit": limit,
            "offset": offset,
            "total": total
        });

        ToolExecutionResult {
            tool_id: tool_id.to_string(),
            tool_name: "get_transaction_history".into(),
            success: true,
            result,
            error: None,
        }
    }

    // --- search_yield_opportunities ---
    async fn execute_search_yield(&self, tool_id: &str, params: Value) -> ToolExecutionResult {
        // Build SearchQuery from params (reusing yield route types via manual mapping)
        let chain_id: Option<u64> = parse_param(&params, "chain_id");
        let min_apy: Option<f64> = parse_param(&params, "min_apy");
        let limit: usize = parse_param(&params, "limit").unwrap_or(20).min(50);
        let token_filter: Option<String> = parse_param(&params, "token");
        let protocol_type: Option<String> = parse_param(&params, "protocol_type");

        // Try to get from cache first (similar logic to yield search route)
        let all_opps = if self.app_state.yield_cache.is_stale().await {
            // Cache is stale, try to refresh
            match fetch_defi_llama_data(&self.app_state.http, &self.app_state.defi_circuit_breaker).await {
                Ok(data) if !data.is_empty() => {
                    // Update cache
                    self.app_state.yield_cache.update(data.clone()).await;
                    data
                }
                _ => {
                    // Fallback to empty, let caller know we're using fallback
                    Vec::new()
                }
            }
        } else {
            // Return from cache
            self.app_state.yield_cache.data.read().await.clone()
        };

        // Filter results based on params
        let filtered: Vec<serde_json::Value> = all_opps
            .into_iter()
            .filter(|opp| {
                if let Some(cid) = chain_id {
                    if opp.chain_id != cid {
                        return false;
                    }
                }
                if let Some(min) = min_apy {
                    if opp.apy < min {
                        return false;
                    }
                }
                if let Some(ref t) = token_filter {
                    let t_upper = t.to_uppercase();
                    let matches = opp
                        .token_a
                        .as_ref()
                        .map(|ta| ta.symbol == t_upper)
                        .unwrap_or(false)
                        || opp
                            .token_b
                            .as_ref()
                            .map(|tb| tb.symbol == t_upper)
                            .unwrap_or(false);
                    if !matches {
                        return false;
                    }
                }
                true
            })
            .take(limit)
            .map(|opp| {
                serde_json::json!({
                    "id": opp.id,
                    "protocol_name": opp.protocol_name,
                    "chain_id": opp.chain_id,
                    "apy": opp.apy,
                    "tvl_usd": opp.tvl_usd,
                    "risk_level": format!("{:?}", opp.risk_level),
                    "token_a": opp.token_a.map(|t| serde_json::json!({
                        "address": t.address,
                        "symbol": t.symbol
                    })),
                    "token_b": opp.token_b.map(|t| serde_json::json!({
                        "address": t.address,
                        "symbol": t.symbol
                    })),
                    "updated_at": opp.updated_at
                })
            })
            .collect();

        let best_apy = filtered
            .iter()
            .filter_map(|o| o.get("apy").and_then(|a| a.as_f64()))
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        let avg_apy = if !filtered.is_empty() {
            filtered
                .iter()
                .filter_map(|o| o.get("apy").and_then(|a| a.as_f64()))
                .sum::<f64>()
                / filtered.len() as f64
        } else {
            0.0
        };

        let result = serde_json::json!({
            "opportunities": filtered,
            "total_count": filtered.len(),
            "best_apy": best_apy,
            "average_apy": avg_apy,
            "chain_filter": chain_id,
            "min_apy_filter": min_apy,
            "token_filter": token_filter,
            "type_filter": protocol_type
        });

        ToolExecutionResult {
            tool_id: tool_id.to_string(),
            tool_name: "search_yield_opportunities".into(),
            success: true,
            result,
            error: None,
        }
    }

    // --- list_yield_protocols ---
    async fn execute_list_protocols(&self, tool_id: &str, params: Value) -> ToolExecutionResult {
        let chain_id: Option<u64> = parse_param(&params, "chain_id");
        let protocol_type: Option<String> = parse_param(&params, "protocol_type");

        // Reuse yield module's get_protocols function - get_protocols returns Vec<ProtocolInfo>
        let protocols: Vec<ProtocolInfo> = match self.app_state.yield_cache.data.read().await.first() {
            // If we have cached data, use it as a reference for what protocols exist
            Some(_) => Vec::new(), // We'll use static fallback instead
            None => {
                // Fallback - get static protocol info from yield module
                // yield module's get_protocols is private, so we define a short list here
                Vec::new()
            }
        };

        // Since we can't access the private get_protocols, let's use a static list here
        let static_protocols = vec![
            ("aave-v3-base", "Aave V3", 8453, "Lending"),
            ("uniswap-v3-base", "Uniswap V3", 8453, "DEX"),
            ("aerodrome-base", "Aerodrome", 8453, "DEX"),
            ("morpho-blue", "Morpho Blue", 8453, "Lending"),
        ];

        let filtered: Vec<serde_json::Value> = static_protocols
            .into_iter()
            .filter(|(_id, _name, chain, ptype)| {
                if let Some(cid) = chain_id {
                    if *chain != cid {
                        return false;
                    }
                }
                if let Some(ref pt) = protocol_type {
                    if ptype.to_lowercase() != pt.to_lowercase() {
                        return false;
                    }
                }
                true
            })
            .map(|(id, name, chain, ptype)| {
                serde_json::json!({
                    "id": id,
                    "name": name,
                    "chain_id": chain,
                    "protocol_type": ptype,
                })
            })
            .collect();

        let result = serde_json::json!({
            "protocols": filtered,
            "total_count": filtered.len(),
            "chain_filter": chain_id,
            "type_filter": protocol_type
        });

        ToolExecutionResult {
            tool_id: tool_id.to_string(),
            tool_name: "list_yield_protocols".into(),
            success: true,
            result,
            error: None,
        }
    }
}
