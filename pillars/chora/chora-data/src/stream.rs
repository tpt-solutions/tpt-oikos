//! Stream state tracking for the chora data layer.
//!
//! A stream is a continuous value flow from one agent to another at a fixed rate
//! per second. [`StreamState`] tracks the flow parameters and cumulative total.

use serde::{Deserialize, Serialize};

/// A continuous value stream between two agents.
///
/// Represents a payment stream where `payer` sends tokens to `payee` at
/// `rate_per_second`. The `total_flow` field accumulates the total amount
/// transferred since the stream started.
///
/// Streams can be stopped by setting `active` to `false`, which freezes the
/// stream state.
///
/// # Examples
///
/// ```rust
/// use chora_data::StreamState;
///
/// let s = StreamState::new("s1", "payer-did", "payee-did", 10.0);
/// assert_eq!(s.rate_per_second, 10.0);
/// assert_eq!(s.total_flow, 0.0);
/// assert!(s.active);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamState {
    /// Unique identifier for this stream.
    pub stream_id: String,
    /// DID of the agent sending tokens.
    pub payer: String,
    /// DID of the agent receiving tokens.
    pub payee: String,
    /// Token flow rate per second.
    pub rate_per_second: f64,
    /// Cumulative total amount transferred since stream start.
    pub total_flow: f64,
    /// Whether this stream is currently active.
    pub active: bool,
    /// Unix timestamp when the stream was created.
    pub started_at: u64,
}

impl StreamState {
    /// Creates a new active stream with zero total flow.
    ///
    /// # Arguments
    ///
    /// * `stream_id` — unique identifier for this stream
    /// * `payer` — DID of the token sender
    /// * `payee` — DID of the token receiver
    /// * `rate_per_second` — flow rate in tokens per second
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::StreamState;
    ///
    /// let s = StreamState::new("s1", "alice", "bob", 5.5);
    /// assert_eq!(s.payer, "alice");
    /// assert_eq!(s.payee, "bob");
    /// assert_eq!(s.rate_per_second, 5.5);
    /// assert_eq!(s.total_flow, 0.0);
    /// assert!(s.active);
    /// assert_eq!(s.started_at, 0);
    /// ```
    pub fn new(stream_id: &str, payer: &str, payee: &str, rate_per_second: f64) -> Self {
        Self {
            stream_id: stream_id.into(),
            payer: payer.into(),
            payee: payee.into(),
            rate_per_second,
            total_flow: 0.0,
            active: true,
            started_at: 0,
        }
    }
}
