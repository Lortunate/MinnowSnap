use crate::config::OcrConfig;
use crate::detector::Detector;
use crate::recognizer::Recognizer;
use crate::utils::crop_image_by_box;
use anyhow::Result;
use image::DynamicImage;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub text: String,
    pub confidence: f32,
    pub box_points: Vec<(i32, i32)>,
}

pub struct OcrEngine {
    detector: Detector,
    recognizer: Recognizer,
}

impl OcrEngine {
    pub fn new(config: OcrConfig) -> Result<Self> {
        info!("Initializing OCR Engine...");

        info!("Initializing Detector...");
        let detector = Detector::new(config.clone())?;

        info!("Initializing Recognizer...");
        let recognizer = Recognizer::new(config)?;

        info!("OCR Engine initialization complete.");
        Ok(Self { detector, recognizer })
    }

    pub fn ocr(&mut self, image: &DynamicImage) -> Result<Vec<OcrResult>> {
        let start_total = Instant::now();
        info!("Starting OCR process...");

        let start_det = Instant::now();
        let boxes = self.detector.detect(image)?;
        debug!("Detection finished in {:?}. Found {} boxes.", start_det.elapsed(), boxes.len());

        if boxes.is_empty() {
            info!("No text detected.");
            return Ok(Vec::new());
        }

        let padding = self.detector.config.det_box_padding;
        let (crops, valid_boxes): (Vec<_>, Vec<_>) = boxes
            .iter()
            .map(|box_points| {
                let crop = crop_image_by_box(image, box_points, padding);
                (crop, box_points.clone())
            })
            .unzip();

        let start_rec = Instant::now();
        info!("Starting batch recognition for {} crops...", crops.len());

        let rec_results = self.recognizer.recognize_batch(&crops)?;
        debug!("Recognition finished in {:?}.", start_rec.elapsed());

        let results: Vec<OcrResult> = rec_results
            .into_iter()
            .zip(valid_boxes)
            .filter(|((text, _), _)| !text.trim().is_empty())
            .map(|((text, confidence), box_points)| OcrResult {
                text,
                confidence,
                box_points,
            })
            .collect();

        info!(
            "OCR process complete in {:?}. Found {} text blocks.",
            start_total.elapsed(),
            results.len()
        );

        Ok(results)
    }
}
