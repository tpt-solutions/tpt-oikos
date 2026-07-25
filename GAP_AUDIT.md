# TPT Oikos — Phase 1 Gap Audit

Generated from the per-pillar audit of each vendored repo against `spec.txt` and `tokenomics.txt`.

---

## tpt-eidos (Consensus Kernel)

**Verdict: Excellent language compiler; NOT a consensus kernel.**

tpt-eidos v0.1.0 is a proof-native, refinement-typed language compiler with zero external dependencies. It verifies division safety, refinement subtyping, and termination via a self-contained Fourier-Motzkin QF_LRA solver. It erases proofs to `no_std` Rust.

### Critical gaps vs spec:
- **No state machine** — no consensus, no validator model, no state transitions
- **No DAG hosting** — lives in tpt-koinon instead
- **No P2P networking** or distributed protocol of any kind
- **Effect types** parsed but completely ignored by the kernel
- **No linear/affine types** (spec calls them a core design pillar)
- **No cross-module verification** — single-file only
- **No verification certificate** export (spec requires machine-readable proof objects)

### Recommendations:
1. Re-scope: tpt-eidos = language/toolchain, NOT consensus protocol
2. Implement the effect system (highest-leverage next feature)
3. Model consensus invariants as eidos specifications verified by the existing toolchain
4. Add C codegen target (spec mentions "Rust/C binaries")

---

## tpt-telos (Execution & Verification)

**Verdict: Mature, closest to spec alignment.**

v1.2 with 8 crates, Phases 7-8 complete. Self-contained SMT solver (Fourier-Motzkin), `@requires`/`@ensures` contracts, Rust/Go dual-backend codegen, FFI bridge, LSP server.

### Gaps:
- No direct integration with tpt-koinon settlement primitives
- No `@invariant` examples for tokenomics-specific invariants (conservation-of-value, fee-conservation, mandate-solvency)
- The `StreamingPayment` example from spec §4 is not ported

### Recommendations:
1. Port the `StreamingPayment` example from spec §4 and verify it compiles/proves
2. Wire `telos verify` into koinon's contract deployment pipeline
3. Express tokenomics invariants as tpt-telos `@invariant` annotations

---

## tpt-koinon (Settlement & Economy)

**Verdict: Stubs with significant structural divergence from spec.**

### Critical gaps vs tokenomics.txt:

| Gap | Detail |
|-----|--------|
| **AgentMandate missing fields** | No `principal: DID`, no `agent: DID`, no `time_bound: Timestamp` |
| **Fee split wrong token type** | Uses `OikosAmount` but spec says fees are paid in **Koin** |
| **Elastic Koin supply** | Entirely absent — zero implementation |
| **StreamingPayment missing invariants** | No `conservation_of_value` or `stream_rate_valid` checks |
| **No runtime invariant enforcement** | Spec says invariants "verified at runtime by tpt-eidos" — nothing exists |
| **TotalValueConservation too narrow** | Only tracks minted/burned for OIKOS; missing koin, treasury, staked, circulating |
| **Gas base cost mismatch** | 100 vs spec's 1000 |
| **Settlement is status-flipping only** | No actual state transitions, fee collection, or balance changes |
| **DAG has no cycle detection** | `DagError::CycleDetected` defined but never triggered |

### Recommendations:
1. Add `principal: DID`, `agent: DID`, `time_bound: Timestamp` to AgentMandate
2. Change FeeSplit to use KoinAmount
3. Implement the elastic Koin supply algorithm (Section 4 of tokenomics.txt)
4. Add `withdraw()` and `deposit()` to StreamingPayment
5. Expand TotalValueConservation to the full dual-token equation
6. Make struct fields private; enforce invariants through controlled mutation
7. Wire settlement to execute actual state transitions

---

## tpt-identity (Identity & Mandate Governance)

**Verdict: Mature SSI platform; zero mandate-specific functionality.**

Full DID layer (web/key/peer/ion), W3C VCs, SD-JWT, OIDC provider, identity bridging, Shamir recovery, duress codes, SHA-256 audit log, 50+ credential schemas.

### Critical gaps:

| Gap | Detail |
|-----|--------|
| **No agent DID type** | `Role` is "user"/"operator"/"admin" — no "agent" role |
| **No delegation chain** | No `ControllerDID`, no human->agent mandate relationship |
| **No mandate credential schema** | 50+ schemas, none for agent mandates |
| **No mandate CRUD API** | No endpoints for create/list/revoke/freeze mandates |
| **Duress doesn't freeze mandates** | `session.duress` event fires but no mandate deactivation |
| **No bridge to koinon's AgentMandate** | No shared data model, no integration |

### Recommendations:
1. Add "agent" role/identity type with `ControllerDID` field
2. Create `agent.mandate-delegation` credential schema matching koinon's AgentMandate
3. Implement mandate CRUD API (POST/GET/PATCH/DELETE /api/v1/mandates)
4. Wire duress code to freeze all active agent mandates
5. Issue mandate-aware SD-JWTs with budget/scope claims

---

## tpt-chora (Human Observation Runtime)

**Verdict: Clean wgpu scaffold; zero functional implementation.**

~681 lines of Rust across 16 files. Triangle render pipeline, basic UI primitives (panel, text, chart, status), data binding structs. No security guarantees met.

### Critical gaps:
- **No interaction** — spec says "presentation AND interaction"; current code only renders
- **No tamper-proof data feed** — no cryptographic authentication of rendered state
- **No agent dashboard** — cannot display mandates, balances, or streaming payments
- **No contract state rendering** — cannot display smart contract state
- **UI primitives are inert** — `contains_point()` hit-tests exist but are never called

### Recommendations:
1. Phase 1: Just render live data from koinon (mandate state, balances)
2. Add input handling (mouse clicks, keyboard) for interactive dashboards
3. Add cryptographic state authentication (block hash / state root verification)
4. Treat Phase 3 chora timelines as optimistic — re-scope after Phase 1

---

## Cross-Cutting Gaps

1. **No DID type in koinon** — everything is `String` or `AccountId(u64)`. The spec's `DID` type doesn't exist as a first-class type anywhere in the settlement layer.
2. **No Timestamp type** — raw `u64` values with no semantic meaning.
3. **No tpt-telos annotations in koinon** — the spec repeatedly references `@invariant`/`@requires`/`@ensures` but the entire settlement codebase is standard Rust with no proof infrastructure.
4. **pub fields everywhere in koinon** — allows external code to bypass invariant checks.
5. **tpt-archon's verify harness** depends on git-pinned versions of eidos-verifier and telos-* that won't match the vendored copies.
