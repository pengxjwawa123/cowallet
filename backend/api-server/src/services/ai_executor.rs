//! AI Tool Execution Engine
//! Handles execution of wallet tools requested by Claude AI

use crate::routes::yield_::{fetch_defi_llama_data, ProtocolInfo};
use crate::state::AppState;
use alloy_primitives::Address;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use std::str::FromStr;

/// Gas estimation result
struct GasEstimate {
    gas_units: u64,
    gas_price_gwei: Option<String>,
    cost_eth: Option<String>,
    cost_usd: Option<String>,
}

/// Execution context for tool calls
#[derive(Clone)]
pub struct ToolContext {
    pub app_state: AppState,
    pub user_id: Option<String>,
    pub wallet_address: Option<String>,
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

/// Parse wallet address from context. Returns None if not provided or invalid.
fn parse_wallet_address(wallet_address: Option<&str>) -> Option<Address> {
    wallet_address.and_then(|addr| Address::from_str(addr).ok())
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

/// Infer chain ID from token symbol when not explicitly provided.
fn infer_chain_id_from_token(token: &str) -> Option<u64> {
    match token.to_uppercase().as_str() {
        "POL" | "MATIC" => Some(137),
        "BNB" => Some(56),
        "ETH" => Some(1),
        _ => None,
    }
}

/// Format U256 value with given decimals (simplified version)
fn format_units(value: alloy_primitives::U256, decimals: u32) -> String {
    let divisor = alloy_primitives::U256::from(10).pow(alloy_primitives::U256::from(decimals));
    let integer = value / divisor;
    let fraction = value % divisor;
    if fraction.is_zero() {
        format!("{}", integer)
    } else {
        format!("{}.{:06}", integer, fraction.to_string().chars().take(6).collect::<String>())
    }
}

fn token_balance_to_json(b: &crate::services::covalent::TokenBalance) -> serde_json::Value {
    serde_json::json!({
        "symbol": b.symbol,
        "name": b.name,
        "balance": b.balance_formatted,
        "balance_raw": b.balance,
        "usd": b.usd,
        "usd_24h": b.usd_24h,
        "quote_rate": b.quote_rate,
        "quote_rate_24h": b.quote_rate_24h,
        "native": b.native_token,
        "contract_address": b.contract_address,
        "decimals": b.decimals,
        "logo_url": b.logo_url,
        "chain_id": b.chain_id,
        "chain_name": b.chain_name,
        "last_transferred_at": b.last_transferred_at,
    })
}

impl ToolContext {
    /// Execute a tool by name with parameters
    pub async fn execute_tool(&self, tool_name: &str, tool_id: &str, params: Value) -> ToolExecutionResult {
        tracing::debug!("Executing tool: {} with params: {:?}", tool_name, params);

        let result = match tool_name {
            "get_balance" => self.execute_get_balance(tool_id, params).await,
            "get_supported_chains" => self.execute_get_supported_chains(tool_id).await,
            "get_token_info" => self.execute_get_token_info(tool_id, params).await,
            "send_transaction" => self.execute_send_transaction(tool_id, params).await,
            "get_transaction_history" => self.execute_get_transaction_history(tool_id, params).await,
            "get_wallet_address" => self.execute_get_wallet_address(tool_id).await,
            "security_audit" => self.execute_security_audit(tool_id).await,
            "swap_token" => self.execute_swap_token(tool_id, params).await,
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
        let chain_id_filter: Option<u64> = parse_param(&params, "chain_id");
        let token_filter: Option<String> = parse_param(&params, "token");
        let owner = match parse_wallet_address(self.wallet_address.as_deref()) {
            Some(a) => a,
            None => return ToolExecutionResult {
                tool_id: tool_id.to_string(),
                tool_name: "get_balance".into(),
                success: false,
                result: Value::Null,
                error: Some("钱包地址未提供".into()),
            },
        };
        let address = format!("0x{:x}", owner);

        // Use Covalent API if configured
        if let Some(api_key) = &self.app_state.covalent_api_key {
            // Multi-chain query if no chain_id specified
            if chain_id_filter.is_none() {
                let supported_chains = vec![1u64, 8453, 42161, 10, 56, 137];
                match crate::services::covalent::get_all_chain_balances(
                    &self.app_state.http,
                    api_key,
                    &address,
                    &supported_chains,
                )
                .await
                {
                    Ok(all_balances) => {
                        let mut chains_data: Vec<serde_json::Value> = Vec::new();

                        for chain in &all_balances.chains {
                            let filtered_tokens: Vec<&crate::services::covalent::TokenBalance> =
                                if let Some(ref symbol) = token_filter {
                                    let s = symbol.to_uppercase();
                                    chain.tokens.iter().filter(|b| b.symbol.to_uppercase() == s).collect()
                                } else {
                                    chain.tokens.iter().collect()
                                };

                            if !filtered_tokens.is_empty() {
                                let tokens: Vec<serde_json::Value> = filtered_tokens
                                    .iter()
                                    .map(|b| token_balance_to_json(b))
                                    .collect();

                                chains_data.push(serde_json::json!({
                                    "chain_id": chain.chain_id,
                                    "chain_name": chain.chain_name,
                                    "tokens": tokens,
                                    "total_usd": chain.total_usd,
                                }));
                            }
                        }

                        let result = serde_json::json!({
                            "address": address,
                            "multi_chain": true,
                            "chains": chains_data,
                            "total_usd": all_balances.total_usd,
                        });

                        return ToolExecutionResult {
                            tool_id: tool_id.to_string(),
                            tool_name: "get_balance".into(),
                            success: true,
                            result,
                            error: None,
                        };
                    }
                    Err(e) => {
                        tracing::warn!("Covalent multi-chain balance query failed: {}", e);
                    }
                }
            } else {
                // Single chain query
                let chain_id = chain_id_filter.unwrap();
                match crate::services::covalent::get_balances(
                    &self.app_state.http,
                    api_key,
                    &address,
                    chain_id,
                )
                .await
                {
                    Ok(balances) => {
                        let filtered: Vec<&crate::services::covalent::TokenBalance> =
                            if let Some(ref symbol) = token_filter {
                                let s = symbol.to_uppercase();
                                balances.iter().filter(|b| b.symbol.to_uppercase() == s).collect()
                            } else {
                                balances.iter().collect()
                            };

                        let total_usd: f64 = filtered
                            .iter()
                            .filter_map(|b| b.usd.parse::<f64>().ok())
                            .sum();

                        let tokens: Vec<serde_json::Value> = filtered
                            .iter()
                            .map(|b| token_balance_to_json(b))
                            .collect();

                        let result = serde_json::json!({
                            "address": address,
                            "chain_id": chain_id,
                            "tokens": tokens,
                            "total_usd": format!("{:.2}", total_usd),
                        });

                        return ToolExecutionResult {
                            tool_id: tool_id.to_string(),
                            tool_name: "get_balance".into(),
                            success: true,
                            result,
                            error: None,
                        };
                    }
                    Err(e) => {
                        tracing::warn!("Covalent balance query failed, falling back to RPC: {}", e);
                    }
                }
            }
        }

        // Fallback: direct RPC query — default to Ethereum mainnet for native balance
        let chain_id = chain_id_filter.unwrap_or(1);
        let rpc_url = self.app_state.rpc_for_chain(chain_id);
        let result = match chain_evm::tokens::query_native_balance(owner, rpc_url).await {
            Ok(balance) => {
                let formatted = format_units(balance, 18);
                serde_json::json!({
                    "address": address,
                    "chain_id": chain_id,
                    "tokens": [{
                        "symbol": "ETH",
                        "balance": formatted,
                        "usd": "—",
                        "native": true,
                    }],
                    "total_usd": "—",
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
        };

        ToolExecutionResult {
            tool_id: tool_id.to_string(),
            tool_name: "get_balance".into(),
            success: true,
            result,
            error: None,
        }
    }

    // --- get_token_info ---
    async fn execute_get_token_info(&self, tool_id: &str, params: Value) -> ToolExecutionResult {
        let token_symbol: String = parse_param(&params, "token").unwrap_or_else(|| "ETH".into());
        let chain_id_param: Option<u64> = parse_param(&params, "chain_id");
        let symbol_upper = token_symbol.to_uppercase();

        let owner = parse_wallet_address(self.wallet_address.as_deref());
        let address_str = owner.map(|a| format!("0x{:x}", a));

        // Determine which chain to query: explicit param > infer from token > search all chains
        let chain_id = chain_id_param
            .or_else(|| infer_chain_id_from_token(&token_symbol))
            .unwrap_or(0); // 0 = search all chains

        // Get balance from Covalent if available
        let mut balance_info = serde_json::json!(null);
        let mut resolved_chain_id = chain_id;
        if let (Some(api_key), Some(ref addr)) = (&self.app_state.covalent_api_key, &address_str) {
            if chain_id == 0 {
                // Multi-chain search: find which chain has this token
                let all_chains: &[u64] = &[1, 137, 8453, 42161, 10, 56];
                for &cid in all_chains {
                    if let Ok(balances) = crate::services::covalent::get_balances(
                        &self.app_state.http, api_key, addr, cid,
                    ).await {
                        if let Some(token) = balances.iter().find(|b| b.symbol.to_uppercase() == symbol_upper) {
                            resolved_chain_id = cid;
                            balance_info = serde_json::json!({
                                "balance": token.balance_formatted,
                                "balance_raw": token.balance,
                                "usd_value": token.usd,
                                "usd_24h": token.usd_24h,
                                "quote_rate": token.quote_rate,
                                "quote_rate_24h": token.quote_rate_24h,
                                "contract_address": token.contract_address,
                                "is_native": token.native_token,
                            });
                            break;
                        }
                    }
                }
            } else {
                if let Ok(balances) = crate::services::covalent::get_balances(
                    &self.app_state.http, api_key, addr, chain_id,
                ).await {
                    if let Some(token) = balances.iter().find(|b| b.symbol.to_uppercase() == symbol_upper) {
                        resolved_chain_id = chain_id;
                        balance_info = serde_json::json!({
                            "balance": token.balance_formatted,
                            "balance_raw": token.balance,
                            "usd_value": token.usd,
                            "usd_24h": token.usd_24h,
                            "quote_rate": token.quote_rate,
                            "quote_rate_24h": token.quote_rate_24h,
                            "contract_address": token.contract_address,
                            "decimals": token.decimals,
                            "is_native": token.native_token,
                            "logo_url": token.logo_url,
                            "last_transferred_at": token.last_transferred_at,
                        });
                    }
                }
            }
        }

        // Get price from PriceCache (DeFiLlama primary, CoinGecko fallback)
        let mut price_usd = self.app_state.price_cache
            .get_usd_price(&self.app_state.http, &symbol_upper)
            .await;

        // Fallback: if symbol lookup failed, try by contract address via DeFiLlama
        if price_usd.is_none() {
            let contract_addr = balance_info.get("contract_address")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            if let Some(addr) = contract_addr {
                price_usd = self.app_state.price_cache
                    .get_token_price_by_address(&self.app_state.http, resolved_chain_id, addr)
                    .await;
            }
        }

        // Build known token metadata
        let token_meta = match symbol_upper.as_str() {
            "ETH" => serde_json::json!({
                "name": "Ethereum",
                "symbol": "ETH",
                "decimals": 18,
                "type": "native",
                "description": "Native gas token of Ethereum and L2 networks",
            }),
            "USDC" => serde_json::json!({
                "name": "USD Coin",
                "symbol": "USDC",
                "decimals": 6,
                "type": "ERC-20",
                "issuer": "Circle",
                "contract_address": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
                "description": "Fully reserved stablecoin pegged to USD, issued by Circle",
            }),
            "USDT" => serde_json::json!({
                "name": "Tether USD",
                "symbol": "USDT",
                "decimals": 6,
                "type": "ERC-20",
                "issuer": "Tether",
                "description": "Most widely used stablecoin pegged to USD",
            }),
            "WETH" => serde_json::json!({
                "name": "Wrapped Ether",
                "symbol": "WETH",
                "decimals": 18,
                "type": "ERC-20",
                "description": "ERC-20 wrapped version of ETH for DeFi compatibility",
            }),
            "DAI" => serde_json::json!({
                "name": "Dai",
                "symbol": "DAI",
                "decimals": 18,
                "type": "ERC-20",
                "issuer": "MakerDAO",
                "description": "Decentralized stablecoin backed by crypto collateral",
            }),
            _ => serde_json::json!({
                "name": symbol_upper.clone(),
                "symbol": symbol_upper.clone(),
                "type": "ERC-20",
            }),
        };

        let result = serde_json::json!({
            "token": token_meta,
            "balance": balance_info,
            "price_usd": price_usd,
            "chain_id": resolved_chain_id,
            "wallet_address": address_str,
        });

        ToolExecutionResult {
            tool_id: tool_id.to_string(),
            tool_name: "get_token_info".into(),
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

        let token_str: String = parse_param(&params, "token").unwrap_or_else(|| "ETH".into());
        let contract_address: Option<String> = parse_param(&params, "contract_address");
        let decimals: u8 = parse_param::<u8>(&params, "decimals").unwrap_or_else(|| {
            match token_str.to_uppercase().as_str() {
                "USDC" | "USDT" => 6,
                _ => 18,
            }
        });
        let chain_id: u64 = match parse_param(&params, "chain_id")
            .or_else(|| infer_chain_id_from_token(&token_str))
        {
            Some(id) => id,
            None => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "send_transaction".into(),
                    success: false,
                    result: Value::Null,
                    error: Some("Cannot determine target chain. Please ask the user which chain to use for this operation. Multi-chain tokens (USDC, USDT, DAI, WETH, LINK) require an explicit chain_id.".into()),
                };
            }
        };
        let send_all: bool = parse_param(&params, "send_all").unwrap_or(false);

        // Validate contract_address format if provided
        if let Some(ref ca) = contract_address {
            if !ca.starts_with("0x") || ca.len() != 42 {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "send_transaction".into(),
                    success: false,
                    result: Value::Null,
                    error: Some("Invalid contract_address format. Expected 0x-prefixed 40-char hex address".into()),
                };
            }
        }
        let from_address = match parse_wallet_address(self.wallet_address.as_deref()) {
            Some(a) => a,
            None => return ToolExecutionResult {
                tool_id: tool_id.to_string(),
                tool_name: "send_transaction".into(),
                success: false,
                result: Value::Null,
                error: Some("钱包地址未提供".into()),
            },
        };

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

        // Parse value - support both smallest-unit (integer) and human-readable (decimal) formats
        let value_wei_str: String;
        let value_u256 = if value.contains('.') {
            // Human-readable amount - convert to smallest unit using token decimals
            let amount: f64 = match value.parse() {
                Ok(v) => v,
                Err(_) => {
                    return ToolExecutionResult {
                        tool_id: tool_id.to_string(),
                        tool_name: "send_transaction".into(),
                        success: false,
                        result: Value::Null,
                        error: Some("Invalid value format".into()),
                    };
                }
            };
            let factor = 10f64.powi(decimals as i32);
            let smallest = (amount * factor) as u128;
            value_wei_str = smallest.to_string();
            alloy_primitives::U256::from(smallest)
        } else {
            match alloy_primitives::U256::from_str_radix(&value, 10) {
                Ok(v) => {
                    value_wei_str = value.clone();
                    v
                }
                Err(_) => {
                    return ToolExecutionResult {
                        tool_id: tool_id.to_string(),
                        tool_name: "send_transaction".into(),
                        success: false,
                        result: Value::Null,
                        error: Some("Invalid value format. Expected numeric string".into()),
                    };
                }
            }
        };

        let value_formatted = format_units(value_u256, decimals as u32);

        // Estimate gas via RPC
        let gas_estimate = if let Some(ref ca) = contract_address {
            // ERC-20: estimate gas for transfer(to, amount) call on the contract
            self.estimate_gas_for_erc20_transfer(
                &format!("0x{:x}", from_address),
                ca,
                &to_address,
                &value_wei_str,
                chain_id,
            ).await
        } else {
            self.estimate_gas_for_transfer(
                &format!("0x{:x}", from_address),
                &to_address,
                &value_wei_str,
                chain_id,
            ).await
        };

        let is_native = contract_address.is_none();

        // Pre-check balance for native token transfers (amount + gas vs balance)
        let mut needs_deduction = false;
        let mut max_sendable_str: Option<String> = None;
        let mut balance_str: Option<String> = None;
        let mut gas_cost_wei: Option<u128> = None;

        if is_native && !send_all {
            if let Some(native_balance) = self.get_native_balance(
                &format!("0x{:x}", from_address), chain_id
            ).await {
                let gas_wei = gas_estimate.gas_units as u128
                    * self.get_gas_price_wei(chain_id).await.unwrap_or(0);
                let total_needed = value_wei_str.parse::<u128>().unwrap_or(0) + gas_wei;
                if total_needed > native_balance && native_balance > gas_wei {
                    needs_deduction = true;
                    let max_send = native_balance - gas_wei;
                    max_sendable_str = Some(format_units(alloy_primitives::U256::from(max_send), decimals as u32));
                    balance_str = Some(format_units(alloy_primitives::U256::from(native_balance), decimals as u32));
                    gas_cost_wei = Some(gas_wei);
                }
            }
        }

        // --- Policy Engine Evaluation ---
        let policy_result = self.evaluate_transfer_policy(
            &format!("0x{:x}", from_address),
            &to_address,
            &token_str,
            chain_id,
            value_u256,
            decimals,
        ).await;

        // If policy rejects, return early with violation info
        if !policy_result.allowed {
            let violation = policy_result.violation.unwrap_or(policy_engine::limits::PolicyViolation {
                reason: "Policy check failed".into(),
                limit: "unknown".into(),
            });
            return ToolExecutionResult {
                tool_id: tool_id.to_string(),
                tool_name: "send_transaction".into(),
                success: true,
                result: serde_json::json!({
                    "status": "policy_rejected",
                    "from": format!("0x{:x}", from_address),
                    "to": to_address,
                    "value_formatted": format!("{} {}", value_formatted, token_str),
                    "chain_id": chain_id,
                    "policy_violation": {
                        "reason": violation.reason,
                        "limit": violation.limit,
                    },
                }),
                error: None,
            };
        }

        let mut result = serde_json::json!({
            "status": "prepared",
            "from": format!("0x{:x}", from_address),
            "to": to_address,
            "value": value_wei_str,
            "value_formatted": format!("{} {}", value_formatted, token_str),
            "chain_id": chain_id,
            "token": token_str,
            "is_native": is_native,
            "decimals": decimals,
            "send_all": send_all,
            "estimated_gas": gas_estimate.gas_units,
            "warning": "This transaction requires your biometric confirmation before being signed and broadcast. Please verify all parameters carefully.",
            "next_step": "Review the details above and confirm with your biometric authentication to proceed"
        });

        // Add policy warnings if any
        if !policy_result.warnings.is_empty() {
            result["policy_warnings"] = serde_json::json!(policy_result.warnings);
        }
        if policy_result.requires_extra_confirmation {
            result["requires_extra_confirmation"] = serde_json::json!(true);
        }

        if let Some(ref ca) = contract_address {
            result["contract_address"] = serde_json::json!(ca);
        }

        // Add gas cost estimate if available
        if let Some(ref cost_eth) = gas_estimate.cost_eth {
            result["gas_estimate"] = serde_json::json!({
                "gas_units": gas_estimate.gas_units,
                "gas_price_gwei": gas_estimate.gas_price_gwei,
                "cost_eth": cost_eth,
                "cost_usd": gas_estimate.cost_usd,
            });
        }

        // If amount + gas > balance, include deduction info so frontend shows it directly
        if needs_deduction {
            if let (Some(ref max_send), Some(ref balance), Some(gas_cost)) =
                (&max_sendable_str, &balance_str, gas_cost_wei)
            {
                let gas_formatted = format_units(alloy_primitives::U256::from(gas_cost), decimals as u32);
                result["needs_deduction"] = serde_json::json!({
                    "original_amount": value_formatted,
                    "max_sendable": max_send,
                    "gas_cost": gas_formatted,
                    "balance": balance,
                });
            }
        }

        ToolExecutionResult {
            tool_id: tool_id.to_string(),
            tool_name: "send_transaction".into(),
            success: true,
            result,
            error: None,
        }
    }

    /// Query native token balance via RPC eth_getBalance
    async fn get_native_balance(&self, address: &str, chain_id: u64) -> Option<u128> {
        let rpc_url = self.app_state.rpc_for_chain(chain_id);
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [address, "latest"],
            "id": 1
        });
        let resp = self.app_state.http.post(rpc_url).json(&body).send().await.ok()?;
        let json = resp.json::<serde_json::Value>().await.ok()?;
        let hex = json.get("result")?.as_str()?;
        u128::from_str_radix(hex.strip_prefix("0x").unwrap_or(hex), 16).ok()
    }

    /// Get current gas price in wei
    async fn get_gas_price_wei(&self, chain_id: u64) -> Option<u128> {
        let rpc_url = self.app_state.rpc_for_chain(chain_id);
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_gasPrice",
            "params": [],
            "id": 1
        });
        let resp = self.app_state.http.post(rpc_url).json(&body).send().await.ok()?;
        let json = resp.json::<serde_json::Value>().await.ok()?;
        let hex = json.get("result")?.as_str()?;
        u128::from_str_radix(hex.strip_prefix("0x").unwrap_or(hex), 16).ok()
    }

