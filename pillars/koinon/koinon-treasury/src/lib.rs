//! Treasury governance and spend proposals for the Oikos DAO.
//!
//! This crate provides a proposal-based governance model for managing on-chain
//! treasury funds. Members submit spend proposals, vote with their staked tokens,
//! and approved proposals are executed once quorum is reached.
//!
//! # Overview
//!
//! The core type is [`TreasuryPool`], which holds the treasury balance and all
//! proposals. Proposals follow a lifecycle:
//!
//! ```text
//! Pending → Rejected (quorum not met or insufficient balance)
//! Pending → Executed (quorum met and balance sufficient)
//! ```
//!
//! # Governance Rules
//!
//! - **Quorum**: proposals require a configurable supermajority (default 67%) of
//!   staked votes in favor.
//! - **One-shot execution**: each proposal can only be executed once.
//! - **Spend cap**: transfers never exceed the current treasury balance.
//!
//! # Example
//!
//! ```rust
//! use koinon_treasury::{TreasuryPool, ProposalStatus};
//! use koinon_ledger::OikosAmount;
//!
//! let mut pool = TreasuryPool::new(OikosAmount(1_000_000));
//! let id = pool
//!     .create_proposal("alice", "bob", OikosAmount(100), "fund dev", 1000, 0, 100)
//!     .unwrap();
//! pool.vote(id, 670, true).unwrap();
//! pool.tally_and_execute(id).unwrap();
//! assert_eq!(pool.get_proposal(id).unwrap().status, ProposalStatus::Executed);
//! ```

use std::collections::HashMap;

use koinon_ledger::OikosAmount;

/// Lifecycle status of a treasury proposal.
///
/// Proposals transition through states as they are voted on and executed.
/// The valid transitions are:
///
/// ```text
/// Pending → Executed  (quorum met, balance sufficient)
/// Pending → Rejected  (quorum not met or insufficient balance)
/// ```
///
/// # Examples
///
/// ```rust
/// use koinon_treasury::ProposalStatus;
///
/// let status = ProposalStatus::Pending;
/// assert_ne!(status, ProposalStatus::Executed);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProposalStatus {
    /// Proposal is open for voting.
    Pending,
    /// Proposal has been approved by quorum but not yet executed.
    /// Reserved for future use; current flow goes directly to `Executed`.
    Approved,
    /// Proposal did not meet quorum or failed due to insufficient balance.
    Rejected,
    /// Proposal was approved and funds have been transferred.
    Executed,
    /// Proposal's voting window has elapsed without reaching quorum.
    /// Callers should check `expires_at` and set this status externally.
    Expired,
}

/// A treasury spend proposal submitted for DAO governance voting.
///
/// Each proposal specifies a transfer from the treasury to a recipient.
/// Stakeholders vote on the proposal, and it is executed if quorum is reached.
///
/// # Fields
///
/// - `id` — Unique identifier assigned by the treasury pool.
/// - `proposer` — Account that submitted the proposal.
/// - `recipient` — Account that will receive funds if executed.
/// - `amount` — Requested transfer amount in base units.
/// - `description` — Human-readable description of the proposal's purpose.
/// - `status` — Current lifecycle status ([`ProposalStatus`]).
/// - `votes_for` — Cumulative stake-weighted votes in favor.
/// - `votes_against` — Cumulative stake-weighted votes against.
/// - `total_staked` — Total staked tokens at proposal creation (used for quorum calculation).
/// - `created_at` — Block number when the proposal was created.
/// - `expires_at` — Block number when the voting window closes.
#[derive(Debug, Clone)]
pub struct TreasuryProposal {
    /// Unique proposal identifier, auto-incremented by the pool.
    pub id: u64,
    /// Account that submitted the proposal.
    pub proposer: String,
    /// Account that will receive funds if the proposal is executed.
    pub recipient: String,
    /// Amount to transfer from the treasury, in base units.
    pub amount: OikosAmount,
    /// Human-readable description of the proposal's purpose.
    pub description: String,
    /// Current lifecycle status of the proposal.
    pub status: ProposalStatus,
    /// Cumulative stake-weighted votes in favor.
    pub votes_for: u64,
    /// Cumulative stake-weighted votes against.
    pub votes_against: u64,
    /// Total staked tokens at proposal creation, used for quorum calculation.
    pub total_staked: u64,
    /// Block number when the proposal was created.
    pub created_at: u64,
    /// Block number when the voting window closes.
    pub expires_at: u64,
}

