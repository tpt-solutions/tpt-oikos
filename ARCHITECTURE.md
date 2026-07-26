# TPT Oikos — Architecture

TPT Oikos is a vertically integrated, proof-native compute fabric for the autonomous AI agent economy. It unifies five pillars (plus a storage substrate) into a single monorepo.

## Pillar Map

| Pillar | Role | Language | Status |
|--------|------|----------|--------|
| **tpt-eidos** | Proof-native language compiler — refinement types, QF_LRA verification | Rust | Mature MVK; flight-control domain library |
| **tpt-telos** | Execution & verification — SMT-verified contracts, dual Rust/Go codegen | Rust | v1.2, mature |
| **tpt-koinon** | Settlement & economy — DAG ledger, mandates, escrow, streaming, staking, treasury | Rust | Full implementation |
| **tpt-identity** | Identity & mandate governance — DIDs, VCs, OIDC, mandate CRUD | Go | Mature, full-featured |
| **tpt-chora** | Human observation runtime — wgpu-rendered dashboards, data binding | Rust | Early; GPU foundation + data store |
| **tpt-archon** | Storage kernel substrate — zero-copy storage, B-Link tree, MVCC, SQL engine | Rust | Mature; full database stack |

## Directory Layout

```
tpt-oikos/
├── Cargo.toml                  # Unified Rust workspace (all Rust pillars)
├── ARCHITECTURE.md             # This file
├── GETTING_STARTED.md          # Quick-start guide
├── spec.txt                    # Full architectural design document
├── tokenomics.txt              # Economic model specification
├── GAP_AUDIT.md                # Phase 1 gap audit results
├── pillars/
│   ├── eidos/                  # Proof-native language compiler
│   │   └── crates/             #   parser, verifier, kernel, erasure, codegen, flight-math, cli
│   ├── telos/                  # Execution & verification
│   │   ├── crates/             #   parser, ir, verifier, router, agent, codegen, lsp, cli
│   │   └── examples/           #   .telos invariant files (wallet, streaming_payment, tokenomics)
│   ├── koinon/                 # Settlement & economy
│   │   ├── koinon-ledger/      #   dual-token (OIKOS/Koin) account model + genesis + emission + elastic supply
│   │   ├── koinon-dag/         #   transaction DAG, parallel settlement with conflict detection
│   │   ├── koinon-mandates/    #   agent mandate management with time/contract enforcement
│   │   ├── koinon-escrow/      #   escrow (conditions, dispute, auth) + streaming payments
│   │   ├── koinon-gas/         #   deterministic gas pricing (base + steps*10 + storage*100)
│   │   ├── koinon-fee/         #   fee split (70/20/10 burn/validator/treasury), configurable
│   │   ├── koinon-staking/     #   validator staking (100K OIKOS min) + slashing logic
│   │   ├── koinon-rewards/     #   block reward processor, emission schedule integration
│   │   ├── koinon-treasury/    #   DAO treasury governance (proposals, voting, 67% quorum)
│   │   └── koinon-cli/         #   CLI binary (verify, tokenomics, gas, DAG)
│   ├── identity/               # Identity & mandate governance (Go module)
│   │   ├── cmd/                #   CLI entry points (serve, keygen, resolve, issue-vc, verify-vc)
│   │   ├── api/                #   HTTP handlers (identity, credentials, mandates, consents, OIDC)
│   │   ├── oidc/               #   OIDC provider (auth code, PKCE, refresh tokens, JWKS)
│   │   ├── internal/           #   store (SQLite), audit, authn, bridge, events
│   │   └── pkg/                #   did, vc, consent, crypto, recovery, schemas, etc.
│   ├── chora/                  # Human observation runtime
│   │   ├── chora-render/       #   wgpu render graph + triangle pipeline
│   │   ├── chora-ui/           #   dashboard UI primitives (panel, text, chart, status)
│   │   ├── chora-data/         #   DataStore + mandate/balance/stream models
│   │   └── chora-cli/          #   windowed renderer binary
│   └── archon/                 # Storage kernel substrate
│       └── crates/             #   core, bridge, kernel, relational, sql REPL
```

## Crate Count by Pillar

| Pillar | Crates | Tests |
|--------|--------|-------|
| tpt-eidos | 7 | ~100 |
| tpt-telos | 8 | ~150 |
| tpt-koinon | 10 | ~170 |
| tpt-chora | 4 | ~10 |
| tpt-archon | 5 | ~170 |
| **Total** | **34** | **~600** |

## Cross-Pillar Data Flow

```
Human ──► tpt-identity (DID/VC/mandate) ──► tpt-koinon (mandate enforcement)
                                              │
Agent ──► tpt-telos (verified contract) ──────┤
                                              │
                   tpt-eidos (verification) ◄─┤
                                              │
                   tpt-archon (storage) ◄──────┤
                                              │
                   tpt-chora (render) ◄────────┘
```

1. A human creates a DID via **tpt-identity**, which issues a Verifiable Credential representing an agent mandate (scope, budget, time-bound).
2. An AI agent authenticates via its DID and presents the mandate to **tpt-koinon**.
3. The agent writes a contract in **tpt-telos** with `@invariant`/`@requires`/`@ensures` annotations. The SMT verifier proves correctness before deployment.
4. **tpt-koinon** enforces the mandate budget constraint and settles transactions on its DAG ledger.
5. **tpt-eidos** provides compile-time verification of refinement types and flight-control primitives.
6. **tpt-archon** provides the zero-copy storage substrate shared by koinon, chora, and eidos.
7. **tpt-chora** renders live agent dashboards pulling state from koinon/archon.

## Dual-Token Model

- **OIKOS** — Governance & staking token. Fixed 1B supply, 18 decimals.
- **Koin** — Settlement & gas token. Elastic supply, i128 precision (38 decimals).

Fee split per transaction: 70% burned, 20% validators, 10% treasury.

## Tokenomics Pipeline

```
Genesis (40/30/20/10 split) → Block Rewards (emission schedule) → Fee Split (70/20/10)
                                                          ↓
                                   Staking (100K OIKOS min) ←→ Slashing
                                                          ↓
                                   Treasury (67% quorum) ←→ Public Goods
```

## Verification Pipeline

Every contract deployed to koinon must pass through tpt-telos verification:

```
.telos source → telos-parser → telos-ir (extract) → telos-verifier (QF_LRA) → pass/fail
```

The `koinon verify` CLI command gates deployment:
```bash
koinon verify path/to/contract.telos
koinon verify path/to/contract.telos --json  # machine-readable output
```

## Build & CI

```bash
# Rust workspace (all pillars except identity)
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check

# Go identity module
cd pillars/identity && go build ./... && go test ./...

# Verify a telos contract
cargo run -p tpt-telos -- verify pillars/telos/examples/wallet.telos

# Koinon CLI
cargo run -p koinon-cli -- tokenomics emission
cargo run -p koinon-cli -- tokenomics fee-split --amount 10000
cargo run -p koinon-cli -- gas --steps 5 --storage 100
```

See `.github/workflows/ci.yml` for the full CI pipeline.
