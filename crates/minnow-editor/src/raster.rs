use image::imageops;
use image::{Rgba, RgbaImage};
use imageproc::drawing::{
    draw_filled_circle_mut, draw_filled_ellipse_mut, draw_filled_rect_mut, draw_hollow_ellipse_mut, draw_hollow_rect_mut, draw_polygon_mut,
    draw_text_mut,
};
use imageproc::point::Point;
use imageproc::rect::Rect as ImageRect;

use minnow_imaging::ResolvedSelection;
use minnow_imaging::geometry::RectF;

use super::model::{AnnotationItem, AnnotationKind, MosaicMode};
use super::ops::{arrow_geometry, counter_radius, counter_text_size, finite_point, positive_finite, text_line_height, text_size};

fn rgba_from_u32(value: u32) -> Rgba<u8> {
    let r = ((value >> 24) & 0xff) as u8;
    let g = ((value >> 16) & 0xff) as u8;
    let b = ((value >> 8) & 0xff) as u8;
    let a = (value & 0xff) as u8;
    Rgba([r, g, b, a])
}

fn to_image_xy(point: (f64, f64), scale: f64, offset: (f64, f64)) -> (f64, f64) {
    (point.0 * scale - offset.0, point.1 * scale - offset.1)
}

fn clamp_image_rect(image: &RgbaImage, rect: RectF, scale: f64, offset: (f64, f64)) -> Option<ImageRect> {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return None;
    }
    let x = (rect.x * scale - offset.0).floor() as i32;
    let y = (rect.y * scale - offset.1).floor() as i32;
    let w = (rect.width * scale).round().max(1.0) as i32;
    let h = (rect.height * scale).round().max(1.0) as i32;

    let min_x = x.max(0);
    let min_y = y.max(0);
    let max_x = (x + w).min(image.width() as i32);
    let max_y = (y + h).min(image.height() as i32);
    if max_x <= min_x || max_y <= min_y {
        return None;
    }
    Some(ImageRect::at(min_x, min_y).of_size((max_x - min_x) as u32, (max_y - min_y) as u32))
}

fn rect_stroke(image: &mut RgbaImage, rect: ImageRect, color: Rgba<u8>, width: i32) {
    let loops = width.max(1);
    for i in 0..loops {
        let inset_x = rect.left() + i;
        let inset_y = rect.top() + i;
        let inset_w = rect.width().saturating_sub((i as u32) * 2);
        let inset_h = rect.height().saturating_sub((i as u32) * 2);
        if inset_w == 0 || inset_h == 0 {
            break;
        }
        draw_hollow_rect_mut(image, ImageRect::at(inset_x, inset_y).of_size(inset_w, inset_h), color);
    }
}

fn ellipse_stroke(image: &mut RgbaImage, center: (i32, i32), rx: i32, ry: i32, color: Rgba<u8>, width: i32) {
    let loops = width.max(1);
    for i in 0..loops {
        let next_rx = (rx - i).max(1);
        let next_ry = (ry - i).max(1);
        draw_hollow_ellipse_mut(image, center, next_rx, next_ry, color);
    }
}

fn draw_arrow(image: &mut RgbaImage, item: &AnnotationItem, start: (f64, f64), end: (f64, f64), scale: f64, offset: (f64, f64)) {
    let Some(geometry) = arrow_geometry(start, end, item.style.stroke_width) else {
        return;
    };
    let polygon: Vec<Point<i32>> = geometry
        .polygon
        .iter()
        .map(|point| {
            let (x, y) = to_image_xy(*point, scale, offset);
            Point::new(x.round() as i32, y.round() as i32)
        })
        .collect();
    draw_polygon_mut(image, polygon.as_slice(), rgba_from_u32(item.style.stroke_color));
}

fn draw_rectangle(image: &mut RgbaImage, item: &AnnotationItem, rect: RectF, scale: f64, offset: (f64, f64)) {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    // Drawing clips the original shape. Clamping its geometry first would add a
    // new border at the selection edge when a rectangle extends beyond it.
    let (x, y) = to_image_xy((rect.x, rect.y), scale, offset);
    let image_rect = ImageRect::at(x.floor() as i32, y.floor() as i32).of_size(
        (rect.width * scale).round().max(1.0) as u32,
        (rect.height * scale).round().max(1.0) as u32,
    );
    if item.style.fill_enabled {
        draw_filled_rect_mut(image, image_rect, rgba_from_u32(item.style.fill_color));
    }
    rect_stroke(
        image,
        image_rect,
        rgba_from_u32(item.style.stroke_color),
        (item.style.stroke_width * scale).round() as i32,
    );
}

