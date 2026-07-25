/// Elastic Koin supply adjustment algorithm.
///
/// Adjusted every 1000 blocks (~10 minutes).
/// Target: 1 Koin = 1 unit of baseline compute.
/// If compute demand increases, Koin supply expands.
/// If compute demand decreases, Koin supply contracts (via burning).
///
/// Tolerance band: ±1% of target price.
/// Correction factor: 10% of deviation per cycle.

pub const ADJUSTMENT_INTERVAL: u64 = 1000;
pub const TARGET_COMPUTE_PRICE: f64 = 1.0;
pub const TOLERANCE_BAND: f64 = 0.01; // 1%
pub const CORRECTION_FACTOR: f64 = 0.1; // 10%

#[derive(Debug, Clone, PartialEq)]
pub enum SupplyAdjustment {
    None,
    Mint(u128),
    Burn(u128),
}

#[derive(Debug, Clone)]
pub struct ElasticSupplyState {
    pub current_supply: u128,
    pub last_adjustment_block: u64,
}

impl ElasticSupplyState {
    pub fn new(initial_supply: u128) -> Self {
        Self {
            current_supply: initial_supply,
            last_adjustment_block: 0,
        }
    }

    pub fn should_adjust(&self, current_block: u64) -> bool {
        current_block >= self.last_adjustment_block + ADJUSTMENT_INTERVAL
    }

    pub fn compute_adjustment(
        &self,
        average_gas_price: f64,
        current_block: u64,
    ) -> SupplyAdjustment {
        if !self.should_adjust(current_block) {
            return SupplyAdjustment::None;
        }

        let deviation =
            (average_gas_price - TARGET_COMPUTE_PRICE) / TARGET_COMPUTE_PRICE;

        if deviation.abs() < TOLERANCE_BAND {
            return SupplyAdjustment::None;
        }

        let adjustment_factor = deviation * CORRECTION_FACTOR;
        let supply_change = (self.current_supply as f64 * adjustment_factor).abs() as u128;

        if supply_change == 0 {
            return SupplyAdjustment::None;
        }

        if adjustment_factor > 0.0 {
            SupplyAdjustment::Mint(supply_change)
        } else {
            SupplyAdjustment::Burn(supply_change)
        }
    }

    pub fn apply_adjustment(&mut self, adjustment: &SupplyAdjustment, current_block: u64) -> bool {
        match adjustment {
            SupplyAdjustment::None => true,
            SupplyAdjustment::Mint(amount) => {
                self.current_supply = self.current_supply.saturating_add(*amount);
                self.last_adjustment_block = current_block;
                true
            }
            SupplyAdjustment::Burn(amount) => {
                if *amount > self.current_supply {
                    return false;
                }
                self.current_supply -= amount;
                self.last_adjustment_block = current_block;
                true
            }
        }
    }

    pub fn conservation_check(&self, circulating: u128, treasury: u128) -> bool {
        circulating + treasury <= self.current_supply
    }
}

/// Compute average gas price from a window of recent blocks.
pub fn calculate_average_gas_price(recent_prices: &[u64]) -> f64 {
    if recent_prices.is_empty() {
        return TARGET_COMPUTE_PRICE;
    }
    let sum: u64 = recent_prices.iter().sum();
    sum as f64 / recent_prices.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_adjustment_within_tolerance() {
        let state = ElasticSupplyState::new(1_000_000);
        let adj = state.compute_adjustment(1.005, 1000);
        assert_eq!(adj, SupplyAdjustment::None);
    }

    #[test]
    fn test_mint_when_price_high() {
        let state = ElasticSupplyState::new(1_000_000);
        let adj = state.compute_adjustment(1.5, 1000);
        match adj {
            SupplyAdjustment::Mint(amount) => assert!(amount > 0),
            _ => panic!("expected Mint"),
        }
    }

    #[test]
    fn test_burn_when_price_low() {
        let state = ElasticSupplyState::new(1_000_000);
        let adj = state.compute_adjustment(0.5, 1000);
        match adj {
            SupplyAdjustment::Burn(amount) => assert!(amount > 0),
            _ => panic!("expected Burn"),
        }
    }

    #[test]
    fn test_no_adjustment_before_interval() {
        let mut state = ElasticSupplyState::new(1_000_000);
        state.last_adjustment_block = 500;
        assert!(!state.should_adjust(1000));
    }

    #[test]
    fn test_apply_mint() {
        let mut state = ElasticSupplyState::new(1_000_000);
        let adj = SupplyAdjustment::Mint(100);
        assert!(state.apply_adjustment(&adj, 1000));
        assert_eq!(state.current_supply, 1_000_100);
        assert_eq!(state.last_adjustment_block, 1000);
    }

    #[test]
    fn test_apply_burn() {
        let mut state = ElasticSupplyState::new(1_000_000);
        let adj = SupplyAdjustment::Burn(100);
        assert!(state.apply_adjustment(&adj, 1000));
        assert_eq!(state.current_supply, 999_900);
    }

    #[test]
    fn test_burn_exceeds_supply_fails() {
        let mut state = ElasticSupplyState::new(50);
        let adj = SupplyAdjustment::Burn(100);
        assert!(!state.apply_adjustment(&adj, 1000));
        assert_eq!(state.current_supply, 50);
    }

    #[test]
    fn test_conservation_check() {
        let state = ElasticSupplyState::new(1_000_000);
        assert!(state.conservation_check(600_000, 400_000));
        assert!(!state.conservation_check(600_001, 400_000));
    }

    #[test]
    fn test_average_gas_price_empty() {
        assert_eq!(calculate_average_gas_price(&[]), TARGET_COMPUTE_PRICE);
    }

    #[test]
    fn test_average_gas_price() {
        let prices = vec![800, 1000, 1200];
        assert!((calculate_average_gas_price(&prices) - 1000.0).abs() < f64::EPSILON);
    }
}
