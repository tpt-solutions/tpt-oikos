# tpt-oikos-node

The unified node binary for the TPT Oikos settlement layer.

## Overview

`oikos` is the main entry point for running a TPT Oikos node. It boots all settlement layer subsystems together: ledger, DAG, staking, rewards, treasury, fee splitting, gas metering, mandates, and escrow.

## Quick Start

```bash
# Build the node
cargo build -p tpt-oikos-node

# Show version and subsystems
cargo run -p tpt-oikos-node -- version

# Start the node (initializes all subsystems)
cargo run -p tpt-oikos-node -- start

# Check node status
cargo run -p tpt-oikos-node -- status

# View tokenomics
cargo run -p tpt-oikos-node -- tokenomics

# Verify a contract
cargo run -p tpt-oikos-node -- verify path/to/contract.telos

# Estimate gas
cargo run -p tpt-oikos-node -- gas --steps 5 --storage 100
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `oikos start` | Start the node, initialize all subsystems |
| `oikos status` | Show node status (block height, validators, DAG, treasury) |
| `oikos tokenomics` | Display emission schedule and fee structure |
| `oikos verify <file>` | Verify a .telos contract via SMT solver |
| `oikos gas` | Estimate gas cost for a transaction |
| `oikos version` | Show version and subsystem list |

## Architecture

The node boots the following subsystems in order:

1. **Ledger** — Dual-token accounting (OIKOS + Koin)
2. **DAG** — Parallel transaction settlement
3. **Staking** — Validator pool management
4. **Rewards** — Block reward processing
5. **Treasury** — DAO governance
6. **Fee** — 70/20/10 fee splitting
7. **Gas** — Deterministic gas metering
8. **Mandates** — AI agent mandate enforcement
9. **Escrow** — Escrow and streaming payments

## Observability

The node uses the `log` crate with `env_logger` for structured logging:

```bash
# Enable info-level logging
RUST_LOG=info cargo run -p tpt-oikos-node -- start

# Enable debug-level logging
RUST_LOG=debug cargo run -p tpt-oikos-node -- start

# Log only koinon crates
RUST_LOG=koinon=debug cargo run -p tpt-oikos-node -- start
```

Log levels:
- `INFO` — Block processed, transaction settled, major events
- `DEBUG` — Fee splits, reward distributions, state changes
- `WARN` — Conservation violations, unexpected states
- `ERROR` — Critical failures

## Building

```bash
# Debug build
cargo build -p tpt-oikos-node

# Release build (optimized)
cargo build -p tpt-oikos-node --release
```

## Testing

```bash
cargo test -p tpt-oikos-node
```
