use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::mandate::MandateState;
use crate::balance::Balance;
use crate::stream::StreamState;

/// Aggregated snapshot of all live chain state for rendering dashboards.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DataStore {
    mandates: HashMap<String, MandateState>,
    balances: HashMap<String, Balance>,
    streams: HashMap<String, StreamState>,
    last_updated: u64,
}

impl DataStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn last_updated(&self) -> u64 {
        self.last_updated
    }

    pub fn set_timestamp(&mut self, ts: u64) {
        self.last_updated = ts;
    }

    // -- Mandates --

    pub fn upsert_mandate(&mut self, mandate: MandateState) {
        self.mandates.insert(mandate.mandate_id.clone(), mandate);
    }

    pub fn remove_mandate(&mut self, id: &str) -> Option<MandateState> {
        self.mandates.remove(id)
    }

    pub fn get_mandate(&self, id: &str) -> Option<&MandateState> {
        self.mandates.get(id)
    }

    pub fn active_mandates(&self) -> Vec<&MandateState> {
        self.mandates.values().filter(|m| m.active).collect()
    }

    pub fn mandates_for_agent(&self, agent_did: &str) -> Vec<&MandateState> {
        self.mandates
            .values()
            .filter(|m| m.agent_from == agent_did || m.agent_to == agent_did)
            .collect()
    }

    pub fn total_mandate_utilization(&self) -> f32 {
        let active: Vec<&MandateState> = self.active_mandates();
        if active.is_empty() {
            return 0.0;
        }
        let sum: f32 = active.iter().map(|m| m.utilization()).sum();
        sum / active.len() as f32
    }

    pub fn mandate_count(&self) -> usize {
        self.mandates.len()
    }

    pub fn active_mandate_count(&self) -> usize {
        self.mandates.values().filter(|m| m.active).count()
    }

    // -- Balances --

    pub fn upsert_balance(&mut self, balance: Balance) {
        self.balances.insert(balance.agent_id.clone(), balance);
    }

    pub fn get_balance(&self, agent_id: &str) -> Option<&Balance> {
        self.balances.get(agent_id)
    }

    pub fn all_balances(&self) -> Vec<&Balance> {
        self.balances.values().collect()
    }

    pub fn total_balance(&self, token: &str) -> f64 {
        self.balances
            .values()
            .filter(|b| b.token == token)
            .map(|b| b.amount)
            .sum()
    }

    // -- Streams --

    pub fn upsert_stream(&mut self, stream: StreamState) {
        self.streams.insert(stream.stream_id.clone(), stream);
    }

    pub fn remove_stream(&mut self, id: &str) -> Option<StreamState> {
        self.streams.remove(id)
    }

    pub fn get_stream(&self, id: &str) -> Option<&StreamState> {
        self.streams.get(id)
    }

    pub fn active_streams(&self) -> Vec<&StreamState> {
        self.streams.values().filter(|s| s.active).collect()
    }

    pub fn streams_for_agent(&self, agent_id: &str) -> Vec<&StreamState> {
        self.streams
            .values()
            .filter(|s| s.payer == agent_id || s.payee == agent_id)
            .collect()
    }

    pub fn total_stream_rate(&self) -> f64 {
        self.active_streams()
            .iter()
            .map(|s| s.rate_per_second)
            .sum()
    }

    pub fn stream_count(&self) -> usize {
        self.streams.len()
    }

    pub fn active_stream_count(&self) -> usize {
        self.streams.values().filter(|s| s.active).count()
    }

    // -- Dashboard summary --

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub active_mandates: usize,
    pub total_mandates: usize,
    pub avg_mandate_utilization: f32,
    pub active_streams: usize,
    pub total_streams: usize,
    pub total_stream_rate: f64,
    pub total_oikos: f64,
    pub total_koin: f64,
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
