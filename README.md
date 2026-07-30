# TPT Oikos

[![CI](https://github.com/tpt-solutions/tpt-oikos/actions/workflows/ci.yml/badge.svg)](https://github.com/tpt-solutions/tpt-oikos/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](#license)

TPT Oikos is a vertically integrated, proof-native compute fabric for the autonomous AI agent
economy. It unifies six pillars into a single monorepo: a proof-native language compiler, an
SMT-verified contract execution layer, a dual-token settlement ledger, a DID/VC identity service,
a zero-copy storage substrate, and a GPU-rendered observation runtime.

> **Status: pre-release / active development.** Core pillars (telos, koinon, identity, archon) are
> functionally mature with extensive test coverage; the unified node binary is currently an
> in-memory single-node simulation. There is no persistent state, no live P2P network, and no
> mainnet. See [SETUP.md](SETUP.md#whats-simulated-vs-real) for a precise breakdown of what's real
> vs. simulated, and [GAP_AUDIT.md](GAP_AUDIT.md) for open risks.

## Pillars

| Pillar | Role | Language | Status |
|--------|------|----------|--------|
| **eidos** | Proof-native language compiler — refinement types, QF_LRA verification | Rust | Mature MVK; flight-control domain library |
| **telos** | Execution & verification — SMT-verified contracts, dual Rust/Go codegen | Rust | v1.2, mature |
| **koinon** | Settlement & economy — DAG ledger, mandates, escrow, streaming, staking, treasury | Rust | Full implementation |
| **identity** | Identity & mandate governance — DIDs, VCs, OIDC, mandate CRUD | Go | Mature, full-featured |
| **chora** | Human observation runtime — wgpu-rendered dashboards, data binding | Rust | Early; GPU foundation + data store |
| **archon** | Storage kernel substrate — zero-copy storage, B-Link tree, MVCC, SQL engine | Rust | Mature; full database stack |

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full directory layout, cross-pillar data flow, and
dual-token model.

## Quick start

Prerequisites: [Rust](https://rustup.rs) 1.74+ and [Go](https://go.dev/dl/) 1.22+.

```bash
git clone https://github.com/tpt-solutions/tpt-oikos.git
cd tpt-oikos

# Build and test the Rust workspace (34 crates)
cargo build --workspace
cargo test --workspace

# Build and test the Go identity module
cd pillars/identity && go build ./... && go test ./... && cd ../..
```

Try it:

```bash
# Run the node simulation
cargo run -p tpt-oikos-node -- start

# Verify a formally-specified contract via the SMT solver
cargo run -p tpt-telos -- verify pillars/telos/examples/wallet.telos

# View tokenomics (emission schedule, fee split)
cargo run -p tpt-oikos-node -- tokenomics
```

## Documentation

| Doc | Covers |
|-----|--------|
| [GETTING_STARTED.md](GETTING_STARTED.md) | Guided walkthrough: core concepts, CLIs, writing your first verified contract |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Directory layout, cross-pillar data flow, dual-token model, verification pipeline |
| [SETUP.md](SETUP.md) | End-to-end practical setup, including the identity service and what's real vs. simulated |
| [DEPLOYMENT.md](DEPLOYMENT.md) | Single-node and multi-node deployment, Docker, configuration reference |
| [GAP_AUDIT.md](GAP_AUDIT.md) | Phase-by-phase build checklist and known open risks/gaps |
| [spec.txt](spec.txt) | Full architectural design document |
| [tokenomics.txt](tokenomics.txt) | Economic model specification |

## Development

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo build --workspace
cargo test --workspace
```

CI runs the same checks (plus `go vet`/`go test` for the identity module and the archon
verification harness) on every push and pull request — see
[.github/workflows/ci.yml](.github/workflows/ci.yml).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this project shall be dual licensed as above, without any additional terms or conditions.
