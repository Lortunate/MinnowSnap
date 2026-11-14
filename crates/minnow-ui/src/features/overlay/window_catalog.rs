use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WindowInfo {
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub app_name: String,
}

impl From<minnow_application::WindowInfo> for WindowInfo {
    fn from(window: minnow_application::WindowInfo) -> Self {
        Self {
            title: window.title,
            app_name: window.app_name,
            x: window.bounds.x.round() as i32,
            y: window.bounds.y.round() as i32,
            width: window.bounds.width.max(0.0).round() as u32,
            height: window.bounds.height.max(0.0).round() as u32,
        }
    }
}

#[must_use]
pub fn find_window_at(windows: &[WindowInfo], x: f64, y: f64) -> Option<usize> {
    windows
        .iter()
        .enumerate()
        .filter(|(_, w)| {
            let wx = f64::from(w.x);
            let wy = f64::from(w.y);
            let ww = f64::from(w.width);
            let wh = f64::from(w.height);
            x >= wx && x <= (wx + ww) && y >= wy && y <= (wy + wh)
        })
        .min_by_key(|(_, w)| u64::from(w.width) * u64::from(w.height))
        .map(|(i, _)| i)
}
