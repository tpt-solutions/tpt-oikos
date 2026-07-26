//! Validator staking and slashing for the Koinon protocol.
//!
//! This crate provides the core staking logic used by Koinon validators:
//!
//! - **Registration** — operators register a validator identity (DID).
//! - **Staking / Unstaking** — validators delegate or withdraw OIKOS tokens, enforced by a minimum-stake rule.
//! - **Reward distribution** — rewards are distributed proportionally to active, non-jailed validators.
//! - **Slashing** — misbehavior (double-signing, invalid state proofs, prolonged downtime) triggers
//!   proportional stake penalties and optional jail periods.
//!
//! All monetary amounts are represented in base units (`OikosAmount`) from [`koinon_ledger`].

pub mod staking;
pub mod slashing;

pub use staking::{Validator, StakingPool, StakingError, MINIMUM_STAKE};
pub use slashing::{SlashingReason, SlashingResult, calculate_slash, apply_slashing, jail_duration};
