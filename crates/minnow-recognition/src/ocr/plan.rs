//! Allocation admission before crop views, resampling or tensor construction.
use crate::InferenceBudget;
use anyhow::{Context, Result, bail};
use image::RgbaImage;

pub(super) type BoxPoints = [(i32, i32); 4];

pub(super) fn product(values: &[usize], stage: &str) -> Result<usize> {
    values
        .iter()
        .try_fold(1usize, |total, value| total.checked_mul(*value))
        .with_context(|| format!("OCR {stage} size overflow"))
}

fn sum(values: &[usize], stage: &str) -> Result<usize> {
    values
        .iter()
        .try_fold(0usize, |total, value| total.checked_add(*value))
        .with_context(|| format!("OCR {stage} size overflow"))
}

pub(super) fn reserved<T>(capacity: usize, stage: &str) -> Result<Vec<T>> {
    product(&[capacity, size_of::<T>()], stage)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .with_context(|| format!("OCR {stage} allocation refused ({capacity} elements)"))?;
    Ok(values)
}

pub(super) fn input_backing(image: &RgbaImage, budget: &InferenceBudget) -> Result<usize> {
    budget.check_input(image.width(), image.height())?;
    let retained = image.as_raw().capacity();
    if retained > budget.max_working_set_bytes {
        bail!(
            "OCR retained input backing {retained} bytes exceeds working-set budget {}",
            budget.max_working_set_bytes
        );
    }
    Ok(retained)
}

// image 0.25 Triangle resampling uses a source-width x destination-height
// Rgba32F intermediate, an RGBA output, and per-axis filter weights. Count a
// conservative weight-capacity allowance, including Vec growth. The library
// allocates these internally; admission precedes entering the resizer.
fn resize_scratch(width: u32, height: u32, out_width: u32, out_height: u32) -> Result<usize> {
    sum(
        &[
            product(&[width as usize, out_height as usize, 16], "resize intermediate")?,
            product(&[out_width as usize, out_height as usize, 4], "resize output")?,
            product(&[sum(&[width as usize, height as usize], "resize axes")?, 32], "resize weights")?,
            256,
        ],
        "resize scratch",
    )
}

pub(super) struct DetectionPlan {
    pub width: u32,
    pub height: u32,
}

