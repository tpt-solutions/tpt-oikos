# koinon-treasury

DAO treasury governance for the Koinon settlement layer.

## Overview

This crate implements on-chain treasury governance with proposal creation, community voting, and quorum-based execution. The treasury receives 10% of all transaction fees and funds public goods, developer grants, and infrastructure.

### Governance Model

1. **Proposal**: Any stakeholder can submit a spending proposal
2. **Voting**: Stakeholders vote with their staked OIKOS weight
3. **Quorum**: 67% of total staked OIKOS must vote in favor
4. **Execution**: Approved proposals automatically deduct from treasury balance

## Quick Start

```rust
use koinon_treasury::{TreasuryPool, TreasuryProposal};
use koinon_ledger::OikosAmount;

// Create a treasury with initial balance
let mut treasury = TreasuryPool::new(OikosAmount(1_000_000 * 10_u128.pow(18)));

// Create a proposal
let proposal_id = treasury.create_proposal(
    "did:example:proposer",
    "did:example:recipient",
    OikosAmount(10_000 * 10_u128.pow(18)),
    "Fund developer grant",
    500_000, // total staked OIKOS
    100,     // current block
    1000,    // voting period (blocks)
).unwrap();

// Vote (voter must have staked OIKOS)
treasury.vote(proposal_id, 350_000, true).unwrap(); // 350K in favor

// Tally and execute if quorum met
treasury.tally_and_execute(proposal_id).unwrap();
```

## Quorum Rules

- **Quorum threshold**: 67% of total staked OIKOS must vote
- **Approval**: More votes in favor than against
- **Voting period**: Configurable block window (default 1000 blocks)
- **One-time execution**: Each proposal can only be executed once

## API Reference

### TreasuryPool

- `new(balance)` — Create treasury with initial balance
- `create_proposal(...)` — Submit a new spending proposal
- `vote(proposal_id, stake, in_favor)` — Cast a vote
- `tally_and_execute(proposal_id)` — Check quorum and execute
- `get_proposal(id)` — Read proposal details
- `check_invariant()` — Verify balance consistency

### TreasuryProposal

Contains proposal state:
- `id` — Unique proposal identifier
- `proposer` — DID of the proposal creator
- `recipient` — DID of the fund recipient
- `amount` — Requested amount
- `status` — Current status (Pending/Approved/Rejected/Executed)
- `votes_for` / `votes_against` — Current vote tally

## Testing

```bash
cargo test -p koinon-treasury
```