    /// Evaluate the transfer against the user's policy limits.
    /// Returns a PolicyResult indicating allow/deny/warn.
    async fn evaluate_transfer_policy(
        &self,
        from: &str,
        to: &str,
        token: &str,
        chain_id: u64,
        value: alloy_primitives::U256,
        decimals: u8,
    ) -> policy_engine::PolicyResult {
        // Get token price for USD estimation
        let symbol = if token.is_empty() { "ETH" } else { token };
        let price_usd = self.app_state.price_cache
            .get_usd_price(&self.app_state.http, &symbol.to_uppercase())
            .await
            .unwrap_or(0.0);

        // Calculate value in USD
        let divisor = 10f64.powi(decimals as i32);
        let value_f64 = value.to_string().parse::<f64>().unwrap_or(0.0) / divisor;
        let value_usd = value_f64 * price_usd;

        // Load user limits from DB (fallback to defaults)
        let limits = self.load_user_limits().await;

        // Calculate daily total USD from recent transactions
        let daily_total_usd = self.compute_daily_total_usd(chain_id).await;

        // Check if recipient is new
        let is_new_recipient = self.check_new_recipient(to).await;

        let ctx = policy_engine::TxContext {
            from: from.to_string(),
            to: to.to_string(),
            value_usd,
            token: symbol.to_string(),
            chain_id,
            is_new_recipient,
            daily_total_usd,
        };

        policy_engine::limits::evaluate(&ctx, &limits)
    }

