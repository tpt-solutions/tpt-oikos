# koinon-staking

Validator staking and slashing for the Koinon settlement layer.

## Overview

`koinon-staking` manages validator registration, stake delegation, reward distribution, and slashing penalties within the Koinon ecosystem. It enforces a minimum stake requirement (100,000 OIKOS), maintains total-value-conservation invariants, and applies proportional slashing for misbehavior (double-signing, invalid state proofs) or prolonged downtime.

This crate depends on `koinon-ledger` for the `OikosAmount` and `TotalValueConservation` types.

### Key Concepts

- **Validator**: An operator that has registered and staked OIKOS tokens to participate in consensus.
- **Staking Pool**: Manages all validators, enforces minimum stake rules, and distributes rewards.
- **Slashing**: Penalties for misbehavior (double-signing, invalid state proofs, prolonged downtime).
- **Jail**: Temporary removal from active duty after serious misbehavior.

## Quick Start

```rust
use koinon_staking::{StakingPool, SlashingReason, MINIMUM_STAKE};
use koinon_ledger::OikosAmount;

// Create a staking pool
let mut pool = StakingPool::new();

// Register a validator
let validator_id = pool.register_validator("did:example:operator1").unwrap();

// Stake the minimum amount
pool.stake(validator_id, OikosAmount(MINIMUM_STAKE)).unwrap();

// Distribute rewards (called each block)
pool.distribute_rewards(OikosAmount(1000 * 10_u128.pow(18)));

// Validator claims their rewards
let rewards = pool.claim_rewards(validator_id).unwrap();
```

## Slashing Rules

| Reason | Slash Amount | Jail Duration |
|--------|-------------|---------------|
| Double signing | 50% of stake | 10,000 blocks |
| Invalid state proof | 10% of stake | 5,000 blocks |
| Downtime > 3600 blocks | 1% of stake | None |

## Invariants

- `total_staked` always equals the sum of all validators' `staked_amount`.
- No validator can stake below `MINIMUM_STAKE` (except full exit to zero).
- Jailed validators cannot receive rewards or accept new stakes.

## API Reference

### Constants

- `MINIMUM_STAKE` — `100_000 * 10^18` base units. Minimum stake a validator must hold.

### Types

#### `Validator`

A registered validator account. Fields: `id`, `operator_did`, `staked_amount`, `reward_debt`, `active`, `slashed_amount`, `created_at`, `jailed_until`.

#### `StakingPool`

The central type managing all validator state.

- `new()` — Create an empty pool
- `register_validator(did)` — Register a new validator, returns its ID
- `stake(id, amount)` — Add stake (must reach minimum)
- `unstake(id, amount)` — Remove stake (must stay at minimum or exit to zero)
- `get_validator(id)` — Look up a validator by ID
- `active_validators()` — List active, non-jailed validators
- `total_staked()` — Total OIKOS staked across all validators
- `is_active(id)` — Check if a validator is active and not jailed
- `distribute_rewards(amount)` — Distribute proportionally to active validators
- `claim_rewards(id)` — Withdraw accumulated rewards
- `check_invariant()` — Verify pool consistency

#### `StakingError`

Error type with variants: `ValidatorNotFound`, `InsufficientStake`, `BelowMinimumStake`, `ValidatorNotActive`, `ValidatorJailed`.

#### `SlashingReason`

Enum: `DoubleSigning`, `Downtime { duration_blocks }`, `InvalidStateProof`.

#### `SlashingResult`

Result of a slashing event: `validator_id`, `reason`, `original_stake`, `slash_amount`, `remaining_stake`, `jailed`, `jailed_until`.

### Functions

- `calculate_slash(validator, reason)` — Compute slash amount (pure)
- `apply_slashing(pool, validator_id, reason, block)` — Apply penalty to pool
- `jail_duration(reason)` — Get jail duration in blocks for a reason

## Testing

```bash
cargo test -p koinon-staking
```
