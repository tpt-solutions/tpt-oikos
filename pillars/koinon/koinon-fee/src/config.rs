#[derive(Debug, Clone, Copy)]
pub struct FeeConfig {
    pub burn_pct: u32,
    pub validator_pct: u32,
    pub treasury_pct: u32,
}

impl Default for FeeConfig {
    fn default() -> Self {
        Self {
            burn_pct: 70,
            validator_pct: 20,
            treasury_pct: 10,
        }
    }
}

impl FeeConfig {
    pub fn new(burn_pct: u32, validator_pct: u32, treasury_pct: u32) -> Option<Self> {
        if burn_pct + validator_pct + treasury_pct != 100 {
            return None;
        }
        Some(Self {
            burn_pct,
            validator_pct,
            treasury_pct,
        })
    }

    pub fn validate(&self) -> bool {
        self.burn_pct + self.validator_pct + self.treasury_pct == 100
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_valid() {
        let config = FeeConfig::default();
        assert!(config.validate());
        assert_eq!(config.burn_pct, 70);
        assert_eq!(config.validator_pct, 20);
        assert_eq!(config.treasury_pct, 10);
    }

    #[test]
    fn test_new_valid() {
        let config = FeeConfig::new(60, 30, 10);
        assert!(config.is_some());
        let config = config.unwrap();
        assert!(config.validate());
    }

    #[test]
    fn test_new_invalid() {
        assert!(FeeConfig::new(50, 50, 50).is_none());
        assert!(FeeConfig::new(0, 0, 0).is_none());
        assert!(FeeConfig::new(101, 0, 0).is_none());
    }

    #[test]
    fn test_validate_custom() {
        let config = FeeConfig {
            burn_pct: 80,
            validator_pct: 15,
            treasury_pct: 5,
        };
        assert!(config.validate());

        let bad = FeeConfig {
            burn_pct: 33,
            validator_pct: 33,
            treasury_pct: 33,
        };
        assert!(!bad.validate());
    }
}
