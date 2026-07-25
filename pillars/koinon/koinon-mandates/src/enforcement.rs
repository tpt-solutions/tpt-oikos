use crate::mandate::AgentMandate;
use crate::scope::MandateScope;
use koinon_ledger::ContractAddress;

#[derive(Debug, Clone, thiserror::Error)]
pub enum EnforcementError {
    #[error("mandate is not active")]
    MandateInactive,

    #[error("mandate has expired")]
    MandateExpired,

    #[error("OIKOS budget exceeded: remaining {remaining}, requested {requested}")]
    OikosBudgetExceeded { remaining: u128, requested: u128 },

    #[error("Koin budget exceeded: remaining {remaining}, requested {requested}")]
    KoinBudgetExceeded { remaining: i128, requested: i128 },

    #[error("scope not permitted: {0:?}")]
    ScopeNotAllowed(MandateScope),

    #[error("contract not allowed: {0}")]
    ContractNotAllowed(ContractAddress),
}

pub fn check_budget(mandate: &AgentMandate, oikos_cost: u128, koin_cost: i128) -> Result<(), EnforcementError> {
    if !mandate.active {
        return Err(EnforcementError::MandateInactive);
    }

    if oikos_cost > 0 {
        let remaining = mandate.oikos_remaining()
            .ok_or(EnforcementError::OikosBudgetExceeded {
                remaining: 0,
                requested: oikos_cost,
            })?;
        if remaining.0 < oikos_cost {
            return Err(EnforcementError::OikosBudgetExceeded {
                remaining: remaining.0,
                requested: oikos_cost,
            });
        }
    }

    if koin_cost > 0 {
        let remaining = mandate.koin_remaining()
            .ok_or(EnforcementError::KoinBudgetExceeded {
                remaining: 0,
                requested: koin_cost,
            })?;
        if remaining.0 < koin_cost {
            return Err(EnforcementError::KoinBudgetExceeded {
                remaining: remaining.0,
                requested: koin_cost,
            });
        }
    }

    Ok(())
}

pub fn check_budget_with_time(
    mandate: &AgentMandate,
    oikos_cost: u128,
    koin_cost: i128,
    current_time: u64,
) -> Result<(), EnforcementError> {
    if !mandate.active {
        return Err(EnforcementError::MandateInactive);
    }

    if let Some(time_bound) = mandate.time_bound {
        if current_time > time_bound {
            return Err(EnforcementError::MandateExpired);
        }
    }

    check_budget(mandate, oikos_cost, koin_cost)
}

pub fn check_scope(mandate: &AgentMandate, required: &MandateScope) -> Result<(), EnforcementError> {
    if !mandate.active {
        return Err(EnforcementError::MandateInactive);
    }

    if mandate.scopes.contains(required) {
        Ok(())
    } else {
        Err(EnforcementError::ScopeNotAllowed(required.clone()))
    }
}

pub fn check_contract(mandate: &AgentMandate, contract: &ContractAddress) -> Result<(), EnforcementError> {
    if !mandate.active {
        return Err(EnforcementError::MandateInactive);
    }

    if mandate.allowed_contracts.is_empty() {
        return Ok(());
    }

    if mandate.allowed_contracts.contains(contract) {
        Ok(())
    } else {
        Err(EnforcementError::ContractNotAllowed(contract.clone()))
    }
}

