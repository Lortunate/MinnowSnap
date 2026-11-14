use std::f64::consts::PI;

use ab_glyph::{Font, ScaleFont};
use minnow_imaging::geometry::RectF;

use super::model::{AnnotationItem, AnnotationKind, AnnotationStyleState, AnnotationTool, MIN_DRAW_LENGTH};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrowGeometry {
    pub polygon: [(f64, f64); 7],
}

const ARROW_HIT_TOLERANCE: f64 = 6.0;

pub(crate) fn finite_point(point: (f64, f64)) -> bool {
    point.0.is_finite() && point.1.is_finite()
}

pub(crate) fn positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn finite_rect(rect: RectF) -> bool {
    finite_point((rect.x, rect.y))
        && rect.width.is_finite()
        && rect.width >= 0.0
        && rect.height.is_finite()
        && rect.height >= 0.0
        && finite_point((rect.x + rect.width, rect.y + rect.height))
}

pub(crate) fn valid_selection(rect: RectF) -> bool {
    finite_rect(rect) && rect.width > 0.0 && rect.height > 0.0
}

/// Size limits belong to the logical document, before any raster transform.
pub fn counter_radius(style: &AnnotationStyleState) -> f64 {
    style.counter_radius.clamp(10.0, 64.0)
}

pub fn text_size(style: &AnnotationStyleState) -> f64 {
    style.text_size.clamp(12.0, 96.0)
}

pub fn counter_text_size(style: &AnnotationStyleState, number: u32) -> f64 {
    // Keep even a multi-digit label inside its logical circle. The radius and
    // label limits are resolved together before raster scaling.
    let digits = number.to_string().len() as f64;
    text_size(style).min(60.0).min(counter_radius(style) * 1.1 / digits)
}

pub fn text_line_height(size: f64) -> f64 {
    size * 1.35
}

fn expanded_bounds(rect: RectF, margin: f64) -> RectF {
    RectF::new(rect.x - margin, rect.y - margin, rect.width + margin * 2.0, rect.height + margin * 2.0)
}

