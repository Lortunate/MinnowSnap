use crate::OcrResult;
use ab_glyph::{Font, PxScale, ScaleFont};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use imageproc::drawing::{draw_line_segment_mut, draw_text_mut};

pub fn draw_ocr_results(image: &DynamicImage, results: &[OcrResult], font: &impl Font) -> DynamicImage {
    let (img_w, img_h) = image.dimensions();
    let mut canvas = RgbaImage::new(img_w * 2, img_h);
    let original_rgba = image.to_rgba8();

    for y in 0..img_h {
        for x in 0..img_w {
            canvas.put_pixel(x, y, *original_rgba.get_pixel(x, y));
            canvas.put_pixel(x + img_w, y, Rgba([255, 255, 255, 255]));
        }
    }

    let colors = [
        Rgba([255, 0, 0, 255]),
        Rgba([0, 255, 0, 255]),
        Rgba([0, 0, 255, 255]),
        Rgba([255, 215, 0, 255]),
        Rgba([255, 0, 255, 255]),
        Rgba([0, 206, 209, 255]),
        Rgba([255, 140, 0, 255]),
        Rgba([138, 43, 226, 255]),
    ];

    for (i, result) in results.iter().enumerate() {
        if result.box_points.len() < 3 || result.text.trim().is_empty() {
            continue;
        }

        let color = colors[i % colors.len()];
        let text_color = Rgba([0, 0, 0, 255]);
        let points = &result.box_points;

        for j in 0..points.len() {
            let p1 = points[j];
            let p2 = points[(j + 1) % points.len()];
            let start = (p1.0 as f32, p1.1 as f32);
            let end = (p2.0 as f32, p2.1 as f32);

            draw_line_segment_mut(&mut canvas, start, end, color);
            draw_line_segment_mut(&mut canvas, (start.0 + 1.0, start.1 + 1.0), (end.0 + 1.0, end.1 + 1.0), color);

            let start_r = (start.0 + img_w as f32, start.1);
            let end_r = (end.0 + img_w as f32, end.1);
            draw_line_segment_mut(&mut canvas, start_r, end_r, color);
            draw_line_segment_mut(&mut canvas, (start_r.0 + 1.0, start_r.1 + 1.0), (end_r.0 + 1.0, end_r.1 + 1.0), color);
        }

        let min_x = points.iter().map(|p| p.0).min().unwrap_or(0).max(0);
        let max_x = points.iter().map(|p| p.0).max().unwrap_or(0).min(img_w as i32);
        let min_y = points.iter().map(|p| p.1).min().unwrap_or(0).max(0);
        let max_y = points.iter().map(|p| p.1).max().unwrap_or(0).min(img_h as i32);

        let box_width = (max_x - min_x).max(1) as f32;
        let box_height = (max_y - min_y).max(1) as f32;
        let target_height = box_height * 0.8;
        let mut scale_val = target_height.max(10.0);
        let mut scale = PxScale { x: scale_val, y: scale_val };
        let mut scaled_font = font.as_scaled(scale);
        let mut text_width = result.text.chars().map(|c| scaled_font.h_advance(scaled_font.glyph_id(c))).sum::<f32>();
        let max_text_width = box_width * 0.95;

        if text_width > max_text_width {
            let ratio = max_text_width / text_width;
            scale_val *= ratio;
            scale = PxScale { x: scale_val, y: scale_val };
            scaled_font = font.as_scaled(scale);
            text_width = result.text.chars().map(|c| scaled_font.h_advance(scaled_font.glyph_id(c))).sum::<f32>();
        }

        let text_x = min_x + ((box_width - text_width) / 2.0) as i32;
        let text_y = min_y + ((box_height - scale_val) / 2.0) as i32;

        draw_text_mut(&mut canvas, text_color, text_x + img_w as i32, text_y, scale, font, &result.text);
    }

    DynamicImage::ImageRgba8(canvas)
}
