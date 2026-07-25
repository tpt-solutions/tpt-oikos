use crate::{OikosAmount, OIKOS_MAX_SUPPLY};

#[derive(Debug, Clone)]
pub struct TotalValueConservation {
    pub minted: OikosAmount,
    pub burned: OikosAmount,
    pub total_oikos: OikosAmount,
    pub total_staked: OikosAmount,
    pub total_circulating_oikos: OikosAmount,
    pub total_treasury_oikos: OikosAmount,
    pub total_koin: u128,
    pub total_circulating_koin: u128,
    pub total_treasury_koin: u128,
}

impl TotalValueConservation {
    pub fn new() -> Self {
        Self {
            minted: OikosAmount::ZERO,
            burned: OikosAmount::ZERO,
            total_oikos: OikosAmount::ZERO,
            total_staked: OikosAmount::ZERO,
            total_circulating_oikos: OikosAmount::ZERO,
            total_treasury_oikos: OikosAmount::ZERO,
            total_koin: 0,
            total_circulating_koin: 0,
            total_treasury_koin: 0,
        }
    }

    pub fn net_supply(&self) -> Option<OikosAmount> {
        Some(OikosAmount(self.minted.0.checked_sub(self.burned.0)?))
    }

    pub fn check_invariant(&self) -> bool {
        self.minted >= self.burned
    }

    pub fn check_full_invariant(&self) -> bool {
        let oikos_sum = match self.total_staked.0
            .checked_add(self.total_circulating_oikos.0)
            .and_then(|v| v.checked_add(self.total_treasury_oikos.0))
        {
            Some(v) => v,
            None => return false,
        };
        let oikos_equation = self.total_oikos.0 == oikos_sum;
        let koin_sum = match self.total_circulating_koin
            .checked_add(self.total_treasury_koin)
        {
            Some(v) => v,
            None => return false,
        };
        let koin_equation = self.total_koin == koin_sum;
        let supply_ok = self.minted.0.saturating_sub(self.burned.0) <= OIKOS_MAX_SUPPLY;
        oikos_equation && koin_equation && supply_ok
    }

    pub fn check_full_invariant_ok(&self) -> Result<(), String> {
        let oikos_sum = self.total_staked.0
            .checked_add(self.total_circulating_oikos.0)
            .and_then(|v| v.checked_add(self.total_treasury_oikos.0))
            .ok_or_else(|| {
                format!(
                    "oikos decomposition overflow: staked={}, circulating={}, treasury={}",
                    self.total_staked.0, self.total_circulating_oikos.0, self.total_treasury_oikos.0,
                )
            })?;
        if self.total_oikos.0 != oikos_sum {
            return Err(format!(
                "oikos decomposition mismatch: total={}, sum={}",
                self.total_oikos.0, oikos_sum,
            ));
        }
        let koin_sum = self.total_circulating_koin
            .checked_add(self.total_treasury_koin)
            .ok_or_else(|| {
                format!(
                    "koin decomposition overflow: circulating={}, treasury={}",
                    self.total_circulating_koin, self.total_treasury_koin,
                )
            })?;
        if self.total_koin != koin_sum {
            return Err(format!(
                "koin decomposition mismatch: total={}, sum={}",
                self.total_koin, koin_sum,
            ));
        }
        let circulating_supply = self.minted.0.checked_sub(self.burned.0)
            .ok_or_else(|| {
                format!(
                    "burned exceeds minted: minted={}, burned={}",
                    self.minted.0, self.burned.0,
                )
            })?;
        if circulating_supply > OIKOS_MAX_SUPPLY {
            return Err(format!(
                "circulating supply exceeds max: supply={}, max={}",
                circulating_supply, OIKOS_MAX_SUPPLY,
            ));
        }
        Ok(())
    }

    pub fn record_mint(&mut self, amount: OikosAmount) -> bool {
        let new_minted = match self.minted.checked_add(amount) {
            Some(v) => v,
            None => return false,
        };
        if new_minted.0 > OIKOS_MAX_SUPPLY {
            return false;
        }
        self.minted = new_minted;
        match self.total_oikos.checked_add(amount) {
            Some(v) => self.total_oikos = v,
            None => return false,
        }
        true
    }

    pub fn record_burn(&mut self, amount: OikosAmount) -> bool {
        if let Some(new) = self.burned.checked_add(amount) {
            self.burned = new;
            self.total_circulating_oikos = OikosAmount(
                self.total_circulating_oikos.0.saturating_sub(amount.0),
            );
            self.check_invariant()
        } else {
            false
        }
    }

