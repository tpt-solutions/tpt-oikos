//! Block reward processing and emission schedule integration.
//!
//! This crate calculates per-block OIKOS token emissions and splits transaction
//! fees into burn, validator, and treasury portions. It ties the disinflationary
//! emission schedule (defined in [`koinon_ledger`]) to actual block production,
//! ensuring that minted tokens never exceed the maximum supply of 1 billion OIKOS.
//!
//! # Emission Schedule
//!
//! Each year's emission is 80% of the previous year's, starting at 50 million
//! OIKOS in year 1 and halting after year 20. The per-block reward is derived
//! by dividing the annual emission by the configured number of blocks per year.
//!
//! # Fee Split
//!
//! Transaction fees are split using a default 70/20/10 ratio (burn / validator /
//! treasury). The split is computed by [`koinon_fee::FeeSplit`].
//!
//! # Conservation
//!
//! The processor tracks cumulative mints via [`koinon_ledger::invariant::TotalValueConservation`]
//! and rejects any operation that would violate the total-value-conservation invariant
//! or exceed [`OIKOS_MAX_SUPPLY`].

use koinon_fee::FeeSplit;
use koinon_ledger::{
    invariant::TotalValueConservation, emission_at_year, KoinAmount, OikosAmount,
    OIKOS_MAX_SUPPLY,
};

/// Configuration for block reward processing.
///
/// Controls how block numbers map to emission years and the minimum gas
/// required per block.
///
/// # Examples
///
/// ```rust
/// use koinon_rewards::BlockRewardConfig;
///
/// let config = BlockRewardConfig {
///     blocks_per_year: 31_536_000,
///     min_gas_per_block: 1000,
/// };
/// assert_eq!(config.blocks_per_year, 31_536_000);
/// ```
#[derive(Debug, Clone)]
pub struct BlockRewardConfig {
    /// Number of blocks produced per year. Used to calculate the per-block
    /// emission from the annual emission schedule.
    pub blocks_per_year: u64,
    /// Minimum gas units required per block. Reserved for future use in
    /// gas price calculations.
    pub min_gas_per_block: u64,
}

impl Default for BlockRewardConfig {
    /// Returns a configuration with 31,536,000 blocks per year (1-second block time)
    /// and a minimum of 1,000 gas per block.
    fn default() -> Self {
        Self {
            blocks_per_year: 31_536_000,
            min_gas_per_block: 1000,
        }
    }
}

/// The result of processing a single block's rewards.
///
/// Contains the base OIKOS emission for the block and the fee split across
/// burn, validator, and treasury recipients. The sum of fee components is
/// guaranteed to equal the total gas fees collected in the block.
///
/// # Examples
///
/// ```rust
/// use koinon_rewards::{BlockRewardProcessor, BlockRewardConfig};
/// use koinon_ledger::KoinAmount;
///
/// let mut proc = BlockRewardProcessor::new(BlockRewardConfig::default());
/// let reward = proc.process_block(1, KoinAmount(1000)).unwrap();
/// assert_eq!(reward.fee_burn.0 + reward.fee_validator.0 + reward.fee_treasury.0, 1000);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockReward {
    /// The block number this reward corresponds to.
    pub block_number: u64,
    /// The emission year derived from the block number. Returns `0` for
    /// blocks after year 20 (emission has halted).
    pub year: u64,
    /// The base OIKOS emission for this block, calculated as
    /// `annual_emission / blocks_per_year`.
    pub base_reward: OikosAmount,
    /// KOIN amount allocated to the burn pool (permanently removed from supply).
    pub fee_burn: KoinAmount,
    /// KOIN amount allocated to the block validator.
    pub fee_validator: KoinAmount,
    /// KOIN amount allocated to the treasury.
    pub fee_treasury: KoinAmount,
}

