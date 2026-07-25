use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChartType {
    Line,
    Bar,
    Gauge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chart {
    pub label: String,
    pub chart_type: ChartType,
    pub data_points: Vec<f32>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Chart {
    pub fn new(label: &str, chart_type: ChartType, x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            label: label.into(),
            chart_type,
            data_points: Vec::new(),
            x,
            y,
            width: w,
            height: h,
        }
    }

    pub fn push_point(&mut self, value: f32) {
        self.data_points.push(value);
    }
}
