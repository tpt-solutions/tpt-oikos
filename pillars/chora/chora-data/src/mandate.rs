//! Mandate state tracking for the chora data layer.
//!
//! A mandate is a delegated spending authorization where one agent (the grantor)
//! allocates a portion of their budget for another agent (the delegate) to spend.
//! [`MandateState`] tracks the original amount, remaining balance, and utilization.

use serde::{Deserialize, Serialize};

/// A delegated spending authorization between two agents.
///
/// Represents a mandate where `agent_from` (the grantor) has authorized
/// `agent_to` (the delegate) to spend up to `amount` tokens. The `remaining`
/// field tracks how much of the authorization is left, and `utilization()`
/// computes the fraction already spent.
///
/// Mandates can optionally expire (`expires_at`) and can be deactivated by
/// setting `active` to `false`.
///
/// # Examples
///
/// ```rust
/// use chora_data::MandateState;
///
/// let mandate = MandateState::new("m1", "grantor-did", "agent-did", 1000.0);
/// assert_eq!(mandate.remaining, 1000.0);
/// assert_eq!(mandate.utilization(), 0.0);
/// assert!(mandate.active);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandateState {
    /// Unique identifier for this mandate.
    pub mandate_id: String,
    /// DID of the agent granting the mandate (budget owner).
    pub agent_from: String,
    /// DID of the agent receiving the mandate (delegate).
    pub agent_to: String,
    /// Original authorized amount in tokens.
    pub amount: f64,
    /// Remaining amount that can still be spent.
    pub remaining: f64,
    /// Whether this mandate is currently active.
    pub active: bool,
    /// Optional expiration timestamp (Unix seconds). `None` means no expiry.
    pub expires_at: Option<u64>,
}

impl MandateState {
    /// Creates a new active mandate with the full amount remaining.
    ///
    /// # Arguments
    ///
    /// * `mandate_id` — unique identifier for this mandate
    /// * `agent_from` — DID of the grantor (budget owner)
    /// * `agent_to` — DID of the delegate (spender)
    /// * `amount` — authorized spending limit in tokens
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::MandateState;
    ///
    /// let m = MandateState::new("m1", "alice", "bob", 500.0);
    /// assert_eq!(m.amount, 500.0);
    /// assert_eq!(m.remaining, 500.0);
    /// assert!(m.active);
    /// assert!(m.expires_at.is_none());
    /// ```
    pub fn new(mandate_id: &str, agent_from: &str, agent_to: &str, amount: f64) -> Self {
        Self {
            mandate_id: mandate_id.into(),
            agent_from: agent_from.into(),
            agent_to: agent_to.into(),
            amount,
            remaining: amount,
            active: true,
            expires_at: None,
        }
    }

    /// Returns the utilization ratio as a value between 0.0 and 1.0.
    ///
    /// Computed as `1.0 - (remaining / amount)`. Returns `0.0` when the
    /// mandate amount is zero to avoid division by zero.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::MandateState;
    ///
    /// let mut m = MandateState::new("m1", "alice", "bob", 1000.0);
    /// assert_eq!(m.utilization(), 0.0);
    ///
    /// m.remaining = 500.0;
    /// assert_eq!(m.utilization(), 0.5);
    ///
    /// m.remaining = 0.0;
    /// assert_eq!(m.utilization(), 1.0);
    /// ```
    pub fn utilization(&self) -> f32 {
        if self.amount == 0.0 {
            0.0
        } else {
            (1.0 - (self.remaining / self.amount)) as f32
        }
    }
}