    /// Load per-user policy limits from the database.
    async fn load_user_limits(&self) -> policy_engine::UserLimits {
        let db = match self.app_state.require_db() {
            Ok(db) => db,
            Err(_) => return policy_engine::UserLimits::default(),
        };
        let user_id = match &self.user_id {
            Some(uid) => match uuid::Uuid::parse_str(uid) {
                Ok(id) => id,
                Err(_) => return policy_engine::UserLimits::default(),
            },
            None => return policy_engine::UserLimits::default(),
        };

        let row: Option<(f64, f64)> = sqlx::query_as(
            "SELECT single_limit_usd, daily_limit_usd FROM user_policies WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten();

        match row {
            Some((single, daily)) => policy_engine::UserLimits {
                single_limit_usd: single,
                daily_limit_usd: daily,
            },
            None => policy_engine::UserLimits::default(),
        }
    }

    /// Compute cumulative USD value of transfers in the last 24 hours.
    async fn compute_daily_total_usd(&self, _chain_id: u64) -> f64 {
        let db = match self.app_state.require_db() {
            Ok(db) => db,
            Err(_) => return 0.0,
        };
        let user_id = match &self.user_id {
            Some(uid) => match uuid::Uuid::parse_str(uid) {
                Ok(id) => id,
                Err(_) => return 0.0,
            },
            None => return 0.0,
        };

        // Sum all transaction values in the last 24h for this user
        // Value is stored as text (wei), so we query and convert
        let rows: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT value, token FROM transactions
             WHERE user_id = $1 AND created_at > NOW() - INTERVAL '24 hours'
             AND status != 'failed'",
        )
        .bind(user_id)
        .fetch_all(db)
        .await
        .unwrap_or_default();

        let mut total_usd = 0.0;
        for (value_str, token) in &rows {
            let symbol = token.as_deref().unwrap_or("ETH").to_uppercase();
            let decimals: u8 = match symbol.as_str() {
                "USDC" | "USDT" => 6,
                _ => 18,
            };
            let price = self.app_state.price_cache
                .get_usd_price(&self.app_state.http, &symbol)
                .await
                .unwrap_or(0.0);
            let divisor = 10f64.powi(decimals as i32);
            let value_f64 = value_str.parse::<f64>().unwrap_or(0.0) / divisor;
            total_usd += value_f64 * price;
        }

        total_usd
    }

