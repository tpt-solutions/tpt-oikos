use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub agent_id: String,
    pub amount: f64,
    pub token: String,
    pub last_updated: u64,
}

impl Balance {
    pub fn new(agent_id: &str, amount: f64, token: &str) -> Self {
        Self {
            agent_id: agent_id.into(),
            amount,
            token: token.into(),
            last_updated: 0,
        }
    }
}
