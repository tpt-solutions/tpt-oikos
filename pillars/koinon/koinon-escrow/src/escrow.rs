use koinon_ledger::{AccountId, KoinAmount, OikosAmount};

pub type EscrowId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscrowState {
    Funded,
    Released,
    Disputed,
    Refunded,
    Completed,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum EscrowError {
    #[error("escrow is not in a fundable state")]
    NotFunded,
    #[error("insufficient funds to escrow")]
    InsufficientFunds,
    #[error("unauthorized release attempt")]
    Unauthorized,
    #[error("escrow already finalized")]
    AlreadyFinalized,
    #[error("conditions not satisfied")]
    ConditionsNotSatisfied,
    #[error("cannot dispute: escrow not in Funded state")]
    CannotDispute,
}

#[derive(Debug, Clone)]
pub struct Escrow {
    pub id: EscrowId,
    pub sender: AccountId,
    pub receiver: AccountId,
    pub oikos_amount: OikosAmount,
    pub koin_amount: KoinAmount,
    pub state: EscrowState,
    pub conditions: Vec<EscrowCondition>,
}

#[derive(Debug, Clone)]
pub enum EscrowCondition {
    TimeLock(u64),
    MultiSig { required: u32, signers: Vec<AccountId> },
    Oracle(String),
}

impl Escrow {
    pub fn new(
        id: EscrowId,
        sender: AccountId,
        receiver: AccountId,
        oikos_amount: OikosAmount,
        koin_amount: KoinAmount,
    ) -> Self {
        Self {
            id,
            sender,
            receiver,
            oikos_amount,
            koin_amount,
            state: EscrowState::Funded,
            conditions: Vec::new(),
        }
    }

    pub fn add_condition(&mut self, condition: EscrowCondition) {
        self.conditions.push(condition);
    }

    pub fn can_release(&self) -> bool {
        self.state == EscrowState::Funded
    }

    pub fn evaluate_conditions(&self, current_time: u64, approvals: &[AccountId]) -> bool {
        self.conditions.iter().all(|cond| match cond {
            EscrowCondition::TimeLock(lock_time) => current_time >= *lock_time,
            EscrowCondition::MultiSig { required, signers } => {
                let approvals_count = approvals.iter().filter(|a| signers.contains(a)).count() as u32;
                approvals_count >= *required
            }
            EscrowCondition::Oracle(_name) => true,
        })
    }

    pub fn dispute(&mut self) -> Result<(), EscrowError> {
        if self.state != EscrowState::Funded {
            return Err(EscrowError::CannotDispute);
        }
        self.state = EscrowState::Disputed;
        Ok(())
    }

    pub fn check_release_authorization(&self, releaser: AccountId) -> Result<(), EscrowError> {
        if releaser != self.sender {
            return Err(EscrowError::Unauthorized);
        }
        Ok(())
    }

    pub fn release(&mut self) -> Result<(), EscrowError> {
        if !self.can_release() {
            return Err(EscrowError::AlreadyFinalized);
        }
        self.state = EscrowState::Completed;
        Ok(())
    }

    pub fn release_checked(&mut self, current_time: u64, approvals: &[AccountId]) -> Result<(), EscrowError> {
        if !self.can_release() {
            return Err(EscrowError::AlreadyFinalized);
        }
        if !self.evaluate_conditions(current_time, approvals) {
            return Err(EscrowError::ConditionsNotSatisfied);
        }
        self.state = EscrowState::Completed;
        Ok(())
    }

    pub fn refund(&mut self) -> Result<(), EscrowError> {
        match self.state {
            EscrowState::Funded | EscrowState::Disputed => {
                self.state = EscrowState::Refunded;
                Ok(())
            }
            _ => Err(EscrowError::AlreadyFinalized),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basic_escrow() -> Escrow {
        Escrow::new(1, 100, 200, OikosAmount::from_tokens(10), KoinAmount::new(50))
    }

    #[test]
    fn new_escrow_is_funded() {
        let e = basic_escrow();
        assert_eq!(e.state, EscrowState::Funded);
        assert!(e.can_release());
    }

    #[test]
    fn release_transitions_to_completed() {
        let mut e = basic_escrow();
        assert!(e.release().is_ok());
        assert_eq!(e.state, EscrowState::Completed);
    }

    #[test]
    fn release_twice_fails() {
        let mut e = basic_escrow();
        e.release().unwrap();
        assert!(matches!(e.release(), Err(EscrowError::AlreadyFinalized)));
    }

    #[test]
    fn dispute_from_funded() {
        let mut e = basic_escrow();
        assert!(e.dispute().is_ok());
        assert_eq!(e.state, EscrowState::Disputed);
    }

    #[test]
    fn dispute_from_non_funded_fails() {
        let mut e = basic_escrow();
        e.release().unwrap();
        assert!(matches!(e.dispute(), Err(EscrowError::CannotDispute)));
    }

    #[test]
    fn refund_from_funded() {
        let mut e = basic_escrow();
        assert!(e.refund().is_ok());
        assert_eq!(e.state, EscrowState::Refunded);
    }

    #[test]
    fn refund_from_disputed() {
        let mut e = basic_escrow();
        e.dispute().unwrap();
        assert!(e.refund().is_ok());
        assert_eq!(e.state, EscrowState::Refunded);
    }

    #[test]
    fn refund_from_completed_fails() {
        let mut e = basic_escrow();
        e.release().unwrap();
        assert!(matches!(e.refund(), Err(EscrowError::AlreadyFinalized)));
    }

    #[test]
    fn evaluate_conditions_no_conditions() {
        let e = basic_escrow();
        assert!(e.evaluate_conditions(0, &[]));
    }

    #[test]
    fn evaluate_conditions_timelock_satisfied() {
        let mut e = basic_escrow();
        e.add_condition(EscrowCondition::TimeLock(100));
        assert!(e.evaluate_conditions(100, &[]));
        assert!(e.evaluate_conditions(200, &[]));
    }

    #[test]
    fn evaluate_conditions_timelock_not_satisfied() {
        let mut e = basic_escrow();
        e.add_condition(EscrowCondition::TimeLock(100));
        assert!(!e.evaluate_conditions(99, &[]));
    }

    #[test]
    fn evaluate_conditions_multisig_satisfied() {
        let mut e = basic_escrow();
        e.add_condition(EscrowCondition::MultiSig {
            required: 2,
            signers: vec![10, 20, 30],
        });
        assert!(e.evaluate_conditions(0, &[10, 20]));
    }

    #[test]
    fn evaluate_conditions_multisig_not_satisfied() {
        let mut e = basic_escrow();
        e.add_condition(EscrowCondition::MultiSig {
            required: 2,
            signers: vec![10, 20, 30],
        });
        assert!(!e.evaluate_conditions(0, &[10]));
    }

    #[test]
    fn evaluate_conditions_oracle_always_true() {
        let mut e = basic_escrow();
        e.add_condition(EscrowCondition::Oracle("price_feed".into()));
        assert!(e.evaluate_conditions(0, &[]));
    }

    #[test]
    fn release_checked_with_no_conditions() {
        let mut e = basic_escrow();
        assert!(e.release_checked(0, &[]).is_ok());
        assert_eq!(e.state, EscrowState::Completed);
    }

    #[test]
    fn release_checked_with_unsatisfied_timelock() {
        let mut e = basic_escrow();
        e.add_condition(EscrowCondition::TimeLock(100));
        assert!(matches!(
            e.release_checked(50, &[]),
            Err(EscrowError::ConditionsNotSatisfied)
        ));
    }

    #[test]
    fn release_checked_with_satisfied_timelock() {
        let mut e = basic_escrow();
        e.add_condition(EscrowCondition::TimeLock(100));
        assert!(e.release_checked(100, &[]).is_ok());
    }

    #[test]
    fn release_checked_from_non_funded_fails() {
        let mut e = basic_escrow();
        e.release().unwrap();
        assert!(matches!(
            e.release_checked(0, &[]),
            Err(EscrowError::AlreadyFinalized)
        ));
    }

    #[test]
    fn check_release_authorization_sender() {
        let e = basic_escrow();
        assert!(e.check_release_authorization(100).is_ok());
    }

    #[test]
    fn check_release_authorization_non_sender() {
        let e = basic_escrow();
        assert!(matches!(
            e.check_release_authorization(999),
            Err(EscrowError::Unauthorized)
        ));
    }

    #[test]
    fn multiple_conditions_all_must_pass() {
        let mut e = basic_escrow();
        e.add_condition(EscrowCondition::TimeLock(100));
        e.add_condition(EscrowCondition::Oracle("feed".into()));
        e.add_condition(EscrowCondition::MultiSig {
            required: 1,
            signers: vec![10],
        });

        assert!(!e.evaluate_conditions(50, &[10]));
        assert!(!e.evaluate_conditions(100, &[]));
        assert!(e.evaluate_conditions(100, &[10]));
    }
}
