use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamState {
    pub stream_id: String,
    pub payer: String,
    pub payee: String,
    pub rate_per_second: f64,
    pub total_flow: f64,
    pub active: bool,
    pub started_at: u64,
}

impl StreamState {
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
