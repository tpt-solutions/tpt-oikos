//! Validator registration, staking, unstaking, and reward distribution.
//!
//! The central type is [`StakingPool`], which holds all validators and enforces
//! invariants (minimum stake, conservation) across every mutation.

use std::collections::HashMap;
use koinon_ledger::{OikosAmount, TotalValueConservation};

/// Minimum stake a validator must hold, in base units.
///
/// Equals `100_000 * 10^18`. Operations that would bring a validator's
/// stake below this threshold are rejected, except for a full exit (unstake to zero).
pub const MINIMUM_STAKE: u128 = 100_000 * 10_u128.pow(18);

/// A registered validator in the staking pool.
///
/// Validators operator nodes that produce blocks and earn rewards.
/// Each validator is identified by a DID (Decentralized Identifier)
/// and must maintain at least [`MINIMUM_STAKE`] OIKOS to remain active.
#[derive(Debug, Clone)]
pub struct Validator {
    /// Unique validator identifier (auto-incremented).
    pub id: u64,
    /// DID of the operator running this validator.
    pub operator_did: String,
    /// Current amount of OIKOS staked.
    pub staked_amount: OikosAmount,
    /// Accumulated rewards not yet claimed (in base units).
    pub reward_debt: u128,
    /// Whether the validator is active (not slashed/exited).
    pub active: bool,
    /// Total amount slashed historically.
    pub slashed_amount: OikosAmount,
    /// Block number when the validator was created.
    pub created_at: u64,
    /// Block number until which the validator is jailed (0 = not jailed).
    pub jailed_until: u64,
}

/// The staking pool manages all validators and enforces invariants.
///
/// Central type for the staking subsystem. Tracks:
/// - All registered validators
/// - Total staked amount across all validators
/// - Conservation of value via [`TotalValueConservation`]
///
/// # Invariants
///
/// - `total_staked` equals the sum of all validators' `staked_amount`
/// - No active validator has stake below [`MINIMUM_STAKE`] (except full exit)
/// - Jailed validators cannot receive rewards or accept new stakes
#[derive(Debug, Clone)]
pub struct StakingPool {
    /// All registered validators, keyed by ID.
    pub validators: HashMap<u64, Validator>,
    /// Total OIKOS staked across all validators.
    pub total_staked: OikosAmount,
    /// Next validator ID to assign (auto-incremented).
    pub next_validator_id: u64,
    /// Conservation tracker for the staking subsystem.
    pub conservation: TotalValueConservation,
}

/// Errors that can occur during staking operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum StakingError {
    /// The specified validator ID does not exist.
    #[error("validator not found: {0}")]
    ValidatorNotFound(u64),

    /// Insufficient stake for the requested operation.
    #[error("insufficient stake: have {have}, need {need}")]
    InsufficientStake { have: u128, need: u128 },

    /// Operation would bring stake below the minimum threshold.
    #[error("below minimum stake: current {current}, minimum {minimum}")]
    BelowMinimumStake { current: u128, minimum: u128 },

    /// Validator is not active (slashed or exited).
    #[error("validator not active: {0}")]
    ValidatorNotActive(u64),

    /// Validator is currently jailed.
    #[error("validator jailed: {0}")]
    ValidatorJailed(u64),
}

impl StakingPool {
    /// Create a new empty staking pool.
    pub fn new() -> Self {
        Self {
            validators: HashMap::new(),
            total_staked: OikosAmount::ZERO,
            next_validator_id: 1,
            conservation: TotalValueConservation::new(),
        }
    }

    /// Register a new validator with the given operator DID.
    ///
    /// Returns the assigned validator ID. The validator starts with zero stake
    /// and must call [`stake`](Self::stake) to reach [`MINIMUM_STAKE`] before
    /// becoming eligible for rewards.
    pub fn register_validator(&mut self, operator_did: &str) -> Result<u64, StakingError> {
        let id = self.next_validator_id;
        let validator = Validator {
            id,
            operator_did: operator_did.to_string(),
            staked_amount: OikosAmount::ZERO,
            reward_debt: 0,
            active: true,
            slashed_amount: OikosAmount::ZERO,
            created_at: 0,
            jailed_until: 0,
        };
        self.validators.insert(id, validator);
        self.next_validator_id += 1;
        Ok(id)
    }

