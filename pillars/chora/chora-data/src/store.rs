//! Central data store for aggregating and querying live chain state.
//!
//! [`DataStore`] holds in-memory collections of mandates, balances, and streams,
//! providing CRUD operations, filtered queries, and aggregate metrics. Use
//! [`DataStore::summary`] to get a pre-computed [`DashboardSummary`] for
//! rendering.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::mandate::MandateState;
use crate::balance::Balance;
use crate::stream::StreamState;

/// Aggregated snapshot of all live chain state for rendering dashboards.
///
/// The `DataStore` is the central aggregation point in `chora-data`. It holds
/// `HashMap` collections of [`MandateState`], [`Balance`], and [`StreamState`]
/// entries, keyed by their respective IDs.
///
/// All data is held in-memory with no persistence layer. Timestamps are
/// managed externally via [`set_timestamp`](Self::set_timestamp) and read
/// via [`last_updated`](Self::last_updated).
///
/// # Examples
///
/// ```rust
/// use chora_data::{DataStore, MandateState, Balance, StreamState};
///
/// let mut store = DataStore::new();
///
/// // Add mandates, balances, and streams
/// store.upsert_mandate(MandateState::new("m1", "alice", "bob", 1000.0));
/// store.upsert_balance(Balance::new("alice", 500.0, "OIKOS"));
/// store.upsert_stream(StreamState::new("s1", "alice", "bob", 10.0));
///
/// // Query
/// assert_eq!(store.active_mandate_count(), 1);
/// assert_eq!(store.total_balance("OIKOS"), 500.0);
///
/// // Get summary
/// let summary = store.summary();
/// assert_eq!(summary.active_mandates, 1);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataStore {
    mandates: HashMap<String, MandateState>,
    balances: HashMap<String, Balance>,
    streams: HashMap<String, StreamState>,
    last_updated: u64,
}

impl DataStore {
    /// Creates a new empty `DataStore`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::DataStore;
    ///
    /// let store = DataStore::new();
    /// assert_eq!(store.mandate_count(), 0);
    /// assert_eq!(store.stream_count(), 0);
    /// assert!(store.all_balances().is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the last-updated timestamp (Unix seconds).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::DataStore;
    ///
    /// let mut store = DataStore::new();
    /// assert_eq!(store.last_updated(), 0);
    ///
    /// store.set_timestamp(1234567890);
    /// assert_eq!(store.last_updated(), 1234567890);
    /// ```
    pub fn last_updated(&self) -> u64 {
        self.last_updated
    }

    /// Sets the last-updated timestamp (Unix seconds).
    ///
    /// Typically called when syncing state from on-chain sources.
    ///
    /// # Arguments
    ///
    /// * `ts` — Unix timestamp in seconds
    pub fn set_timestamp(&mut self, ts: u64) {
        self.last_updated = ts;
    }

    // -- Mandates --