/// Stateful processor that calculates block rewards and enforces conservation.
///
/// Maintains the current emission year and tracks cumulative mints to ensure
/// the total supply never exceeds [`OIKOS_MAX_SUPPLY`]. Each call to
/// [`process_block`](BlockRewardProcessor::process_block) advances the year
/// if the block crosses a year boundary.
///
/// # Examples
///
/// ```rust
/// use koinon_rewards::{BlockRewardProcessor, BlockRewardConfig};
///
/// let config = BlockRewardConfig::default();
/// let mut proc = BlockRewardProcessor::new(config);
/// assert_eq!(proc.current_year(), 1);
///
/// let reward = proc.process_block(1, koinon_ledger::KoinAmount::ZERO).unwrap();
/// assert_eq!(proc.current_year(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct BlockRewardProcessor {
    config: BlockRewardConfig,
    conservation: TotalValueConservation,
    current_year: u64,
}

impl BlockRewardProcessor {
    /// Creates a new `BlockRewardProcessor` with the given configuration.
    ///
    /// The processor starts at year 1 with an empty conservation tracker.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use koinon_rewards::{BlockRewardProcessor, BlockRewardConfig};
    ///
    /// let config = BlockRewardConfig::default();
    /// let proc = BlockRewardProcessor::new(config);
    /// assert_eq!(proc.current_year(), 1);
    /// ```
    pub fn new(config: BlockRewardConfig) -> Self {
        Self {
            config,
            conservation: TotalValueConservation::new(),
            current_year: 1,
        }
    }

    /// Processes a block and returns its reward breakdown.
    ///
    /// Calculates the emission year from `block_number`, derives the per-block
    /// OIKOS emission, records the mint in the conservation tracker, and splits
    /// `block_gas_fees` across burn/validator/treasury recipients.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block number to process (1-indexed).
    /// * `block_gas_fees` - Total KOIN collected as gas fees in this block.
    ///
    /// # Returns
    ///
    /// A [`BlockReward`] containing the base emission and fee split components.
    ///
    /// # Errors
    ///
    /// Returns [`RewardError::ConservationViolation`] if minting the base reward
    /// would violate the total-value-conservation invariant (e.g., exceeding
    /// `OIKOS_MAX_SUPPLY`).
    ///
    /// # Panics
    ///
    /// This method does not panic.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use koinon_rewards::{BlockRewardProcessor, BlockRewardConfig};
    /// use koinon_ledger::KoinAmount;
    ///
    /// let mut proc = BlockRewardProcessor::new(BlockRewardConfig::default());
    /// let reward = proc.process_block(1, KoinAmount(1000)).unwrap();
    /// assert_eq!(reward.year, 1);
    /// assert_eq!(reward.block_number, 1);
    /// ```
    pub fn process_block(
        &mut self,
        block_number: u64,
        block_gas_fees: KoinAmount,
    ) -> Result<BlockReward, RewardError> {
        let year = self.year_for_block(block_number);
        let annual_emission = emission_at_year(year);
        let per_block = annual_emission / self.config.blocks_per_year as u128;
        let base_reward = OikosAmount(per_block);

        if base_reward.0 > 0 {
            if !self.conservation.record_mint(base_reward) {
                return Err(RewardError::ConservationViolation);
            }
        }

        let split = FeeSplit::from_total(block_gas_fees);

        self.current_year = year;

        Ok(BlockReward {
            block_number,
            year,
            base_reward,
            fee_burn: split.burn,
            fee_validator: split.validator,
            fee_treasury: split.treasury,
        })
    }

    /// Returns the current emission year derived from the last processed block.
    ///
    /// The year starts at 1 and advances as blocks cross year boundaries.
    /// Returns `0` if the last processed block was after year 20 (emission halted).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use koinon_rewards::{BlockRewardProcessor, BlockRewardConfig};
    /// use koinon_ledger::KoinAmount;
    ///
    /// let config = BlockRewardConfig { blocks_per_year: 100, ..Default::default() };
    /// let mut proc = BlockRewardProcessor::new(config);
    /// assert_eq!(proc.current_year(), 1);
    ///
    /// proc.process_block(101, KoinAmount::ZERO).unwrap();
    /// assert_eq!(proc.current_year(), 2);
    /// ```
    pub fn current_year(&self) -> u64 {
        self.current_year
    }

