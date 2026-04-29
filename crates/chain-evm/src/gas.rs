use serde::{Deserialize, Serialize};

/// Gas model variants for different EVM chains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GasModel {
    Eip1559,
    ArbitrumNitro,
    OpBedrock,
    Legacy,
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
    let gas_limit = if is_erc20 {
        ERC20_TRANSFER_GAS
    } else {
        TRANSFER_GAS
    };

    match gas_model {
        GasModel::Eip1559 => {
            let max_fee = base_fee * 2 + priority_fee;
            GasEstimate {
                gas_limit,
                max_fee_per_gas: Some(max_fee),
                max_priority_fee_per_gas: Some(priority_fee),
                gas_price: None,
                l1_data_fee: None,
                estimated_cost_wei: gas_limit as u128 * max_fee,
            }
        }
        GasModel::ArbitrumNitro => {
            let max_fee = base_fee * 2 + priority_fee;
            let l1_fee = l1_data_fee.unwrap_or(0);
            GasEstimate {
                gas_limit,
                max_fee_per_gas: Some(max_fee),
                max_priority_fee_per_gas: Some(priority_fee),
                gas_price: None,
                l1_data_fee: Some(l1_fee),
                estimated_cost_wei: gas_limit as u128 * max_fee + l1_fee,
            }
        }
        GasModel::OpBedrock => {
            let max_fee = base_fee * 2 + priority_fee;
            let l1_fee = l1_data_fee.unwrap_or(0);
            GasEstimate {
                gas_limit,
                max_fee_per_gas: Some(max_fee),
                max_priority_fee_per_gas: Some(priority_fee),
                gas_price: None,
                l1_data_fee: Some(l1_fee),
                estimated_cost_wei: gas_limit as u128 * max_fee + l1_fee,
            }
        }
        GasModel::Legacy => {
            let gas_price = base_fee + priority_fee;
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
}
