use crate::token::{OikosAmount, KoinAmount};

pub type AccountId = u64;

#[derive(Debug, Clone)]
pub struct Account {
    pub id: AccountId,
    pub oikos_balance: OikosAmount,
    pub koin_balance: KoinAmount,
    pub nonce: u64,
}

impl Account {
    pub fn new(id: AccountId) -> Self {
        Self {
            id,
            oikos_balance: OikosAmount::ZERO,
            koin_balance: KoinAmount::ZERO,
            nonce: 0,
        }
    }

    pub fn has_sufficient_oikos(&self, amount: OikosAmount) -> bool {
        self.oikos_balance >= amount
    }

    pub fn has_sufficient_koin(&self, amount: KoinAmount) -> bool {
        amount.is_negative() || self.koin_balance >= amount
    }
}
