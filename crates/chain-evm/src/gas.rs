use crate::chains::GasModel;
use serde::{Deserialize, Serialize};

/// Gas strategy for transaction speed/cost trade-off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GasStrategy {
    /// Lower fee, slower confirmation (1.0x multiplier)
    Slow,
    /// Standard fee, normal confirmation (1.2x multiplier)
    Normal,
    /// Higher fee, faster confirmation (1.5x multiplier)
    Fast,
}

impl GasStrategy {
    /// Get the fee multiplier for this strategy.
    pub fn multiplier(&self) -> f64 {
        match self {
            GasStrategy::Slow => 1.0,
            GasStrategy::Normal => 1.2,
            GasStrategy::Fast => 1.5,
        }
    }

    /// Apply the strategy multiplier to a base fee.
    pub fn apply_to_fee(&self, base_fee: u128) -> u128 {
        let multiplier = self.multiplier();
        ((base_fee as f64) * multiplier) as u128
    }
}

impl Default for GasStrategy {
    fn default() -> Self {
        GasStrategy::Normal
    }
}

/// Gas estimation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasEstimate {
    pub gas_limit: u64,
    pub max_fee_per_gas: Option<u128>,
    pub max_priority_fee_per_gas: Option<u128>,
    pub gas_price: Option<u128>,
    pub l1_data_fee: Option<u128>,
    pub estimated_cost_wei: u128,
}

/// Base gas costs for common operations.
const TRANSFER_GAS: u64 = 21_000;
const ERC20_TRANSFER_GAS: u64 = 65_000;

/// Estimate gas for a transaction based on the chain's gas model.
pub fn estimate_gas(
    gas_model: GasModel,
    is_erc20: bool,
    base_fee: u128,
    priority_fee: u128,
    l1_data_fee: Option<u128>,
) -> GasEstimate {
    estimate_gas_with_strategy(
        gas_model,
        is_erc20,
        base_fee,
        priority_fee,
        l1_data_fee,
        GasStrategy::Normal,
    )
}

/// Estimate gas with a specific strategy (Slow/Normal/Fast).
pub fn estimate_gas_with_strategy(
    gas_model: GasModel,
    is_erc20: bool,
    base_fee: u128,
    priority_fee: u128,
    l1_data_fee: Option<u128>,
    strategy: GasStrategy,
) -> GasEstimate {
    let gas_limit = if is_erc20 {
        ERC20_TRANSFER_GAS
    } else {
        TRANSFER_GAS
    };

    match gas_model {
        GasModel::Eip1559 => {
            let adjusted_base = strategy.apply_to_fee(base_fee);
            let adjusted_priority = strategy.apply_to_fee(priority_fee);
            let max_fee = adjusted_base * 2 + adjusted_priority;
            GasEstimate {
                gas_limit,
                max_fee_per_gas: Some(max_fee),
                max_priority_fee_per_gas: Some(adjusted_priority),
                gas_price: None,
                l1_data_fee: None,
                estimated_cost_wei: gas_limit as u128 * max_fee,
            }
        }
        GasModel::ArbitrumNitro => {
            let adjusted_base = strategy.apply_to_fee(base_fee);
            let adjusted_priority = strategy.apply_to_fee(priority_fee);
            let max_fee = adjusted_base * 2 + adjusted_priority;
            let l1_fee = l1_data_fee.unwrap_or(0);
            GasEstimate {
                gas_limit,
                max_fee_per_gas: Some(max_fee),
                max_priority_fee_per_gas: Some(adjusted_priority),
                gas_price: None,
                l1_data_fee: Some(l1_fee),
                estimated_cost_wei: gas_limit as u128 * max_fee + l1_fee,
            }
        }
        GasModel::OpBedrock => {
            let adjusted_base = strategy.apply_to_fee(base_fee);
            let adjusted_priority = strategy.apply_to_fee(priority_fee);
            let max_fee = adjusted_base * 2 + adjusted_priority;
            let l1_fee = l1_data_fee.unwrap_or(0);
            GasEstimate {
                gas_limit,
                max_fee_per_gas: Some(max_fee),
                max_priority_fee_per_gas: Some(adjusted_priority),
                gas_price: None,
                l1_data_fee: Some(l1_fee),
                estimated_cost_wei: gas_limit as u128 * max_fee + l1_fee,
            }
        }
        GasModel::Legacy => {
            let adjusted_base = strategy.apply_to_fee(base_fee);
            let adjusted_priority = strategy.apply_to_fee(priority_fee);
            let gas_price = adjusted_base + adjusted_priority;
            GasEstimate {
                gas_limit,
                max_fee_per_gas: None,
                max_priority_fee_per_gas: None,
                gas_price: Some(gas_price),
                l1_data_fee: None,
                estimated_cost_wei: gas_limit as u128 * gas_price,
            }
        }
    }
}