fn draw_circle(image: &mut RgbaImage, item: &AnnotationItem, rect: RectF, scale: f64, offset: (f64, f64)) {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    let (cx, cy) = to_image_xy((rect.x + rect.width / 2.0, rect.y + rect.height / 2.0), scale, offset);
    let rx = ((rect.width * scale) / 2.0).round().max(1.0) as i32;
    let ry = ((rect.height * scale) / 2.0).round().max(1.0) as i32;
    let center = (cx.round() as i32, cy.round() as i32);
    if item.style.fill_enabled {
        draw_filled_ellipse_mut(image, center, rx, ry, rgba_from_u32(item.style.fill_color));
    }
    ellipse_stroke(
        image,
        center,
        rx,
        ry,
        rgba_from_u32(item.style.stroke_color),
        (item.style.stroke_width * scale).round() as i32,
    );
}

fn draw_counter(
    image: &mut RgbaImage,
    item: &AnnotationItem,
    center: (f64, f64),
    number: u32,
    scale: f64,
    offset: (f64, f64),
    font: Option<&ab_glyph::FontArc>,
) {
    let (cx, cy) = to_image_xy(center, scale, offset);
    let radius = (counter_radius(&item.style) * scale).round() as i32;
    let center_i = (cx.round() as i32, cy.round() as i32);
    draw_filled_circle_mut(image, center_i, radius, rgba_from_u32(item.style.stroke_color));
    ellipse_stroke(image, center_i, radius, radius, Rgba([255, 255, 255, 255]), scale.round() as i32);

    if let Some(font) = font {
        let size = counter_text_size(&item.style, number);
        let font_scale = ab_glyph::PxScale::from((size * scale) as f32);
        let text = number.to_string();
        let (text_x, text_y) = to_image_xy((center.0 - text.len() as f64 * size / 4.0, center.1 - size / 2.0), scale, offset);
        draw_text_mut(
            image,
            Rgba([255, 255, 255, 255]),
            text_x.round() as i32,
            text_y.round() as i32,
            font_scale,
            font,
            &text,
        );
    }
}

fn draw_text(
    image: &mut RgbaImage,
    item: &AnnotationItem,
    origin: (f64, f64),
    text: &str,
    scale: f64,
    offset: (f64, f64),
    font: Option<&ab_glyph::FontArc>,
) {
    let Some(font) = font else {
        return;
    };
    let size = text_size(&item.style);
    let font_scale = ab_glyph::PxScale::from((size * scale) as f32);
    for (line, text) in text.lines().enumerate() {
        let logical_top = origin.1 - size + line as f64 * text_line_height(size);
        let (x, y) = to_image_xy((origin.0, logical_top), scale, offset);
        draw_text_mut(
            image,
            rgba_from_u32(item.style.stroke_color),
            x.round() as i32,
            y.round() as i32,
            font_scale,
            font,
            text,
        );
    }
}

fn draw_mosaic_pixelate(image: &mut RgbaImage, image_rect: ImageRect, block: u32) {
    let block = block.max(2);
    let min_x = image_rect.left().max(0) as u32;
    let min_y = image_rect.top().max(0) as u32;
    let max_x = image_rect.right().min(image.width() as i32).max(0) as u32;
    let max_y = image_rect.bottom().min(image.height() as i32).max(0) as u32;

    let mut y = min_y;
    while y < max_y {
        let mut x = min_x;
        while x < max_x {
            let bx = (x + block).min(max_x);
            let by = (y + block).min(max_y);
            let mut sum = [0u64; 4];
            let mut count = 0u64;
            for py in y..by {
                for px in x..bx {
                    let p = image.get_pixel(px, py).0;
                    sum[0] += u64::from(p[0]);
                    sum[1] += u64::from(p[1]);
                    sum[2] += u64::from(p[2]);
                    sum[3] += u64::from(p[3]);
                    count += 1;
                }
            }
            let count = count.max(1);
            let color = Rgba([
                (sum[0] / count) as u8,
                (sum[1] / count) as u8,
                (sum[2] / count) as u8,
                (sum[3] / count) as u8,
            ]);
            for py in y..by {
                for px in x..bx {
                    image.put_pixel(px, py, color);
                }
            }
            x = bx;
        }
        y = (y + block).min(max_y.max(y + 1));
    }
}

