//! Data binding layer connecting koinon state to the chora presentation runtime.
//!
//! This crate provides the in-memory data models and aggregation layer used by
//! dashboard renderers to display live chain state. It stores three categories
//! of data:
//!
//! - **Mandates** — delegated spending authorizations between agents
//! - **Balances** — token holdings per agent
//! - **Streams** — continuous value flows between agents
//!
//! The [`DataStore`](store::DataStore) struct ties these together and exposes
//! filtered queries, aggregate metrics, and a pre-computed
//! [`DashboardSummary`](store::DashboardSummary) suitable for JSON transport.
//!
//! # Example
//!
//! ```rust
//! use chora_data::{DataStore, MandateState, Balance, StreamState};
//!
//! let mut store = DataStore::new();
//! store.upsert_mandate(MandateState::new("m1", "alice", "bob", 1000.0));
//! store.upsert_balance(Balance::new("bob", 500.0, "OIKOS"));
//! store.upsert_stream(StreamState::new("s1", "alice", "bob", 10.0));
//!
//! let summary = store.summary();
//! assert_eq!(summary.active_mandates, 1);
//! ```

pub mod mandate;
pub mod balance;
pub mod stream;
pub mod store;

pub use mandate::MandateState;
pub use balance::Balance;
pub use stream::StreamState;
pub use store::{DataStore, DashboardSummary};
