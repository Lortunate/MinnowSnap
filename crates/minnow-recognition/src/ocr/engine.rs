use super::config::OcrConfig;
use super::detector::Detector;
use super::plan::{BoxPoints, RecognitionPlan, reserved};
use super::recognizer::Recognizer;
use crate::InferenceBudget;
use anyhow::{Context, Result};
use image::RgbaImage;
use ort::environment::Environment;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct OcrResult {
    pub text: String,
    pub confidence: f32,
    pub box_points: BoxPoints,
}

pub(super) struct OcrEngine {
    detector: Detector,
    recognizer: Recognizer,
    budget: InferenceBudget,
}

impl OcrEngine {
    pub fn new(config: OcrConfig, environment: &Environment, budget: InferenceBudget) -> Result<Self> {
        budget.validate()?;
        info!("Initializing OCR Engine...");
        let detector = Detector::new(config.clone(), environment)?;
        let recognizer = Recognizer::new(config, environment)?;
        Ok(Self {
            detector,
            recognizer,
            budget,
        })
    }

    pub fn ocr(&mut self, image: &RgbaImage) -> Result<Vec<OcrResult>> {
        let start_total = Instant::now();
        let boxes = self.detector.detect(image, &self.budget)?;
        debug!("Detection finished in {:?}. Found {} boxes.", start_total.elapsed(), boxes.len());
        if boxes.is_empty() {
            return Ok(Vec::new());
        }
        let plan = RecognitionPlan::new(image, &boxes, self.detector.config.det_box_padding, self.recognizer.shape(), &self.budget)?;
        let mut ordered = reserved(boxes.len(), "ordered recognition slots")?;
        ordered.resize_with(boxes.len(), || None);
        let start_rec = Instant::now();
        for batch in &plan.batches {
            let crops = &plan.crops[batch.start..batch.end];
            let results = self.recognizer.recognize_batch(image, crops, batch)?;
            if results.len() != crops.len() {
                anyhow::bail!("OCR recognizer returned incomplete batch results");
            }
            for (crop, result) in crops.iter().zip(results) {
                if ordered[crop.original_index].replace(result).is_some() {
                    anyhow::bail!("OCR planner repeated a source box");
                }
            }
        }
        debug!("Recognition finished in {:?} across {} batches.", start_rec.elapsed(), plan.batches.len());
        let results = finish_results(boxes, ordered)?;
        info!(
            "OCR process complete in {:?}. Found {} text blocks.",
            start_total.elapsed(),
            results.len()
        );
        Ok(results)
    }
}

fn finish_results(boxes: Vec<BoxPoints>, ordered: Vec<Option<(String, f32)>>) -> Result<Vec<OcrResult>> {
    if boxes.len() != ordered.len() {
        anyhow::bail!("OCR result count does not match source boxes");
    }
    let mut results = reserved(boxes.len(), "OCR output records")?;
    for (box_points, result) in boxes.into_iter().zip(ordered) {
        let (text, confidence) = result.context("OCR source box was not recognized")?;
        if !text.trim().is_empty() {
            results.push(OcrResult {
                text,
                confidence,
                box_points,
            });
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_batches_restore_original_box_order_without_truncating_text() {
        let boxes = vec![[(0, 0); 4], [(1, 1); 4], [(2, 2); 4]];
        let long = "complete text ".repeat(1000);
        let slots = vec![Some((long.clone(), 0.9)), Some(("second".into(), 0.8)), Some(("third".into(), 0.7))];
        let results = finish_results(boxes.clone(), slots).unwrap();
        assert_eq!(results.iter().map(|result| result.box_points).collect::<Vec<_>>(), boxes);
        assert_eq!(results[0].text, long);
        assert_eq!(results[1].text, "second");
        assert_eq!(results[2].text, "third");
        assert!(finish_results(boxes, vec![Some(("a".into(), 1.0)), None, Some(("c".into(), 1.0))]).is_err());
    }
}