    /// Delegate additional OIKOS tokens to a validator.
    ///
    /// The validator must be active and not jailed. After staking, the total must
    /// meet or exceed [`MINIMUM_STAKE`].
    ///
    /// # Errors
    ///
    /// - [`StakingError::ValidatorNotFound`] if `validator_id` does not exist.
    /// - [`StakingError::ValidatorNotActive`] if the validator has been deactivated.
    /// - [`StakingError::ValidatorJailed`] if the validator is currently jailed.
    /// - [`StakingError::BelowMinimumStake`] if the resulting stake would be below the minimum.
    /// - [`StakingError::InsufficientStake`] on arithmetic overflow.
    pub fn stake(&mut self, validator_id: u64, amount: OikosAmount) -> Result<(), StakingError> {
        let validator = self.validators.get_mut(&validator_id)
            .ok_or(StakingError::ValidatorNotFound(validator_id))?;

        if !validator.active {
            return Err(StakingError::ValidatorNotActive(validator_id));
        }
        if validator.jailed_until > 0 {
            return Err(StakingError::ValidatorJailed(validator_id));
        }

        let new_staked = validator.staked_amount.checked_add(amount)
            .ok_or(StakingError::InsufficientStake {
                have: validator.staked_amount.0,
                need: amount.0,
            })?;

        if new_staked.0 < MINIMUM_STAKE {
            return Err(StakingError::BelowMinimumStake {
                current: new_staked.0,
                minimum: MINIMUM_STAKE,
            });
        }

        validator.staked_amount = new_staked;
        self.total_staked = self.total_staked.checked_add(amount)
            .ok_or(StakingError::InsufficientStake {
                have: self.total_staked.0,
                need: amount.0,
            })?;
        self.conservation.record_stake(amount);

        Ok(())
    }

    /// Withdraw staked OIKOS tokens from a validator.
    ///
    /// A full exit (unstaking the entire balance) is allowed even when it drops the
    /// validator to zero stake. Partial unstakes must leave at least [`MINIMUM_STAKE`].
    ///
    /// # Errors
    ///
    /// - [`StakingError::ValidatorNotFound`] if `validator_id` does not exist.
    /// - [`StakingError::ValidatorNotActive`] if the validator has been deactivated.
    /// - [`StakingError::InsufficientStake`] if the validator does not hold enough stake.
    /// - [`StakingError::BelowMinimumStake`] if the remaining stake would be non-zero but below minimum.
    pub fn unstake(&mut self, validator_id: u64, amount: OikosAmount) -> Result<(), StakingError> {
        let validator = self.validators.get_mut(&validator_id)
            .ok_or(StakingError::ValidatorNotFound(validator_id))?;

        if !validator.active {
            return Err(StakingError::ValidatorNotActive(validator_id));
        }

        let new_staked = validator.staked_amount.checked_sub(amount)
            .ok_or(StakingError::InsufficientStake {
                have: validator.staked_amount.0,
                need: amount.0,
            })?;

        if new_staked.0 != 0 && new_staked.0 < MINIMUM_STAKE {
            return Err(StakingError::BelowMinimumStake {
                current: new_staked.0,
                minimum: MINIMUM_STAKE,
            });
        }

        validator.staked_amount = new_staked;
        self.total_staked = self.total_staked.checked_sub(amount)
            .ok_or(StakingError::InsufficientStake {
                have: self.total_staked.0,
                need: amount.0,
            })?;
        self.conservation.record_unstake(amount);

        Ok(())
    }

    /// Retrieve a validator by ID.
    ///
    /// Returns `None` if no validator with the given ID exists.
    pub fn get_validator(&self, id: u64) -> Option<&Validator> {
        self.validators.get(&id)
    }

