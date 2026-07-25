pub mod layout;
pub mod panel;
pub mod text;
pub mod chart;
pub mod status;

pub use layout::{Layout, Rect};
pub use panel::Panel;
pub use text::TextBlock;
pub use chart::Chart;
pub use status::StatusIndicator;

pub struct UiLayout {
    pub root: Option<Panel>,
}

impl UiLayout {
    pub fn new() -> Self {
        Self { root: None }
    }

    pub fn set_root(&mut self, panel: Panel) {
        self.root = Some(panel);
    }
}

impl Default for UiLayout {
    fn default() -> Self {
        Self::new()
    }
}
