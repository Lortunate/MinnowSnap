use std::f64::consts::PI;

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