    /// List all active, non-jailed validators.
    ///
    /// Returns validators whose `active` flag is `true` and `jailed_until` is `0`.
    pub fn active_validators(&self) -> Vec<&Validator> {
        self.validators.values()
            .filter(|v| v.active && v.jailed_until == 0)
            .collect()
    }

    /// Returns the total OIKOS staked across all validators.
    pub fn total_staked(&self) -> OikosAmount {
        self.total_staked
    }

    /// Check whether a validator is active and not jailed.
    ///
    /// Returns `false` if the validator does not exist, is inactive, or is jailed.
    pub fn is_active(&self, validator_id: u64) -> bool {
        self.validators.get(&validator_id)
            .map_or(false, |v| v.active && v.jailed_until == 0)
    }

    /// Distribute a reward amount proportionally among active, non-jailed validators.
    ///
    /// Each validator's share is `validator.stake / total_staked * total_reward`.
    /// The last validator in the iteration receives the remainder to avoid rounding loss.
    /// Jailed and inactive validators receive nothing.
    ///
    /// This is a no-op when `total_reward` is zero or there are no active validators.
    pub fn distribute_rewards(&mut self, total_reward: OikosAmount) {
        if total_reward.0 == 0 {
            return;
        }
        let total = self.total_staked.0;
        if total == 0 {
            return;
        }

        let validators: Vec<u64> = self.validators.keys().copied().collect();
        let active_ids: Vec<u64> = validators.iter()
            .filter(|&&vid| {
                self.validators.get(&vid)
                    .map_or(false, |v| v.active && v.jailed_until == 0)
            })
            .copied()
            .collect();

        if active_ids.is_empty() {
            return;
        }

        let mut distributed: u128 = 0;
        for (i, &vid) in active_ids.iter().enumerate() {
            let share = if i == active_ids.len() - 1 {
                total_reward.0.saturating_sub(distributed)
            } else {
                let stake = self.validators[&vid].staked_amount.0;
                pro_rata_share(stake, total_reward.0, total)
            };

            if share > 0 {
                if let Some(v) = self.validators.get_mut(&vid) {
                    v.reward_debt = v.reward_debt.saturating_add(share);
                    distributed = distributed.saturating_add(share);
                }
            }
        }
    }

    /// Claim all accumulated rewards for a validator.
    ///
    /// Returns the total rewards claimed and resets the validator's `reward_debt` to zero.
    ///
    /// # Errors
    ///
    /// - [`StakingError::ValidatorNotFound`] if `validator_id` does not exist.
    pub fn claim_rewards(&mut self, validator_id: u64) -> Result<OikosAmount, StakingError> {
        let validator = self.validators.get_mut(&validator_id)
            .ok_or(StakingError::ValidatorNotFound(validator_id))?;

        let rewards = validator.reward_debt;
        validator.reward_debt = 0;

        Ok(OikosAmount(rewards))
    }

    /// Verify the conservation invariant: sum of all validator stakes equals `total_staked`.
    ///
    /// Returns `true` if the invariant holds.
    pub fn check_invariant(&self) -> bool {
        let computed_total: u128 = self.validators.values()
            .map(|v| v.staked_amount.0)
            .sum();
        computed_total == self.total_staked.0
    }
}

