# TPT Oikos — Architecture

TPT Oikos is a vertically integrated, proof-native compute fabric for the autonomous AI agent economy. It unifies five pillars (plus a storage substrate) into a single monorepo.

## Pillar Map

| Pillar | Role | Language | Status |
|--------|------|----------|--------|
| **tpt-eidos** | Consensus kernel — dependently-typed state machine | Rust | MVK (flight-control); no DAG/P2P yet |
| **tpt-telos** | Execution & verification — SMT-verified contracts | Rust | v1.2, mature |
| **tpt-koinon** | Settlement & economy — DAG ledger, mandates, escrow, streaming | Rust | New — stubs from spec |
| **tpt-identity** | Identity & mandate governance — DIDs, VCs, OIDC | Go | Mature, full-featured |
| **tpt-chora** | Human observation runtime — wgpu-rendered dashboards | Rust | New — triangle demo scaffold |
| **tpt-archon** | Storage kernel substrate — zero-copy storage engine | Rust | Early but functional |

## Directory Layout

```
tpt-oikos/
├── Cargo.toml              # Unified Rust workspace (all Rust pillars)
├── pillars/
│   ├── eidos/              # Consensus kernel
│   │   └── crates/         #   parser, verifier, kernel, erasure, codegen, cli
│   ├── telos/              # Execution & verification
│   │   └── crates/         #   parser, ir, verifier, router, agent, codegen, lsp, cli
│   ├── koinon/             # Settlement & economy
│   │   ├── koinon-ledger/  #   dual-token (OIKOS/Koin) account model
│   │   ├── koinon-dag/     #   transaction DAG, parallel settlement
│   │   ├── koinon-mandates/#   agent mandate management
│   │   ├── koinon-escrow/  #   escrow + streaming payments
│   │   ├── koinon-gas/     #   deterministic gas pricing
│   │   ├── koinon-fee/     #   fee split (70/20/10 burn/validator/treasury)
│   │   └── koinon-cli/     #   CLI binary
│   ├── identity/           # Identity & mandate governance (Go module)
│   │   ├── cmd/            #   CLI entry points
│   │   ├── api/            #   HTTP handlers
│   │   ├── oidc/           #   OIDC provider
│   │   ├── internal/       #   store, audit, authn, bridge, events
│   │   └── pkg/            #   did, vc, consent, crypto, recovery, etc.
│   ├── chora/              # Human observation runtime
│   │   ├── chora-render/   #   wgpu render graph + triangle pipeline
│   │   ├── chora-ui/       #   dashboard UI primitives
│   │   ├── chora-data/     #   koinon state bindings
│   │   └── chora-cli/      #   windowed renderer binary
│   └── archon/             # Storage kernel substrate
│       └── crates/         #   core, bridge, kernel, relational, sql
├── spec.txt                # Full architectural design document
└── tokenomics.txt          # Economic model specification
```

## Cross-Pillar Data Flow

```
Human ──► tpt-identity (DID/VC/mandate) ──► tpt-koinon (mandate enforcement)
                                              │
Agent ──► tpt-telos (verified contract) ──────┤
                                              │
                   tpt-eidos (consensus) ◄────┤
                                              │
                   tpt-archon (storage) ◄─────┤
                                              │
                   tpt-chora (render) ◄───────┘
```

1. A human creates a DID via **tpt-identity**, which issues a Verifiable Credential representing an agent mandate (scope, budget, time-bound).
2. An AI agent authenticates via its DID and presents the mandate to **tpt-koinon**.
3. The agent writes a contract in **tpt-telos** with `@invariant`/`@requires`/`@ensures` annotations. The SMT verifier proves correctness before deployment.
4. **tpt-koinon** enforces the mandate budget constraint and settles transactions on its DAG ledger.
5. **tpt-eidos** provides the consensus state machine (future: DAG consensus, P2P networking).
6. **tpt-archon** provides the zero-copy storage substrate shared by koinon, chora, and eidos.
7. **tpt-chora** renders live agent dashboards pulling state from koinon/archon.

## Dual-Token Model

- **OIKOS** — Governance & staking token. Fixed 1B supply, 18 decimals.
- **Koin** — Settlement & gas token. Elastic supply, i128 precision (38 decimals).

Fee split per transaction: 70% burned, 20% validators, 10% treasury.

## Build & CI

```bash
# Rust workspace (all pillars except identity)
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check

# Go identity module
cd pillars/identity && go build ./... && go test ./...
```

See `.github/workflows/ci.yml` for the full CI pipeline.
