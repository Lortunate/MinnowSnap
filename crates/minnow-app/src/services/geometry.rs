use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct RectF {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl RectF {
    #[must_use]
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    #[must_use]
    pub fn contains_point(self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

impl Rect {
    #[must_use]
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self::new(0, 0, 0, 0)
    }

    #[must_use]
    pub const fn has_area(self) -> bool {
        self.width > 0 && self.height > 0
    }

    #[must_use]
    #[inline]
    pub fn intersect(self, other: Self) -> Option<Self> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);

        let s_x2 = i64::from(self.x) + i64::from(self.width.max(0));
        let o_x2 = i64::from(other.x) + i64::from(other.width.max(0));
        let x2 = s_x2.min(o_x2);

        let s_y2 = i64::from(self.y) + i64::from(self.height.max(0));
        let o_y2 = i64::from(other.y) + i64::from(other.height.max(0));
        let y2 = s_y2.min(o_y2);

        if x2 > i64::from(x1) && y2 > i64::from(y1) {
            Some(Self::new(x1, y1, (x2 - i64::from(x1)) as i32, (y2 - i64::from(y1)) as i32))
        } else {
            None
        }
    }

    #[must_use]
    #[inline]
    pub fn is_inside(self, other: Self) -> bool {
        let s_x2 = i64::from(self.x) + i64::from(self.width.max(0));
        let s_y2 = i64::from(self.y) + i64::from(self.height.max(0));
        let o_x2 = i64::from(other.x) + i64::from(other.width.max(0));
        let o_y2 = i64::from(other.y) + i64::from(other.height.max(0));

        self.x >= other.x && self.y >= other.y && s_x2 <= o_x2 && s_y2 <= o_y2
    }
}

pub struct NormalizedRect {
    pub cx: f64,
    pub cy: f64,
    pub width: f64,
    pub height: f64,
    pub angle: f64,
}

pub fn normalize_polygon(points: &[(i32, i32)], img_w: f64, img_h: f64) -> NormalizedRect {
    if points.len() != 4 {
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for (x, y) in points {
            if *x < min_x {
                min_x = *x;
            }
            if *x > max_x {
                max_x = *x;
            }
            if *y < min_y {
                min_y = *y;
            }
            if *y > max_y {
                max_y = *y;
            }
        }

        let x = min_x as f64;
        let y = min_y as f64;
        let w = (max_x - min_x) as f64;
        let h = (max_y - min_y) as f64;

        return NormalizedRect {
            cx: (x + w / 2.0) / img_w,
            cy: (y + h / 2.0) / img_h,
            width: w / img_w,
            height: h / img_h,
            angle: 0.0,
        };
    }

    let p0 = (points[0].0 as f64, points[0].1 as f64);
    let p1 = (points[1].0 as f64, points[1].1 as f64);
    let p2 = (points[2].0 as f64, points[2].1 as f64);
    let p3 = (points[3].0 as f64, points[3].1 as f64);

    let w_top = ((p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2)).sqrt();
    let w_bot = ((p2.0 - p3.0).powi(2) + (p2.1 - p3.1).powi(2)).sqrt();
    let w = (w_top + w_bot) / 2.0;

    let h_left = ((p3.0 - p0.0).powi(2) + (p3.1 - p0.1).powi(2)).sqrt();
    let h_right = ((p2.0 - p1.0).powi(2) + (p2.1 - p1.1).powi(2)).sqrt();
    let h = (h_left + h_right) / 2.0;

    let cx = (p0.0 + p1.0 + p2.0 + p3.0) / 4.0;
    let cy = (p0.1 + p1.1 + p2.1 + p3.1) / 4.0;

    let dx = p1.0 - p0.0;
    let dy = p1.1 - p0.1;
    let angle_rad = dy.atan2(dx);
    let angle_deg = angle_rad * 180.0 / PI;

    NormalizedRect {
        cx: cx / img_w,
        cy: cy / img_h,
        width: w / img_w,
        height: h / img_h,
        angle: angle_deg,
    }
}