impl Default for StakingPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute `a * b / c` without overflowing u128.
/// Uses scale-down when the direct product would overflow.
fn pro_rata_share(a: u128, b: u128, c: u128) -> u128 {
    if c == 0 {
        return 0;
    }
    if let Some(product) = a.checked_mul(b) {
        product / c
    } else {
        // Both a and b are base-unit amounts (multiples of 10^18).
        // Divide out the 10^18 scale factor to avoid overflow.
        let scale = 10_u128.pow(18);
        let a_s = a / scale;
        let b_s = b / scale;
        let c_s = c / scale;
        if c_s == 0 {
            return 0;
        }
        a_s * b_s / c_s * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool_with_validator(did: &str) -> (StakingPool, u64) {
        let mut pool = StakingPool::new();
        let id = pool.register_validator(did).unwrap();
        (pool, id)
    }

    fn min_stake() -> OikosAmount {
        OikosAmount(MINIMUM_STAKE)
    }

    #[test]
    fn register_validator_returns_incrementing_id() {
        let mut pool = StakingPool::new();
        let id1 = pool.register_validator("did:example:op1").unwrap();
        let id2 = pool.register_validator("did:example:op2").unwrap();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn stake_above_minimum_succeeds() {
        let (mut pool, id) = pool_with_validator("did:example:op1");
        assert!(pool.stake(id, min_stake()).is_ok());
        assert_eq!(pool.total_staked(), min_stake());
    }

    #[test]
    fn stake_below_minimum_fails() {
        let (mut pool, id) = pool_with_validator("did:example:op1");
        let below_min = OikosAmount(99_999 * 10_u128.pow(18));
        assert!(matches!(
            pool.stake(id, below_min),
            Err(StakingError::BelowMinimumStake { .. })
        ));
    }

    #[test]
    fn stake_jailed_validator_fails() {
        let (mut pool, id) = pool_with_validator("did:example:op1");
        pool.stake(id, min_stake()).unwrap();
        let validator = pool.validators.get_mut(&id).unwrap();
        validator.jailed_until = 100;
        assert!(matches!(
            pool.stake(id, min_stake()),
            Err(StakingError::ValidatorJailed(1))
        ));
    }

    #[test]
    fn unstake_to_minimum_succeeds() {
        let (mut pool, id) = pool_with_validator("did:example:op1");
        pool.stake(id, OikosAmount(200_000 * 10_u128.pow(18))).unwrap();
        let unstake_amount = OikosAmount(100_000 * 10_u128.pow(18));
        assert!(pool.unstake(id, unstake_amount).is_ok());
        assert_eq!(pool.total_staked(), min_stake());
    }

    #[test]
    fn unstake_below_minimum_fails() {
        let (mut pool, id) = pool_with_validator("did:example:op1");
        pool.stake(id, OikosAmount(200_000 * 10_u128.pow(18))).unwrap();
        let too_much = OikosAmount(100_001 * 10_u128.pow(18));
        assert!(matches!(
            pool.unstake(id, too_much),
            Err(StakingError::BelowMinimumStake { .. })
        ));
    }

    #[test]
    fn full_exit_unstake_succeeds() {
        let (mut pool, id) = pool_with_validator("did:example:op1");
        pool.stake(id, min_stake()).unwrap();
        assert!(pool.unstake(id, min_stake()).is_ok());
        assert_eq!(pool.total_staked(), OikosAmount::ZERO);
        let v = pool.get_validator(id).unwrap();
        assert_eq!(v.staked_amount, OikosAmount::ZERO);
    }

    #[test]
    fn unstake_nonexistent_validator_fails() {
        let mut pool = StakingPool::new();
        assert!(matches!(
            pool.unstake(999, min_stake()),
            Err(StakingError::ValidatorNotFound(999))
        ));
    }

    #[test]
    fn distribute_rewards_proportionally() {
        let mut pool = StakingPool::new();
        let id1 = pool.register_validator("did:example:op1").unwrap();
        let id2 = pool.register_validator("did:example:op2").unwrap();
        // id1: 300K, id2: 100K — ratio 3:1
        pool.stake(id1, OikosAmount(300_000 * 10_u128.pow(18))).unwrap();
        pool.stake(id2, OikosAmount(100_000 * 10_u128.pow(18))).unwrap();

        pool.distribute_rewards(OikosAmount(4000 * 10_u128.pow(18)));

        let v1 = pool.get_validator(id1).unwrap();
        let v2 = pool.get_validator(id2).unwrap();
        // 300K/400K * 4000 = 3000, 100K/400K * 4000 = 1000
        assert_eq!(v1.reward_debt, 3000 * 10_u128.pow(18));
        assert_eq!(v2.reward_debt, 1000 * 10_u128.pow(18));
    }

    #[test]
    fn distribute_rewards_skips_jailed() {
        let mut pool = StakingPool::new();
        let id1 = pool.register_validator("did:example:op1").unwrap();
        let id2 = pool.register_validator("did:example:op2").unwrap();
        pool.stake(id1, OikosAmount(100_000 * 10_u128.pow(18))).unwrap();
        pool.stake(id2, OikosAmount(100_000 * 10_u128.pow(18))).unwrap();

        // Jail id1
        pool.validators.get_mut(&id1).unwrap().jailed_until = 100;

        pool.distribute_rewards(OikosAmount(2000 * 10_u128.pow(18)));

        let v1 = pool.get_validator(id1).unwrap();
        let v2 = pool.get_validator(id2).unwrap();
        assert_eq!(v1.reward_debt, 0);
        // Only id2 gets rewards (100% since id1 is jailed)
        assert_eq!(v2.reward_debt, 2000 * 10_u128.pow(18));
    }

    #[test]
    fn claim_rewards_resets_debt() {
        let (mut pool, id) = pool_with_validator("did:example:op1");
        pool.stake(id, min_stake()).unwrap();
        pool.distribute_rewards(OikosAmount(500 * 10_u128.pow(18)));

        let claimed = pool.claim_rewards(id).unwrap();
        assert_eq!(claimed, OikosAmount(500 * 10_u128.pow(18)));

        let v = pool.get_validator(id).unwrap();
        assert_eq!(v.reward_debt, 0);
    }

    #[test]
    fn claim_rewards_nonexistent_validator_fails() {
        let mut pool = StakingPool::new();
        assert!(matches!(
            pool.claim_rewards(999),
            Err(StakingError::ValidatorNotFound(999))
        ));
    }

    #[test]
    fn invariant_holds_after_operations() {
        let (mut pool, id) = pool_with_validator("did:example:op1");
        assert!(pool.check_invariant());

        pool.stake(id, OikosAmount(300_000 * 10_u128.pow(18))).unwrap();
        assert!(pool.check_invariant());

        pool.unstake(id, OikosAmount(100_000 * 10_u128.pow(18))).unwrap();
        assert!(pool.check_invariant());
    }

    #[test]
    fn is_active_considers_jail() {
        let (mut pool, id) = pool_with_validator("did:example:op1");
        pool.stake(id, min_stake()).unwrap();
        assert!(pool.is_active(id));

        pool.validators.get_mut(&id).unwrap().jailed_until = 100;
        assert!(!pool.is_active(id));
    }

    #[test]
    fn active_validators_excludes_jailed() {
        let mut pool = StakingPool::new();
        let id1 = pool.register_validator("did:example:op1").unwrap();
        let id2 = pool.register_validator("did:example:op2").unwrap();
        pool.stake(id1, min_stake()).unwrap();
        pool.stake(id2, min_stake()).unwrap();

        pool.validators.get_mut(&id1).unwrap().jailed_until = 100;

        let active = pool.active_validators();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, id2);
    }

    #[test]
    fn distribute_zero_rewards_is_noop() {
        let (mut pool, id) = pool_with_validator("did:example:op1");
        pool.stake(id, min_stake()).unwrap();
        pool.distribute_rewards(OikosAmount::ZERO);
        let v = pool.get_validator(id).unwrap();
        assert_eq!(v.reward_debt, 0);
    }

    #[test]
    fn unstake_nonexistent_validator() {
        let mut pool = StakingPool::new();
        assert!(matches!(
            pool.unstake(42, min_stake()),
            Err(StakingError::ValidatorNotFound(42))
        ));
    }

    #[test]
    fn stake_nonexistent_validator() {
        let mut pool = StakingPool::new();
        assert!(matches!(
            pool.stake(42, min_stake()),
            Err(StakingError::ValidatorNotFound(42))
        ));
    }
}
