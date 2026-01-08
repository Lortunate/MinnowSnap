use log::debug;
use serde::{Deserialize, Serialize};
use xcap::{Monitor, Window};

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WindowInfo {
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub app_name: String,
}

impl Rect {
    #[must_use]
    #[inline]
    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);

        let s_x2 = i64::from(self.x) + i64::from(self.width);
        let o_x2 = i64::from(other.x) + i64::from(other.width);
        let x2 = s_x2.min(o_x2);

        let s_y2 = i64::from(self.y) + i64::from(self.height);
        let o_y2 = i64::from(other.y) + i64::from(other.height);
        let y2 = s_y2.min(o_y2);

        if x2 > i64::from(x1) && y2 > i64::from(y1) {
            Some(Rect {
                x: x1,
                y: y1,
                width: (x2 - i64::from(x1)) as u32,
                height: (y2 - i64::from(y1)) as u32,
            })
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub fn is_inside(&self, other: &Rect) -> bool {
        let s_x2 = i64::from(self.x) + i64::from(self.width);
        let s_y2 = i64::from(self.y) + i64::from(self.height);
        let o_x2 = i64::from(other.x) + i64::from(other.width);
        let o_y2 = i64::from(other.y) + i64::from(other.height);

        self.x >= other.x && self.y >= other.y && s_x2 <= o_x2 && s_y2 <= o_y2
    }
}

#[must_use]
pub fn fetch_windows_data() -> Vec<WindowInfo> {
    let windows = Window::all().unwrap_or_default();
    let monitors = Monitor::all().unwrap_or_default();
    debug!("Fetching window data, total windows found: {}", windows.len());

    let screen_rect = if monitors.is_empty() {
        Rect {
            x: 0,
            y: 0,
            width: 10000,
            height: 10000,
        }
    } else {
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for m in &monitors {
            let x = m.x().unwrap_or(0);
            let y = m.y().unwrap_or(0);
            let w = m.width().unwrap_or(0) as i32;
            let h = m.height().unwrap_or(0) as i32;

            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + w);
            max_y = max_y.max(y + h);
        }

        if max_x < min_x {
            max_x = min_x + 1920;
        }
        if max_y < min_y {
            max_y = min_y + 1080;
        }

        Rect {
            x: min_x,
            y: min_y,
            width: (max_x - min_x).try_into().unwrap_or(0),
            height: (max_y - min_y).try_into().unwrap_or(0),
        }
    };

    let mut results = Vec::with_capacity(windows.len());
    let mut visible_rects: Vec<Rect> = Vec::with_capacity(windows.len());

    const SYSTEM_OVERLAYS: &[&str] = &["程序坞", "Dock", "Window Server", "Control Center", "Notification Center", "Spotlight"];

    for window in windows {
        if window.is_minimized().unwrap_or(true) {
            continue;
        }

        let Ok(w) = window.width() else { continue };
        if w == 0 {
            continue;
        }
        let Ok(h) = window.height() else { continue };
        if h == 0 {
            continue;
        }

        let x = window.x().unwrap_or(0);
        let y = window.y().unwrap_or(0);

        let current_rect = Rect { x, y, width: w, height: h };

        let Some(valid_rect) = current_rect.intersect(&screen_rect) else {
            continue;
        };

        if visible_rects.iter().any(|blocker| valid_rect.is_inside(blocker)) {
            continue;
        }

        let app_name = window.app_name().unwrap_or_else(|_| "Unknown".to_string());
        let is_system_overlay = SYSTEM_OVERLAYS.contains(&app_name.as_str());

        if !is_system_overlay {
            visible_rects.push(current_rect);
        }

        let title = window.title().unwrap_or_else(|_| "Unknown".to_string());

        results.push(WindowInfo {
            title,
            x,
            y,
            width: w,
            height: h,
            app_name,
        });
    }

    debug!("Filtered visible windows: {}", results.len());
    results
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