pub fn clamp_point(x: f64, y: f64, screen_w: f64, screen_h: f64) -> (f64, f64) {
    if screen_w <= 0.0 || screen_h <= 0.0 {
        (x, y)
    } else {
        (x.clamp(0.0, screen_w), y.clamp(0.0, screen_h))
    }
}

pub fn clamp_rect_move(x: f64, y: f64, w: f64, h: f64, screen_w: f64, screen_h: f64) -> (f64, f64) {
    if screen_w <= 0.0 || screen_h <= 0.0 {
        (x, y)
    } else {
        let max_x = (screen_w - w).max(0.0);
        let max_y = (screen_h - h).max(0.0);
        (x.clamp(0.0, max_x), y.clamp(0.0, max_y))
    }
}

pub fn clamp_rect_resize(x: f64, y: f64, w: f64, h: f64, screen_w: f64, screen_h: f64) -> (f64, f64, f64, f64) {
    if screen_w <= 0.0 || screen_h <= 0.0 {
        (x, y, w, h)
    } else {
        let left = x.max(0.0);
        let top = y.max(0.0);
        let right = (x + w).min(screen_w);
        let bottom = (y + h).min(screen_h);

        let new_x = left;
        let new_y = top;
        let new_w = (right - left).max(0.0);
        let new_h = (bottom - top).max(0.0);
        (new_x, new_y, new_w, new_h)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn calculate_toolbar_position(
    target_x: f64,
    target_y: f64,
    target_w: f64,
    target_h: f64,
    item_w: f64,
    item_h: f64,
    is_above: bool,
    padding: f64,
    spacing_above: f64,
    spacing_below: f64,
    default_y: f64,
    screen_w: f64,
    screen_h: f64,
) -> (f64, f64) {
    let desired_x = target_x + target_w - item_w;
    let max_x = (screen_w - item_w - padding).max(padding);
    let x = desired_x.clamp(padding, max_x);

    let y = if is_above {
        let above_y = target_y - item_h - spacing_above;
        if above_y >= 0.0 { above_y } else { target_y + target_h + spacing_above }
    } else {
        let below_y = target_y + target_h + spacing_below;
        let above_y = target_y - item_h - spacing_below;
        if below_y + item_h <= screen_h {
            below_y
        } else if above_y >= 0.0 {
            above_y
        } else {
            default_y
        }
    };

    (x, y)
}

#[allow(clippy::too_many_arguments)]
pub fn calculate_resize(
    start_x: f64,
    start_y: f64,
    start_w: f64,
    start_h: f64,
    dx: f64,
    dy: f64,
    corner: &str,
    screen_w: f64,
    screen_h: f64,
) -> (f64, f64, f64, f64) {
    let mut new_x = start_x;
    let mut new_y = start_y;
    let mut new_w = start_w;
    let mut new_h = start_h;

    if corner.contains("right") {
        new_w += dx;
    } else if corner.contains("left") {
        new_x += dx;
        new_w -= dx;
    }

    if corner.contains("bottom") {
        new_h += dy;
    } else if corner.contains("top") {
        new_y += dy;
        new_h -= dy;
    }

    if new_w < 10.0 {
        if corner.contains("left") {
            new_x = start_x + start_w - 10.0;
        }
        new_w = 10.0;
    }
    if new_h < 10.0 {
        if corner.contains("top") {
            new_y = start_y + start_h - 10.0;
        }
        new_h = 10.0;
    }

    clamp_rect_resize(new_x, new_y, new_w, new_h, screen_w, screen_h)
}

pub fn normalize_rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    let nx = x.floor() as i32;
    let ny = y.floor() as i32;
    let nw = w.ceil().max(1.0) as i32;
    let nh = h.ceil().max(1.0) as i32;
    Rect::new(nx, ny, nw, nh)
}