    pub fn record_stake(&mut self, amount: OikosAmount) -> bool {
        if let (Some(new_staked), Some(new_circ)) = (
            self.total_staked.checked_add(amount),
            self.total_circulating_oikos.checked_sub(amount),
        ) {
            self.total_staked = new_staked;
            self.total_circulating_oikos = new_circ;
            true
        } else {
            false
        }
    }

    pub fn record_unstake(&mut self, amount: OikosAmount) -> bool {
        if let (Some(new_staked), Some(new_circ)) = (
            self.total_staked.checked_sub(amount),
            self.total_circulating_oikos.checked_add(amount),
        ) {
            self.total_staked = new_staked;
            self.total_circulating_oikos = new_circ;
            true
        } else {
            false
        }
    }

    pub fn record_treasury_oikos(&mut self, amount: OikosAmount) -> bool {
        if let Some(new) = self.total_treasury_oikos.checked_add(amount) {
            self.total_treasury_oikos = new;
            true
        } else {
            false
        }
    }

    pub fn record_treasury_koin(&mut self, amount: u128) -> bool {
        if let Some(new) = self.total_treasury_koin.checked_add(amount) {
            self.total_treasury_koin = new;
            true
        } else {
            false
        }
    }

    pub fn record_circulating_koin(&mut self, amount: u128) -> bool {
        if let Some(new) = self.total_circulating_koin.checked_add(amount) {
            self.total_circulating_koin = new;
            true
        } else {
            false
        }
    }

    pub fn record_koin_mint(&mut self, amount: u128) -> bool {
        let new_total = match self.total_koin.checked_add(amount) {
            Some(v) => v,
            None => return false,
        };
        let new_circ = match self.total_circulating_koin.checked_add(amount) {
            Some(v) => v,
            None => return false,
        };
        self.total_koin = new_total;
        self.total_circulating_koin = new_circ;
        true
    }

    pub fn record_koin_burn(&mut self, amount: u128) -> bool {
        let new_total = match self.total_koin.checked_sub(amount) {
            Some(v) => v,
            None => return false,
        };
        let new_circ = match self.total_circulating_koin.checked_sub(amount) {
            Some(v) => v,
            None => return false,
        };
        self.total_koin = new_total;
        self.total_circulating_koin = new_circ;
        true
    }
}

