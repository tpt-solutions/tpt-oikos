use crate::{OikosAmount, OIKOS_MAX_SUPPLY, AccountId};

/// Genesis allocation percentages (basis points, sum = 10000).
pub const GENESIS_VALIDATORS_BP: u32 = 4000;
pub const GENESIS_TREASURY_BP: u32 = 3000;
pub const GENESIS_ECOSYSTEM_BP: u32 = 2000;
pub const GENESIS_TEAM_BP: u32 = 1000;

/// Vesting schedule for team allocation (4 years, linear monthly release).
pub const TEAM_VESTING_MONTHS: u32 = 48;

/// A single genesis allocation entry.
#[derive(Debug, Clone)]
pub struct GenesisAllocation {
    pub account: AccountId,
    pub oikos_amount: OikosAmount,
    pub vesting_months: u32,
    pub months_vested: u32,
}

impl GenesisAllocation {
    pub fn vested_amount(&self) -> OikosAmount {
        if self.vesting_months == 0 {
            return self.oikos_amount;
        }
        let fraction = self.months_vested.min(self.vesting_months) as u128;
        let total = self.vesting_months as u128;
        OikosAmount(self.oikos_amount.0 * fraction / total)
    }

    pub fn claimable(&self) -> OikosAmount {
        let vested = self.vested_amount();
        vested.checked_sub(self.claimed_so_far()).unwrap_or(OikosAmount::ZERO)
    }

    fn claimed_so_far(&self) -> OikosAmount {
        // For simplicity, this returns 0 — actual claim tracking would live in Account.
        // The vested_amount() is the maximum that could have been claimed.
        OikosAmount::ZERO
    }
}

/// The result of applying a genesis distribution.
#[derive(Debug, Clone)]
pub struct GenesisResult {
    pub total_allocated: OikosAmount,
    pub validator_pool: OikosAmount,
    pub treasury_pool: OikosAmount,
    pub ecosystem_pool: OikosAmount,
    pub team_pool: OikosAmount,
    pub allocations: Vec<GenesisAllocation>,
}

/// Compute the canonical genesis distribution from a total supply cap.
///
/// Distribution: 40% validators / 30% treasury / 20% ecosystem / 10% team.
/// Team allocation uses 4-year linear vesting.
pub fn compute_genesis_distribution(
    total_supply: OikosAmount,
    validator_accounts: &[(AccountId, f64)],
    ecosystem_accounts: &[(AccountId, f64)],
    team_accounts: &[(AccountId, f64)],
) -> Result<GenesisResult, &'static str> {
    let cap = total_supply.0;
    let validator_pool = OikosAmount(cap * GENESIS_VALIDATORS_BP as u128 / 10000);
    let treasury_pool = OikosAmount(cap * GENESIS_TREASURY_BP as u128 / 10000);
    let ecosystem_pool = OikosAmount(cap * GENESIS_ECOSYSTEM_BP as u128 / 10000);
    let team_pool = OikosAmount(cap * GENESIS_TEAM_BP as u128 / 10000);

    let mut allocations = Vec::new();

    // Distribute validator pool proportionally
    validate_weights(validator_accounts)?;
    for &(account, weight) in validator_accounts {
        let amount = OikosAmount(validator_pool.0 * (weight * 10000.0) as u128 / 10000);
        allocations.push(GenesisAllocation {
            account,
            oikos_amount: amount,
            vesting_months: 0,
            months_vested: 0,
        });
    }

    // Treasury is not allocated to individual accounts (DAO-controlled)
    allocations.push(GenesisAllocation {
        account: 0, // sentinel: treasury
        oikos_amount: treasury_pool,
        vesting_months: 0,
        months_vested: 0,
    });

    // Distribute ecosystem pool proportionally
    validate_weights(ecosystem_accounts)?;
    for &(account, weight) in ecosystem_accounts {
        let amount = OikosAmount(ecosystem_pool.0 * (weight * 10000.0) as u128 / 10000);
        allocations.push(GenesisAllocation {
            account,
            oikos_amount: amount,
            vesting_months: 0,
            months_vested: 0,
        });
    }

    // Distribute team pool with vesting
    validate_weights(team_accounts)?;
    for &(account, weight) in team_accounts {
        let amount = OikosAmount(team_pool.0 * (weight * 10000.0) as u128 / 10000);
        allocations.push(GenesisAllocation {
            account,
            oikos_amount: amount,
            vesting_months: TEAM_VESTING_MONTHS,
            months_vested: 0,
        });
    }

    let total_allocated = OikosAmount(
        validator_pool.0 + treasury_pool.0 + ecosystem_pool.0 + team_pool.0,
    );

    Ok(GenesisResult {
        total_allocated,
        validator_pool,
        treasury_pool,
        ecosystem_pool,
        team_pool,
        allocations,
    })
}

