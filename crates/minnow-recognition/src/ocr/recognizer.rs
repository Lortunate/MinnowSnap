use super::config::OcrConfig;
use super::contracts::{Stage, validate_session};
use super::plan::{BatchPlan, CropPlan, RecognitionShape, product, reserved};
use super::preprocess::preprocess_batch;
use super::session::create_onnx_session;
use anyhow::{Context, Result};
use image::RgbaImage;
use ndarray::ArrayView2;
use ort::{environment::Environment, inputs, session::Session, value::Value};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::Instant;
use tracing::debug;

pub(super) struct Recognizer {
    session: Session,
    keys: Vec<String>,
    shape: RecognitionShape,
}

impl Recognizer {
    pub fn new(config: OcrConfig, environment: &Environment) -> Result<Self> {
        let session = create_onnx_session(environment, &config.rec_model_path, config.threads)?;
        let keys = Self::load_keys(&config.keys_path, config.dictionary_entries, &config.dictionary_sha256)?;
        let classes = keys.len().checked_add(1).context("Recognizer dictionary class count overflow")?;
        let max_key_bytes = keys.iter().map(String::len).max().context("Recognizer dictionary is empty")?;
        validate_session(&session, Stage::Recognizer { classes })?;
        let character = session
            .metadata()?
            .custom("character")
            .context("Recognizer is missing its dictionary character metadata")?;
        Self::validate_dictionary_identity(&keys, &character)?;

        Ok(Self {
            session,
            keys,
            shape: RecognitionShape {
                height: config.rec_img_h,
                classes,
                max_key_bytes,
            },
        })
    }

    fn load_keys(path: &Path, expected_entries: usize, expected_sha256: &str) -> Result<Vec<String>> {
        let bytes = std::fs::read(path).with_context(|| format!("Recognizer dictionary could not be read: {}", path.display()))?;
        let actual_hash: String = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect();
        if actual_hash != expected_sha256 {
            anyhow::bail!("Recognizer dictionary {} does not match its catalog SHA256", path.display());
        }
        Self::dictionary_keys(
            std::str::from_utf8(&bytes).context("Recognizer dictionary must be UTF-8")?,
            expected_entries,
        )
    }

    fn validate_dictionary_identity(keys: &[String], character: &str) -> Result<()> {
        if keys.is_empty() || keys[..keys.len() - 1].join("\n") != character {
            anyhow::bail!("Recognizer embedded character metadata does not match the verified dictionary in file order");
        }
        Ok(())
    }

    fn dictionary_keys(text: &str, expected_entries: usize) -> Result<Vec<String>> {
        let mut keys: Vec<String> = text.lines().map(String::from).collect();
        if expected_entries == 0 || keys.len() != expected_entries || keys.iter().any(String::is_empty) {
            anyhow::bail!(
                "Recognizer dictionary cardinality mismatch: expected {expected_entries} nonempty entries, got {}",
                keys.len()
            );
        }
        keys.push(" ".into());
        Ok(keys)
    }

    pub fn shape(&self) -> RecognitionShape {
        self.shape
    }

