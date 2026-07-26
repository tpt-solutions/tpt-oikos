# TPT Oikos — Getting Started

TPT Oikos is a vertically integrated, proof-native compute fabric for the autonomous AI agent economy. This guide walks you through building, testing, and using the system.

## Prerequisites

- **Rust** 1.74+ (stable): https://rustup.rs
- **Go** 1.22+ (for tpt-identity): https://go.dev/dl/
- **Git**

## Quick Build

```bash
# Clone the repo
git clone https://github.com/AnarKitted/tpt-oikos.git
cd tpt-oikos

# Build everything (Rust workspace)
cargo build --workspace

# Run all tests
cargo test --workspace
```

The Go identity module builds separately:
```bash
cd pillars/identity
go build ./...
go test ./...
```

## Try It Now

```bash
# Run the node simulation
cargo run -p tpt-oikos-node -- start

# Verify a contract
cargo run -p tpt-telos -- verify pillars/telos/examples/wallet.telos

# View tokenomics
cargo run -p tpt-oikos-node -- tokenomics
```

For detailed setup instructions (identity service, key generation, API usage), see **SETUP.md**.

## What's Inside

The monorepo contains 35 Rust crates across 6 pillars, plus a Go identity service:

| Pillar | What It Does | Key Crates |
|--------|-------------|------------|
| **eidos** | Proof-native language compiler | `eidos-parser`, `eidos-verifier`, `eidos-kernel`, `eidos-codegen` |
| **telos** | Contract verification & code generation | `telos-parser`, `telos-ir`, `telos-verifier`, `telos-codegen` |
| **koinon** | Settlement, economy, governance | `koinon-ledger`, `koinon-dag`, `koinon-staking`, `koinon-treasury` |
| **identity** | DIDs, VCs, OIDC, mandate management | Go module with REST API |
| **archon** | Zero-copy storage, SQL engine | `tpt-archon-core`, `tpt-archon-relational` |
| **chora** | GPU-rendered dashboards | `chora-render`, `chora-data`, `chora-ui` |

## Core Concepts

### Dual-Token Model

- **OIKOS** — Governance token. Fixed 1B supply, 18 decimals. Used for staking and governance.
- **Koin** — Settlement token. Elastic supply, i128 precision. Used for gas fees and agent micropayments.

### Verified Contracts

Contracts are written in `.telos` files with mathematical invariants:

```
@boundary(cpu_bound)
module PaymentGateway {
    invariant Wallet {
        balance >= 0
    }

    func transfer(from: Wallet, to: Wallet, amount: PositiveInt)
        requires from.balance >= amount
        ensures from.balance == old(from.balance) - amount
        ensures to.balance == old(to.balance) + amount
    {
        mutate state {
            from.balance -= amount
            to.balance += amount
        }
    }
}
```

The SMT solver proves these invariants hold at compile time.

### Agent Mandates

Agents operate under scoped mandates with budgets:

```rust
struct AgentMandate {
    principal_did: String,    // who granted authority
    agent_did: String,        // the agent
    koin_budget: KoinAmount,  // spending limit
    time_bound: Option<u64>,  // expiration
    scopes: Vec<MandateScope>, // permitted actions
}
```

Mandates are enforced at runtime — agents cannot exceed their budget or act outside their scope.

## Using the CLIs

### koinon CLI

```bash
# Estimate gas for a transaction
cargo run -p koinon-cli -- gas --steps 5 --storage 100

# Show the OIKOS emission schedule
cargo run -p koinon-cli -- tokenomics emission

# Show fee split for a given amount
cargo run -p koinon-cli -- tokenomics fee-split --amount 10000

# Verify a .telos contract before deployment
cargo run -p koinon-cli -- verify pillars/telos/examples/wallet.telos
```

### telos CLI

```bash
# Verify a contract
cargo run -p tpt-telos -- verify pillars/telos/examples/wallet.telos

# Verify with JSON output (for CI)
cargo run -p tpt-telos -- verify pillars/telos/examples/wallet.telos --json

# Transpile to Rust
cargo run -p tpt-telos -- transpile pillars/telos/examples/wallet.telos

# Build a verified Rust crate
cargo run -p tpt-telos -- build pillars/telos/examples/wallet.telos --out-dir /tmp/wallet

# Generate a dual-backend project (Rust + Go + FFI)
cargo run -p tpt-telos -- project pillars/telos/examples/microservice.telos --check

# Start the LSP server (for IDE integration)
cargo run -p tpt-telos -- lsp
```

### eidos CLI

```bash
# Verify an .eidos source file
cargo run -p tpt-eidos-cli -- check pillars/eidos/examples/calibrate_gyro.eidos

# Build a verified no_std Rust crate
cargo run -p tpt-eidos-cli -- build pillars/eidos/examples/calibrate_gyro.eidos --out-dir /tmp/output
```

### archon SQL REPL

