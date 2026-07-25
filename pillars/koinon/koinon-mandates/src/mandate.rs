use koinon_ledger::{OikosAmount, KoinAmount, ContractAddress};
use crate::scope::MandateScope;

pub type MandateId = u64;

#[derive(Debug, Clone)]
pub struct MandateConfig {
    pub did: String,
    pub principal_did: String,
    pub agent_did: String,
    pub oikos_budget: OikosAmount,
    pub koin_budget: KoinAmount,
    pub scopes: Vec<MandateScope>,
    pub allowed_contracts: Vec<ContractAddress>,
    pub time_bound: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct AgentMandate {
    pub id: MandateId,
    pub did: String,
    pub principal_did: String,
    pub agent_did: String,
    pub oikos_budget: OikosAmount,
    pub oikos_spent: OikosAmount,
    pub koin_budget: KoinAmount,
    pub koin_spent: KoinAmount,
    pub scopes: Vec<MandateScope>,
    pub allowed_contracts: Vec<ContractAddress>,
    pub time_bound: Option<u64>,
    pub active: bool,
}

impl AgentMandate {
    pub fn create(id: MandateId, config: MandateConfig) -> Self {
        Self {
            id,
            did: config.did,
            principal_did: config.principal_did,
            agent_did: config.agent_did,
            oikos_budget: config.oikos_budget,
            oikos_spent: OikosAmount::ZERO,
            koin_budget: config.koin_budget,
            koin_spent: KoinAmount::ZERO,
            scopes: config.scopes,
            allowed_contracts: config.allowed_contracts,
            time_bound: config.time_bound,
            active: true,
        }
    }

    pub fn oikos_remaining(&self) -> Option<OikosAmount> {
        self.oikos_budget.checked_sub(self.oikos_spent)
    }

    pub fn koin_remaining(&self) -> Option<KoinAmount> {
        let r = self.koin_budget.checked_sub(self.koin_spent)?;
        if r.is_negative() { None } else { Some(r) }
    }

    pub fn record_oikos_spend(&mut self, amount: OikosAmount) -> bool {
        if !self.active {
            return false;
        }
        if let Some(remaining) = self.oikos_remaining() {
            if remaining >= amount {
                if let Some(new_spent) = self.oikos_spent.checked_add(amount) {
                    self.oikos_spent = new_spent;
                    return true;
                }
            }
        }
        false
    }

    pub fn record_koin_spend(&mut self, amount: KoinAmount) -> bool {
        if !self.active || amount.is_negative() {
            return false;
        }
        if let Some(new_spent) = self.koin_spent.checked_add(amount) {
            if new_spent <= self.koin_budget {
                self.koin_spent = new_spent;
                return true;
            }
        }
        false
    }

    pub fn revoke(&mut self) {
        self.active = false;
    }

    pub fn set_time_bound(&mut self, time_bound: u64) {
        self.time_bound = Some(time_bound);
    }

    pub fn add_allowed_contract(&mut self, contract: ContractAddress) {
        if !self.allowed_contracts.contains(&contract) {
            self.allowed_contracts.push(contract);
        }
    }

    pub fn check_invariant(&self) -> bool {
        let budget_invariant = self.oikos_spent <= self.oikos_budget
            && KoinAmount(self.koin_spent.0) <= self.koin_budget;
        let time_invariant = self.time_bound.is_none()
            || self.time_bound.map_or(true, |tb| tb > 0);
        budget_invariant && time_invariant
    }
}