    /// Inserts or replaces a mandate in the store.
    ///
    /// If a mandate with the same `mandate_id` already exists, it is replaced.
    ///
    /// # Arguments
    ///
    /// * `mandate` — the [`MandateState`] to insert or update
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, MandateState};
    ///
    /// let mut store = DataStore::new();
    /// store.upsert_mandate(MandateState::new("m1", "alice", "bob", 1000.0));
    /// assert_eq!(store.mandate_count(), 1);
    /// ```
    pub fn upsert_mandate(&mut self, mandate: MandateState) {
        self.mandates.insert(mandate.mandate_id.clone(), mandate);
    }

    /// Removes a mandate by ID, returning it if it existed.
    ///
    /// # Arguments
    ///
    /// * `id` — the mandate ID to remove
    ///
    /// # Returns
    ///
    /// The removed [`MandateState`], or `None` if no mandate with that ID exists.
    pub fn remove_mandate(&mut self, id: &str) -> Option<MandateState> {
        self.mandates.remove(id)
    }

    /// Returns a reference to a mandate by ID.
    ///
    /// # Arguments
    ///
    /// * `id` — the mandate ID to look up
    ///
    /// # Returns
    ///
    /// A reference to the [`MandateState`], or `None` if not found.
    pub fn get_mandate(&self, id: &str) -> Option<&MandateState> {
        self.mandates.get(id)
    }

    /// Returns all active mandates.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, MandateState};
    ///
    /// let mut store = DataStore::new();
    /// store.upsert_mandate(MandateState::new("m1", "alice", "bob", 1000.0));
    /// store.upsert_mandate(MandateState::new("m2", "alice", "carol", 500.0));
    ///
    /// assert_eq!(store.active_mandates().len(), 2);
    /// ```
    pub fn active_mandates(&self) -> Vec<&MandateState> {
        self.mandates.values().filter(|m| m.active).collect()
    }

    /// Returns all mandates involving a given agent (as grantor or delegate).
    ///
    /// # Arguments
    ///
    /// * `agent_did` — DID of the agent to filter by
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, MandateState};
    ///
    /// let mut store = DataStore::new();
    /// store.upsert_mandate(MandateState::new("m1", "alice", "bob", 1000.0));
    /// store.upsert_mandate(MandateState::new("m2", "carol", "alice", 500.0));
    /// store.upsert_mandate(MandateState::new("m3", "bob", "carol", 300.0));
    ///
    /// // alice appears as agent_from in m1 and agent_to in m2
    /// assert_eq!(store.mandates_for_agent("alice").len(), 2);
    /// ```
    pub fn mandates_for_agent(&self, agent_did: &str) -> Vec<&MandateState> {
        self.mandates
            .values()
            .filter(|m| m.agent_from == agent_did || m.agent_to == agent_did)
            .collect()
    }

    /// Returns the average utilization across all active mandates.
    ///
    /// Utilization is computed per-mandate as `1.0 - (remaining / amount)`.
    /// Returns `0.0` if there are no active mandates.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, MandateState};
    ///
    /// let mut store = DataStore::new();
    /// store.upsert_mandate(MandateState::new("m1", "alice", "bob", 1000.0));
    ///
    /// assert_eq!(store.total_mandate_utilization(), 0.0);
    /// ```
    pub fn total_mandate_utilization(&self) -> f32 {
        let active: Vec<&MandateState> = self.active_mandates();
        if active.is_empty() {
            return 0.0;
        }
        let sum: f32 = active.iter().map(|m| m.utilization()).sum();
        sum / active.len() as f32
    }

    /// Returns the total number of mandates (active and inactive).
    pub fn mandate_count(&self) -> usize {
        self.mandates.len()
    }

    /// Returns the number of currently active mandates.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, MandateState};
    ///
    /// let mut store = DataStore::new();
    /// store.upsert_mandate(MandateState::new("m1", "alice", "bob", 1000.0));
    /// store.upsert_mandate(MandateState::new("m2", "alice", "carol", 500.0));
    ///
    /// assert_eq!(store.active_mandate_count(), 2);
    /// ```
    pub fn active_mandate_count(&self) -> usize {
        self.mandates.values().filter(|m| m.active).count()
    }

    // -- Balances --

    /// Inserts or replaces a balance in the store.
    ///
    /// Balances are keyed by `agent_id`. If an agent already has a balance
    /// record, it is replaced.
    ///
    /// # Arguments
    ///
    /// * `balance` — the [`Balance`] to insert or update
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, Balance};
    ///
    /// let mut store = DataStore::new();
    /// store.upsert_balance(Balance::new("alice", 100.0, "OIKOS"));
    /// assert_eq!(store.get_balance("alice").unwrap().amount, 100.0);
    /// ```
    pub fn upsert_balance(&mut self, balance: Balance) {
        self.balances.insert(balance.agent_id.clone(), balance);
    }

    /// Returns a reference to an agent's balance.
    ///
    /// # Arguments
    ///
    /// * `agent_id` — DID of the agent to look up
    ///
    /// # Returns
    ///
    /// A reference to the [`Balance`], or `None` if the agent has no balance record.
    pub fn get_balance(&self, agent_id: &str) -> Option<&Balance> {
        self.balances.get(agent_id)
    }

    /// Returns references to all stored balances.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, Balance};
    ///
    /// let mut store = DataStore::new();
    /// store.upsert_balance(Balance::new("alice", 100.0, "OIKOS"));
    /// store.upsert_balance(Balance::new("bob", 200.0, "Koin"));
    ///
    /// assert_eq!(store.all_balances().len(), 2);
    /// ```
    pub fn all_balances(&self) -> Vec<&Balance> {
        self.balances.values().collect()
    }

    /// Returns the total amount of a given token across all agents.
    ///
    /// # Arguments
    ///
    /// * `token` — token symbol to sum (e.g. `"OIKOS"`, `"Koin"`)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, Balance};
    ///
    /// let mut store = DataStore::new();
    /// store.upsert_balance(Balance::new("alice", 100.0, "OIKOS"));
    /// store.upsert_balance(Balance::new("bob", 200.0, "OIKOS"));
    /// store.upsert_balance(Balance::new("carol", 50.0, "Koin"));
    ///
    /// assert_eq!(store.total_balance("OIKOS"), 300.0);
    /// assert_eq!(store.total_balance("Koin"), 50.0);
    /// ```
    pub fn total_balance(&self, token: &str) -> f64 {
        self.balances
            .values()
            .filter(|b| b.token == token)
            .map(|b| b.amount)
            .sum()
    }

    // -- Streams --

    /// Inserts or replaces a stream in the store.
    ///
    /// If a stream with the same `stream_id` already exists, it is replaced.
    ///
    /// # Arguments
    ///
    /// * `stream` — the [`StreamState`] to insert or update
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, StreamState};
    ///
    /// let mut store = DataStore::new();
    /// store.upsert_stream(StreamState::new("s1", "alice", "bob", 10.0));
    /// assert_eq!(store.stream_count(), 1);
    /// ```
    pub fn upsert_stream(&mut self, stream: StreamState) {
        self.streams.insert(stream.stream_id.clone(), stream);
    }

    /// Removes a stream by ID, returning it if it existed.
    ///
    /// # Arguments
    ///
    /// * `id` — the stream ID to remove
    ///
    /// # Returns
    ///
    /// The removed [`StreamState`], or `None` if no stream with that ID exists.
    pub fn remove_stream(&mut self, id: &str) -> Option<StreamState> {
        self.streams.remove(id)
    }

    /// Returns a reference to a stream by ID.
    ///
    /// # Arguments
    ///
    /// * `id` — the stream ID to look up
    ///
    /// # Returns
    ///
    /// A reference to the [`StreamState`], or `None` if not found.
    pub fn get_stream(&self, id: &str) -> Option<&StreamState> {
        self.streams.get(id)
    }

    /// Returns all active streams.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, StreamState};
    ///
    /// let mut store = DataStore::new();
    /// store.upsert_stream(StreamState::new("s1", "alice", "bob", 10.0));
    /// store.upsert_stream(StreamState::new("s2", "carol", "alice", 20.0));
    ///
    /// assert_eq!(store.active_streams().len(), 2);
    /// ```
    pub fn active_streams(&self) -> Vec<&StreamState> {
        self.streams.values().filter(|s| s.active).collect()
    }

    /// Returns all streams involving a given agent (as payer or payee).
    ///
    /// # Arguments
    ///
    /// * `agent_id` — DID of the agent to filter by
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, StreamState};
    ///
    /// let mut store = DataStore::new();
    /// store.upsert_stream(StreamState::new("s1", "alice", "bob", 10.0));
    /// store.upsert_stream(StreamState::new("s2", "carol", "alice", 20.0));
    ///
    /// // alice appears as payer in s1 and payee in s2
    /// assert_eq!(store.streams_for_agent("alice").len(), 2);
    /// ```
    pub fn streams_for_agent(&self, agent_id: &str) -> Vec<&StreamState> {
        self.streams
            .values()
            .filter(|s| s.payer == agent_id || s.payee == agent_id)
            .collect()
    }

    /// Returns the sum of `rate_per_second` across all active streams.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, StreamState};
    ///
    /// let mut store = DataStore::new();
    /// store.upsert_stream(StreamState::new("s1", "alice", "bob", 10.0));
    /// store.upsert_stream(StreamState::new("s2", "carol", "alice", 20.0));
    ///
    /// assert_eq!(store.total_stream_rate(), 30.0);
    /// ```
    pub fn total_stream_rate(&self) -> f64 {
        self.active_streams()
            .iter()
            .map(|s| s.rate_per_second)
            .sum()
    }

    /// Returns the total number of streams (active and inactive).
    pub fn stream_count(&self) -> usize {
        self.streams.len()
    }

    /// Returns the number of currently active streams.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, StreamState};
    ///
    /// let mut store = DataStore::new();
    /// store.upsert_stream(StreamState::new("s1", "alice", "bob", 10.0));
    /// store.upsert_stream(StreamState::new("s2", "carol", "alice", 20.0));
    ///
    /// assert_eq!(store.active_stream_count(), 2);
    /// ```
    pub fn active_stream_count(&self) -> usize {
        self.streams.values().filter(|s| s.active).count()
    }

    // -- Dashboard summary --

    /// Returns a pre-computed [`DashboardSummary`] for rendering.
    ///
    /// Aggregates counts, rates, and balances into a single snapshot suitable
    /// for JSON transport to the dashboard frontend.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::{DataStore, MandateState, Balance, StreamState};
    ///
    /// let mut store = DataStore::new();
    /// store.set_timestamp(12345);
    /// store.upsert_mandate(MandateState::new("m1", "alice", "bob", 1000.0));
    /// store.upsert_balance(Balance::new("alice", 100.0, "OIKOS"));
    /// store.upsert_stream(StreamState::new("s1", "alice", "bob", 10.0));
    ///
    /// let summary = store.summary();
    /// assert_eq!(summary.active_mandates, 1);
    /// assert_eq!(summary.total_oikos, 100.0);
    /// assert_eq!(summary.last_updated, 12345);
    /// ```
    pub fn summary(&self) -> DashboardSummary {
        DashboardSummary {
            active_mandates: self.active_mandate_count(),
            total_mandates: self.mandate_count(),
            avg_mandate_utilization: self.total_mandate_utilization(),
            active_streams: self.active_stream_count(),
            total_streams: self.stream_count(),
            total_stream_rate: self.total_stream_rate(),
            total_oikos: self.total_balance("OIKOS"),
            total_koin: self.total_balance("Koin"),
            last_updated: self.last_updated,
        }
    }
}

