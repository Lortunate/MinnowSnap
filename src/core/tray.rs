use crate::core::geometry::RectF;
use serde::Deserialize;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenData {
    pub virtual_x: f64,
    pub virtual_y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default = "default_device_pixel_ratio")]
    pub device_pixel_ratio: f64,
}

fn default_device_pixel_ratio() -> f64 {
    1.0
}

pub fn parse_screens(screens_json: &str) -> Vec<ScreenData> {
    serde_json::from_str(screens_json).unwrap_or_default()
}

pub fn parse_geometry(geometry_json: &str) -> Option<RectF> {
    let trimmed = geometry_json.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

pub fn centered_position(screen: ScreenData, menu_width: f64, menu_height: f64) -> (f64, f64) {
    (
        screen.virtual_x + (screen.width - menu_width) / 2.0,
        screen.virtual_y + (screen.height - menu_height) / 2.0,
    )
}

pub fn normalize_geometry(mut rect: RectF, screens: &[ScreenData], platform_os: &str) -> RectF {
    if platform_os != "windows" {
        return rect;
    }

    let is_logical = screens
        .iter()
        .any(|s| rect.x >= s.virtual_x && rect.x < s.virtual_x + s.width && rect.y >= s.virtual_y && rect.y < s.virtual_y + s.height);

    if is_logical {
        return rect;
    }

    for screen in screens {
        let dpr = if screen.device_pixel_ratio <= 0.0 {
            1.0
        } else {
            screen.device_pixel_ratio
        };
        let px = rect.x / dpr;
        let py = rect.y / dpr;
        if px >= screen.virtual_x && px < screen.virtual_x + screen.width && py >= screen.virtual_y && py < screen.virtual_y + screen.height {
            rect.x = px;
            rect.y = py;
            rect.width /= dpr;
            rect.height /= dpr;
            break;
        }
    }

    rect
}

pub fn find_screen(rect: RectF, screens: &[ScreenData]) -> Option<ScreenData> {
    let cx = rect.x + rect.width / 2.0;
    let cy = rect.y + rect.height / 2.0;
    screens
        .iter()
        .copied()
        .find(|s| cx >= s.virtual_x && cx < s.virtual_x + s.width && cy >= s.virtual_y && cy < s.virtual_y + s.height)
        .or_else(|| screens.first().copied())
}

pub fn calculate_position(rect: RectF, screen: ScreenData, menu_width: f64, menu_height: f64) -> (f64, f64) {
    let cy = rect.y + rect.height / 2.0;

    let mut target_x = rect.x;
    let mut target_y = if cy > screen.virtual_y + screen.height / 2.0 {
        rect.y - menu_height - 5.0
    } else {
        rect.y + rect.height + 5.0
    };

    let padding = 6.0;
    target_x = target_x.clamp(
        screen.virtual_x + padding,
        (screen.virtual_x + screen.width - menu_width - padding).max(screen.virtual_x + padding),
    );
    target_y = target_y.clamp(
        screen.virtual_y + padding,
        (screen.virtual_y + screen.height - menu_height - padding).max(screen.virtual_y + padding),
    );

    (target_x, target_y)
}