impl DetectionPlan {
    pub fn new(image: &RgbaImage, limit: f32, budget: &InferenceBudget) -> Result<Self> {
        let input = input_backing(image, budget)?;
        if !limit.is_finite() || limit <= 0.0 {
            bail!("OCR detector resize limit must be finite and positive");
        }
        let (width, height) = image.dimensions();
        let max_dimension = width.max(height) as f32;
        let ratio = if max_dimension > limit { limit / max_dimension } else { 1.0 };
        let align = |side: u32| {
            ((side as f32 * ratio) as u32 / 32)
                .max(1)
                .checked_mul(32)
                .context("Detector dimension overflow")
        };
        let plan = Self {
            width: align(width)?,
            height: align(height)?,
        };
        let pixels = product(&[plan.width as usize, plan.height as usize], "detector pixels")?;
        let tensor = product(&[pixels, 3, 4], "detector tensor")?;
        // Fixed detector output is N1HW at input resolution. Include its f32
        // output plus a conservative allowance for threshold/contour storage
        // in imageproc (visited pixels, contour points and contour records).
        let postprocess = product(&[pixels, 132], "detector output/contours")?;
        let working = sum(
            &[input, tensor, postprocess, resize_scratch(width, height, plan.width, plan.height)?],
            "detector working set",
        )?;
        if tensor > budget.max_batch_tensor_bytes || working > budget.max_working_set_bytes {
            bail!(
                "OCR detector admission refused: tensor={tensor}, estimated working set={working} bytes (limits {}, {})",
                budget.max_batch_tensor_bytes,
                budget.max_working_set_bytes
            );
        }
        Ok(plan)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CropRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl CropRect {
    pub fn new(image_width: u32, image_height: u32, points: &BoxPoints, padding: i32) -> Result<Self> {
        if padding < 0 {
            bail!("OCR crop padding must be nonnegative");
        }
        let left = points.iter().map(|p| i64::from(p.0)).min().context("Empty OCR box")?;
        let right = points.iter().map(|p| i64::from(p.0)).max().context("Empty OCR box")?;
        let top = points.iter().map(|p| i64::from(p.1)).min().context("Empty OCR box")?;
        let bottom = points.iter().map(|p| i64::from(p.1)).max().context("Empty OCR box")?;
        let x = (left - i64::from(padding)).clamp(0, i64::from(image_width));
        let y = (top - i64::from(padding)).clamp(0, i64::from(image_height));
        let right = (right + i64::from(padding)).clamp(0, i64::from(image_width));
        let bottom = (bottom + i64::from(padding)).clamp(0, i64::from(image_height));
        if right <= x || bottom <= y {
            bail!("OCR crop has no pixels after clipping to its source");
        }
        Ok(Self {
            x: x as u32,
            y: y as u32,
            width: (right - x) as u32,
            height: (bottom - y) as u32,
        })
    }
}

#[derive(Debug)]
pub(super) struct CropPlan {
    pub original_index: usize,
    pub rect: CropRect,
    pub normalized_width: u32,
    pub padded_width: u32,
    scratch_bytes: usize,
}

#[derive(Debug)]
pub(super) struct BatchPlan {
    pub start: usize,
    pub end: usize,
    pub width: u32,
    pub tensor_elements: usize,
    pub output_bytes: usize,
    pub working_bytes: usize,
}

#[derive(Clone, Copy)]
pub(super) struct RecognitionShape {
    pub height: u32,
    pub classes: usize,
    pub max_key_bytes: usize,
}

pub(super) struct RecognitionPlan {
    pub crops: Vec<CropPlan>,
    pub batches: Vec<BatchPlan>,
}

// Bounds/plans/batches, ordered slots, temporary decoded records and final port
// records all have length <= box count. This also allows for Vec headers; a
// test audits the current record sizes. UTF-8 payload is accounted separately.
const BOOKKEEPING_PER_BOX: usize = 512;

impl RecognitionPlan {
    pub fn new(image: &RgbaImage, boxes: &[BoxPoints], padding: i32, shape: RecognitionShape, budget: &InferenceBudget) -> Result<Self> {
        let input = input_backing(image, budget)?;
        if boxes.len() > budget.max_boxes {
            bail!("OCR detected {} boxes, exceeds limit {}", boxes.len(), budget.max_boxes);
        }
        if shape.height == 0 || shape.classes == 0 || shape.max_key_bytes == 0 {
            bail!("OCR recognizer dimensions/dictionary must be positive");
        }
        let bookkeeping = product(&[boxes.len(), BOOKKEEPING_PER_BOX], "recognition bookkeeping")?;
        let mut base = sum(&[input, bookkeeping], "recognition retained set")?;
        if base > budget.max_working_set_bytes {
            bail!("OCR input and box bookkeeping exceed working-set budget");
        }
        let mut crops = reserved(boxes.len(), "crop plans")?;
        for (original_index, points) in boxes.iter().enumerate() {
            let rect = CropRect::new(image.width(), image.height(), points, padding)?;
            let crop_pixels = product(&[rect.width as usize, rect.height as usize], "crop pixels")?;
            if crop_pixels > budget.max_crop_pixels {
                bail!("OCR crop {original_index} exceeds pixel limit {}", budget.max_crop_pixels);
            }
            // Preserve the pinned preprocessing recipe, including f32 rounding:
            // divide target height first, multiply source width, then truncate.
            // Integer division changes valid widths (95x19 -> 239, not 240).
            let ratio = shape.height as f32 / rect.height as f32;
            let normalized_width = (rect.width as f32 * ratio) as u32;
            if normalized_width == 0 {
                bail!("OCR crop {original_index} rounds to zero normalized width");
            }
            let padded_width = normalized_width
                .checked_add(31)
                .map(|width| width / 32 * 32)
                .context("OCR width alignment overflow")?;
            if padded_width > budget.max_normalized_width {
                bail!(
                    "OCR crop {original_index} normalized width {padded_width} exceeds limit {}; text was not truncated",
                    budget.max_normalized_width
                );
            }
            let scratch_bytes = resize_scratch(rect.width, rect.height, normalized_width, shape.height)?;
            // Buckets keep padding below 2x an item's aligned width. Reserve
            // worst-case retained decoded UTF-8 for every item before batching.
            base = sum(
                &[base, product(&[padded_width as usize, 2, shape.max_key_bytes], "retained decoded text")?],
                "recognition retained set",
            )?;
            crops.push(CropPlan {
                original_index,
                rect,
                normalized_width,
                padded_width,
                scratch_bytes,
            });
        }
        if base > budget.max_working_set_bytes {
            bail!("OCR retained text estimate exceeds working-set budget");
        }
        crops.sort_unstable_by_key(|crop| (crop.padded_width, crop.original_index));
        let mut batches = reserved(crops.len(), "recognition batches")?;
        let mut start = 0;
        while start < crops.len() {
            let bucket = crops[start]
                .padded_width
                .checked_next_power_of_two()
                .context("OCR width bucket overflow")?;
            let mut end = start;
            let mut scratch = 0;
            let mut admitted = None;
            while end < crops.len() && crops[end].padded_width <= bucket {
                scratch = scratch.max(crops[end].scratch_bytes);
                let width = crops[end].padded_width;
                let count = end - start + 1;
                let tensor_elements = product(&[count, 3, shape.height as usize, width as usize], "recognition tensor")?;
                let tensor_bytes = product(&[tensor_elements, 4], "recognition tensor bytes")?;
                // Current pinned PP-OCRv5 models downsample the width. T <= W
                // deliberately overestimates their output without relying on a
                // precise stride. Run's actual shape is checked against it too;
                // this is an estimate, not a native ORT allocation restriction.
                let output_bytes = product(&[count, width as usize, shape.classes, 4], "recognizer NTC output")?;
                let working_bytes = sum(&[base, tensor_bytes, output_bytes, scratch], "recognition working set")?;
                if tensor_bytes > budget.max_batch_tensor_bytes || working_bytes > budget.max_working_set_bytes {
                    if end == start {
                        bail!(
                            "OCR crop {} cannot fit one recognition batch: input={tensor_bytes}, output estimate={output_bytes}, working set={working_bytes} bytes",
                            crops[start].original_index
                        );
                    }
                    break;
                }
                admitted = Some(BatchPlan {
                    start,
                    end: end + 1,
                    width,
                    tensor_elements,
                    output_bytes,
                    working_bytes,
                });
                end += 1;
            }
            let batch = admitted.context("OCR batch planner made no progress")?;
            start = batch.end;
            batches.push(batch);
        }
        Ok(Self { crops, batches })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocr::engine::OcrResult;
    use minnow_application::ports::OcrBlock;

    fn box_at(width: i32, height: i32) -> BoxPoints {
        [(0, 0), (width, 0), (width, height), (0, height)]
    }
    fn shape() -> RecognitionShape {
        RecognitionShape {
            height: 48,
            classes: 18_385,
            max_key_bytes: 4,
        }
    }

    #[test]
    fn width_groups_keep_indices_and_split_for_dictionary_output_budget() {
        let image = RgbaImage::new(400, 60);
        let budget = InferenceBudget {
            max_working_set_bytes: 27 * 1024 * 1024,
            ..Default::default()
        };
        let plan = RecognitionPlan::new(
            &image,
            &[box_at(320, 48), box_at(40, 48), box_at(300, 48), box_at(40, 48)],
            0,
            shape(),
            &budget,
        )
        .unwrap();
        // Raw widths 300/320 both admit padded width 320. Equal padded widths
        // keep their original indices rather than introducing another reorder.
        assert_eq!(plan.crops.iter().map(|crop| crop.original_index).collect::<Vec<_>>(), [1, 3, 0, 2]);
        assert_eq!(plan.batches.iter().map(|batch| batch.end - batch.start).collect::<Vec<_>>(), [2, 1, 1]);
        assert!(plan.batches.iter().all(|batch| batch.working_bytes <= budget.max_working_set_bytes));
    }

    #[test]
    fn planned_widths_preserve_the_original_f32_resize_recipe() {
        let image = RgbaImage::new(200, 20);
        let plan = RecognitionPlan::new(
            &image,
            &[box_at(95, 19), box_at(171, 19), box_at(190, 19)],
            0,
            shape(),
            &InferenceBudget::default(),
        )
        .unwrap();
        assert_eq!(plan.crops.iter().map(|crop| crop.normalized_width).collect::<Vec<_>>(), [239, 431, 479]);
        assert_eq!(plan.crops.iter().map(|crop| crop.padded_width).collect::<Vec<_>>(), [256, 448, 480]);
    }

    #[test]
    fn admission_rejects_backing_capacity_boxes_extreme_width_and_overflow() {
        let budget = InferenceBudget::default();
        let mut backing = Vec::with_capacity(4096);
        backing.resize(4, 0);
        let image = RgbaImage::from_raw(1, 1, backing).unwrap();
        assert!(
            input_backing(
                &image,
                &InferenceBudget {
                    max_working_set_bytes: 1024,
                    ..budget
                }
            )
            .is_err()
        );
        let image = RgbaImage::new(400, 60);
        assert!(RecognitionPlan::new(&image, &[box_at(400, 1)], 0, shape(), &budget).is_err());
        assert!(RecognitionPlan::new(&image, &[box_at(20, 20); 2], 0, shape(), &InferenceBudget { max_boxes: 1, ..budget }).is_err());
        assert!(
            RecognitionPlan::new(
                &image,
                &[box_at(20, 20)],
                0,
                RecognitionShape {
                    classes: usize::MAX,
                    ..shape()
                },
                &budget
            )
            .is_err()
        );
        assert!(product(&[usize::MAX, 2], "test").is_err());
        assert!(CropRect::new(400, 60, &box_at(0, 0), 0).is_err());
        assert!(CropRect::new(400, 60, &box_at(20, 20), i32::MAX).is_ok());
    }

    #[test]
    fn bookkeeping_allowance_covers_all_module_records() {
        let records = size_of::<BoxPoints>()
            + size_of::<CropPlan>()
            + size_of::<BatchPlan>()
            + size_of::<Option<(String, f32)>>()
            + size_of::<(String, f32)>()
            + size_of::<OcrResult>()
            + size_of::<OcrBlock>()
            + 6 * size_of::<Vec<()>>();
        assert!(records <= BOOKKEEPING_PER_BOX, "update bookkeeping estimate: {records}");
    }
}