/// Estimate gas for a specific chain with strategy.
pub fn estimate_gas_for_chain(
    chain_id: u64,
    is_erc20: bool,
    base_fee: u128,
    priority_fee: u128,
    l1_data_fee: Option<u128>,
    strategy: GasStrategy,
) -> Result<GasEstimate, String> {
    use crate::chains::ChainConfig;

    let chain = ChainConfig::by_chain_id(chain_id)
        .ok_or_else(|| format!("unsupported chain_id: {}", chain_id))?;

    Ok(estimate_gas_with_strategy(
        chain.gas_model,
        is_erc20,
        base_fee,
        priority_fee,
        l1_data_fee,
        strategy,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eip1559_estimate() {
        let est = estimate_gas(
            GasModel::Eip1559,
            false,
            30_000_000_000,
            1_500_000_000,
            None,
        );
        assert_eq!(est.gas_limit, 21_000);
        assert!(est.max_fee_per_gas.unwrap() > 0);
        assert!(est.gas_price.is_none());
        assert!(est.l1_data_fee.is_none());
    }

    #[test]
    fn test_legacy_estimate() {
        let est = estimate_gas(GasModel::Legacy, false, 5_000_000_000, 0, None);
        assert_eq!(est.gas_limit, 21_000);
        assert!(est.gas_price.is_some());
        assert!(est.max_fee_per_gas.is_none());
    }

    #[test]
    fn test_arbitrum_includes_l1_fee() {
        let l1_fee = 50_000_000_000_000u128;
        let est = estimate_gas(
            GasModel::ArbitrumNitro,
            false,
            100_000_000,
            1_000_000,
            Some(l1_fee),
        );
        assert_eq!(est.l1_data_fee, Some(l1_fee));
        assert!(est.estimated_cost_wei > l1_fee);
    }

    #[test]
    fn test_op_bedrock_includes_l1_fee() {
        let l1_fee = 30_000_000_000_000u128;
        let est = estimate_gas(
            GasModel::OpBedrock,
            true,
            100_000_000,
            1_000_000,
            Some(l1_fee),
        );
        assert_eq!(est.gas_limit, ERC20_TRANSFER_GAS);
        assert_eq!(est.l1_data_fee, Some(l1_fee));
    }

    #[test]
    fn test_erc20_higher_gas_limit() {
        let est = estimate_gas(GasModel::Eip1559, true, 30_000_000_000, 1_500_000_000, None);
        assert_eq!(est.gas_limit, ERC20_TRANSFER_GAS);
        assert!(est.gas_limit > TRANSFER_GAS);
    }

    #[test]
    fn test_gas_strategy_multipliers() {
        assert_eq!(GasStrategy::Slow.multiplier(), 1.0);
        assert_eq!(GasStrategy::Normal.multiplier(), 1.2);
        assert_eq!(GasStrategy::Fast.multiplier(), 1.5);
    }

    #[test]
    fn test_gas_strategy_apply_to_fee() {
        let base_fee = 1_000_000_000u128; // 1 gwei

        assert_eq!(GasStrategy::Slow.apply_to_fee(base_fee), 1_000_000_000);
        assert_eq!(GasStrategy::Normal.apply_to_fee(base_fee), 1_200_000_000);
        assert_eq!(GasStrategy::Fast.apply_to_fee(base_fee), 1_500_000_000);
    }

    #[test]
    fn test_gas_strategy_default() {
        let default_strategy = GasStrategy::default();
        assert_eq!(default_strategy, GasStrategy::Normal);
    }

    #[test]
    fn test_estimate_gas_with_slow_strategy() {
        let est = estimate_gas_with_strategy(
            GasModel::Eip1559,
            false,
            10_000_000_000,
            1_000_000_000,
            None,
            GasStrategy::Slow,
        );

        // Slow strategy uses 1.0x multiplier
        let expected_base = 10_000_000_000u128;
        let expected_priority = 1_000_000_000u128;
        let expected_max_fee = expected_base * 2 + expected_priority;

        assert_eq!(est.max_fee_per_gas, Some(expected_max_fee));
        assert_eq!(est.max_priority_fee_per_gas, Some(expected_priority));
    }

    #[test]
    fn test_estimate_gas_with_fast_strategy() {
        let est = estimate_gas_with_strategy(
            GasModel::Eip1559,
            false,
            10_000_000_000,
            1_000_000_000,
            None,
            GasStrategy::Fast,
        );

        // Fast strategy uses 1.5x multiplier
        let expected_base = 15_000_000_000u128; // 10 gwei * 1.5
        let expected_priority = 1_500_000_000u128; // 1 gwei * 1.5
        let expected_max_fee = expected_base * 2 + expected_priority;

        assert_eq!(est.max_fee_per_gas, Some(expected_max_fee));
        assert_eq!(est.max_priority_fee_per_gas, Some(expected_priority));
    }

    #[test]
    fn test_estimate_gas_bounds() {
        let est = estimate_gas(
            GasModel::Eip1559,
            false,
            30_000_000_000,
            1_500_000_000,
            None,
        );

        // Gas limit should be within reasonable bounds
        assert!(est.gas_limit >= TRANSFER_GAS);
        assert!(est.gas_limit <= 10_000_000); // Block gas limit sanity check

        // Fees should be positive
        assert!(est.max_fee_per_gas.unwrap() > 0);
        assert!(est.estimated_cost_wei > 0);
    }

    #[test]
    fn test_erc20_gas_limit_bounds() {
        let est = estimate_gas(
            GasModel::Eip1559,
            true,
            30_000_000_000,
            1_500_000_000,
            None,
        );

        // ERC-20 transfers need more gas than native transfers
        assert!(est.gas_limit > TRANSFER_GAS);
        assert_eq!(est.gas_limit, ERC20_TRANSFER_GAS);
    }

    #[test]
    fn test_gas_multiplier_increases_cost() {
        let base_fee = 10_000_000_000u128;
        let priority = 1_000_000_000u128;

        let slow = estimate_gas_with_strategy(
            GasModel::Eip1559,
            false,
            base_fee,
            priority,
            None,
            GasStrategy::Slow,
        );
        let normal = estimate_gas_with_strategy(
            GasModel::Eip1559,
            false,
            base_fee,
            priority,
            None,
            GasStrategy::Normal,
        );
        let fast = estimate_gas_with_strategy(
            GasModel::Eip1559,
            false,
            base_fee,
            priority,
            None,
            GasStrategy::Fast,
        );

        // Faster strategies should cost more
        assert!(slow.estimated_cost_wei < normal.estimated_cost_wei);
        assert!(normal.estimated_cost_wei < fast.estimated_cost_wei);
    }

    #[test]
    fn test_estimate_gas_for_chain_ethereum() {
        let est = estimate_gas_for_chain(1, false, 30_000_000_000, 1_500_000_000, None, GasStrategy::Normal);
        assert!(est.is_ok());
        let est = est.unwrap();
        assert_eq!(est.gas_limit, TRANSFER_GAS);
        assert!(est.max_fee_per_gas.is_some());
        assert!(est.l1_data_fee.is_none()); // Ethereum is not an L2
    }

    #[test]
    fn test_estimate_gas_for_chain_base() {
        let l1_fee = 50_000_000_000_000u128;
        let est = estimate_gas_for_chain(
            8453,
            false,
            100_000_000,
            1_000_000,
            Some(l1_fee),
            GasStrategy::Normal,
        );
        assert!(est.is_ok());
        let est = est.unwrap();
        assert_eq!(est.l1_data_fee, Some(l1_fee)); // Base is OpBedrock L2
        assert!(est.estimated_cost_wei > l1_fee); // Total includes L1 fee
    }

    #[test]
    fn test_estimate_gas_for_chain_unsupported() {
        let est = estimate_gas_for_chain(
            999999,
            false,
            30_000_000_000,
            1_500_000_000,
            None,
            GasStrategy::Normal,
        );
        assert!(est.is_err());
        assert!(est.unwrap_err().contains("unsupported chain_id"));
    }

    #[test]
    fn test_legacy_gas_model() {
        let est = estimate_gas(GasModel::Legacy, false, 5_000_000_000, 0, None);

        // Legacy model should use gas_price, not EIP-1559 fields
        assert!(est.gas_price.is_some());
        assert!(est.max_fee_per_gas.is_none());
        assert!(est.max_priority_fee_per_gas.is_none());
    }

    #[test]
    fn test_l1_data_fee_calculation() {
        let l1_fee = 100_000_000_000_000u128;
        let base_fee = 100_000_000u128;
        let priority = 1_000_000u128;

        let est = estimate_gas(
            GasModel::OpBedrock,
            false,
            base_fee,
            priority,
            Some(l1_fee),
        );

        // L1 fee should be added to the total cost
        assert_eq!(est.l1_data_fee, Some(l1_fee));
        let l2_cost = est.gas_limit as u128 * est.max_fee_per_gas.unwrap();
        assert_eq!(est.estimated_cost_wei, l2_cost + l1_fee);
    }
}
