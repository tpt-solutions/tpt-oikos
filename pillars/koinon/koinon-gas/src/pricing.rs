use crate::config::GasConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GasCost(pub u64);

impl GasCost {
    pub fn zero() -> Self {
        Self(0)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for GasCost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} gas", self.0)
    }
}

/// Calculate deterministic gas cost.
///
/// Formula: `base + steps * 10 + storage_bytes * 100`
pub fn calculate_gas(steps: u64, storage_bytes: u64) -> GasCost {
    let config = GasConfig::default();
    calculate_gas_with_config(&config, steps, storage_bytes)
}

/// Calculate gas cost with custom configuration.
pub fn calculate_gas_with_config(config: &GasConfig, steps: u64, storage_bytes: u64) -> GasCost {
    let cost = config.base_cost
        .saturating_add(steps.saturating_mul(config.per_step))
        .saturating_add(storage_bytes.saturating_mul(config.per_storage_byte));
    GasCost(cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_gas_default() {
        let cost = calculate_gas(5, 100);
        assert_eq!(cost.0, 1000 + 5 * 10 + 100 * 100);
    }

    #[test]
    fn test_calculate_gas_zero() {
        let cost = calculate_gas(0, 0);
        assert_eq!(cost.0, 1000);
    }
}
