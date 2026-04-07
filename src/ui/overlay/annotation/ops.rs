use std::f64::consts::PI;

use crate::core::geometry::{RectF, normalize_rect};

use super::model::{AnnotationItem, AnnotationKind, AnnotationStyleState, AnnotationTool, MIN_DRAW_LENGTH};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ArrowGeometry {
    pub polygon: [(f64, f64); 7],
}

fn point_in_ellipse(point: (f64, f64), rect: RectF) -> bool {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return false;
    }
    let cx = rect.x + rect.width / 2.0;
    let cy = rect.y + rect.height / 2.0;
    let rx = rect.width / 2.0;
    let ry = rect.height / 2.0;
    let dx = (point.0 - cx) / rx;
    let dy = (point.1 - cy) / ry;
    dx * dx + dy * dy <= 1.0
}

fn distance_to_segment(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> f64 {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    if dx.abs() < f64::EPSILON && dy.abs() < f64::EPSILON {
        return ((point.0 - start.0).powi(2) + (point.1 - start.1).powi(2)).sqrt();
    }
    let t = (((point.0 - start.0) * dx + (point.1 - start.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    let proj_x = start.0 + t * dx;
    let proj_y = start.1 + t * dy;
    ((point.0 - proj_x).powi(2) + (point.1 - proj_y).powi(2)).sqrt()
}

fn line_intersection(a1: (f64, f64), a2: (f64, f64), b1: (f64, f64), b2: (f64, f64)) -> Option<(f64, f64)> {
    let d = (a1.0 - a2.0) * (b1.1 - b2.1) - (a1.1 - a2.1) * (b1.0 - b2.0);
    if d.abs() <= f64::EPSILON {
        return None;
    }

    let a = a1.0 * a2.1 - a1.1 * a2.0;
    let b = b1.0 * b2.1 - b1.1 * b2.0;

    let x = (a * (b1.0 - b2.0) - (a1.0 - a2.0) * b) / d;
    let y = (a * (b1.1 - b2.1) - (a1.1 - a2.1) * b) / d;
    Some((x, y))
}

pub(crate) fn text_layout(text: &str) -> (usize, usize) {
    let mut max_width = 0usize;
    let mut lines = 0usize;
    for line in text.lines() {
        lines += 1;
        max_width = max_width.max(line.chars().count());
    }
    if lines == 0 { (1, 1) } else { (max_width.max(1), lines.max(1)) }
}

fn normalize_draw_rect(start: (f64, f64), current: (f64, f64)) -> RectF {
    let rect = normalize_rect(
        start.0.min(current.0),
        start.1.min(current.1),
        (current.0 - start.0).abs(),
        (current.1 - start.1).abs(),
    );
    RectF::new(rect.x as f64, rect.y as f64, rect.width as f64, rect.height as f64)
}

fn clamp_next(value: f64, delta: f64, min: f64, max: f64) -> f64 {
    (value + delta).clamp(min, max)
}

pub(crate) fn arrow_geometry(start: (f64, f64), end: (f64, f64), stroke_width: f64) -> Option<ArrowGeometry> {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 1.0 {
        return None;
    }

    let ux = dx / len;
    let uy = dy / len;
    let angle = dy.atan2(dx);
    let head_length = (12.0 + stroke_width * 3.0).clamp(8.0, 42.0);
    let arrow_angle = PI / 7.0;
    let indent_ratio = 0.2;
    let indent_dist = head_length * (1.0 - indent_ratio);
    let inner = (end.0 - indent_dist * ux, end.1 - indent_dist * uy);

    let nx = -uy;
    let ny = ux;
    let half_width = (stroke_width / 2.0).max(0.5);
    let tail_radius = (half_width * 0.1).max(0.5);

    let b1 = (inner.0 + nx * half_width, inner.1 + ny * half_width);
    let b2 = (inner.0 - nx * half_width, inner.1 - ny * half_width);
    let t1 = (start.0 + nx * tail_radius, start.1 + ny * tail_radius);
    let t2 = (start.0 - nx * tail_radius, start.1 - ny * tail_radius);

    let w1 = (
        end.0 - head_length * (angle - arrow_angle).cos(),
        end.1 - head_length * (angle - arrow_angle).sin(),
    );
    let w2 = (
        end.0 - head_length * (angle + arrow_angle).cos(),
        end.1 - head_length * (angle + arrow_angle).sin(),
    );

    let i1 = line_intersection(t1, b1, inner, w1).unwrap_or(b1);
    let i2 = line_intersection(t2, b2, inner, w2).unwrap_or(b2);

    Some(ArrowGeometry {
        polygon: [t1, i1, w1, end, w2, i2, t2],
    })
}

impl AnnotationItem {
    pub(crate) fn bounds(&self) -> RectF {
        match &self.kind {
            AnnotationKind::Arrow { start, end } => {
                let x = start.0.min(end.0);
                let y = start.1.min(end.1);
                let width = (end.0 - start.0).abs();
                let height = (end.1 - start.1).abs();
                RectF::new(x, y, width, height)
            }
            AnnotationKind::Rectangle { rect } | AnnotationKind::Circle { rect } | AnnotationKind::Mosaic { rect, .. } => *rect,
            AnnotationKind::Counter { center, .. } => {
                let r = self.style.counter_radius.max(8.0);
                RectF::new(center.0 - r, center.1 - r, r * 2.0, r * 2.0)
            }
            AnnotationKind::Text { origin, text } => text_bounds(*origin, self.style.text_size, text),
        }
    }

    pub(crate) fn move_by(&mut self, dx: f64, dy: f64) {
        match &mut self.kind {
            AnnotationKind::Arrow { start, end } => {
                start.0 += dx;
                start.1 += dy;
                end.0 += dx;
                end.1 += dy;
            }
            AnnotationKind::Rectangle { rect } | AnnotationKind::Circle { rect } | AnnotationKind::Mosaic { rect, .. } => {
                rect.x += dx;
                rect.y += dy;
            }
            AnnotationKind::Counter { center, .. } => {
                center.0 += dx;
                center.1 += dy;
            }
            AnnotationKind::Text { origin, .. } => {
                origin.0 += dx;
                origin.1 += dy;
            }
        }
    }

    pub(crate) fn resize_by_wheel(&mut self, delta_steps: f64) -> bool {
        let prev_style = self.style;
        let prev_kind = self.kind.clone();

        match &mut self.kind {
            AnnotationKind::Arrow { .. } | AnnotationKind::Rectangle { .. } | AnnotationKind::Circle { .. } => {
                self.style.stroke_width = clamp_next(self.style.stroke_width, delta_steps, 1.0, 18.0);
            }
            AnnotationKind::Counter { .. } => {
                self.style.counter_radius = clamp_next(self.style.counter_radius, delta_steps * 4.0, 10.0, 64.0);
            }
            AnnotationKind::Text { .. } => {
                self.style.text_size = clamp_next(self.style.text_size, delta_steps * 2.0, 12.0, 96.0);
            }
            AnnotationKind::Mosaic { intensity, .. } => {
                *intensity = clamp_next(*intensity, delta_steps * 2.0, 2.0, 64.0);
                self.style.mosaic_intensity = *intensity;
            }
        }

        prev_style != self.style || prev_kind != self.kind
    }

    pub(crate) fn primary_metric(&self) -> f64 {
        match &self.kind {
            AnnotationKind::Arrow { .. } | AnnotationKind::Rectangle { .. } | AnnotationKind::Circle { .. } => self.style.stroke_width,
            AnnotationKind::Counter { .. } => self.style.counter_radius,
            AnnotationKind::Text { .. } => self.style.text_size,
            AnnotationKind::Mosaic { intensity, .. } => *intensity,
        }
    }
}

pub(crate) fn text_bounds(origin: (f64, f64), text_size: f64, text: &str) -> RectF {
    let (width_units, line_count) = text_layout(text);
    let width = (width_units as f64 * text_size * 0.58).max(text_size * 1.8);
    let height = line_count as f64 * text_size * 1.35;
    RectF::new(origin.0, origin.1 - text_size, width, height)
}

pub(crate) fn contains_point_with_bounds(item: &AnnotationItem, point: (f64, f64), bounds: RectF) -> bool {
    match &item.kind {
        AnnotationKind::Arrow { start, end } => distance_to_segment(point, *start, *end) <= item.style.stroke_width.max(3.0) + 6.0,
        AnnotationKind::Rectangle { rect } | AnnotationKind::Mosaic { rect, .. } => rect.contains_point(point.0, point.1),
        AnnotationKind::Circle { rect } => point_in_ellipse(point, *rect),
        AnnotationKind::Counter { center, .. } => {
            let dx = point.0 - center.0;
            let dy = point.1 - center.1;
            let radius = item.style.counter_radius.max(8.0);
            dx * dx + dy * dy <= radius * radius
        }
        AnnotationKind::Text { .. } => bounds.contains_point(point.0, point.1),
    }
}

pub(crate) fn build_drawing_item(
    tool: AnnotationTool,
    start: (f64, f64),
    current: (f64, f64),
    style: AnnotationStyleState,
    id: u64,
) -> Option<AnnotationItem> {
    let kind = match tool {
        AnnotationTool::Arrow => AnnotationKind::Arrow { start, end: current },
        AnnotationTool::Rectangle => AnnotationKind::Rectangle {
            rect: normalize_draw_rect(start, current),
        },
        AnnotationTool::Circle => AnnotationKind::Circle {
            rect: normalize_draw_rect(start, current),
        },
        AnnotationTool::Mosaic => AnnotationKind::Mosaic {
            rect: normalize_draw_rect(start, current),
            mode: style.mosaic_mode,
            intensity: style.mosaic_intensity,
        },
        AnnotationTool::Counter | AnnotationTool::Text => return None,
    };

    Some(AnnotationItem { id, style, kind })
}

pub(crate) fn annotation_item_large_enough(item: &AnnotationItem, min_selection_size: f64) -> bool {
    match &item.kind {
        AnnotationKind::Arrow { start, end } => {
            let length = ((end.0 - start.0).powi(2) + (end.1 - start.1).powi(2)).sqrt();
            length >= MIN_DRAW_LENGTH
        }
        AnnotationKind::Rectangle { rect } | AnnotationKind::Circle { rect } | AnnotationKind::Mosaic { rect, .. } => {
            rect.width >= min_selection_size && rect.height >= min_selection_size
        }
        AnnotationKind::Counter { .. } | AnnotationKind::Text { .. } => true,
    }
}

pub(crate) fn sync_style_from_item(style: &mut AnnotationStyleState, item: &AnnotationItem) {
    style.stroke_color = item.style.stroke_color;
    style.fill_color = item.style.fill_color;
    style.fill_enabled = item.style.fill_enabled;
    style.stroke_width = item.style.stroke_width;
    style.text_size = item.style.text_size;
    style.counter_radius = item.style.counter_radius;
    match &item.kind {
        AnnotationKind::Mosaic { mode, intensity, .. } => {
            style.mosaic_mode = *mode;
            style.mosaic_intensity = *intensity;
        }
        _ => {
            style.mosaic_mode = item.style.mosaic_mode;
            style.mosaic_intensity = item.style.mosaic_intensity;
        }
    }
}

pub(crate) fn ensure_mosaic_kind_style(kind: &mut AnnotationKind, style: &AnnotationStyleState) {
    if let AnnotationKind::Mosaic { mode, intensity, .. } = kind {
        *mode = style.mosaic_mode;
        *intensity = style.mosaic_intensity;
    }
}
