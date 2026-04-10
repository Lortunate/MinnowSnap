use image::imageops;
use image::{Rgba, RgbaImage};
use imageproc::drawing::{
    draw_filled_circle_mut, draw_filled_ellipse_mut, draw_filled_rect_mut, draw_hollow_circle_mut, draw_hollow_ellipse_mut, draw_hollow_rect_mut,
    draw_polygon_mut, draw_text_mut,
};
use imageproc::point::Point;
use imageproc::rect::Rect as ImageRect;
use std::fs;
use std::sync::LazyLock;

use minnow_core::geometry::RectF;

use super::model::{AnnotationItem, AnnotationKind, MosaicMode};
use super::ops::arrow_geometry;

static OVERLAY_TEXT_FONT: LazyLock<Option<ab_glyph::FontArc>> = LazyLock::new(load_text_font);

fn rgba_from_u32(value: u32) -> Rgba<u8> {
    let r = ((value >> 24) & 0xff) as u8;
    let g = ((value >> 16) & 0xff) as u8;
    let b = ((value >> 8) & 0xff) as u8;
    let a = (value & 0xff) as u8;
    Rgba([r, g, b, a])
}

fn load_text_font() -> Option<ab_glyph::FontArc> {
    use font_kit::family_name::FamilyName;
    use font_kit::handle::Handle;
    use font_kit::properties::Properties;
    use font_kit::source::SystemSource;

    let preferred = [
        "Microsoft YaHei",
        "PingFang SC",
        "Noto Sans CJK SC",
        "Source Han Sans SC",
        "Segoe UI",
        "Arial",
        "Helvetica",
    ];
    let source = SystemSource::new();

    for family in preferred {
        let Ok(handle) = source.select_best_match(&[FamilyName::Title(family.to_string())], &Properties::new()) else {
            continue;
        };
        let bytes = match handle {
            Handle::Path { path, .. } => fs::read(path).ok(),
            Handle::Memory { bytes, .. } => Some(bytes.as_ref().clone()),
        };
        if let Some(data) = bytes
            && let Ok(font) = ab_glyph::FontArc::try_from_vec(data)
        {
            return Some(font);
        }
    }

    None
}

fn to_image_xy(point: (f64, f64), scale: f64, offset: (f64, f64)) -> (f64, f64) {
    ((point.0 - offset.0) * scale, (point.1 - offset.1) * scale)
}

fn clamp_image_rect(image: &RgbaImage, rect: RectF, scale: f64, offset: (f64, f64)) -> Option<ImageRect> {
    let x = ((rect.x - offset.0) * scale).floor() as i32;
    let y = ((rect.y - offset.1) * scale).floor() as i32;
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
    let Some(image_rect) = clamp_image_rect(image, rect, scale, offset) else {
        return;
    };
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

fn draw_counter(image: &mut RgbaImage, item: &AnnotationItem, center: (f64, f64), number: u32, scale: f64, offset: (f64, f64)) {
    let (cx, cy) = to_image_xy(center, scale, offset);
    let radius = (item.style.counter_radius * scale).clamp(10.0, 64.0) as i32;
    let center_i = (cx.round() as i32, cy.round() as i32);
    draw_filled_circle_mut(image, center_i, radius, rgba_from_u32(item.style.stroke_color));
    draw_hollow_circle_mut(image, center_i, radius, Rgba([255, 255, 255, 255]));

    if let Some(font) = OVERLAY_TEXT_FONT.as_ref() {
        let font_scale = ab_glyph::PxScale::from((item.style.text_size * scale).clamp(12.0, 60.0) as f32);
        let text = number.to_string();
        let text_x = (cx.round() as i32 - (text.len() as i32 * font_scale.x as i32 / 4)).max(0);
        let text_y = (cy.round() as i32 - (font_scale.y as i32 / 2)).max(0);
        draw_text_mut(image, Rgba([255, 255, 255, 255]), text_x, text_y, font_scale, font, &text);
    }
}

fn draw_text(image: &mut RgbaImage, item: &AnnotationItem, origin: (f64, f64), text: &str, scale: f64, offset: (f64, f64)) {
    let Some(font) = OVERLAY_TEXT_FONT.as_ref() else {
        return;
    };
    let font_scale = ab_glyph::PxScale::from((item.style.text_size * scale).clamp(12.0, 96.0) as f32);
    let (x, y) = to_image_xy((origin.0, origin.1 - item.style.text_size), scale, offset);
    draw_text_mut(
        image,
        rgba_from_u32(item.style.stroke_color),
        x.round().max(0.0) as i32,
        y.round().max(0.0) as i32,
        font_scale,
        font,
        text,
    );
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
            if count > 0 {
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

pub(crate) fn draw_annotation_item(image: &mut RgbaImage, item: &AnnotationItem, scale: f64, offset: (f64, f64)) {
    match &item.kind {
        AnnotationKind::Arrow { start, end } => draw_arrow(image, item, *start, *end, scale, offset),
        AnnotationKind::Rectangle { rect } => draw_rectangle(image, item, *rect, scale, offset),
        AnnotationKind::Circle { rect } => draw_circle(image, item, *rect, scale, offset),
        AnnotationKind::Counter { center, number } => draw_counter(image, item, *center, *number, scale, offset),
        AnnotationKind::Text { origin, text } => draw_text(image, item, *origin, text, scale, offset),
        AnnotationKind::Mosaic { rect, mode, intensity } => draw_mosaic(image, *rect, *mode, *intensity, scale, offset),
    }
}

pub(crate) fn compose_background_with_annotations(background: &RgbaImage, items: &[AnnotationItem], scale: f64) -> RgbaImage {
    let mut image = background.clone();
    for item in items {
        draw_annotation_item(&mut image, item, scale, (0.0, 0.0));
    }
    image
}

pub(crate) fn compose_selection_base(background: &RgbaImage, selection: RectF, items: &[AnnotationItem], scale: f64) -> Option<RgbaImage> {
    let source_rect = clamp_image_rect(background, selection, scale, (0.0, 0.0))?;
    let mut layer = imageops::crop_imm(
        background,
        source_rect.left() as u32,
        source_rect.top() as u32,
        source_rect.width(),
        source_rect.height(),
    )
    .to_image();
    for item in items {
        draw_annotation_item(&mut layer, item, scale, (selection.x, selection.y));
    }
    Some(layer)
}

pub(crate) fn draw_items_on_selection(layer: &mut RgbaImage, selection: RectF, items: &[AnnotationItem], scale: f64) {
    for item in items {
        draw_annotation_item(layer, item, scale, (selection.x, selection.y));
    }
}
