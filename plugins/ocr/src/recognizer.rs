use crate::config::OcrConfig;
use crate::utils::{create_onnx_session, preprocess_batch};
use anyhow::{Context, Result};
use image::DynamicImage;
use log::debug;
use ndarray::ArrayView2;
use ort::{inputs, session::Session, value::Value};
use rayon::prelude::*;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

pub struct Recognizer {
    session: Session,
    keys: Vec<String>,
    img_height: u32,
}

impl Recognizer {
    pub fn new(config: OcrConfig) -> Result<Self> {
        let session = create_onnx_session(&config.rec_model_path, config.threads)?;
        let keys = Self::load_keys(&config.keys_path)?;

        Ok(Self {
            session,
            keys,
            img_height: config.rec_img_h,
        })
    }

    fn load_keys(path: &Path) -> Result<Vec<String>> {
        let file = File::open(path).with_context(|| format!("Failed to open keys file: {:?}", path))?;
        let reader = BufReader::new(file);

        let mut keys: Vec<String> = Vec::new();
        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            if i == 0 && line == "'" {
                continue;
            }
            keys.push(line);
        }

        keys.push(" ".to_string());

        Ok(keys)
    }

    pub fn recognize_batch(&mut self, images: &[DynamicImage]) -> Result<Vec<(String, f32)>> {
        if images.is_empty() {
            return Ok(Vec::new());
        }

        let start = Instant::now();
        let tensor = preprocess_batch(images, self.img_height)?;
        debug!("Recognizer preprocess time: {:?}", start.elapsed());

        let start_infer = Instant::now();
        let input_value = Value::from_array(tensor)?;
        let outputs = self.session.run(inputs!["x" => input_value])?;

        let (shape, data) = outputs[0].try_extract_tensor::<f32>()?;
        debug!("Recognizer inference time: {:?}", start_infer.elapsed());

        let batch_size = shape[0] as usize;
        let time_steps = shape[1] as usize;
        let num_classes = shape[2] as usize;

        let start_decode = Instant::now();

        let results: Vec<(String, f32)> = data
            .par_chunks(time_steps * num_classes)
            .map(|batch_data| {
                let view = ArrayView2::from_shape((time_steps, num_classes), batch_data).expect("Data shape mismatch during decoding");

                Self::ctc_decode(&self.keys, view)
            })
            .collect();

        debug!("Recognizer decode time: {:?}", start_decode.elapsed());

        if results.len() != batch_size {
            log::warn!("Recognizer output count mismatch: expected {}, got {}", batch_size, results.len());
        }

        Ok(results)
    }

    fn ctc_decode(keys: &[String], output: ArrayView2<f32>) -> (String, f32) {
        let mut text = String::with_capacity(32);
        let mut confidence_sum = 0.0;
        let mut conf_count = 0;
        let mut last_index = 0;

        for row in output.outer_iter() {
            let (max_idx, max_val) = row
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or((0, &0.0));

            if max_idx > 0 && max_idx != last_index {
                if let Some(key) = keys.get(max_idx - 1) {
                    text.push_str(key);
                    confidence_sum += max_val;
                    conf_count += 1;
                }
            }
            last_index = max_idx;
        }

        let confidence = if conf_count > 0 { confidence_sum / conf_count as f32 } else { 0.0 };

        (text, confidence)
    }
}