/// A pre-computed summary for rendering the dashboard header.
///
/// Returned by [`DataStore::summary`]. Contains aggregate counts and totals
/// across all mandates, balances, and streams, ready for JSON serialization.
///
/// # Fields
///
/// * `active_mandates` — number of currently active mandates
/// * `total_mandates` — total number of mandates (active + inactive)
/// * `avg_mandate_utilization` — average utilization ratio across active mandates
/// * `active_streams` — number of currently active streams
/// * `total_streams` — total number of streams (active + inactive)
/// * `total_stream_rate` — combined rate per second across all active streams
/// * `total_oikos` — total OIKOS token balance across all agents
/// * `total_koin` — total Koin token balance across all agents
/// * `last_updated` — Unix timestamp of the last state refresh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    /// Number of currently active mandates.
    pub active_mandates: usize,
    /// Total number of mandates (active + inactive).
    pub total_mandates: usize,
    /// Average utilization ratio across active mandates.
    pub avg_mandate_utilization: f32,
    /// Number of currently active streams.
    pub active_streams: usize,
    /// Total number of streams (active + inactive).
    pub total_streams: usize,
    /// Combined rate per second across all active streams.
    pub total_stream_rate: f64,
    /// Total OIKOS token balance across all agents.
    pub total_oikos: f64,
    /// Total Koin token balance across all agents.
    pub total_koin: f64,
    /// Unix timestamp of the last state refresh.
    pub last_updated: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_mandate(id: &str, agent: &str, amount: f64) -> MandateState {
        MandateState::new(id, "grantor", agent, amount)
    }

    fn sample_balance(agent: &str, amount: f64, token: &str) -> Balance {
        Balance::new(agent, amount, token)
    }

    fn sample_stream(id: &str, payer: &str, payee: &str, rate: f64) -> StreamState {
        StreamState::new(id, payer, payee, rate)
    }

    #[test]
    fn test_store_new_is_empty() {
        let store = DataStore::new();
        assert_eq!(store.mandate_count(), 0);
        assert_eq!(store.stream_count(), 0);
        assert!(store.all_balances().is_empty());
    }

    #[test]
    fn test_mandate_crud() {
        let mut store = DataStore::new();
        store.upsert_mandate(sample_mandate("m1", "agent1", 1000.0));
        assert_eq!(store.mandate_count(), 1);
        assert!(store.get_mandate("m1").is_some());

        store.remove_mandate("m1");
        assert_eq!(store.mandate_count(), 0);
    }

    #[test]
    fn test_active_mandates() {
        let mut store = DataStore::new();
        store.upsert_mandate(sample_mandate("m1", "agent1", 1000.0));
        store.upsert_mandate(sample_mandate("m2", "agent2", 500.0));

        let mut m2 = store.get_mandate("m2").unwrap().clone();
        m2.active = false;
        store.upsert_mandate(m2);

        assert_eq!(store.active_mandate_count(), 1);
        assert_eq!(store.active_mandates()[0].mandate_id, "m1");
    }

    #[test]
    fn test_mandates_for_agent() {
        let mut store = DataStore::new();
        store.upsert_mandate(sample_mandate("m1", "agent1", 1000.0));
        store.upsert_mandate(sample_mandate("m2", "agent2", 500.0));
        store.upsert_mandate(sample_mandate("m3", "agent1", 300.0));

        let agent1 = store.mandates_for_agent("agent1");
        assert_eq!(agent1.len(), 2);
    }

    #[test]
    fn test_balance_crud() {
        let mut store = DataStore::new();
        store.upsert_balance(sample_balance("agent1", 100.0, "OIKOS"));
        assert_eq!(store.get_balance("agent1").unwrap().amount, 100.0);

        store.upsert_balance(sample_balance("agent1", 200.0, "OIKOS"));
        assert_eq!(store.get_balance("agent1").unwrap().amount, 200.0);
    }

    #[test]
    fn test_total_balance() {
        let mut store = DataStore::new();
        store.upsert_balance(sample_balance("a1", 100.0, "OIKOS"));
        store.upsert_balance(sample_balance("a2", 200.0, "OIKOS"));
        store.upsert_balance(sample_balance("a3", 50.0, "Koin"));

        assert_eq!(store.total_balance("OIKOS"), 300.0);
        assert_eq!(store.total_balance("Koin"), 50.0);
    }

    #[test]
    fn test_stream_crud() {
        let mut store = DataStore::new();
        store.upsert_stream(sample_stream("s1", "payer1", "payee1", 10.0));
        assert_eq!(store.stream_count(), 1);

        store.remove_stream("s1");
        assert_eq!(store.stream_count(), 0);
    }

    #[test]
    fn test_active_streams() {
        let mut store = DataStore::new();
        store.upsert_stream(sample_stream("s1", "p1", "q1", 10.0));
        store.upsert_stream(sample_stream("s2", "p2", "q2", 20.0));

        let mut s2 = store.get_stream("s2").unwrap().clone();
        s2.active = false;
        store.upsert_stream(s2);

        assert_eq!(store.active_stream_count(), 1);
        assert_eq!(store.total_stream_rate(), 10.0);
    }

    #[test]
    fn test_streams_for_agent() {
        let mut store = DataStore::new();
        store.upsert_stream(sample_stream("s1", "agent1", "agent2", 10.0));
        store.upsert_stream(sample_stream("s2", "agent3", "agent1", 20.0));
        store.upsert_stream(sample_stream("s3", "agent2", "agent3", 5.0));

        let agent1 = store.streams_for_agent("agent1");
        assert_eq!(agent1.len(), 2);
    }

    #[test]
    fn test_dashboard_summary() {
        let mut store = DataStore::new();
        store.set_timestamp(12345);
        store.upsert_mandate(sample_mandate("m1", "agent1", 1000.0));
        store.upsert_balance(sample_balance("agent1", 100.0, "OIKOS"));
        store.upsert_stream(sample_stream("s1", "p1", "q1", 10.0));

        let summary = store.summary();
        assert_eq!(summary.active_mandates, 1);
        assert_eq!(summary.total_mandates, 1);
        assert_eq!(summary.total_oikos, 100.0);
        assert_eq!(summary.active_streams, 1);
        assert_eq!(summary.total_stream_rate, 10.0);
        assert_eq!(summary.last_updated, 12345);
    }
}