pub fn check_all(
    mandate: &AgentMandate,
    oikos_cost: u128,
    koin_cost: i128,
    contract: &ContractAddress,
    current_time: u64,
) -> Result<(), EnforcementError> {
    check_budget_with_time(mandate, oikos_cost, koin_cost, current_time)?;
    check_contract(mandate, contract)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mandate::{MandateConfig, AgentMandate};
    use crate::scope::MandateScope;
    use koinon_ledger::{OikosAmount, KoinAmount, ContractAddress};

    fn test_config() -> MandateConfig {
        MandateConfig {
            did: "did:example:agent1".into(),
            principal_did: "did:example:principal1".into(),
            agent_did: "did:example:agent1".into(),
            oikos_budget: OikosAmount::from_tokens(100),
            koin_budget: KoinAmount::new(500),
            scopes: vec![
                MandateScope::TransferOikos,
                MandateScope::TransferKoin,
            ],
            allowed_contracts: vec![],
            time_bound: None,
        }
    }

    fn active_mandate() -> AgentMandate {
        AgentMandate::create(1, test_config())
    }

    #[test]
    fn check_budget_within_limits() {
        let mandate = active_mandate();
        assert!(check_budget(&mandate, 1, 1).is_ok());
    }

    #[test]
    fn check_budget_oikos_exceeded() {
        let mut mandate = active_mandate();
        mandate.oikos_spent = OikosAmount::from_tokens(100);
        assert!(matches!(
            check_budget(&mandate, 1, 0),
            Err(EnforcementError::OikosBudgetExceeded { .. })
        ));
    }

    #[test]
    fn check_budget_koin_exceeded() {
        let mut mandate = active_mandate();
        mandate.koin_spent = KoinAmount::new(500);
        assert!(matches!(
            check_budget(&mandate, 0, 1),
            Err(EnforcementError::KoinBudgetExceeded { .. })
        ));
    }

    #[test]
    fn check_budget_inactive() {
        let mut mandate = active_mandate();
        mandate.revoke();
        assert!(matches!(
            check_budget(&mandate, 0, 0),
            Err(EnforcementError::MandateInactive)
        ));
    }

    #[test]
    fn check_budget_koin_underflow_returns_none() {
        let mut mandate = active_mandate();
        mandate.koin_spent = KoinAmount::new(600);
        let remaining = mandate.koin_remaining();
        assert!(remaining.is_none());
    }

    #[test]
    fn check_budget_with_time_within_bound() {
        let mut config = test_config();
        config.time_bound = Some(1000);
        let mandate = AgentMandate::create(1, config);
        assert!(check_budget_with_time(&mandate, 1, 1, 500).is_ok());
    }

    #[test]
    fn check_budget_with_time_expired() {
        let mut config = test_config();
        config.time_bound = Some(1000);
        let mandate = AgentMandate::create(1, config);
        assert!(matches!(
            check_budget_with_time(&mandate, 1, 1, 1001),
            Err(EnforcementError::MandateExpired)
        ));
    }

    #[test]
    fn check_budget_with_time_no_bound() {
        let mandate = active_mandate();
        assert!(check_budget_with_time(&mandate, 1, 1, u64::MAX).is_ok());
    }

    #[test]
    fn check_scope_allowed() {
        let mandate = active_mandate();
        assert!(check_scope(&mandate, &MandateScope::TransferOikos).is_ok());
    }

    #[test]
    fn check_scope_not_allowed() {
        let mandate = active_mandate();
        assert!(matches!(
            check_scope(&mandate, &MandateScope::MintKoin),
            Err(EnforcementError::ScopeNotAllowed(MandateScope::MintKoin))
        ));
    }

    #[test]
    fn check_scope_inactive() {
        let mut mandate = active_mandate();
        mandate.revoke();
        assert!(matches!(
            check_scope(&mandate, &MandateScope::TransferOikos),
            Err(EnforcementError::MandateInactive)
        ));
    }

    #[test]
    fn check_contract_empty_allowlist() {
        let mandate = active_mandate();
        let contract = ContractAddress::new("0xanything");
        assert!(check_contract(&mandate, &contract).is_ok());
    }

    #[test]
    fn check_contract_allowed() {
        let mut config = test_config();
        config.allowed_contracts = vec![ContractAddress::new("0xabc")];
        let mandate = AgentMandate::create(1, config);
        assert!(check_contract(&mandate, &ContractAddress::new("0xabc")).is_ok());
    }

    #[test]
    fn check_contract_not_allowed() {
        let mut config = test_config();
        config.allowed_contracts = vec![ContractAddress::new("0xabc")];
        let mandate = AgentMandate::create(1, config);
        assert!(matches!(
            check_contract(&mandate, &ContractAddress::new("0xdef")),
            Err(EnforcementError::ContractNotAllowed(_))
        ));
    }

    #[test]
    fn check_contract_inactive() {
        let mut mandate = active_mandate();
        mandate.revoke();
        assert!(matches!(
            check_contract(&mandate, &ContractAddress::new("0xabc")),
            Err(EnforcementError::MandateInactive)
        ));
    }

    #[test]
    fn check_all_ok() {
        let mandate = active_mandate();
        let contract = ContractAddress::new("0xanything");
        assert!(check_all(&mandate, 1, 1, &contract, 0).is_ok());
    }

    #[test]
    fn check_all_expired() {
        let mut config = test_config();
        config.time_bound = Some(1000);
        let mandate = AgentMandate::create(1, config);
        let contract = ContractAddress::new("0xanything");
        assert!(matches!(
            check_all(&mandate, 1, 1, &contract, 1001),
            Err(EnforcementError::MandateExpired)
        ));
    }

    #[test]
    fn check_all_contract_blocked() {
        let mut config = test_config();
        config.allowed_contracts = vec![ContractAddress::new("0xabc")];
        let mandate = AgentMandate::create(1, config);
        let contract = ContractAddress::new("0xdef");
        assert!(matches!(
            check_all(&mandate, 1, 1, &contract, 0),
            Err(EnforcementError::ContractNotAllowed(_))
        ));
    }
}
