use crate::ui::pin::session::PinFrame;
use gpui::{Bounds, Path, PathBuilder, Pixels, Point, Size, Window, point, px, size};

#[derive(Clone)]
pub(super) struct OcrBlockGeometry {
    pub(super) index: usize,
    pub(super) center: Point<Pixels>,
    pub(super) width: Pixels,
    pub(super) height: Pixels,
    angle_rad: f32,
}

pub(super) fn compute_block_geometries(frame: &PinFrame, viewport: Size<Pixels>) -> (Bounds<Pixels>, Vec<OcrBlockGeometry>) {
    let painted = painted_bounds(viewport, frame.base_size);
    let mut geometries = Vec::with_capacity(frame.ocr.blocks.len());
    for (index, block) in frame.ocr.blocks.iter().enumerate() {
        let width = px((block.width * painted.size.width.to_f64()) as f32);
        let height = px((block.height * painted.size.height.to_f64()) as f32);
        if width <= px(0.0) || height <= px(0.0) {
            continue;
        }
        let center = point(
            painted.origin.x + px((block.cx * painted.size.width.to_f64()) as f32),
            painted.origin.y + px((block.cy * painted.size.height.to_f64()) as f32),
        );
        geometries.push(OcrBlockGeometry {
            index,
            center,
            width,
            height,
            angle_rad: (block.angle as f32).to_radians(),
        });
    }
    (painted, geometries)
}

fn painted_bounds(viewport: Size<Pixels>, image_size: (f32, f32)) -> Bounds<Pixels> {
    let viewport_w = viewport.width.to_f64() as f32;
    let viewport_h = viewport.height.to_f64() as f32;
    let viewport_w = viewport_w.max(1.0);
    let viewport_h = viewport_h.max(1.0);
    let image_w = image_size.0.max(1.0);
    let image_h = image_size.1.max(1.0);
    let image_ratio = image_w / image_h;
    let viewport_ratio = viewport_w / viewport_h;

    if viewport_ratio > image_ratio {
        let painted_h = viewport_h;
        let painted_w = painted_h * image_ratio;
        let x = (viewport_w - painted_w) / 2.0;
        Bounds::new(point(px(x), px(0.0)), size(px(painted_w), px(painted_h)))
    } else {
        let painted_w = viewport_w;
        let painted_h = painted_w / image_ratio;
        let y = (viewport_h - painted_h) / 2.0;
        Bounds::new(point(px(0.0), px(y)), size(px(painted_w), px(painted_h)))
    }
}

pub(super) fn hit_test_block(pointer: Point<Pixels>, geometries: &[OcrBlockGeometry]) -> Option<&OcrBlockGeometry> {
    geometries.iter().rev().find(|geometry| point_in_rotated_rect(pointer, geometry))
}

fn point_in_rotated_rect(pointer: Point<Pixels>, geometry: &OcrBlockGeometry) -> bool {
    let dx = pixels_to_f32(pointer.x) - pixels_to_f32(geometry.center.x);
    let dy = pixels_to_f32(pointer.y) - pixels_to_f32(geometry.center.y);
    let cos = geometry.angle_rad.cos();
    let sin = geometry.angle_rad.sin();
    let local_x = dx * cos + dy * sin;
    let local_y = -dx * sin + dy * cos;
    local_x.abs() <= pixels_to_f32(geometry.width) / 2.0 && local_y.abs() <= pixels_to_f32(geometry.height) / 2.0
}

pub(super) fn point_to_char_index(pointer: Point<Pixels>, geometry: &OcrBlockGeometry, text: &str) -> usize {
    let char_count = text.chars().count();
    if char_count == 0 {
        return 0;
    }
    let dx = pixels_to_f32(pointer.x) - pixels_to_f32(geometry.center.x);
    let dy = pixels_to_f32(pointer.y) - pixels_to_f32(geometry.center.y);
    let cos = geometry.angle_rad.cos();
    let sin = geometry.angle_rad.sin();
    let local_x = dx * cos + dy * sin;
    let half_width = pixels_to_f32(geometry.width) / 2.0;
    let normalized = ((local_x + half_width) / (half_width * 2.0)).clamp(0.0, 1.0);
    (normalized * char_count as f32).floor().clamp(0.0, char_count as f32) as usize
}

pub(super) fn paint_rotated_rect(window: &mut Window, geometry: &OcrBlockGeometry, fill: gpui::Hsla, border: gpui::Hsla) {
    if let Some(path) = build_rotated_rect_path(geometry) {
        if fill.a > 0.0 {
            window.paint_path(path.clone(), fill);
        }
        if border.a > 0.0 {
            window.paint_path(path, border);
        }
    }
}

pub(super) fn paint_rotated_stroke(window: &mut Window, geometry: &OcrBlockGeometry, color: gpui::Hsla, width: Pixels) {
    if let Some(path) = build_rotated_rect_stroke_path(geometry, width) {
        window.paint_path(path, color);
    }
}

