# koinon-rewards

Block reward processing and emission schedule integration for the Koinon settlement layer.

## Overview

This crate wires the OIKOS emission schedule into the block production pipeline. Each block produces a reward proportional to the current emission year, with the rate decaying by 80% annually and halting after year 20.

### Tokenomics

- **Emission Schedule**: Disinflationary — 50M OIKOS in year 1, decaying 80% per year
- **Fee Split**: 70% burned, 20% validators, 10% treasury
- **Conservation**: Total minted + burned + staked + circulating + treasury is always conserved

## Quick Start

```rust
use koinon_rewards::{BlockRewardProcessor, BlockRewardConfig};
use koinon_ledger::KoinAmount;

// Create a processor with default config
let mut processor = BlockRewardProcessor::new(BlockRewardConfig::default());

// Process block 1 with 5000 Koin in fees
let reward = processor.process_block(1, KoinAmount(5000));
println!("Block reward: {:?}", reward.base_reward);
println!("Fee burned: {:?}", reward.fee_burn);
```

## Emission Schedule

| Year | Annual Emission | Cumulative |
|------|----------------|------------|
| 1 | 50.000M | 50.000M |
| 2 | 40.000M | 90.000M |
| 3 | 32.000M | 122.000M |
| ... | ... | ... |
| 20 | ~100K | ~999.9M |

After year 20, no new OIKOS is minted. Validators rely entirely on transaction fees.

## API Reference

### BlockRewardProcessor

- `new(config)` — Create a new processor
- `process_block(block_number, fees)` — Process a block, returns `BlockReward`
- `current_year()` — Get the current emission year
- `check_conservation()` — Verify conservation invariant

### BlockReward

Contains the results of processing a single block:
- `block_number` — The block that was processed
- `year` — Which emission year
- `base_reward` — Newly minted OIKOS
- `fee_burn` — Koin burned from fees
- `fee_validator` — Koin distributed to validators
- `fee_treasury` — Koin sent to treasury

## Testing

```bash
cargo test -p koinon-rewards
```