fn draw_mosaic_blur(image: &mut RgbaImage, image_rect: ImageRect, sigma: f32) {
    let min_x = image_rect.left().max(0) as u32;
    let min_y = image_rect.top().max(0) as u32;
    let max_x = image_rect.right().min(image.width() as i32).max(0) as u32;
    let max_y = image_rect.bottom().min(image.height() as i32).max(0) as u32;
    if max_x <= min_x || max_y <= min_y {
        return;
    }
    let sub = imageops::crop_imm(image, min_x, min_y, max_x - min_x, max_y - min_y).to_image();
    let blurred = imageops::blur(&sub, sigma.max(0.5));
    imageops::replace(image, &blurred, i64::from(min_x), i64::from(min_y));
}

fn draw_mosaic(image: &mut RgbaImage, rect: RectF, mode: MosaicMode, intensity: f64, scale: f64, offset: (f64, f64)) {
    let Some(image_rect) = clamp_image_rect(image, rect, scale, offset) else {
        return;
    };
    match mode {
        MosaicMode::Pixelate => draw_mosaic_pixelate(image, image_rect, (intensity * scale).round() as u32),
        MosaicMode::Blur => draw_mosaic_blur(image, image_rect, (intensity * scale * 0.3) as f32),
    }
}

pub(crate) fn draw_annotation_item(image: &mut RgbaImage, item: &AnnotationItem, scale: f64, offset: (f64, f64), font: Option<&ab_glyph::FontArc>) {
    if !item.is_valid() || !positive_finite(scale) || !finite_point(offset) {
        return;
    }
    match &item.kind {
        AnnotationKind::Arrow { start, end } => draw_arrow(image, item, *start, *end, scale, offset),
        AnnotationKind::Rectangle { rect } => draw_rectangle(image, item, *rect, scale, offset),
        AnnotationKind::Circle { rect } => draw_circle(image, item, *rect, scale, offset),
        AnnotationKind::Counter { center, number } => draw_counter(image, item, *center, *number, scale, offset, font),
        AnnotationKind::Text { origin, text } => draw_text(image, item, *origin, text, scale, offset, font),
        AnnotationKind::Mosaic { rect, mode, intensity } => draw_mosaic(image, *rect, *mode, *intensity, scale, offset),
    }
}

pub(crate) fn compose_background_with_annotations(
    background: &RgbaImage,
    items: &[AnnotationItem],
    scale: f64,
    font: Option<&ab_glyph::FontArc>,
) -> RgbaImage {
    let mut image = background.clone();
    for item in items {
        draw_annotation_item(&mut image, item, scale, (0.0, 0.0), font);
    }
    image
}

pub(crate) fn compose_selection_background(background: &RgbaImage, selection: &ResolvedSelection) -> RgbaImage {
    let source_rect = selection.pixel_rect();
    imageops::crop_imm(background, source_rect.x, source_rect.y, source_rect.width, source_rect.height).to_image()
}

pub(crate) fn compose_selection_base(
    background: &RgbaImage,
    selection: &ResolvedSelection,
    items: &[AnnotationItem],
    scale: f64,
    font: Option<&ab_glyph::FontArc>,
) -> RgbaImage {
    let mut layer = compose_selection_background(background, selection);
    draw_items_on_selection(&mut layer, selection, items, scale, font);
    layer
}

pub(crate) fn draw_items_on_selection(
    layer: &mut RgbaImage,
    selection: &ResolvedSelection,
    items: &[AnnotationItem],
    scale: f64,
    font: Option<&ab_glyph::FontArc>,
) {
    // Keep the integer source origin in pixel space. Reconstructing it through
    // logical coordinates would reintroduce rounding at fractional display scales.
    let rect = selection.pixel_rect();
    for item in items {
        draw_annotation_item(layer, item, scale, (f64::from(rect.x), f64::from(rect.y)), font);
    }
}
