use super::plan::{BatchPlan, CropPlan, product, reserved};
use anyhow::Result;
use image::{RgbaImage, imageops::FilterType};
use ndarray::Array4;

pub const PPOCR_MEAN: [f32; 3] = [0.5, 0.5, 0.5];
pub const PPOCR_STD: [f32; 3] = [0.5, 0.5, 0.5];

fn write_normalized(image: &RgbaImage, values: &mut [f32], width: usize) {
    let plane = image.height() as usize * width;
    for (x, y, pixel) in image.enumerate_pixels() {
        let index = y as usize * width + x as usize;
        for channel in 0..3 {
            values[channel * plane + index] = (pixel[channel] as f32 / 255.0 - PPOCR_MEAN[channel]) / PPOCR_STD[channel];
        }
    }
}

pub(super) fn normalize_image(image: &RgbaImage) -> Result<Array4<f32>> {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let elements = product(&[3, height, width], "detector normalization")?;
    let mut values = reserved(elements, "detector normalization")?;
    values.resize(elements, 0.0);
    write_normalized(image, &mut values, width);
    Ok(Array4::from_shape_vec((1, 3, height, width), values)?)
}

/// One admitted batch, one resampled crop at a time. Source crops are views;
/// pixels and per-channel tensors are never retained for the whole image.
pub(super) fn preprocess_batch(image: &RgbaImage, crops: &[CropPlan], batch: &BatchPlan, height: u32) -> Result<Array4<f32>> {
    let expected = product(&[crops.len(), 3, height as usize, batch.width as usize], "recognition normalization")?;
    if expected != batch.tensor_elements || crops.len() != batch.end - batch.start {
        anyhow::bail!("OCR preprocessing does not match the admitted batch");
    }
    let mut values = reserved(batch.tensor_elements, "recognition batch tensor")?;
    values.resize(batch.tensor_elements, 0.0);
    let stride = product(&[3, height as usize, batch.width as usize], "recognition sample")?;
    for (crop, sample) in crops.iter().zip(values.chunks_exact_mut(stride)) {
        let rect = crop.rect;
        let view = image::imageops::crop_imm(image, rect.x, rect.y, rect.width, rect.height);
        let resized = image::imageops::resize(&*view, crop.normalized_width, height, FilterType::Triangle);
        write_normalized(&resized, sample, batch.width as usize);
    }
    Ok(Array4::from_shape_vec((crops.len(), 3, height as usize, batch.width as usize), values)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        InferenceBudget,
        ocr::plan::{CropRect, RecognitionPlan, RecognitionShape},
    };
    use image::{GenericImageView, Rgba};

    #[test]
    fn text_crops_keep_original_source_pixel_coordinates() {
        let source = RgbaImage::from_fn(100, 80, |x, y| Rgba([x as u8, y as u8, 42, 255]));
        let rect = CropRect::new(100, 80, &[(20, 10), (40, 10), (40, 25), (20, 25)], 3).unwrap();
        let crop = image::imageops::crop_imm(&source, rect.x, rect.y, rect.width, rect.height);
        assert_eq!(crop.dimensions(), (26, 21));
        assert_eq!(crop.get_pixel(0, 0), *source.get_pixel(17, 7));
        assert_eq!(crop.get_pixel(25, 20), *source.get_pixel(42, 27));
    }

    #[test]
    fn batch_tensor_is_contiguous_nchw_with_padding_and_all_pixels_preserved() {
        let source = RgbaImage::from_pixel(64, 48, Rgba([255, 0, 128, 255]));
        let boxes = [[(0, 0), (33, 0), (33, 48), (0, 48)], [(0, 0), (64, 0), (64, 48), (0, 48)]];
        let shape = RecognitionShape {
            height: 48,
            classes: 4,
            max_key_bytes: 4,
        };
        let plan = RecognitionPlan::new(&source, &boxes, 0, shape, &InferenceBudget::default()).unwrap();
        assert_eq!(plan.batches.len(), 1);
        let tensor = preprocess_batch(&source, &plan.crops, &plan.batches[0], 48).unwrap();
        assert!(tensor.is_standard_layout());
        assert_eq!(tensor.shape(), [2, 3, 48, 64]);
        assert_eq!(tensor[[0, 0, 47, 32]], 1.0);
        assert_eq!(tensor[[0, 1, 47, 32]], -1.0);
        assert_eq!(tensor[[0, 0, 47, 33]], 0.0);
        assert_eq!(tensor[[1, 0, 47, 63]], 1.0);
    }
}
