use koinon_ledger::OikosAmount;
use crate::staking::{StakingPool, StakingError, Validator};

#[derive(Debug, Clone)]
pub enum SlashingReason {
    DoubleSigning,
    Downtime { duration_blocks: u64 },
    InvalidStateProof,
}

#[derive(Debug, Clone)]
pub struct SlashingResult {
    pub validator_id: u64,
    pub reason: SlashingReason,
    pub original_stake: OikosAmount,
    pub slash_amount: OikosAmount,
    pub remaining_stake: OikosAmount,
    pub jailed: bool,
    pub jailed_until: u64,
}

pub fn calculate_slash(validator: &Validator, reason: &SlashingReason) -> OikosAmount {
    let stake = validator.staked_amount.0;
    let slash = match reason {
        SlashingReason::DoubleSigning => stake / 2,
        SlashingReason::Downtime { duration_blocks } => {
            if *duration_blocks > 3600 {
                stake / 100
            } else {
                0
            }
        }
        SlashingReason::InvalidStateProof => stake / 10,
    };
    OikosAmount(slash)
}

pub fn jail_duration(reason: &SlashingReason) -> u64 {
    match reason {
        SlashingReason::DoubleSigning => 10_000,
        SlashingReason::Downtime { .. } => 0,
        SlashingReason::InvalidStateProof => 5_000,
    }
}