    pub fn recognize_batch(&mut self, image: &RgbaImage, crops: &[CropPlan], batch: &BatchPlan) -> Result<Vec<(String, f32)>> {
        let start = Instant::now();
        let tensor = preprocess_batch(image, crops, batch, self.shape.height)?;
        debug!(
            "Recognizer preprocess time: {:?}, admitted working set {} bytes",
            start.elapsed(),
            batch.working_bytes
        );
        let start_infer = Instant::now();
        let input_value = Value::from_array(tensor)?;
        let outputs = self.session.run(inputs!["x" => input_value])?;
        let (shape, data) = outputs[0].try_extract_tensor::<f32>()?;
        debug!("Recognizer inference time: {:?}", start_infer.elapsed());
        if shape.len() != 3 {
            anyhow::bail!("Recognizer output must have three dimensions, got {:?}", shape);
        }
        let batch_size = usize::try_from(shape[0]).context("Recognizer batch dimension is negative")?;
        let time_steps = usize::try_from(shape[1]).context("Recognizer time dimension is negative")?;
        let num_classes = usize::try_from(shape[2]).context("Recognizer class dimension is negative")?;
        if batch_size != crops.len() || time_steps == 0 || time_steps > batch.width as usize || num_classes != self.shape.classes {
            anyhow::bail!(
                "Recognizer output exceeds admitted contract: batch={batch_size} (expected {}), time_steps={time_steps} (limit {}), classes={num_classes} (expected {})",
                crops.len(),
                batch.width,
                self.shape.classes
            );
        }
        let batch_stride = product(&[time_steps, num_classes], "recognizer output stride")?;
        let expected_len = product(&[batch_size, batch_stride], "recognizer output elements")?;
        if data.len() != expected_len || product(&[expected_len, 4], "recognizer output bytes")? > batch.output_bytes {
            anyhow::bail!(
                "Recognizer output length exceeds admitted contract: expected {expected_len}, got {}",
                data.len()
            );
        }
        let mut results = reserved(batch_size, "decoded recognition records")?;
        for batch_data in data.chunks_exact(batch_stride) {
            let view = ArrayView2::from_shape((time_steps, num_classes), batch_data)
                .map_err(|err| anyhow::anyhow!("Data shape mismatch during decoding: {err}"))?;
            results.push(Self::ctc_decode(&self.keys, view)?);
        }
        Ok(results)
    }

    fn ctc_decode(keys: &[String], output: ArrayView2<f32>) -> Result<(String, f32)> {
        if output.ncols() != keys.len() + 1 || output.nrows() == 0 || output.iter().any(|value| !value.is_finite()) {
            anyhow::bail!(
                "Recognizer CTC output has invalid scores or {} classes for {} keys plus blank",
                output.ncols(),
                keys.len()
            );
        }
        let max_key_bytes = keys.iter().map(String::len).max().context("Recognizer dictionary is empty")?;
        let capacity = product(&[output.nrows(), max_key_bytes], "decoded UTF-8")?;
        let mut text = String::new();
        text.try_reserve_exact(capacity).context("Recognizer decoded text allocation refused")?;
        let mut confidence_sum = 0.0;
        let mut conf_count = 0usize;
        let mut last_index = 0;

        for row in output.outer_iter() {
            let (max_idx, &max_val) = row
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.total_cmp(b))
                .context("Recognizer CTC output row has no classes")?;

            if max_idx > 0 && max_idx != last_index {
                let key = keys
                    .get(max_idx - 1)
                    .ok_or_else(|| anyhow::anyhow!("Recognizer CTC class {max_idx} is outside the verified dictionary"))?;
                text.push_str(key);
                confidence_sum += max_val;
                conf_count += 1;
            }
            last_index = max_idx;
        }

        let confidence = if conf_count > 0 { confidence_sum / conf_count as f32 } else { 0.0 };

        Ok((text, confidence))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    #[test]
    fn dictionary_convention_preserves_first_entry_and_appends_exactly_one_space() {
        let keys = Recognizer::dictionary_keys("\'\na\n", 2).unwrap();
        assert_eq!(keys, ["'", "a", " "]);
        assert!(Recognizer::validate_dictionary_identity(&keys, "'\na").is_ok());
        assert!(Recognizer::validate_dictionary_identity(&keys, "a\n'").is_err());
        assert!(Recognizer::dictionary_keys("a\n", 2).is_err());
        assert!(Recognizer::dictionary_keys("a\n\nb\n", 3).is_err());
    }

    #[test]
    fn ctc_blank_repeat_and_space_mapping_are_explicit_and_mismatches_fail() {
        let keys = vec!["A".into(), " ".into()];
        let output = array![[0., 1., 0.], [0., 1., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
        assert_eq!(Recognizer::ctc_decode(&keys, output.view()).unwrap().0, "AA ");
        assert!(Recognizer::ctc_decode(&keys, array![[0., 0., 0., 1.]].view()).is_err());
        assert!(Recognizer::ctc_decode(&keys, array![[0., f32::NAN, 1.]].view()).is_err());
    }
}
