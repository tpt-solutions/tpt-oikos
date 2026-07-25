pub mod token;
pub mod account;
pub mod invariant;
pub mod timestamp;
pub mod genesis;
pub mod emission;
pub mod elastic;

pub use token::{OikosToken, KoinToken, TokenId, OikosAmount, KoinAmount};
pub use account::{AccountId, Account};
pub use invariant::TotalValueConservation;
pub use timestamp::{Timestamp, ContractAddress};
pub use genesis::GenesisAllocation;
pub use emission::{emission_at_year, emission_schedule, EmissionEntry};
pub use elastic::{ElasticSupplyState, SupplyAdjustment, calculate_average_gas_price};

pub const OIKOS_DECIMALS: u8 = 18;
pub const OIKOS_MAX_SUPPLY: u128 = 1_000_000_000 * 10_u128.pow(18);

#[derive(Debug, Clone, thiserror::Error)]
pub enum LedgerError {
    #[error("insufficient balance: have {have}, need {need}")]
    InsufficientBalance { have: u128, need: u128 },
    #[error("insufficient koin balance: have {have}, need {need}")]
    InsufficientKoin { have: i128, need: i128 },
    #[error("OIKOS supply invariant violated")]
    SupplyInvariant,
    #[error("account not found: {0}")]
    AccountNotFound(AccountId),
    #[error("overflow in arithmetic operation")]
    Overflow,
}

pub type Result<T> = std::result::Result<T, LedgerError>;