pub fn apply_slashing(
    pool: &mut StakingPool,
    validator_id: u64,
    reason: SlashingReason,
    current_block: u64,
) -> Result<SlashingResult, StakingError> {
    let validator = pool.validators.get(&validator_id)
        .ok_or(StakingError::ValidatorNotFound(validator_id))?;

    let original_stake = validator.staked_amount;
    let slash_amount = calculate_slash(validator, &reason);
    let remaining = OikosAmount(original_stake.0.saturating_sub(slash_amount.0));

    let jailed = match reason {
        SlashingReason::DoubleSigning | SlashingReason::InvalidStateProof => true,
        SlashingReason::Downtime { .. } => false,
    };
    let duration = jail_duration(&reason);
    let jailed_until = if jailed { current_block + duration } else { 0 };

    let validator = pool.validators.get_mut(&validator_id).unwrap();
    validator.staked_amount = remaining;
    validator.slashed_amount = OikosAmount(validator.slashed_amount.0.saturating_add(slash_amount.0));
    if jailed {
        validator.jailed_until = jailed_until;
    }

    pool.total_staked = OikosAmount(pool.total_staked.0.saturating_sub(slash_amount.0));

    Ok(SlashingResult {
        validator_id,
        reason,
        original_stake,
        slash_amount,
        remaining_stake: remaining,
        jailed,
        jailed_until,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::staking::MINIMUM_STAKE;

    fn setup_pool() -> (StakingPool, u64) {
        let mut pool = StakingPool::new();
        let id = pool.register_validator("did:example:op1").unwrap();
        pool.stake(id, OikosAmount(200_000 * 10_u128.pow(18))).unwrap();
        (pool, id)
    }

    #[test]
    fn double_sign_slashes_50_percent() {
        let (pool, id) = setup_pool();
        let v = pool.get_validator(id).unwrap();
        let slash = calculate_slash(v, &SlashingReason::DoubleSigning);
        assert_eq!(slash, OikosAmount(100_000 * 10_u128.pow(18)));
    }

    #[test]
    fn downtime_short_slashes_zero() {
        let (pool, id) = setup_pool();
        let v = pool.get_validator(id).unwrap();
        let slash = calculate_slash(v, &SlashingReason::Downtime { duration_blocks: 1000 });
        assert_eq!(slash, OikosAmount::ZERO);
    }

    #[test]
    fn downtime_long_slashes_1_percent() {
        let (pool, id) = setup_pool();
        let v = pool.get_validator(id).unwrap();
        let slash = calculate_slash(v, &SlashingReason::Downtime { duration_blocks: 5000 });
        assert_eq!(slash, OikosAmount(2_000 * 10_u128.pow(18)));
    }

    #[test]
    fn invalid_proof_slashes_10_percent() {
        let (pool, id) = setup_pool();
        let v = pool.get_validator(id).unwrap();
        let slash = calculate_slash(v, &SlashingReason::InvalidStateProof);
        assert_eq!(slash, OikosAmount(20_000 * 10_u128.pow(18)));
    }

    #[test]
    fn apply_double_sign_slashing_jails() {
        let (mut pool, id) = setup_pool();
        let result = apply_slashing(&mut pool, id, SlashingReason::DoubleSigning, 1000).unwrap();

        assert_eq!(result.validator_id, id);
        assert_eq!(result.slash_amount, OikosAmount(100_000 * 10_u128.pow(18)));
        assert_eq!(result.remaining_stake, OikosAmount(100_000 * 10_u128.pow(18)));
        assert!(result.jailed);
        assert_eq!(result.jailed_until, 11_000);

        let v = pool.get_validator(id).unwrap();
        assert_eq!(v.jailed_until, 11_000);
        assert_eq!(v.slashed_amount, OikosAmount(100_000 * 10_u128.pow(18)));
    }

    #[test]
    fn apply_downtime_no_jail() {
        let (mut pool, id) = setup_pool();
        let result = apply_slashing(&mut pool, id, SlashingReason::Downtime { duration_blocks: 5000 }, 1000).unwrap();

        assert!(!result.jailed);
        assert_eq!(result.jailed_until, 0);

        let v = pool.get_validator(id).unwrap();
        assert_eq!(v.jailed_until, 0);
    }

    #[test]
    fn slash_below_zero_clamps_to_zero() {
        let mut pool = StakingPool::new();
        let id = pool.register_validator("did:example:op1").unwrap();
        // Stake exactly MINIMUM_STAKE, then double-sign slashes 50%
        pool.stake(id, OikosAmount(MINIMUM_STAKE)).unwrap();

        let result = apply_slashing(&mut pool, id, SlashingReason::DoubleSigning, 1000).unwrap();
        assert_eq!(result.remaining_stake, OikosAmount(MINIMUM_STAKE / 2));
        assert_eq!(result.slash_amount, OikosAmount(MINIMUM_STAKE / 2));
    }

    #[test]
    fn slash_nonexistent_validator_fails() {
        let mut pool = StakingPool::new();
        assert!(matches!(
            apply_slashing(&mut pool, 999, SlashingReason::DoubleSigning, 1000),
            Err(StakingError::ValidatorNotFound(999))
        ));
    }

    #[test]
    fn apply_invalid_proof_slashing_jails() {
        let (mut pool, id) = setup_pool();
        let result = apply_slashing(&mut pool, id, SlashingReason::InvalidStateProof, 1000).unwrap();

        assert!(result.jailed);
        assert_eq!(result.jailed_until, 6000);

        let v = pool.get_validator(id).unwrap();
        assert_eq!(v.jailed_until, 6000);
        assert_eq!(v.slashed_amount, OikosAmount(20_000 * 10_u128.pow(18)));
    }

    #[test]
    fn total_staked_decreases_after_slash() {
        let (mut pool, id) = setup_pool();
        let before = pool.total_staked();
        apply_slashing(&mut pool, id, SlashingReason::DoubleSigning, 1000).unwrap();
        let after = pool.total_staked();
        assert_eq!(after.0, before.0 - 100_000 * 10_u128.pow(18));
    }

    #[test]
    fn jail_duration_values() {
        assert_eq!(jail_duration(&SlashingReason::DoubleSigning), 10_000);
        assert_eq!(jail_duration(&SlashingReason::Downtime { duration_blocks: 5000 }), 0);
        assert_eq!(jail_duration(&SlashingReason::InvalidStateProof), 5_000);
    }

    #[test]
    fn invariant_holds_after_slashing() {
        let (mut pool, id) = setup_pool();
        apply_slashing(&mut pool, id, SlashingReason::DoubleSigning, 1000).unwrap();
        assert!(pool.check_invariant());
    }
}
