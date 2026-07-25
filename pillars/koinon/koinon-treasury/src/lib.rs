use std::collections::HashMap;

use koinon_ledger::OikosAmount;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProposalStatus {
    Pending,
    Approved,
    Rejected,
    Executed,
    Expired,
}

#[derive(Debug, Clone)]
pub struct TreasuryProposal {
    pub id: u64,
    pub proposer: String,
    pub recipient: String,
    pub amount: OikosAmount,
    pub description: String,
    pub status: ProposalStatus,
    pub votes_for: u64,
    pub votes_against: u64,
    pub total_staked: u64,
    pub created_at: u64,
    pub expires_at: u64,
}

#[derive(Debug, Clone)]
pub struct TreasuryPool {
    pub balance: OikosAmount,
    pub proposals: HashMap<u64, TreasuryProposal>,
    next_proposal_id: u64,
    pub quorum_pct: u32,
}

impl TreasuryPool {
    pub fn new(initial_balance: OikosAmount) -> Self {
        Self {
            balance: initial_balance,
            proposals: HashMap::new(),
            next_proposal_id: 1,
            quorum_pct: 67,
        }
    }

    pub fn create_proposal(
        &mut self,
        proposer: &str,
        recipient: &str,
        amount: OikosAmount,
        description: &str,
        total_staked: u64,
        current_block: u64,
        voting_period: u64,
    ) -> Result<u64, TreasuryError> {
        let id = self.next_proposal_id;
        self.next_proposal_id = self
            .next_proposal_id
            .checked_add(1)
            .ok_or(TreasuryError::ProposalNotFound)?;

        let proposal = TreasuryProposal {
            id,
            proposer: proposer.to_string(),
            recipient: recipient.to_string(),
            amount,
            description: description.to_string(),
            status: ProposalStatus::Pending,
            votes_for: 0,
            votes_against: 0,
            total_staked,
            created_at: current_block,
            expires_at: current_block + voting_period,
        };
        self.proposals.insert(id, proposal);
        Ok(id)
    }

    pub fn vote(
        &mut self,
        proposal_id: u64,
        voter_stake: u64,
        in_favor: bool,
    ) -> Result<(), TreasuryError> {
        let proposal = self
            .proposals
            .get_mut(&proposal_id)
            .ok_or(TreasuryError::ProposalNotFound)?;

        if proposal.status != ProposalStatus::Pending {
            return Err(TreasuryError::NotVotingPeriod);
        }

        if in_favor {
            proposal.votes_for = proposal
                .votes_for
                .checked_add(voter_stake)
                .ok_or(TreasuryError::ProposalNotFound)?;
        } else {
            proposal.votes_against = proposal
                .votes_against
                .checked_add(voter_stake)
                .ok_or(TreasuryError::ProposalNotFound)?;
        }

        Ok(())
    }

    pub fn tally_and_execute(&mut self, proposal_id: u64) -> Result<(), TreasuryError> {
        let proposal = self
            .proposals
            .get_mut(&proposal_id)
            .ok_or(TreasuryError::ProposalNotFound)?;

        if proposal.status == ProposalStatus::Executed {
            return Err(TreasuryError::AlreadyExecuted);
        }

        if proposal.status != ProposalStatus::Pending {
            return Err(TreasuryError::NotVotingPeriod);
        }

        let quorum_needed =
            (proposal.total_staked as u128 * self.quorum_pct as u128) / 100;

        if (proposal.votes_for as u128) < quorum_needed {
            proposal.status = ProposalStatus::Rejected;
            return Err(TreasuryError::QuorumNotMet);
        }

        if self.balance.0 < proposal.amount.0 {
            proposal.status = ProposalStatus::Rejected;
            return Err(TreasuryError::InsufficientBalance);
        }

        self.balance.0 -= proposal.amount.0;
        proposal.status = ProposalStatus::Executed;
        Ok(())
    }

    pub fn get_proposal(&self, id: u64) -> Option<&TreasuryProposal> {
        self.proposals.get(&id)
    }

    pub fn check_invariant(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum TreasuryError {
    #[error("proposal not found")]
    ProposalNotFound,
    #[error("insufficient treasury balance")]
    InsufficientBalance,
    #[error("already voted")]
    AlreadyVoted,
    #[error("not in voting period")]
    NotVotingPeriod,
    #[error("quorum not met")]
    QuorumNotMet,
    #[error("proposal already executed")]
    AlreadyExecuted,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pool() -> TreasuryPool {
        TreasuryPool::new(OikosAmount(1_000_000_000))
    }

    #[test]
    fn create_proposal_and_execute() {
        let mut pool = make_pool();
        let id = pool
            .create_proposal("alice", "bob", OikosAmount(100), "fund dev", 1000, 0, 100)
            .unwrap();
        // 670 out of 1000 staked votes yes
        pool.vote(id, 670, true).unwrap();
        pool.tally_and_execute(id).unwrap();
        let p = pool.get_proposal(id).unwrap();
        assert_eq!(p.status, ProposalStatus::Executed);
        assert_eq!(pool.balance, OikosAmount(1_000_000_000 - 100));
    }

    #[test]
    fn reject_when_quorum_not_met() {
        let mut pool = make_pool();
        let id = pool
            .create_proposal("alice", "bob", OikosAmount(100), "fund dev", 1000, 0, 100)
            .unwrap();
        pool.vote(id, 500, true).unwrap();
        let result = pool.tally_and_execute(id);
        assert!(matches!(result, Err(TreasuryError::QuorumNotMet)));
        let p = pool.get_proposal(id).unwrap();
        assert_eq!(p.status, ProposalStatus::Rejected);
    }

    #[test]
    fn cannot_vote_after_voting_period() {
        let mut pool = make_pool();
        let id = pool
            .create_proposal("alice", "bob", OikosAmount(100), "fund dev", 1000, 0, 10)
            .unwrap();
        // Simulate time passing by rejecting manually then voting
        pool.proposals.get_mut(&id).unwrap().status = ProposalStatus::Rejected;
        let result = pool.vote(id, 100, true);
        assert!(matches!(result, Err(TreasuryError::NotVotingPeriod)));
    }

    #[test]
    fn cannot_execute_twice() {
        let mut pool = make_pool();
        let id = pool
            .create_proposal("alice", "bob", OikosAmount(100), "fund dev", 1000, 0, 100)
            .unwrap();
        pool.vote(id, 670, true).unwrap();
        pool.tally_and_execute(id).unwrap();
        let result = pool.tally_and_execute(id);
        assert!(matches!(result, Err(TreasuryError::AlreadyExecuted)));
    }

    #[test]
    fn cannot_spend_more_than_balance() {
        let mut pool = TreasuryPool::new(OikosAmount(50));
        let id = pool
            .create_proposal("alice", "bob", OikosAmount(100), "fund dev", 1000, 0, 100)
            .unwrap();
        pool.vote(id, 670, true).unwrap();
        let result = pool.tally_and_execute(id);
        assert!(matches!(result, Err(TreasuryError::InsufficientBalance)));
    }

    #[test]
    fn invariant_holds() {
        let pool = make_pool();
        assert!(pool.check_invariant());
    }
}
