use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MandateState {
    pub mandate_id: String,
    pub agent_from: String,
    pub agent_to: String,
    pub amount: f64,
    pub remaining: f64,
    pub active: bool,
    pub expires_at: Option<u64>,
}

impl MandateState {
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

    pub fn utilization(&self) -> f32 {
        if self.amount == 0.0 {
            0.0
        } else {
            (1.0 - (self.remaining / self.amount)) as f32
        }
    }
}