    /// Returns a reference to the internal conservation tracker.
    ///
    /// The [`TotalValueConservation`] records cumulative mints and can be
    /// inspected to verify that the total supply invariant holds.
    pub fn conservation(&self) -> &TotalValueConservation {
        &self.conservation
    }

    /// Checks whether the total-value-conservation invariant holds.
    ///
    /// Returns `true` if:
    /// 1. The internal conservation tracker's invariant is satisfied, AND
    /// 2. The cumulative minted amount does not exceed [`OIKOS_MAX_SUPPLY`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use koinon_rewards::{BlockRewardProcessor, BlockRewardConfig};
    /// use koinon_ledger::KoinAmount;
    ///
    /// let mut proc = BlockRewardProcessor::new(BlockRewardConfig::default());
    /// proc.process_block(1, KoinAmount::ZERO).unwrap();
    /// assert!(proc.check_conservation());
    /// ```
    pub fn check_conservation(&self) -> bool {
        self.conservation.check_invariant()
            && self.conservation.minted.0 <= OIKOS_MAX_SUPPLY
    }

    /// Maps a block number to its emission year.
    ///
    /// Block 1 is year 1. Subsequent blocks increment the year after every
    /// `blocks_per_year` blocks. Returns `0` for block 0 or when
    /// `blocks_per_year` is zero. Returns `0` for blocks after year 20.
    fn year_for_block(&self, block_number: u64) -> u64 {
        if block_number == 0 {
            return 0;
        }
        let blocks_per_year = self.config.blocks_per_year;
        if blocks_per_year == 0 {
            return 0;
        }
        let year = (block_number - 1) / blocks_per_year + 1;
        if year > 20 {
            0
        } else {
            year
        }
    }
}

/// Errors that can occur during block reward processing.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RewardError {
    /// The conservation invariant was violated after minting a block reward.
    /// This occurs when cumulative mints would exceed [`OIKOS_MAX_SUPPLY`].
    #[error("conservation invariant violated after mint")]
    ConservationViolation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_1_produces_year_1_reward() {
        let mut proc = BlockRewardProcessor::new(BlockRewardConfig::default());
        let reward = proc.process_block(1, KoinAmount::ZERO).unwrap();
        let expected_per_block = 50_000_000u128 / 31_536_000u128;
        assert_eq!(reward.year, 1);
        assert_eq!(reward.base_reward, OikosAmount(expected_per_block));
    }

    #[test]
    fn block_at_year_boundary_produces_year_2() {
        let config = BlockRewardConfig {
            blocks_per_year: 100,
            ..Default::default()
        };
        let mut proc = BlockRewardProcessor::new(config);
        let reward = proc.process_block(101, KoinAmount::ZERO).unwrap();
        assert_eq!(reward.year, 2);
        let expected_per_block = 40_000_000u128 / 100;
        assert_eq!(reward.base_reward, OikosAmount(expected_per_block));
    }

    #[test]
    fn fee_split_is_conserved() {
        let mut proc = BlockRewardProcessor::new(BlockRewardConfig::default());
        let reward = proc.process_block(1, KoinAmount(1000)).unwrap();
        let total = reward.fee_burn.0 + reward.fee_validator.0 + reward.fee_treasury.0;
        assert_eq!(total, 1000);
    }

    #[test]
    fn conservation_holds_after_many_blocks() {
        let config = BlockRewardConfig {
            blocks_per_year: 1000,
            ..Default::default()
        };
        let mut proc = BlockRewardProcessor::new(config);
        for i in 1..=10_000 {
            proc.process_block(i, KoinAmount(50)).unwrap();
        }
        assert!(proc.check_conservation());
    }

    #[test]
    fn emission_halts_after_year_20() {
        let config = BlockRewardConfig {
            blocks_per_year: 1,
            ..Default::default()
        };
        let mut proc = BlockRewardProcessor::new(config);
        let reward = proc.process_block(21, KoinAmount::ZERO).unwrap();
        assert_eq!(reward.base_reward, OikosAmount(0));
        assert_eq!(reward.year, 0);
    }
}
