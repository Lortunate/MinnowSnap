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