impl ArrowGeometry {
    pub fn bounds(&self) -> RectF {
        let (mut min_x, mut min_y) = self.polygon[0];
        let (mut max_x, mut max_y) = self.polygon[0];
        for &(x, y) in &self.polygon[1..] {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        RectF::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    fn contains_point(&self, point: (f64, f64), tolerance: f64) -> bool {
        let mut inside = false;
        let mut previous = self.polygon[self.polygon.len() - 1];
        for &current in &self.polygon {
            if distance_to_segment(point, previous, current) <= tolerance {
                return true;
            }
            if (current.1 > point.1) != (previous.1 > point.1)
                && point.0 < (previous.0 - current.0) * (point.1 - current.1) / (previous.1 - current.1) + current.0
            {
                inside = !inside;
            }
            previous = current;
        }
        inside
    }
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
    finite_point((x, y)).then_some((x, y))
}

pub fn text_layout(text: &str) -> (usize, usize) {
    let mut max_width = 0usize;
    let mut lines = 0usize;
    for line in text.lines() {
        lines += 1;
        max_width = max_width.max(line.chars().count());
    }
    if lines == 0 { (1, 1) } else { (max_width.max(1), lines.max(1)) }
}

fn normalize_draw_rect(start: (f64, f64), current: (f64, f64)) -> RectF {
    RectF::new(
        start.0.min(current.0),
        start.1.min(current.1),
        (current.0 - start.0).abs(),
        (current.1 - start.1).abs(),
    )
}

fn clamp_next(value: f64, delta: f64, min: f64, max: f64) -> f64 {
    (value + delta).clamp(min, max)
}

pub fn arrow_geometry(start: (f64, f64), end: (f64, f64), stroke_width: f64) -> Option<ArrowGeometry> {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len = dx.hypot(dy);
    if !finite_point(start) || !finite_point(end) || !len.is_finite() || len <= 1.0 {
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
    /// Whether this caller-constructible item has finite logical geometry and
    /// legal tool sizes. Empty rectangles are allowed while drawing a draft.
    pub fn is_valid(&self) -> bool {
        self.style.is_valid()
            && match self.kind {
                AnnotationKind::Arrow { start, end } => {
                    finite_point(start) && finite_point(end) && (end.0 - start.0).hypot(end.1 - start.1).is_finite()
                }
                AnnotationKind::Rectangle { rect } | AnnotationKind::Circle { rect } => finite_rect(rect),
                AnnotationKind::Mosaic { rect, intensity, .. } => finite_rect(rect) && (2.0..=64.0).contains(&intensity),
                AnnotationKind::Counter { center, .. } => finite_point(center),
                AnnotationKind::Text { origin, .. } => finite_point(origin),
            }
    }

    pub(crate) fn can_move_by(&self, dx: f64, dy: f64) -> bool {
        if !finite_point((dx, dy)) || !self.is_valid() {
            return false;
        }
        let moved = |point: (f64, f64)| (point.0 + dx, point.1 + dy);
        match self.kind {
            AnnotationKind::Arrow { start, end } => {
                let start = moved(start);
                let end = moved(end);
                finite_point(start) && finite_point(end) && (end.0 - start.0).hypot(end.1 - start.1).is_finite()
            }
            AnnotationKind::Rectangle { rect } | AnnotationKind::Circle { rect } | AnnotationKind::Mosaic { rect, .. } => {
                finite_rect(RectF::new(rect.x + dx, rect.y + dy, rect.width, rect.height))
            }
            AnnotationKind::Counter { center, .. } => finite_point(moved(center)),
            AnnotationKind::Text { origin, .. } => finite_point(moved(origin)),
        }
    }

    /// Compares the logical output, excluding selection identity and style
    /// fields that this shape does not render.
    pub(crate) fn output_eq(&self, other: &Self) -> bool {
        if self.kind != other.kind {
            return false;
        }
        let style = &self.style;
        let next = &other.style;
        match self.kind {
            AnnotationKind::Arrow { .. } => style.stroke_color == next.stroke_color && style.stroke_width == next.stroke_width,
            AnnotationKind::Rectangle { .. } | AnnotationKind::Circle { .. } => {
                style.stroke_color == next.stroke_color
                    && style.stroke_width == next.stroke_width
                    && style.fill_enabled == next.fill_enabled
                    && (!style.fill_enabled || style.fill_color == next.fill_color)
            }
            AnnotationKind::Counter { number, .. } => {
                style.stroke_color == next.stroke_color
                    && counter_radius(style) == counter_radius(next)
                    && counter_text_size(style, number) == counter_text_size(next, number)
            }
            AnnotationKind::Text { .. } => style.stroke_color == next.stroke_color && text_size(style) == text_size(next),
            AnnotationKind::Mosaic { .. } => true,
        }
    }

    /// Logical bounds without a font context. Engine outlines and hit testing
    /// additionally include actual glyph extents for the configured font.
    pub fn bounds(&self) -> RectF {
        self.bounds_with_font(None)
    }

    pub(crate) fn bounds_with_font(&self, font: Option<&ab_glyph::FontArc>) -> RectF {
        if !self.is_valid() {
            return RectF::default();
        }
        match &self.kind {
            AnnotationKind::Arrow { start, end } => arrow_geometry(*start, *end, self.style.stroke_width)
                .map(|geometry| geometry.bounds())
                .unwrap_or_else(|| RectF::new(start.0, start.1, 0.0, 0.0)),
            AnnotationKind::Rectangle { rect } | AnnotationKind::Circle { rect } | AnnotationKind::Mosaic { rect, .. } => *rect,
            AnnotationKind::Counter { center, .. } => {
                let r = counter_radius(&self.style);
                RectF::new(center.0 - r, center.1 - r, r * 2.0, r * 2.0)
            }
            AnnotationKind::Text { origin, text } => {
                let size = text_size(&self.style);
                let bounds = text_bounds(*origin, size, text);
                font.map_or(bounds, |font| text_bounds_with_font(*origin, size, text, font, bounds))
            }
        }
    }

    /// Broad-phase bounds enclose the same shape and tolerance as precise hit
    /// testing. Outlines continue to use the logical visual bounds above.
    pub fn hit_bounds(&self) -> RectF {
        self.hit_bounds_with_font(None)
    }

    pub(crate) fn hit_bounds_with_font(&self, font: Option<&ab_glyph::FontArc>) -> RectF {
        if !self.is_valid() {
            return RectF::default();
        }
        match self.kind {
            AnnotationKind::Arrow { .. } => expanded_bounds(self.bounds(), ARROW_HIT_TOLERANCE),
            _ => self.bounds_with_font(font),
        }
    }

    /// Applies a finite translation atomically. Invalid deltas or coordinates
    /// that would overflow leave the item unchanged.
    pub fn move_by(&mut self, dx: f64, dy: f64) -> bool {
        if !self.can_move_by(dx, dy) || (dx.abs() <= f64::EPSILON && dy.abs() <= f64::EPSILON) {
            return false;
        }
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
        true
    }

    pub fn resize_by_wheel(&mut self, delta_steps: f64) -> bool {
        if !delta_steps.is_finite() || !self.is_valid() {
            return false;
        }
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

    pub fn primary_metric(&self) -> f64 {
        match &self.kind {
            AnnotationKind::Arrow { .. } | AnnotationKind::Rectangle { .. } | AnnotationKind::Circle { .. } => self.style.stroke_width,
            AnnotationKind::Counter { .. } => counter_radius(&self.style),
            AnnotationKind::Text { .. } => text_size(&self.style),
            AnnotationKind::Mosaic { intensity, .. } => *intensity,
        }
    }
}

pub fn text_bounds(origin: (f64, f64), text_size: f64, text: &str) -> RectF {
    let text_size = text_size.clamp(12.0, 96.0);
    let (width_units, line_count) = text_layout(text);
    let width = (width_units as f64 * text_size).max(text_size * 1.8);
    let height = line_count as f64 * text_line_height(text_size);
    RectF::new(origin.0, origin.1 - text_size, width, height)
}

fn text_bounds_with_font(origin: (f64, f64), size: f64, text: &str, font: &ab_glyph::FontArc, fallback: RectF) -> RectF {
    let scaled = font.as_scaled(size as f32);
    let mut min_x = fallback.x;
    let mut min_y = fallback.y;
    let mut max_x = fallback.x + fallback.width;
    let mut max_y = fallback.y + fallback.height;
    for (line, text) in text.lines().enumerate() {
        let top = origin.1 - size + line as f64 * text_line_height(size);
        let mut advance = 0.0;
        let mut previous = None;
        for character in text.chars() {
            let id = scaled.glyph_id(character);
            let glyph = id.with_scale_and_position(size as f32, ab_glyph::point(advance, scaled.ascent()));
            advance += scaled.h_advance(id);
            if let Some(outline) = scaled.outline_glyph(glyph) {
                // Match imageproc's glyph placement, including overhangs.
                if let Some(previous) = previous {
                    advance += scaled.kern(id, previous);
                }
                previous = Some(id);
                let bounds = outline.px_bounds();
                min_x = min_x.min(origin.0 + f64::from(bounds.min.x));
                min_y = min_y.min(top + f64::from(bounds.min.y));
                max_x = max_x.max(origin.0 + f64::from(bounds.max.x));
                max_y = max_y.max(top + f64::from(bounds.max.y));
            }
        }
    }
    RectF::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

pub fn contains_point_with_bounds(item: &AnnotationItem, point: (f64, f64), bounds: RectF) -> bool {
    if !finite_point(point) || !item.is_valid() {
        return false;
    }
    match &item.kind {
        AnnotationKind::Arrow { start, end } => {
            arrow_geometry(*start, *end, item.style.stroke_width).is_some_and(|geometry| geometry.contains_point(point, ARROW_HIT_TOLERANCE))
        }
        AnnotationKind::Rectangle { rect } | AnnotationKind::Mosaic { rect, .. } => rect.contains_point(point.0, point.1),
        AnnotationKind::Circle { rect } => point_in_ellipse(point, *rect),
        AnnotationKind::Counter { center, .. } => {
            let dx = point.0 - center.0;
            let dy = point.1 - center.1;
            let radius = counter_radius(&item.style);
            dx * dx + dy * dy <= radius * radius
        }
        AnnotationKind::Text { .. } => bounds.contains_point(point.0, point.1),
    }
}

pub fn build_drawing_item(
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

    let item = AnnotationItem { id, style, kind };
    item.is_valid().then_some(item)
}

pub fn annotation_item_large_enough(item: &AnnotationItem, min_selection_size: f64) -> bool {
    if !positive_finite(min_selection_size) || !item.is_valid() {
        return false;
    }
    match &item.kind {
        AnnotationKind::Arrow { start, end } => {
            let length = (end.0 - start.0).hypot(end.1 - start.1);
            length >= MIN_DRAW_LENGTH
        }
        AnnotationKind::Rectangle { rect } | AnnotationKind::Circle { rect } | AnnotationKind::Mosaic { rect, .. } => {
            rect.width >= min_selection_size && rect.height >= min_selection_size
        }
        AnnotationKind::Counter { .. } | AnnotationKind::Text { .. } => true,
    }
}

pub fn sync_style_from_item(style: &mut AnnotationStyleState, item: &AnnotationItem) {
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

pub fn ensure_mosaic_kind_style(kind: &mut AnnotationKind, style: &AnnotationStyleState) {
    if let AnnotationKind::Mosaic { mode, intensity, .. } = kind {
        *mode = style.mosaic_mode;
        *intensity = style.mosaic_intensity;
    }
}