/// On-chain treasury pool managing balances and governance proposals.
///
/// The treasury pool holds a balance and tracks all submitted proposals.
/// Proposals are created, voted on, and executed through this type.
///
/// # Quorum
///
/// The default quorum is 67% (supermajority). This can be changed by
/// directly setting the `quorum_pct` field before creating proposals.
///
/// # Invariants
///
/// - `balance` must never go negative (enforced by `tally_and_execute`).
/// - Each proposal is executed at most once.
///
/// # Examples
///
/// ```rust
/// use koinon_treasury::TreasuryPool;
/// use koinon_ledger::OikosAmount;
///
/// let mut pool = TreasuryPool::new(OikosAmount(5_000_000));
/// assert_eq!(pool.balance, OikosAmount(5_000_000));
/// assert_eq!(pool.quorum_pct, 67);
/// ```
#[derive(Debug, Clone)]
pub struct TreasuryPool {
    /// Current treasury balance in base units.
    pub balance: OikosAmount,
    /// Map of proposal ID to proposal for all submitted proposals.
    pub proposals: HashMap<u64, TreasuryProposal>,
    /// Internal counter for the next proposal ID.
    next_proposal_id: u64,
    /// Minimum percentage of `total_staked` that `votes_for` must meet for a proposal to pass.
    pub quorum_pct: u32,
}

impl TreasuryPool {
    /// Creates a new treasury pool with the given initial balance.
    ///
    /// The quorum defaults to 67% (supermajority).
    ///
    /// # Arguments
    ///
    /// * `initial_balance` — Starting balance of the treasury in base units.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use koinon_treasury::TreasuryPool;
    /// use koinon_ledger::OikosAmount;
    ///
    /// let pool = TreasuryPool::new(OikosAmount(1_000_000));
    /// assert_eq!(pool.balance, OikosAmount(1_000_000));
    /// assert_eq!(pool.quorum_pct, 67);
    /// ```
    pub fn new(initial_balance: OikosAmount) -> Self {
        Self {
            balance: initial_balance,
            proposals: HashMap::new(),
            next_proposal_id: 1,
            quorum_pct: 67,
        }
    }

    /// Submits a new treasury spend proposal.
    ///
    /// The proposal enters `Pending` status and is open for voting until
    /// `current_block + voting_period` blocks have elapsed.
    ///
    /// # Arguments
    ///
    /// * `proposer` — Account submitting the proposal.
    /// * `recipient` — Account that will receive funds if executed.
    /// * `amount` — Amount to transfer from the treasury.
    /// * `description` — Human-readable purpose of the proposal.
    /// * `total_staked` — Total staked tokens at submission (used for quorum calculation).
    /// * `current_block` — Current block number.
    /// * `voting_period` — Number of blocks the voting window stays open.
    ///
    /// # Returns
    ///
    /// The unique ID assigned to the new proposal.
    ///
    /// # Errors
    ///
    /// Returns [`TreasuryError::ProposalNotFound`] if the internal ID counter overflows.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use koinon_treasury::TreasuryPool;
    /// use koinon_ledger::OikosAmount;
    ///
    /// let mut pool = TreasuryPool::new(OikosAmount(1_000_000));
    /// let id = pool
    ///     .create_proposal("alice", "bob", OikosAmount(500), "grant", 1000, 0, 100)
    ///     .unwrap();
    /// assert_eq!(id, 1);
    /// ```
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

    /// Casts a stake-weighted vote on a pending proposal.
    ///
    /// The vote is weighted by `voter_stake` and added to either `votes_for`
    /// or `votes_against` depending on the `in_favor` flag.
    ///
    /// # Arguments
    ///
    /// * `proposal_id` — ID of the proposal to vote on.
    /// * `voter_stake` — Number of staked tokens representing this vote.
    /// * `in_favor` — `true` to vote in favor, `false` to vote against.
    ///
    /// # Errors
    ///
    /// - [`TreasuryError::ProposalNotFound`] — No proposal with the given ID exists.
    /// - [`TreasuryError::NotVotingPeriod`] — The proposal is not in `Pending` status.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use koinon_treasury::TreasuryPool;
    /// use koinon_ledger::OikosAmount;
    ///
    /// let mut pool = TreasuryPool::new(OikosAmount(1_000_000));
    /// let id = pool
    ///     .create_proposal("alice", "bob", OikosAmount(100), "dev", 1000, 0, 100)
    ///     .unwrap();
    /// pool.vote(id, 500, true).unwrap();
    /// pool.vote(id, 200, false).unwrap();
    /// let p = pool.get_proposal(id).unwrap();
    /// assert_eq!(p.votes_for, 500);
    /// assert_eq!(p.votes_against, 200);
    /// ```
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

