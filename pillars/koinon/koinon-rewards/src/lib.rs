use koinon_fee::FeeSplit;
use koinon_ledger::{
    invariant::TotalValueConservation, emission_at_year, KoinAmount, OikosAmount,
    OIKOS_MAX_SUPPLY,
};

#[derive(Debug, Clone)]
pub struct BlockRewardConfig {
    pub blocks_per_year: u64,
    pub min_gas_per_block: u64,
}

impl Default for BlockRewardConfig {
    fn default() -> Self {
        Self {
            blocks_per_year: 31_536_000,
            min_gas_per_block: 1000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockReward {
    pub block_number: u64,
    pub year: u64,
    pub base_reward: OikosAmount,
    pub fee_burn: KoinAmount,
    pub fee_validator: KoinAmount,
    pub fee_treasury: KoinAmount,
}

#[derive(Debug, Clone)]
pub struct BlockRewardProcessor {
    config: BlockRewardConfig,
    conservation: TotalValueConservation,
    current_year: u64,
}

impl BlockRewardProcessor {
    pub fn new(config: BlockRewardConfig) -> Self {
        Self {
            config,
            conservation: TotalValueConservation::new(),
            current_year: 1,
        }
    }

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

    pub fn current_year(&self) -> u64 {
        self.current_year
    }

    pub fn conservation(&self) -> &TotalValueConservation {
        &self.conservation
    }

    pub fn check_conservation(&self) -> bool {
        self.conservation.check_invariant()
            && self.conservation.minted.0 <= OIKOS_MAX_SUPPLY
    }

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

#[derive(Debug, Clone, thiserror::Error)]
pub enum RewardError {
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
