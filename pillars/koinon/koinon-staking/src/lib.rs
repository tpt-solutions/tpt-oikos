pub mod staking;
pub mod slashing;

pub use staking::{Validator, StakingPool, StakingError, MINIMUM_STAKE};
pub use slashing::{SlashingReason, SlashingResult, calculate_slash, apply_slashing, jail_duration};