fn build_rotated_rect_path(geometry: &OcrBlockGeometry) -> Option<Path<Pixels>> {
    let corners = rotated_corners(geometry);
    let mut builder = PathBuilder::fill();
    builder.move_to(corners[0]);
    builder.line_to(corners[1]);
    builder.line_to(corners[2]);
    builder.line_to(corners[3]);
    builder.close();
    builder.build().ok()
}

fn build_rotated_rect_stroke_path(geometry: &OcrBlockGeometry, width: Pixels) -> Option<Path<Pixels>> {
    let corners = rotated_corners(geometry);
    let mut builder = PathBuilder::stroke(width);
    builder.move_to(corners[0]);
    builder.line_to(corners[1]);
    builder.line_to(corners[2]);
    builder.line_to(corners[3]);
    builder.line_to(corners[0]);
    builder.build().ok()
}

fn rotated_corners(geometry: &OcrBlockGeometry) -> [Point<Pixels>; 4] {
    let half_w = pixels_to_f32(geometry.width) / 2.0;
    let half_h = pixels_to_f32(geometry.height) / 2.0;
    let cos = geometry.angle_rad.cos();
    let sin = geometry.angle_rad.sin();
    let local = [(-half_w, -half_h), (half_w, -half_h), (half_w, half_h), (-half_w, half_h)];
    local.map(|(lx, ly)| {
        let x = pixels_to_f32(geometry.center.x) + lx * cos - ly * sin;
        let y = pixels_to_f32(geometry.center.y) + lx * sin + ly * cos;
        point(px(x), px(y))
    })
}

pub(super) fn sub_geometry_by_ratio(base: &OcrBlockGeometry, start_ratio: f32, end_ratio: f32) -> OcrBlockGeometry {
    let start_ratio = start_ratio.clamp(0.0, 1.0);
    let end_ratio = end_ratio.clamp(0.0, 1.0);
    let ratio_min = start_ratio.min(end_ratio);
    let ratio_max = start_ratio.max(end_ratio);
    let ratio_width = (ratio_max - ratio_min).max(0.0);
    let width = px(pixels_to_f32(base.width) * ratio_width);
    let center_shift = (ratio_min + ratio_max - 1.0) * 0.5 * pixels_to_f32(base.width);
    let cos = base.angle_rad.cos();
    let sin = base.angle_rad.sin();
    let center = point(base.center.x + px(center_shift * cos), base.center.y + px(center_shift * sin));
    OcrBlockGeometry {
        index: base.index,
        center,
        width,
        height: base.height,
        angle_rad: base.angle_rad,
    }
}

pub(super) fn bounds_from_points(a: Point<Pixels>, b: Point<Pixels>) -> Bounds<Pixels> {
    let min_x = a.x.min(b.x);
    let min_y = a.y.min(b.y);
    let max_x = a.x.max(b.x);
    let max_y = a.y.max(b.y);
    Bounds::new(point(min_x, min_y), size(max_x - min_x, max_y - min_y))
}

pub(super) fn point_in_bounds(point_value: Point<Pixels>, bounds: Bounds<Pixels>) -> bool {
    let right = bounds.origin.x + bounds.size.width;
    let bottom = bounds.origin.y + bounds.size.height;
    point_value.x >= bounds.origin.x && point_value.x <= right && point_value.y >= bounds.origin.y && point_value.y <= bottom
}

fn pixels_to_f32(value: Pixels) -> f32 {
    value.to_f64() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(index: usize, center: (f32, f32), size_px: (f32, f32), angle_deg: f32) -> OcrBlockGeometry {
        OcrBlockGeometry {
            index,
            center: point(px(center.0), px(center.1)),
            width: px(size_px.0),
            height: px(size_px.1),
            angle_rad: angle_deg.to_radians(),
        }
    }

    #[test]
    fn rotated_hit_test_detects_inside_and_outside() {
        let geom = geometry(0, (100.0, 100.0), (80.0, 20.0), 35.0);
        assert!(point_in_rotated_rect(point(px(100.0), px(100.0)), &geom));
        assert!(!point_in_rotated_rect(point(px(180.0), px(180.0)), &geom));
    }

    #[test]
    fn char_index_mapping_clamps_at_bounds() {
        let geom = geometry(0, (100.0, 100.0), (100.0, 20.0), 0.0);
        let text = "abcdef";
        assert_eq!(point_to_char_index(point(px(50.0), px(100.0)), &geom, text), 0);
        assert_eq!(point_to_char_index(point(px(150.0), px(100.0)), &geom, text), 6);
    }

    #[test]
    fn selection_bounds_include_center_points() {
        let rect = bounds_from_points(point(px(10.0), px(20.0)), point(px(30.0), px(60.0)));
        assert!(point_in_bounds(point(px(20.0), px(40.0)), rect));
        assert!(!point_in_bounds(point(px(40.0), px(40.0)), rect));
    }
}
