use koinon_ledger::KoinAmount;
use crate::config::FeeConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FeeSplit {
    pub burn: KoinAmount,
    pub validator: KoinAmount,
    pub treasury: KoinAmount,
}

impl FeeSplit {
    /// Create a fee split from a total fee amount using default 70/20/10 split.
    pub fn from_total(total: KoinAmount) -> Self {
        Self::from_config(&FeeConfig::default(), total)
    }

    /// Create a fee split from a total fee amount using the given config percentages.
    pub fn from_config(config: &FeeConfig, total: KoinAmount) -> Self {
        let raw = total.0;
        if raw < 0 {
            return Self {
                burn: KoinAmount::ZERO,
                validator: KoinAmount::ZERO,
                treasury: KoinAmount::ZERO,
            };
        }
        let burn = KoinAmount(raw * config.burn_pct as i128 / 100);
        let validator = KoinAmount(raw * config.validator_pct as i128 / 100);
        let treasury = KoinAmount(raw - burn.0 - validator.0);
        Self {
            burn,
            validator,
            treasury,
        }
    }

    pub fn total(&self) -> KoinAmount {
        KoinAmount(self.burn.0 + self.validator.0 + self.treasury.0)
    }

    pub fn burn_pct(&self) -> f64 {
        let total = self.total().0;
        if total == 0 {
            0.0
        } else {
            self.burn.0 as f64 / total as f64 * 100.0
        }
    }

    pub fn validator_pct(&self) -> f64 {
        let total = self.total().0;
        if total == 0 {
            0.0
        } else {
            self.validator.0 as f64 / total as f64 * 100.0
        }
    }

    pub fn treasury_pct(&self) -> f64 {
        let total = self.total().0;
        if total == 0 {
            0.0
        } else {
            self.treasury.0 as f64 / total as f64 * 100.0
        }
    }

    pub fn check_conservation(&self, fee_paid: KoinAmount) -> bool {
        let distributed = self.burn.0 + self.validator.0 + self.treasury.0;
        fee_paid.0 == distributed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_split_proportions() {
        let total = KoinAmount(1000);
        let split = FeeSplit::from_total(total);
        assert_eq!(split.burn.0, 700);
        assert_eq!(split.validator.0, 200);
        assert_eq!(split.treasury.0, 100);
        assert_eq!(split.total(), total);
    }

    #[test]
    fn test_check_conservation() {
        let total = KoinAmount(1000);
        let split = FeeSplit::from_total(total);
        assert!(split.check_conservation(KoinAmount(1000)));
        assert!(!split.check_conservation(KoinAmount(999)));
    }

    #[test]
    fn test_from_config_custom() {
        let config = FeeConfig::new(50, 30, 20).unwrap();
        let split = FeeSplit::from_config(&config, KoinAmount(1000));
        assert_eq!(split.burn.0, 500);
        assert_eq!(split.validator.0, 300);
        assert_eq!(split.treasury.0, 200);
        assert!(split.check_conservation(KoinAmount(1000)));
    }

    #[test]
    fn test_negative_input_returns_zero() {
        let split = FeeSplit::from_total(KoinAmount(-100));
        assert_eq!(split.burn, KoinAmount::ZERO);
        assert_eq!(split.validator, KoinAmount::ZERO);
        assert_eq!(split.treasury, KoinAmount::ZERO);
    }

    #[test]
    fn test_zero_input() {
        let split = FeeSplit::from_total(KoinAmount(0));
        assert_eq!(split.burn, KoinAmount::ZERO);
        assert_eq!(split.validator, KoinAmount::ZERO);
        assert_eq!(split.treasury, KoinAmount::ZERO);
        assert!(split.check_conservation(KoinAmount(0)));
    }

    #[test]
    fn test_remainder_to_treasury() {
        // 101 * 70 / 100 = 70, 101 * 20 / 100 = 20, treasury = 101 - 70 - 20 = 11
        let split = FeeSplit::from_total(KoinAmount(101));
        assert_eq!(split.burn.0, 70);
        assert_eq!(split.validator.0, 20);
        assert_eq!(split.treasury.0, 11);
        assert!(split.check_conservation(KoinAmount(101)));
    }
}
