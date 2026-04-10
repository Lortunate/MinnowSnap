use minnow_core::geometry::Rect;
use serde::{Deserialize, Serialize};
use tracing::info;
use xcap::{Monitor, Window};

const MIN_VIRTUAL_WIDTH: i32 = 1920;
const MIN_VIRTUAL_HEIGHT: i32 = 1080;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WindowInfo {
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub app_name: String,
}

#[must_use]
pub fn fetch_windows_data() -> Vec<WindowInfo> {
    let windows = Window::all().unwrap_or_default();
    let monitors = Monitor::all().unwrap_or_default();
    let active_target = minnow_core::capture::active_monitor_target();
    let scale_factor = active_target
        .map(|target| target.effective_scale())
        .or_else(|| monitors.first().and_then(|m| m.scale_factor().ok()))
        .filter(|scale| *scale > 0.0)
        .unwrap_or(1.0);
    info!(
        "Fetching window data, total windows found: {}, scale_factor: {}",
        windows.len(),
        scale_factor
    );

    let (screen_rect, offset_x, offset_y) = if let Some(target) = active_target {
        (target.rect(), target.x, target.y)
    } else if monitors.is_empty() {
        (
            Rect {
                x: 0,
                y: 0,
                width: 10000,
                height: 10000,
            },
            0,
            0,
        )
    } else {
        let (min_x, min_y, max_x, max_y) = monitors
            .iter()
            .fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |(min_x, min_y, max_x, max_y), m| {
                let x = m.x().unwrap_or(0);
                let y = m.y().unwrap_or(0);
                let w = i32::try_from(m.width().unwrap_or(0)).unwrap_or(0);
                let h = i32::try_from(m.height().unwrap_or(0)).unwrap_or(0);
                (min_x.min(x), min_y.min(y), max_x.max(x + w), max_y.max(y + h))
            });

        (
            Rect {
                x: min_x,
                y: min_y,
                width: (max_x - min_x).max(MIN_VIRTUAL_WIDTH),
                height: (max_y - min_y).max(MIN_VIRTUAL_HEIGHT),
            },
            0,
            0,
        )
    };

    let mut visible_rects: Vec<Rect> = Vec::with_capacity(windows.len());
    const SYSTEM_OVERLAYS: &[&str] = &["程序坞", "Dock", "Window Server", "Control Center", "Notification Center", "Spotlight"];

    let results: Vec<WindowInfo> = windows
        .into_iter()
        .filter(|w| !w.is_minimized().unwrap_or(true))
        .filter_map(|window| {
            let w = window.width().ok().filter(|&w| w > 0)?;
            let h = window.height().ok().filter(|&h| h > 0)?;
            let x = window.x().unwrap_or(0);
            let y = window.y().unwrap_or(0);
            let w_i32 = i32::try_from(w).ok()?;
            let h_i32 = i32::try_from(h).ok()?;

            let current_rect = Rect {
                x,
                y,
                width: w_i32,
                height: h_i32,
            };
            let valid_rect = current_rect.intersect(screen_rect)?;

            if visible_rects.iter().any(|blocker| valid_rect.is_inside(*blocker)) {
                return None;
            }

            let app_name = window.app_name().unwrap_or_else(|_| "Unknown".to_string());
            if !SYSTEM_OVERLAYS.contains(&app_name.as_str()) {
                visible_rects.push(valid_rect);
            }

            let logical_x = ((valid_rect.x - offset_x) as f32 / scale_factor) as i32;
            let logical_y = ((valid_rect.y - offset_y) as f32 / scale_factor) as i32;
            let logical_w = (valid_rect.width as f32 / scale_factor).max(1.0) as u32;
            let logical_h = (valid_rect.height as f32 / scale_factor).max(1.0) as u32;

            Some(WindowInfo {
                title: window.title().unwrap_or_else(|_| "Unknown".to_string()),
                x: logical_x,
                y: logical_y,
                width: logical_w,
                height: logical_h,
                app_name,
            })
        })
        .collect();

    info!("Filtered visible windows: {}", results.len());
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