    /// Check if we have previously sent to this address.
    async fn check_new_recipient(&self, to_address: &str) -> bool {
        let db = match self.app_state.require_db() {
            Ok(db) => db,
            Err(_) => return false,
        };
        let user_id = match &self.user_id {
            Some(uid) => match uuid::Uuid::parse_str(uid) {
                Ok(id) => id,
                Err(_) => return false,
            },
            None => return false,
        };

        // to_addr is stored as BYTEA — decode the hex address for comparison
        let addr_bytes = match hex::decode(to_address.strip_prefix("0x").unwrap_or(to_address)) {
            Ok(b) => b,
            Err(_) => return false,
        };

        let count: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM transactions WHERE user_id = $1 AND to_addr = $2 AND status != 'failed'",
        )
        .bind(user_id)
        .bind(&addr_bytes)
        .fetch_optional(db)
        .await
        .ok()
        .flatten();

        match count {
            Some((c,)) => c == 0,
            None => false,
        }
    }

    /// Estimate gas for an ERC-20 transfer(to, amount) call
    async fn estimate_gas_for_erc20_transfer(
        &self,
        from: &str,
        contract: &str,
        to: &str,
        amount_raw: &str,
        chain_id: u64,
    ) -> GasEstimate {
        let rpc_url = self.app_state.rpc_for_chain(chain_id);
        let http = &self.app_state.http;

        // Encode ERC-20 transfer(address,uint256) calldata
        // selector: 0xa9059cbb
        let to_padded = format!("{:0>64}", to.trim_start_matches("0x"));
        let amount_u256 = amount_raw.parse::<u128>().unwrap_or(0);
        let amount_padded = format!("{:064x}", amount_u256);
        let data = format!("0xa9059cbb{}{}", to_padded, amount_padded);

        let tx_obj = serde_json::json!({
            "from": from,
            "to": contract,
            "data": data,
        });

        let estimate_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_estimateGas",
            "params": [tx_obj, "latest"],
            "id": 1
        });

        let gas_price_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_gasPrice",
            "params": [],
            "id": 2
        });

        let (estimate_resp, price_resp) = tokio::join!(
            http.post(rpc_url).json(&estimate_body).send(),
            http.post(rpc_url).json(&gas_price_body).send(),
        );

        let gas_units = if let Ok(resp) = estimate_resp {
            match resp.json::<serde_json::Value>().await {
                Ok(json) => {
                    let hex = json.get("result").and_then(|r| r.as_str()).unwrap_or("0x10000");
                    u64::from_str_radix(hex.strip_prefix("0x").unwrap_or(hex), 16).unwrap_or(65000)
                }
                Err(_) => 65000,
            }
        } else {
            65000 // Default for ERC-20 transfer
        };

        let gas_price_wei = if let Ok(resp) = price_resp {
            match resp.json::<serde_json::Value>().await {
                Ok(json) => {
                    let hex = json.get("result").and_then(|r| r.as_str()).unwrap_or("0x0");
                    u128::from_str_radix(hex.strip_prefix("0x").unwrap_or(hex), 16).unwrap_or(0)
                }
                Err(_) => 0,
            }
        } else {
            0
        };

        if gas_price_wei == 0 {
            return GasEstimate {
                gas_units,
                gas_price_gwei: None,
                cost_eth: None,
                cost_usd: None,
            };
        }

        let gas_price_gwei = gas_price_wei as f64 / 1e9;
        let cost_wei = gas_units as u128 * gas_price_wei;
        let cost_eth = cost_wei as f64 / 1e18;

        let native_sym = crate::services::covalent::native_symbol(chain_id);
        let cost_usd = self
            .app_state
            .price_cache
            .get_usd_price(&self.app_state.http, native_sym)
            .await
            .map(|native_price| format!("${:.2}", cost_eth * native_price));

        GasEstimate {
            gas_units,
            gas_price_gwei: Some(format!("{:.2}", gas_price_gwei)),
            cost_eth: Some(format!("{:.6}", cost_eth)),
            cost_usd,
        }
    }

    /// Estimate gas for a simple ETH transfer via RPC
    async fn estimate_gas_for_transfer(
        &self,
        from: &str,
        to: &str,
        value_wei: &str,
        chain_id: u64,
    ) -> GasEstimate {
        let rpc_url = self.app_state.rpc_for_chain(chain_id);
        let http = &self.app_state.http;

        // Convert value to hex for RPC
        let value_hex = match value_wei.parse::<u128>() {
            Ok(v) => format!("0x{:x}", v),
            Err(_) => "0x0".to_string(),
        };

        let tx_obj = serde_json::json!({
            "from": from,
            "to": to,
            "value": value_hex,
        });

        let estimate_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_estimateGas",
            "params": [tx_obj, "latest"],
            "id": 1
        });

        let gas_price_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_gasPrice",
            "params": [],
            "id": 2
        });

        // Execute both RPC calls concurrently
        let (estimate_resp, price_resp) = tokio::join!(
            http.post(rpc_url).json(&estimate_body).send(),
            http.post(rpc_url).json(&gas_price_body).send(),
        );

        // Parse gas units
        let gas_units = if let Ok(resp) = estimate_resp {
            match resp.json::<serde_json::Value>().await {
                Ok(json) => {
                    let hex = json.get("result").and_then(|r| r.as_str()).unwrap_or("0x5208");
                    u64::from_str_radix(hex.strip_prefix("0x").unwrap_or(hex), 16).unwrap_or(21000)
                }
                Err(_) => 21000,
            }
        } else {
            21000 // Default for simple ETH transfer
        };

        // Parse gas price
        let gas_price_wei = if let Ok(resp) = price_resp {
            match resp.json::<serde_json::Value>().await {
                Ok(json) => {
                    let hex = json.get("result").and_then(|r| r.as_str()).unwrap_or("0x0");
                    u128::from_str_radix(hex.strip_prefix("0x").unwrap_or(hex), 16).unwrap_or(0)
                }
                Err(_) => 0,
            }
        } else {
            0
        };

        if gas_price_wei == 0 {
            return GasEstimate {
                gas_units,
                gas_price_gwei: None,
                cost_eth: None,
                cost_usd: None,
            };
        }

        let gas_price_gwei = gas_price_wei as f64 / 1e9;
        let cost_wei = gas_units as u128 * gas_price_wei;
        let cost_eth = cost_wei as f64 / 1e18;

        // Try to get native token price for USD conversion
        let native_sym = crate::services::covalent::native_symbol(chain_id);
        let cost_usd = self
            .app_state
            .price_cache
            .get_usd_price(&self.app_state.http, native_sym)
            .await
            .map(|native_price| format!("${:.2}", cost_eth * native_price));

        GasEstimate {
            gas_units,
            gas_price_gwei: Some(format!("{:.2}", gas_price_gwei)),
            cost_eth: Some(format!("{:.6}", cost_eth)),
            cost_usd,
        }
    }

    // --- get_transaction_history ---
    async fn execute_get_transaction_history(&self, tool_id: &str, params: Value) -> ToolExecutionResult {
        let limit: i64 = parse_param(&params, "limit").unwrap_or(20).min(100);
        let offset: i64 = parse_param(&params, "offset").unwrap_or(0);
        let chain_id_filter: Option<u64> = parse_param(&params, "chain_id");

        // Try Covalent API first if no chain_id filter and wallet address available
        if chain_id_filter.is_none() {
            if let (Some(api_key), Some(addr)) = (&self.app_state.covalent_api_key, &self.wallet_address) {
                let supported_chains = vec![1u64, 8453, 42161, 10, 56, 137];
                match crate::services::covalent::get_all_chain_transactions(
                    &self.app_state.http,
                    api_key,
                    addr,
                    &supported_chains,
                )
                .await
                {
                    Ok(txs) => {
                        let transactions: Vec<serde_json::Value> = txs
                            .into_iter()
                            .take(limit as usize)
                            .map(|tx| {
                                let token = &tx.token_symbol;
                                let decimals: u32 = if token == "USDC" || token == "USDT" { 6 } else { 18 };
                                let formatted_value = crate::services::covalent::format_value(&tx.value, decimals);
                                serde_json::json!({
                                    "chain_id": tx.chain_id,
                                    "chain_name": tx.chain_name,
                                    "tx_hash": tx.tx_hash,
                                    "from_addr": tx.from,
                                    "to_addr": tx.to,
                                    "value": formatted_value,
                                    "value_raw": tx.value,
                                    "token": tx.token_symbol,
                                    "timestamp": tx.timestamp,
                                    "status": tx.status,
                                })
                            })
                            .collect();

                        let result = serde_json::json!({
                            "transactions": transactions,
                            "multi_chain": true,
                            "limit": limit,
                            "total": transactions.len()
                        });

                        return ToolExecutionResult {
                            tool_id: tool_id.to_string(),
                            tool_name: "get_transaction_history".into(),
                            success: true,
                            result,
                            error: None,
                        };
                    }
                    Err(e) => {
                        tracing::warn!("Covalent multi-chain tx query failed, falling back to DB: {}", e);
                    }
                }
            }
        }

        // Fallback to database query
        let db = match self.app_state.require_db() {
            Ok(db) => db,
            Err(_) => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "get_transaction_history".into(),
                    success: false,
                    result: Value::Null,
                    error: Some("数据库不可用".into()),
                };
            }
        };

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

        // Query database with optional chain_id filter
        let rows = if let Some(chain_id) = chain_id_filter {
            sqlx::query(
                "SELECT id, chain_id, to_addr, value, token, tx_hash, status, created_at
                 FROM transactions WHERE user_id = $1 AND chain_id = $2
                 ORDER BY created_at DESC LIMIT $3 OFFSET $4",
            )
            .bind(user_id)
            .bind(chain_id as i64)
            .bind(limit)
            .bind(offset)
            .fetch_all(db)
            .await
        } else {
            sqlx::query(
                "SELECT id, chain_id, to_addr, value, token, tx_hash, status, created_at
                 FROM transactions WHERE user_id = $1
                 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
            )
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(db)
            .await
        };

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
                let chain_id = row.get::<i64, _>("chain_id") as u64;
                let chain_name = crate::services::covalent::chain_display_name(chain_id);
                serde_json::json!({
                    "id": row.get::<uuid::Uuid, _>("id").to_string(),
                    "chain_id": chain_id,
                    "chain_name": chain_name,
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

    // --- get_supported_chains ---
    async fn execute_get_supported_chains(&self, tool_id: &str) -> ToolExecutionResult {
        let chains = vec![
            serde_json::json!({
                "chain_id": 1,
                "name": "Ethereum",
                "symbol": "ETH",
                "type": "mainnet"
            }),
            serde_json::json!({
                "chain_id": 8453,
                "name": "Base",
                "symbol": "ETH",
                "type": "mainnet"
            }),
            serde_json::json!({
                "chain_id": 42161,
                "name": "Arbitrum One",
                "symbol": "ETH",
                "type": "mainnet"
            }),
            serde_json::json!({
                "chain_id": 10,
                "name": "Optimism",
                "symbol": "ETH",
                "type": "mainnet"
            }),
            serde_json::json!({
                "chain_id": 56,
                "name": "BNB Chain",
                "symbol": "BNB",
                "type": "mainnet"
            }),
            serde_json::json!({
                "chain_id": 137,
                "name": "Polygon",
                "symbol": "POL",
                "type": "mainnet"
            }),
        ];

        let result = serde_json::json!({
            "chains": chains,
            "total_count": chains.len(),
        });

        ToolExecutionResult {
            tool_id: tool_id.to_string(),
            tool_name: "get_supported_chains".into(),
            success: true,
            result,
            error: None,
        }
    }

    // --- get_wallet_address ---
    async fn execute_get_wallet_address(&self, tool_id: &str) -> ToolExecutionResult {
        let address = match parse_wallet_address(self.wallet_address.as_deref()) {
            Some(a) => a,
            None => return ToolExecutionResult {
                tool_id: tool_id.to_string(),
                tool_name: "get_wallet_address".into(),
                success: false,
                result: Value::Null,
                error: Some("钱包地址未提供".into()),
            },
        };
        let result = serde_json::json!({
            "address": format!("0x{:x}", address),
            "supported_chains": [
                {"chain_id": 1, "name": "Ethereum"},
                {"chain_id": 8453, "name": "Base"},
                {"chain_id": 42161, "name": "Arbitrum One"},
                {"chain_id": 10, "name": "Optimism"},
                {"chain_id": 56, "name": "BNB Chain"},
                {"chain_id": 137, "name": "Polygon"},
            ],
            "note": "同一地址适用于所有 EVM 链",
        });

        ToolExecutionResult {
            tool_id: tool_id.to_string(),
            tool_name: "get_wallet_address".into(),
            success: true,
            result,
            error: None,
        }
    }

    // --- security_audit ---
    async fn execute_security_audit(&self, tool_id: &str) -> ToolExecutionResult {
        let address = match parse_wallet_address(self.wallet_address.as_deref()) {
            Some(a) => a,
            None => return ToolExecutionResult {
                tool_id: tool_id.to_string(),
                tool_name: "security_audit".into(),
                success: false,
                result: Value::Null,
                error: Some("钱包地址未提供".into()),
            },
        };

        // Check token approvals (simplified - in production, query on-chain approvals)
        let mut findings: Vec<Value> = Vec::new();
        let mut score: u32 = 100;

        // Check if DB has recent suspicious transactions
        if let Ok(db) = self.app_state.require_db() {
            if let Some(uid) = &self.user_id {
                if let Ok(user_uuid) = uuid::Uuid::parse_str(uid) {
                    let suspicious = sqlx::query(
                        "SELECT COUNT(*) as cnt FROM transactions WHERE user_id = $1 AND status = 'failed' AND created_at > NOW() - INTERVAL '7 days'"
                    )
                    .bind(user_uuid)
                    .fetch_one(db)
                    .await;

                    if let Ok(row) = suspicious {
                        let failed_count: i64 = row.get("cnt");
                        if failed_count > 3 {
                            score -= 15;
                            findings.push(serde_json::json!({
                                "severity": "medium",
                                "type": "failed_transactions",
                                "message": format!("过去7天有 {} 笔失败交易，可能存在风险操作", failed_count),
                            }));
                        }
                    }
                }
            }
        }

        // Static checks
        findings.push(serde_json::json!({
            "severity": "info",
            "type": "mpc_protection",
            "message": "MPC 多方计算保护已启用 (2-of-3 门限签名)",
        }));

        findings.push(serde_json::json!({
            "severity": "info",
            "type": "biometric_auth",
            "message": "生物识别认证已启用",
        }));

        let risk_level = if score >= 90 { "low" } else if score >= 70 { "medium" } else { "high" };

        let result = serde_json::json!({
            "address": format!("0x{:x}", address),
            "score": score,
            "risk_level": risk_level,
            "findings": findings,
            "recommendations": [
                "定期检查代币授权额度",
                "对大额转账启用多签确认",
                "不要点击不明链接或授权未知合约"
            ],
        });

        ToolExecutionResult {
            tool_id: tool_id.to_string(),
            tool_name: "security_audit".into(),
            success: true,
            result,
            error: None,
        }
    }

    // --- swap_token ---
    async fn execute_swap_token(&self, tool_id: &str, params: Value) -> ToolExecutionResult {
        let from_token: String = match parse_param(&params, "from_token") {
            Some(t) => t,
            None => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "swap_token".into(),
                    success: false,
                    result: Value::Null,
                    error: Some("Missing required parameter: from_token".into()),
                };
            }
        };

        let to_token: String = match parse_param(&params, "to_token") {
            Some(t) => t,
            None => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "swap_token".into(),
                    success: false,
                    result: Value::Null,
                    error: Some("Missing required parameter: to_token".into()),
                };
            }
        };

        let amount: String = match parse_param(&params, "amount") {
            Some(a) => a,
            None => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "swap_token".into(),
                    success: false,
                    result: Value::Null,
                    error: Some("Missing required parameter: amount".into()),
                };
            }
        };

        let slippage: f64 = parse_param(&params, "slippage").unwrap_or(0.5);
        let chain_id: u64 = match parse_param(&params, "chain_id")
            .or_else(|| infer_chain_id_from_token(&from_token))
        {
            Some(id) => id,
            None => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "swap_token".into(),
                    success: false,
                    result: Value::Null,
                    error: Some("Cannot determine target chain. Please ask the user which chain to use for this swap. Multi-chain tokens (USDC, USDT, DAI, WETH, LINK) require an explicit chain_id.".into()),
                };
            }
        };

        let from_upper = from_token.to_uppercase();
        let to_upper = to_token.to_uppercase();

        // Resolve token addresses for 0x API
        let sell_addr = match crate::services::dex::token_address(&from_upper, chain_id) {
            Some(addr) => addr.to_string(),
            None => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "swap_token".into(),
                    success: false,
                    result: Value::Null,
                    error: Some(format!("不支持的代币: {} (chain {})", from_upper, chain_id)),
                };
            }
        };
        let buy_addr = match crate::services::dex::token_address(&to_upper, chain_id) {
            Some(addr) => addr.to_string(),
            None => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "swap_token".into(),
                    success: false,
                    result: Value::Null,
                    error: Some(format!("不支持的代币: {} (chain {})", to_upper, chain_id)),
                };
            }
        };

        // Convert amount to raw units
        let sell_decimals = crate::services::dex::token_decimals(&from_upper);
        let raw_amount = match crate::services::dex::amount_to_raw(&amount, sell_decimals) {
            Ok(raw) => raw,
            Err(e) => {
                return ToolExecutionResult {
                    tool_id: tool_id.to_string(),
                    tool_name: "swap_token".into(),
                    success: false,
                    result: Value::Null,
                    error: Some(format!("无效金额: {}", e)),
                };
            }
        };

        // Try to get a real quote from 0x API
        let buy_decimals = crate::services::dex::token_decimals(&to_upper);
        let (estimated_output, exchange_rate, price_impact, gas_estimate, sources) =
            match crate::services::dex::get_quote(
                &self.app_state.http,
                self.app_state.zerox_api_key.as_deref(),
                chain_id,
                &sell_addr,
                &buy_addr,
                &raw_amount,
            )
            .await
            {
                Ok(quote) => {
                    let output_formatted = crate::services::dex::raw_to_amount(&quote.buy_amount, buy_decimals);
                    (
                        output_formatted,
                        quote.price.clone(),
                        quote.price_impact.clone(),
                        quote.estimated_gas.clone(),
                        quote.sources.clone(),
                    )
                }
                Err(e) => {
                    tracing::warn!("[DEX] 0x quote failed, falling back to price estimate: {}", e);
                    // Fallback to price-based estimation
                    let from_price = self.app_state.price_cache
                        .get_usd_price(&self.app_state.http, &from_upper)
                        .await;
                    let to_price = self.app_state.price_cache
                        .get_usd_price(&self.app_state.http, &to_upper)
                        .await;

                    match (from_price, to_price) {
                        (Some(fp), Some(tp)) if tp > 0.0 => {
                            let amt: f64 = amount.parse().unwrap_or(0.0);
                            let output = amt * fp / tp;
                            let output_str = if tp >= 1.0 { format!("{:.2}", output) } else { format!("{:.6}", output) };
                            let rate = format!("{:.6}", fp / tp);
                            (output_str, rate, None, "200000".to_string(), vec!["price_estimate".to_string()])
                        }
                        _ => {
                            return ToolExecutionResult {
                                tool_id: tool_id.to_string(),
                                tool_name: "swap_token".into(),
                                success: false,
                                result: Value::Null,
                                error: Some(format!("无法获取 {}/{} 报价", from_upper, to_upper)),
                            };
                        }
                    }
                }
            };

        let result = serde_json::json!({
            "status": "pending_confirmation",
            "from_token": from_upper,
            "to_token": to_upper,
            "amount": amount,
            "estimated_output": estimated_output,
            "exchange_rate": exchange_rate,
            "price_impact": price_impact,
            "gas_estimate": gas_estimate,
            "slippage": slippage,
            "chain_id": chain_id,
            "sources": sources,
            "route": format!("{} → {}", from_upper, to_upper),
            "warning": "兑换需要您确认后执行。实际到账金额可能因市场波动略有差异。",
        });

        ToolExecutionResult {
            tool_id: tool_id.to_string(),
            tool_name: "swap_token".into(),
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
