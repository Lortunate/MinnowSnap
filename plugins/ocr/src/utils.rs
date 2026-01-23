use anyhow::Result;
use image::{imageops::FilterType, DynamicImage, GenericImageView, Rgb};
use ndarray::{Array3, Array4};
use ort::session::{builder::GraphOptimizationLevel, Session};
use rayon::prelude::*;
use std::path::Path;

pub const PPOCR_MEAN: [f32; 3] = [0.5, 0.5, 0.5];
pub const PPOCR_STD: [f32; 3] = [0.5, 0.5, 0.5];

pub fn create_onnx_session<P: AsRef<Path>>(model_path: P, threads: usize) -> Result<Session> {
    Ok(Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(threads)?
        .commit_from_file(model_path)?)
}

#[inline(always)]
fn normalize_pixel(pixel: &Rgb<u8>, mean: &[f32; 3], std: &[f32; 3]) -> [f32; 3] {
    [
        (pixel[0] as f32 / 255.0 - mean[0]) / std[0],
        (pixel[1] as f32 / 255.0 - mean[1]) / std[1],
        (pixel[2] as f32 / 255.0 - mean[2]) / std[2],
    ]
}

pub fn normalize_image(img: &DynamicImage, mean: &[f32; 3], std: &[f32; 3]) -> Array4<f32> {
    let (w, h) = img.dimensions();
    let img_rgb = img.to_rgb8();

    let pixels: Vec<f32> = img_rgb.pixels().flat_map(|p| normalize_pixel(p, mean, std)).collect();

    Array3::from_shape_vec((h as usize, w as usize, 3), pixels)
        .unwrap()
        .permuted_axes([2, 0, 1])
        .insert_axis(ndarray::Axis(0))
        .to_owned()
}

pub fn preprocess_batch(images: &[DynamicImage], height: u32) -> Result<Array4<f32>> {
    if images.is_empty() {
        return Ok(Array4::zeros((0, 3, height as usize, 0)));
    }

    let resized_images: Vec<_> = images
        .par_iter()
        .map(|img| {
            let (orig_w, orig_h) = img.dimensions();
            let ratio = height as f32 / orig_h as f32;
            let w = (orig_w as f32 * ratio) as u32;
            let resized = img.resize_exact(w, height, FilterType::Triangle);
            (resized, w)
        })
        .collect();

    let max_width = resized_images.iter().map(|(_, w)| *w).max().unwrap_or(0).div_ceil(32).max(1) * 32;

    let batch_size = images.len();

    let mut batch_data = Vec::with_capacity(batch_size * 3 * height as usize * max_width as usize);

    for (img, w) in resized_images {
        let img_rgb = img.to_rgb8();
        let h_usize = height as usize;
        let max_w_usize = max_width as usize;
        let w_usize = w as usize;

        let mut channel_r = vec![0.0; h_usize * max_w_usize];
        let mut channel_g = vec![0.0; h_usize * max_w_usize];
        let mut channel_b = vec![0.0; h_usize * max_w_usize];

        for (y, row) in img_rgb.rows().enumerate() {
            for (x, pixel) in row.enumerate() {
                if x >= w_usize || y >= h_usize {
                    continue;
                }

                let [r, g, b] = normalize_pixel(pixel, &PPOCR_MEAN, &PPOCR_STD);
                let idx = y * max_w_usize + x;

                channel_r[idx] = r;
                channel_g[idx] = g;
                channel_b[idx] = b;
            }
        }

        batch_data.extend(channel_r);
        batch_data.extend(channel_g);
        batch_data.extend(channel_b);
    }

    Ok(Array4::from_shape_vec((batch_size, 3, height as usize, max_width as usize), batch_data)?)
}

pub fn get_bounding_rect<T>(points: &[(T, T)]) -> (T, T, T, T)
where
    T: Copy + PartialOrd + num_traits::Bounded,
{
    points.iter().fold(
        (T::max_value(), T::max_value(), T::min_value(), T::min_value()),
        |(min_x, min_y, max_x, max_y), &(x, y)| {
            (
                if x < min_x { x } else { min_x },
                if y < min_y { y } else { min_y },
                if x > max_x { x } else { max_x },
                if y > max_y { y } else { max_y },
            )
        },
    )
}

pub fn crop_image_by_box(image: &DynamicImage, box_points: &[(i32, i32)], padding: i32) -> DynamicImage {
    let (min_x, min_y, max_x, max_y) = get_bounding_rect(box_points);
    let (img_w, img_h) = image.dimensions();
    let (img_w, img_h) = (img_w as i32, img_h as i32);

    let x = (min_x - padding).max(0) as u32;
    let y = (min_y - padding).max(0) as u32;
    let w = ((max_x + padding).min(img_w) as u32).saturating_sub(x).max(1);
    let h = ((max_y + padding).min(img_h) as u32).saturating_sub(y).max(1);

    image.crop_imm(x, y, w, h)
}
