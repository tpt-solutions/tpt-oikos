# chora-data

Data binding layer connecting koinon state to the presentation runtime.

## Overview

`chora-data` is the data layer for the chora human observation runtime. It stores mandates, balances, and streams in an in-memory `DataStore`, providing aggregated queries and pre-computed summaries suitable for dashboard rendering.

The crate models three core concepts from the koinon protocol:

- **Mandates** — delegated spending authorizations between agents, with tracked utilization
- **Balances** — token holdings per agent (e.g. OIKOS, Koin)
- **Streams** — continuous value flows between agents at a fixed rate per second

All types are `serde`-serializable, making them suitable for JSON transport to frontend renderers.

## Quick Start

```rust
use chora_data::{DataStore, MandateState, Balance, StreamState};

// Create an empty store
let mut store = DataStore::new();

// Add a mandate
let mandate = MandateState::new("m1", "grantor-did", "agent-did", 1000.0);
store.upsert_mandate(mandate);

// Add a balance
let balance = Balance::new("agent-did", 500.0, "OIKOS");
store.upsert_balance(balance);

// Add a stream
let stream = StreamState::new("s1", "payer-did", "payee-did", 10.0);
store.upsert_stream(stream);

// Get a dashboard summary
let summary = store.summary();
println!("Active mandates: {}", summary.active_mandates);
```

## Data Models

### MandateState

A spending authorization that delegates a portion of one agent's budget to another. Tracks the original `amount`, the `remaining` balance, and whether the mandate is still `active`. Supports utilization calculation as a ratio of spent vs. total.

### Balance

Represents an agent's holdings in a specific token. Each balance is keyed by `agent_id` and stores the current `amount` and `token` symbol.

### StreamState

A continuous value flow from a `payer` to a `payee` at a fixed `rate_per_second`. Tracks cumulative `total_flow` and whether the stream is currently `active`.

## DataStore

`DataStore` is the central aggregation point. It holds `HashMap` collections of all mandates, balances, and streams, and provides:

- **CRUD operations** — `upsert_mandate`, `remove_mandate`, `get_mandate`, and equivalents for balances and streams
- **Filtered queries** — `active_mandates`, `mandates_for_agent`, `streams_for_agent`, `total_balance`
- **Aggregate metrics** — `total_mandate_utilization`, `total_stream_rate`

All data is held in-memory. Timestamps are managed via `set_timestamp` / `last_updated`.

## Dashboard Summary

Call `store.summary()` to get a `DashboardSummary` with pre-computed values:

```rust
let summary = store.summary();
// summary.active_mandates: usize
// summary.total_mandates: usize
// summary.avg_mandate_utilization: f32
// summary.active_streams: usize
// summary.total_streams: usize
// summary.total_stream_rate: f64
// summary.total_oikos: f64
// summary.total_koin: f64
// summary.last_updated: u64
```

This is designed to be serialized directly to JSON for the dashboard header.

## Testing

```bash
cargo test -p chora-data
```

The test suite covers CRUD operations, filtered queries, aggregate metrics, and dashboard summary generation.
