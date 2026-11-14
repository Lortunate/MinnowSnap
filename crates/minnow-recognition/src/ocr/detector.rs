use super::config::OcrConfig;
use super::contracts::{Stage, detector_output, validate_session};
use super::plan::{BoxPoints, DetectionPlan, reserved};
use super::preprocess::normalize_image;
use super::session::create_onnx_session;
use crate::InferenceBudget;
use anyhow::Result;
use image::{GrayImage, RgbaImage, imageops::FilterType};
use imageproc::contours::find_contours;
use ort::{environment::Environment, inputs, session::Session, value::Value};
use std::time::Instant;
use tracing::debug;

pub(super) struct Detector {
    session: Session,
    pub config: OcrConfig,
}

impl Detector {
    pub fn new(config: OcrConfig, environment: &Environment) -> Result<Self> {
        let session = create_onnx_session(environment, &config.det_model_path, config.threads)?;
        validate_session(&session, Stage::Detector)?;
        Ok(Self { session, config })
    }

    pub fn detect(&mut self, image: &RgbaImage, budget: &InferenceBudget) -> Result<Vec<BoxPoints>> {
        let start = Instant::now();
        let plan = DetectionPlan::new(image, self.config.limit_side_len, budget)?;
        let resized = image::imageops::resize(image, plan.width, plan.height, FilterType::Triangle);
        let tensor = normalize_image(&resized)?;
        drop(resized);
        let input_value = Value::from_array(tensor)?;
        let outputs = self.session.run(inputs!["x" => input_value])?;
        let (shape, data) = outputs[0].try_extract_tensor::<f32>()?;
        let (out_h, out_w) = detector_output(shape, data.len())?;
        if out_w != plan.width as usize || out_h != plan.height as usize {
            anyhow::bail!(
                "Detector output resolution exceeds admitted contract: {out_w}x{out_h}, expected {}x{}",
                plan.width,
                plan.height
            );
        }
        if data.iter().any(|value| !value.is_finite()) {
            anyhow::bail!("Detector output contains nonfinite scores");
        }
        debug!("Detector inference time: {:?}", start.elapsed());
        Self::postprocess(
            data,
            out_w,
            out_h,
            image.width() as f32,
            image.height() as f32,
            self.config.det_thresh,
            budget.max_boxes,
        )
    }

    fn postprocess(data: &[f32], out_w: usize, out_h: usize, orig_w: f32, orig_h: f32, threshold: f32, max_boxes: usize) -> Result<Vec<BoxPoints>> {
        let start = Instant::now();
        let mut gray_pixels = reserved(data.len(), "detector threshold map")?;
        gray_pixels.extend(data.iter().map(|&value| if value > threshold { 255 } else { 0 }));
        let gray_img = GrayImage::from_vec(out_w as u32, out_h as u32, gray_pixels)
            .ok_or_else(|| anyhow::anyhow!("Failed to construct gray image from inference output"))?;
        let contours = find_contours::<u32>(&gray_img);
        let scale_x = orig_w / out_w as f32;
        let scale_y = orig_h / out_h as f32;
        let mut boxes = reserved(contours.len().min(max_boxes), "detector boxes")?;
        for contour in contours.iter().filter(|contour| contour.points.len() >= 4) {
            let (min_x, min_y, max_x, max_y) = contour.points.iter().fold((u32::MAX, u32::MAX, 0, 0), |bounds, point| {
                (bounds.0.min(point.x), bounds.1.min(point.y), bounds.2.max(point.x), bounds.3.max(point.y))
            });
            if max_x - min_x < 3 || max_y - min_y < 3 {
                continue;
            }
            let width = (max_x - min_x) as f32;
            let height = (max_y - min_y) as f32;
            let offset = width * height * 1.5 / (2.0 * (width + height));
            let x1 = (((min_x as f32 - offset).max(0.0) * scale_x) as i32).clamp(0, orig_w as i32);
            let y1 = (((min_y as f32 - offset).max(0.0) * scale_y) as i32).clamp(0, orig_h as i32);
            let x2 = (((max_x as f32 + offset).min(out_w as f32) * scale_x) as i32).clamp(0, orig_w as i32);
            let y2 = (((max_y as f32 + offset).min(out_h as f32) * scale_y) as i32).clamp(0, orig_h as i32);
            if x2 - x1 < 5 || y2 - y1 < 5 {
                continue;
            }
            if boxes.len() == max_boxes {
                anyhow::bail!("OCR detector box count exceeds limit {max_boxes}; input was not partially recognized");
            }
            boxes.push([(x1, y1), (x2, y1), (x2, y2), (x1, y2)]);
        }
        debug!("Detector postprocess time: {:?}, found {} boxes", start.elapsed(), boxes.len());
        Ok(boxes)
    }
}