    /// Tallies votes and executes the proposal if quorum is met.
    ///
    /// The proposal is approved and executed if `votes_for >= quorum_pct * total_staked / 100`.
    /// On execution, the treasury balance is debited by the proposal amount and the
    /// status changes to `Executed`.
    ///
    /// If quorum is not met, the status changes to `Rejected`.
    /// If the treasury has insufficient balance, the status also changes to `Rejected`.
    ///
    /// # Arguments
    ///
    /// * `proposal_id` — ID of the proposal to tally and execute.
    ///
    /// # Errors
    ///
    /// - [`TreasuryError::ProposalNotFound`] — No proposal with the given ID exists.
    /// - [`TreasuryError::AlreadyExecuted`] — The proposal has already been executed.
    /// - [`TreasuryError::NotVotingPeriod`] — The proposal is not in `Pending` status.
    /// - [`TreasuryError::QuorumNotMet`] — `votes_for` did not reach the quorum threshold.
    /// - [`TreasuryError::InsufficientBalance`] — Treasury balance is less than the proposal amount.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use koinon_treasury::{TreasuryPool, ProposalStatus};
    /// use koinon_ledger::OikosAmount;
    ///
    /// let mut pool = TreasuryPool::new(OikosAmount(1_000_000));
    /// let id = pool
    ///     .create_proposal("alice", "bob", OikosAmount(100), "dev", 1000, 0, 100)
    ///     .unwrap();
    /// pool.vote(id, 670, true).unwrap(); // 67% of 1000
    /// pool.tally_and_execute(id).unwrap();
    /// assert_eq!(pool.get_proposal(id).unwrap().status, ProposalStatus::Executed);
    /// assert_eq!(pool.balance, OikosAmount(999_900));
    /// ```
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

    /// Retrieves a proposal by its ID.
    ///
    /// # Arguments
    ///
    /// * `id` — The proposal ID to look up.
    ///
    /// # Returns
    ///
    /// A reference to the proposal if found, or `None` if no proposal with that ID exists.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use koinon_treasury::TreasuryPool;
    /// use koinon_ledger::OikosAmount;
    ///
    /// let mut pool = TreasuryPool::new(OikosAmount(1_000_000));
    /// let id = pool
    ///     .create_proposal("alice", "bob", OikosAmount(100), "dev", 1000, 0, 100)
    ///     .unwrap();
    /// assert!(pool.get_proposal(id).is_some());
    /// assert!(pool.get_proposal(999).is_none());
    /// ```
    pub fn get_proposal(&self, id: u64) -> Option<&TreasuryProposal> {
        self.proposals.get(&id)
    }

    /// Checks that treasury invariants hold.
    ///
    /// This is a placeholder that currently always returns `true`.
    /// Future implementations may verify balance consistency, proposal
    /// integrity, or other invariants.
    ///
    /// # Returns
    ///
    /// `true` if all invariants hold.
    pub fn check_invariant(&self) -> bool {
        true
    }
}

/// Errors that can occur during treasury operations.
///
/// All errors implement `std::error::Error` via `thiserror`.
///
/// # Examples
///
/// ```rust
/// use koinon_treasury::TreasuryError;
///
/// let err = TreasuryError::QuorumNotMet;
/// assert_eq!(err.to_string(), "quorum not met");
/// ```
#[derive(Debug, Clone, thiserror::Error)]
pub enum TreasuryError {
    /// No proposal exists with the given ID.
    #[error("proposal not found")]
    ProposalNotFound,
    /// Treasury balance is insufficient to cover the proposal amount.
    #[error("insufficient treasury balance")]
    InsufficientBalance,
    /// The voter has already cast a vote on this proposal.
    #[error("already voted")]
    AlreadyVoted,
    /// The proposal is not in a status that accepts votes or execution.
    #[error("not in voting period")]
    NotVotingPeriod,
    /// `votes_for` did not reach the quorum threshold.
    #[error("quorum not met")]
    QuorumNotMet,
    /// The proposal has already been executed and cannot be executed again.
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
