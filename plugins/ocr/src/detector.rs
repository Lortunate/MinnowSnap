use crate::config::OcrConfig;
use crate::utils::{create_onnx_session, get_bounding_rect, normalize_image, PPOCR_MEAN, PPOCR_STD};
use anyhow::Result;
use image::{imageops::FilterType, DynamicImage, GenericImageView, GrayImage};
use imageproc::contours::find_contours;
use log::debug;
use ort::{inputs, session::Session, value::Value};
use rayon::prelude::*;
use std::time::Instant;

type BoxPoints = Vec<(i32, i32)>;

pub struct Detector {
    session: Session,
    pub config: OcrConfig,
}

impl Detector {
    pub fn new(config: OcrConfig) -> Result<Self> {
        let session = create_onnx_session(&config.det_model_path, config.threads)?;
        Ok(Self { session, config })
    }

    pub fn detect(&mut self, image: &DynamicImage) -> Result<Vec<BoxPoints>> {
        let start = Instant::now();

        let tensor = self.preprocess(image)?;

        let input_value = Value::from_array(tensor)?;
        let outputs = self.session.run(inputs!["x" => input_value])?;
        let (shape, data) = outputs[0].try_extract_tensor::<f32>()?;

        let out_h = shape[2] as usize;
        let out_w = shape[3] as usize;
        let data_vec = data.to_vec();

        drop(outputs);

        debug!("Detector inference time: {:?}", start.elapsed());

        let (orig_w, orig_h) = image.dimensions();
        self.postprocess(&data_vec, out_w, out_h, orig_w as f32, orig_h as f32)
    }

    fn preprocess(&self, image: &DynamicImage) -> Result<ndarray::Array4<f32>> {
        let (w, h) = image.dimensions();
        let limit_side = self.config.limit_side_len;

        let max_dim = w.max(h) as f32;
        let ratio = if max_dim > limit_side { limit_side / max_dim } else { 1.0 };

        let resize_w = ((w as f32 * ratio) as u32 / 32).max(1) * 32;
        let resize_h = ((h as f32 * ratio) as u32 / 32).max(1) * 32;

        let resized = image.resize_exact(resize_w, resize_h, FilterType::Triangle);

        Ok(normalize_image(&resized, &PPOCR_MEAN, &PPOCR_STD))
    }

    fn postprocess(&self, data: &[f32], out_w: usize, out_h: usize, orig_w: f32, orig_h: f32) -> Result<Vec<BoxPoints>> {
        let start = Instant::now();
        let thresh = self.config.det_thresh;

        // Binarize the probability map
        let gray_pixels: Vec<u8> = data.iter().map(|&val| if val > thresh { 255 } else { 0 }).collect();

        let gray_img = GrayImage::from_vec(out_w as u32, out_h as u32, gray_pixels)
            .ok_or_else(|| anyhow::anyhow!("Failed to construct gray image from inference output"))?;

        let contours = find_contours::<u32>(&gray_img);

        let scale_x = orig_w / out_w as f32;
        let scale_y = orig_h / out_h as f32;

        let boxes: Vec<BoxPoints> = contours
            .par_iter()
            .filter(|c| c.points.len() >= 4) // Filter out noise
            .filter_map(|contour| {
                let points: Vec<(u32, u32)> = contour.points.iter().map(|p| (p.x, p.y)).collect();
                let (min_x, min_y, max_x, max_y) = get_bounding_rect(&points);

                if max_x - min_x < 5 || max_y - min_y < 5 {
                    return None;
                }

                let x1 = (min_x as f32 * scale_x) as i32;
                let y1 = (min_y as f32 * scale_y) as i32;
                let x2 = (max_x as f32 * scale_x) as i32;
                let y2 = (max_y as f32 * scale_y) as i32;

                let clamp_x = |v: i32| v.clamp(0, orig_w as i32);
                let clamp_y = |v: i32| v.clamp(0, orig_h as i32);

                Some(vec![
                    (clamp_x(x1), clamp_y(y1)),
                    (clamp_x(x2), clamp_y(y1)),
                    (clamp_x(x2), clamp_y(y2)),
                    (clamp_x(x1), clamp_y(y2)),
                ])
            })
            .collect();

        debug!("Detector postprocess time: {:?}, found {} boxes", start.elapsed(), boxes.len());
        Ok(boxes)
    }
}