impl Default for TotalValueConservation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invariant_holds_initially() {
        let tc = TotalValueConservation::new();
        assert!(tc.check_invariant());
        assert!(tc.check_full_invariant());
    }

    #[test]
    fn test_mint_within_supply() {
        let mut tc = TotalValueConservation::new();
        let max = OikosAmount(OIKOS_MAX_SUPPLY);
        assert!(tc.record_mint(max));
        assert!(!tc.record_mint(OikosAmount(1)));
    }

    #[test]
    fn test_full_invariant_oikos() {
        let mut tc = TotalValueConservation::new();
        tc.record_mint(OikosAmount(1000));
        tc.total_circulating_oikos = OikosAmount(700);
        tc.total_treasury_oikos = OikosAmount(100);
        tc.total_staked = OikosAmount(200);
        assert!(tc.check_full_invariant());
    }

    #[test]
    fn test_full_invariant_fails_oikos() {
        let mut tc = TotalValueConservation::new();
        tc.record_mint(OikosAmount(1000));
        tc.total_circulating_oikos = OikosAmount(600);
        tc.total_treasury_oikos = OikosAmount(100);
        tc.total_staked = OikosAmount(200);
        assert!(!tc.check_full_invariant());
    }

    #[test]
    fn test_full_invariant_koin() {
        let mut tc = TotalValueConservation::new();
        tc.total_koin = 500;
        tc.total_circulating_koin = 400;
        tc.total_treasury_koin = 100;
        assert!(tc.check_full_invariant());
    }

    #[test]
    fn test_full_invariant_fails_koin() {
        let mut tc = TotalValueConservation::new();
        tc.total_koin = 500;
        tc.total_circulating_koin = 300;
        tc.total_treasury_koin = 100;
        assert!(!tc.check_full_invariant());
    }

    #[test]
    fn test_check_full_invariant_ok_returns_errors() {
        let mut tc = TotalValueConservation::new();
        tc.record_mint(OikosAmount(1000));
        tc.total_circulating_oikos = OikosAmount(700);
        tc.total_treasury_oikos = OikosAmount(100);
        tc.total_staked = OikosAmount(200);
        tc.total_koin = 500;
        tc.total_circulating_koin = 400;
        tc.total_treasury_koin = 100;
        assert!(tc.check_full_invariant_ok().is_ok());

        let mut bad = tc.clone();
        bad.total_treasury_oikos = OikosAmount(200);
        assert!(bad.check_full_invariant_ok().is_err());
    }

    #[test]
    fn test_koin_overflow_does_not_silently_pass() {
        let mut tc = TotalValueConservation::new();
        tc.total_koin = u128::MAX;
        tc.total_circulating_koin = 1;
        tc.total_treasury_koin = u128::MAX; // u128::MAX + 1 overflows
        assert!(!tc.check_full_invariant());
        assert!(tc.check_full_invariant_ok().is_err());
    }

    #[test]
    fn test_net_supply() {
        let mut tc = TotalValueConservation::new();
        tc.record_mint(OikosAmount(100));
        tc.record_burn(OikosAmount(30));
        assert_eq!(tc.net_supply(), Some(OikosAmount(70)));
    }

    #[test]
    fn test_net_supply_burn_exceeds_mint() {
        let mut tc = TotalValueConservation::new();
        tc.burned = OikosAmount(10);
        tc.minted = OikosAmount(5);
        assert_eq!(tc.net_supply(), None);
    }

    #[test]
    fn test_record_treasury_oikos() {
        let mut tc = TotalValueConservation::new();
        assert!(tc.record_treasury_oikos(OikosAmount(500)));
        assert_eq!(tc.total_treasury_oikos, OikosAmount(500));
        assert!(tc.record_treasury_oikos(OikosAmount(300)));
        assert_eq!(tc.total_treasury_oikos, OikosAmount(800));
    }

    #[test]
    fn test_record_treasury_oikos_overflow() {
        let mut tc = TotalValueConservation::new();
        tc.total_treasury_oikos = OikosAmount(u128::MAX);
        assert!(!tc.record_treasury_oikos(OikosAmount(1)));
    }

    #[test]
    fn test_record_treasury_koin() {
        let mut tc = TotalValueConservation::new();
        assert!(tc.record_treasury_koin(500));
        assert_eq!(tc.total_treasury_koin, 500);
        assert!(tc.record_treasury_koin(300));
        assert_eq!(tc.total_treasury_koin, 800);
    }

    #[test]
    fn test_record_treasury_koin_overflow() {
        let mut tc = TotalValueConservation::new();
        tc.total_treasury_koin = u128::MAX;
        assert!(!tc.record_treasury_koin(1));
    }

    #[test]
    fn test_record_circulating_koin() {
        let mut tc = TotalValueConservation::new();
        assert!(tc.record_circulating_koin(100));
        assert_eq!(tc.total_circulating_koin, 100);
    }

    #[test]
    fn test_record_circulating_koin_overflow() {
        let mut tc = TotalValueConservation::new();
        tc.total_circulating_koin = u128::MAX;
        assert!(!tc.record_circulating_koin(1));
    }

    #[test]
    fn test_record_koin_mint() {
        let mut tc = TotalValueConservation::new();
        assert!(tc.record_koin_mint(500));
        assert_eq!(tc.total_koin, 500);
        assert_eq!(tc.total_circulating_koin, 500);
        assert!(tc.record_koin_mint(200));
        assert_eq!(tc.total_koin, 700);
        assert_eq!(tc.total_circulating_koin, 700);
    }

    #[test]
    fn test_record_koin_mint_overflow() {
        let mut tc = TotalValueConservation::new();
        tc.total_koin = u128::MAX;
        assert!(!tc.record_koin_mint(1));
        // total_koin overflow should not have mutated circulating
        assert_eq!(tc.total_circulating_koin, 0);
    }

    #[test]
    fn test_record_koin_burn() {
        let mut tc = TotalValueConservation::new();
        tc.total_koin = 1000;
        tc.total_circulating_koin = 600;
        assert!(tc.record_koin_burn(200));
        assert_eq!(tc.total_koin, 800);
        assert_eq!(tc.total_circulating_koin, 400);
    }

    #[test]
    fn test_record_koin_burn_overflow() {
        let mut tc = TotalValueConservation::new();
        tc.total_koin = 10;
        tc.total_circulating_koin = 5;
        assert!(!tc.record_koin_burn(10));
        // no mutation on failure
        assert_eq!(tc.total_koin, 10);
        assert_eq!(tc.total_circulating_koin, 5);
    }
}