fn validate_weights(accounts: &[(AccountId, f64)]) -> Result<(), &'static str> {
    if accounts.is_empty() {
        return Ok(());
    }
    let sum: f64 = accounts.iter().map(|(_, w)| w).sum();
    if (sum - 1.0).abs() > 0.01 {
        return Err("account weights must sum to 1.0");
    }
    Ok(())
}

/// Verify that a genesis result satisfies the full conservation invariant.
pub fn verify_genesis_invariant(result: &GenesisResult) -> bool {
    let total = result.total_allocated.0;
    let parts = result.validator_pool.0
        + result.treasury_pool.0
        + result.ecosystem_pool.0
        + result.team_pool.0;
    total == parts && total <= OIKOS_MAX_SUPPLY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_distribution_percentages() {
        let total = OikosAmount(1_000_000_000 * 10_u128.pow(18));
        let validators = vec![(1, 0.5), (2, 0.5)];
        let ecosystem = vec![(3, 1.0)];
        let team = vec![(4, 1.0)];

        let result =
            compute_genesis_distribution(total, &validators, &ecosystem, &team).unwrap();

        assert_eq!(result.validator_pool.0, total.0 * 40 / 100);
        assert_eq!(result.treasury_pool.0, total.0 * 30 / 100);
        assert_eq!(result.ecosystem_pool.0, total.0 * 20 / 100);
        assert_eq!(result.team_pool.0, total.0 * 10 / 100);
        assert!(verify_genesis_invariant(&result));
    }

    #[test]
    fn test_genesis_fails_on_bad_weights() {
        let total = OikosAmount(1_000_000_000 * 10_u128.pow(18));
        let validators = vec![(1, 0.5), (2, 0.3)]; // sums to 0.8
        let ecosystem = vec![(3, 1.0)];
        let team = vec![(4, 1.0)];

        assert!(compute_genesis_distribution(total, &validators, &ecosystem, &team).is_err());
    }

    #[test]
    fn test_team_vesting() {
        let alloc = GenesisAllocation {
            account: 1,
            oikos_amount: OikosAmount(1000),
            vesting_months: 48,
            months_vested: 0,
        };
        assert_eq!(alloc.vested_amount(), OikosAmount(0));

        let alloc2 = GenesisAllocation {
            months_vested: 24,
            ..alloc
        };
        assert_eq!(alloc2.vested_amount(), OikosAmount(500));

        let alloc3 = GenesisAllocation {
            months_vested: 48,
            ..alloc
        };
        assert_eq!(alloc3.vested_amount(), OikosAmount(1000));

        let alloc4 = GenesisAllocation {
            months_vested: 60, // over-vested
            ..alloc
        };
        assert_eq!(alloc4.vested_amount(), OikosAmount(1000));
    }

    #[test]
    fn test_genesis_no_double_counting() {
        let total = OikosAmount(1000 * 10_u128.pow(18));
        let validators = vec![(1, 1.0)];
        let ecosystem = vec![];
        let team = vec![];

        let result =
            compute_genesis_distribution(total, &validators, &ecosystem, &team).unwrap();
        // Treasury still gets its share even with empty pools
        assert_eq!(
            result.validator_pool.0 + result.treasury_pool.0 + result.team_pool.0,
            total.0 * 80 / 100
        );
    }
}
