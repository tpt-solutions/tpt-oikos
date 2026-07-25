# tpt-koinon — Settlement & AI Economy Layer

Part of the [TPT Oikos](https://github.com/AnarKitted/tpt-oikos) protocol.

## Overview

koinon is the settlement and AI economy layer. It provides:

- **Append-only Transaction DAG** — parallel settlement of nano-denominated transactions
- **Dual-token model** — OIKOS (governance, fixed 1 B supply, 18 decimals) + Koin (settlement, i128 precision, elastic supply)
- **Conservation-of-value invariants** — built-in accounting guarantees
- **Agent mandates** — DID-linked budget enforcement and scope checking
- **Escrow & streaming payments** — primitive building blocks for agent-to-agent commerce
- **Deterministic gas & fees** — transparent pricing with a 70/20/10 burn/validator/treasury split

## Workspace Crates

| Crate | Purpose |
|---|---|
| `koinon-ledger` | Core ledger types, account model, dual-token balances |
| `koinon-dag` | Transaction DAG structure, parallel settlement |
| `koinon-mandates` | AgentMandate, budget enforcement, scope checking |
| `koinon-escrow` | Escrow and streaming payment primitives |
| `koinon-gas` | Deterministic gas pricing |
| `koinon-fee` | Fee split logic (burn / validator / treasury) |
| `koinon-cli` | CLI binary |

## Build

```bash
cargo build
```

## MSRV

Rust **1.74** (edition 2021, resolver v2).

## License

Apache-2.0 OR MIT
