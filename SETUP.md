# TPT Oikos — Practical Setup Guide

This guide walks you through actually running the system end-to-end.

## What You Can Run Today

The system has three runnable components:

1. **`oikos` node** — Simulates the settlement layer (in-memory, single-node)
2. **`tpt-identity` server** — Full DID/VC/OIDC identity service (Go, needs config)
3. **`telos` CLI** — Verifies .telos contracts via SMT solver

## Quick Start (5 minutes)

### 1. Build everything

```bash
# Rust workspace
cargo build --workspace

# Go identity service
cd pillars/identity && go build ./cmd/tpt-identity/... && cd ../..
```

### 2. Run the node simulation

```bash
# Start the node (simulates 10 blocks with random transactions)
cargo run -p tpt-oikos-node -- start

# Check status
cargo run -p tpt-oikos-node -- status

# View tokenomics
cargo run -p tpt-oikos-node -- tokenomics
```

### 3. Verify a contract

```bash
# Verify the wallet contract
cargo run -p tpt-telos -- verify pillars/telos/examples/wallet.telos

# Verify the streaming payment contract
cargo run -p tpt-telos -- verify pillars/telos/examples/streaming_payment.telos

# Verify tokenomics invariants
cargo run -p tpt-telos -- verify pillars/telos/examples/tokenomics_invariants.telos
```

## Running the Identity Service

The identity service is a separate Go process that provides REST APIs for DID management, credential issuance, and mandate CRUD.

### 1. Generate keys

```bash
cd pillars/identity

# Generate Ed25519 + X25519 keypair
go run ./cmd/tpt-identity keygen \
  --method web \
  --domain localhost \
  --out-sign keys/ed25519.pem \
  --out-enc keys/x25519.pem \
  --passphrase ""
```

### 2. Create config

Copy `config.yaml.example` to `config.yaml` and edit:

```yaml
issuer: "https://localhost:8080"

identity:
  signing_key: "keys/ed25519.pem"
  enc_key: "keys/x25519.pem"
  passphrase: ""

server:
  addr: "0.0.0.0:8080"

database:
  path: "data/tpt-identity.db"
```

### 3. Start the server

```bash
go run ./cmd/tpt-identity serve --config config.yaml
```

The server starts on `http://localhost:8080`. Test it:

```bash
# Health check
curl http://localhost:8080/healthz

# OIDC discovery
curl http://localhost:8080/.well-known/openid-configuration

# Create an identity
curl -X POST http://localhost:8080/api/v1/identities \
  -H "Content-Type: application/json" \
  -d '{"method": "key"}'
```

### 4. Issue a mandate credential

```bash
# First, get the issuer DID from the keygen output
ISSUER_DID=$(go run ./cmd/tpt-identity resolve keys/ed25519.pem 2>/dev/null | head -1)

# Issue a mandate VC
go run ./cmd/tpt-identity issue-vc \
  --issuer "$ISSUER_DID" \
  --key keys/ed25519.pem \
  --subject did:peer:agent1 \
  --schema mandate.authority \
  --claim grantorDID="$ISSUER_DID" \
  --claim agentDID=did:peer:agent1 \
  --claim scope=transfer,escrow,stream \
  --valid-for 8760h
```

## End-to-End Flow

Here's the full flow from the spec, implemented across the pillars:

```
1. Human creates DID via tpt-identity
   → POST /api/v1/identities

2. Human issues agent mandate VC
   → POST /api/v1/credentials (schema: mandate.authority)

3. Agent authenticates via DID
   → OIDC flow via tpt-identity

4. Agent deploys a verified contract
   → telos verify contract.telos (SMT solver proves invariants)

5. Agent transacts on koinon
   → DAG settlement with fee split (70/20/10)

6. Mandate budget enforced
   → koin_spent <= koin_budget (proven via telos)

7. Dashboard renders live state
   → chora-data DataStore with mandates/balances/streams
```

## Writing Your Own Contract

1. Create `my_contract.telos`:

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

4. Or build a verified crate:

```bash
cargo run -p tpt-telos -- build my_contract.telos --out-dir /tmp/my_crate
```

## Koinon CLI

```bash
# Estimate gas
cargo run -p koinon-cli -- gas --steps 5 --storage 100

# Show emission schedule
cargo run -p koinon-cli -- tokenomics emission

# Show fee split
cargo run -p koinon-cli -- tokenomics fee-split --amount 10000

# Verify a contract
cargo run -p koinon-cli -- verify pillars/telos/examples/wallet.telos
```

## What's Simulated vs. Real

| Component | Status | Notes |
|-----------|--------|-------|
| Telos verification | **Real** | SMT solver proves invariants at compile time |
| Koinon ledger/tokenomics | **Real** | Full dual-token accounting, emission, fee split |
| Koinon staking/slashing | **Real** | Full validator pool with 100K OIKOS minimum |
| Koinon treasury | **Real** | Full DAO governance with proposals/voting |
| Koinon DAG | **Real** | Parallel settlement with conflict detection |
| Identity service | **Real** | Full DID/VC/OIDC with REST API |
| Node binary | **Simulated** | In-memory demo, not a live network |
| Multi-node networking | **Not built** | Requires P2P layer |
| Persistent state | **Not built** | All state is in-memory |
| Block explorer | **Not built** | Would need a separate service |

## What You'd Need for a Live Network

1. **Persistent storage** — Wire koinon to archon for state persistence
2. **P2P networking** — Implement gossip protocol for multi-node consensus
3. **Block production** — Leader election, block proposal, validation
4. **Transaction mempool** — Pending transaction pool with ordering
5. **RPC API** — External interface for wallets and tools
6. **Block explorer** — Web UI for inspecting chain state
7. **Wallet** — Key management and transaction signing
8. **Faucet** — Testnet token distribution

## Troubleshooting

### "cargo build" fails with wgpu errors

wgpu requires system dependencies. On Linux:
```bash
sudo apt install libxkbcommon-dev libwayland-dev libvulkan-dev
```

### Identity service won't start

Check that port 8080 is free:
```bash
lsof -i :8080
```

### Telos verification is slow

First run compiles the SMT solver. Subsequent runs are fast.

### Node binary panics

The node simulation expects valid block numbers. If you see panics, check that the emission schedule and conservation tracker are initialized correctly.
