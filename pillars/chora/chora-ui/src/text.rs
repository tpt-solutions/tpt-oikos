use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    pub content: String,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub color: [f32; 4],
}

impl TextBlock {
    pub fn new(content: &str, x: f32, y: f32) -> Self {
        Self {
            content: content.into(),
            x,
            y,
            font_size: 14.0,
            color: [0.9, 0.9, 0.9, 1.0],
        }
    }

    pub fn with_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn with_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.color = [r, g, b, a];
        self
    }
}
