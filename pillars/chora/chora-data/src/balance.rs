//! Token balance tracking for the chora data layer.
//!
//! [`Balance`] represents an agent's holdings in a specific token denomination.
//! Balances are keyed by agent ID and support multiple tokens (e.g. OIKOS, Koin).

use serde::{Deserialize, Serialize};

/// An agent's token balance.
///
/// Tracks the `amount` of a specific `token` held by `agent_id`. The
/// `last_updated` field records when this balance was last refreshed from
/// on-chain state.
///
/// # Examples
///
/// ```rust
/// use chora_data::Balance;
///
/// let b = Balance::new("agent-did", 100.0, "OIKOS");
/// assert_eq!(b.agent_id, "agent-did");
/// assert_eq!(b.amount, 100.0);
/// assert_eq!(b.token, "OIKOS");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    /// DID of the agent holding this balance.
    pub agent_id: String,
    /// Current token amount.
    pub amount: f64,
    /// Token symbol (e.g. `"OIKOS"`, `"Koin"`).
    pub token: String,
    /// Unix timestamp of the last sync from on-chain state.
    pub last_updated: u64,
}

impl Balance {
    /// Creates a new balance with `last_updated` set to 0.
    ///
    /// # Arguments
    ///
    /// * `agent_id` — DID of the token holder
    /// * `amount` — initial token amount
    /// * `token` — token symbol
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chora_data::Balance;
    ///
    /// let b = Balance::new("alice", 250.5, "Koin");
    /// assert_eq!(b.amount, 250.5);
    /// assert_eq!(b.token, "Koin");
    /// assert_eq!(b.last_updated, 0);
    /// ```
    pub fn new(agent_id: &str, amount: f64, token: &str) -> Self {
        Self {
            agent_id: agent_id.into(),
            amount,
            token: token.into(),
            last_updated: 0,
        }
    }
}
