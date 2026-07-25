use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Panel {
    pub title: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub children: Vec<PanelChild>,
    pub border_color: [f32; 4],
    pub background_color: [f32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PanelChild {
    Text(String),
    Chart(String),
    Status(String),
    Nested(Panel),
}

impl Panel {
    pub fn new(title: &str, x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            title: title.into(),
            x,
            y,
            width,
            height,
            children: Vec::new(),
            border_color: [0.3, 0.3, 0.4, 1.0],
            background_color: [0.05, 0.05, 0.1, 0.9],
        }
    }

    pub fn with_child(mut self, child: PanelChild) -> Self {
        self.children.push(child);
        self
    }

    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }
}