```bash
# Start the interactive SQL shell
cargo run -p out-archon-sql

# Then try:
# CREATE TABLE users (id INT, name TEXT, age INT);
# INSERT INTO users VALUES (1, 'Alice', 30);
# INSERT INTO users VALUES (2, 'Bob', 25);
# SELECT * FROM users WHERE age > 26 ORDER BY age;
```

## Writing Your First Contract

1. Create a file `my_contract.telos`:

```
@boundary(cpu_bound)
module Counter {
    invariant NonNegative {
        count >= 0
    }

    func increment(c: Counter, amount: PositiveInt)
        ensures c.count == old(c.count) + amount
    {
        mutate state {
            c.count += amount
        }
    }

    func decrement(c: Counter, amount: PositiveInt)
        requires c.count >= amount
        ensures c.count == old(c.count) - amount
    {
        mutate state {
            c.count -= amount
        }
    }
}
```

2. Verify it:

```bash
cargo run -p tpt-telos -- verify my_contract.telos
```

3. If verification passes, transpile to Rust:

```bash
cargo run -p tpt-telos -- transpile my_contract.telos --out my_contract.rs
```

## Koinon Crate Dependency Graph

```
koinon-cli
  ├── koinon-ledger      (core types: OikosAmount, KoinAmount, Account)
  ├── koinon-dag         (transaction DAG, parallel settlement)
  ├── koinon-mandates    (agent mandate management, enforcement)
  ├── koinon-escrow      (escrow, streaming payments)
  ├── koinon-gas         (deterministic gas pricing)
  ├── koinon-fee         (fee split logic)
  ├── koinon-staking     (validator staking, slashing)
  ├── koinon-rewards     (block rewards, emission)
  └── koinon-treasury    (DAO governance, proposals)
```

## Identity Service (Go)

The identity service provides REST APIs for DID management, credential issuance, and mandate CRUD:

```bash
cd pillars/identity

# Build
go build ./cmd/tpt-identity/...

# Generate keys
go run ./cmd/tpt-identity keygen --method web --domain example.com \
  --out-sign keys/ed25519.pem --out-enc keys/x25519.pem --passphrase ""

# Start the server
go run ./cmd/tpt-identity serve --config config.yaml

# Issue a mandate credential
go run ./cmd/tpt-identity issue-vc \
  --issuer did:web:example.com --key ed25519.pem \
  --subject did:peer:xyz --schema mandate.authority \
  --claim grantorDID=did:web:example.com \
  --claim agentDID=did:peer:xyz \
  --claim scope=transfer,escrow \
  --valid-for 8760h
```

### Key API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/identities` | Create a new identity |
| GET | `/api/v1/identities/{did}` | Resolve a DID |
| POST | `/api/v1/credentials` | Issue a VC |
| POST | `/api/v1/credentials/verify` | Verify a VC |
| POST | `/api/v1/mandates` | Create an agent mandate |
| GET | `/api/v1/mandates` | List mandates |
| POST | `/api/v1/mandates/{id}/revoke` | Revoke a mandate |
| POST | `/oidc/authorize` | OIDC authorization |
| POST | `/oidc/token` | OIDC token exchange |
| GET | `/healthz` | Health check |

## Testing Strategy

Every koinon crate has comprehensive unit tests. Run them with:

```bash
# All koinon tests
cargo test -p koinon-ledger -p koinon-dag -p koinon-mandates \
  -p koinon-escrow -p koinon-gas -p koinon-fee \
  -p koinon-staking -p koinon-rewards -p koinon-treasury

# Specific crate
cargo test -p koinon-staking

# With output
cargo test -p koinon-staking -- --nocapture
```

## Verified Invariants

The following invariants are formally verified via tpt-telos:

| Invariant | File | What It Proves |
|-----------|------|----------------|
| `FeeConservation` | `tokenomics_invariants.telos` | `fee_paid == burned + validator_reward + treasury` |
| `MandateSolvency` | `tokenomics_invariants.telos` | `koin_spent <= koin_budget` |
| `TotalValueConservation` | `tokenomics_invariants.telos` | `total == staked + circulating + treasury` |
| `Stream` | `streaming_payment.telos` | `total_withdrawn <= total_deposited` |
| `Rate` | `streaming_payment.telos` | `rate_per_second > 0` |
| `Wallet` | `wallet.telos` | `balance >= 0` |

## Architecture Decision Records

Design decisions are documented in `pillars/archon/docs/`:

- **ADR 0001**: Inside-out architecture rationale (core → bridge → kernel → relational)
- **ADR 0002**: Zero-allocation primitives (why no `tpt-zero-bytes` dependency)
- **ADR 0003**: Verification strategy (tested-now-proven-later, not Coq/Lean)

## Next Steps

1. **Read the spec**: `spec.txt` covers the full system architecture
2. **Read the tokenomics**: `tokenomics.txt` covers the economic model
3. **Explore the examples**: `pillars/telos/examples/` has verified contracts
4. **Try the SQL engine**: `cargo run -p out-archon-sql` for an interactive shell
5. **Run the full test suite**: `cargo test --workspace` (650+ tests)
6. **See SETUP.md**: For detailed instructions on running each component
