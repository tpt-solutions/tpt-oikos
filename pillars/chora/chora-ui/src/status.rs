use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Active,
    Warning,
    Error,
    Inactive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusIndicator {
    pub label: String,
    pub status: Status,
    pub x: f32,
    pub y: f32,
}

impl StatusIndicator {
    pub fn new(label: &str, status: Status, x: f32, y: f32) -> Self {
        Self {
            label: label.into(),
            status,
            x,
            y,
        }
    }

    pub fn color(&self) -> [f32; 4] {
        match self.status {
            Status::Active => [0.2, 0.8, 0.3, 1.0],
            Status::Warning => [0.9, 0.7, 0.1, 1.0],
            Status::Error => [0.9, 0.2, 0.2, 1.0],
            Status::Inactive => [0.4, 0.4, 0.4, 1.0],
        }
    }
}
