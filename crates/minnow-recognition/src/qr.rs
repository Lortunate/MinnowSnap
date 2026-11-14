use image::RgbaImage;
use minnow_application::ports::{ErrorKind, OperationError, OperationResult, QrDecoder};

/// Stateless QR decoding with no OCR runtime, model directory, or download policy.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalQrDecoder;

impl LocalQrDecoder {
    pub const fn new() -> Self {
        Self
    }
}

impl QrDecoder for LocalQrDecoder {
    fn decode_qr(&self, image: &RgbaImage) -> OperationResult<Option<String>> {
        if image.width() == 0 || image.height() == 0 {
            return Err(OperationError::new(ErrorKind::InvalidInput, "QR input image has no pixels"));
        }
        let gray = image::imageops::grayscale(image);
        let (width, height) = gray.dimensions();
        let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(width as usize, height as usize, |x, y| gray.get_pixel(x as u32, y as u32)[0]);
        Ok(prepared
            .detect_grids()
            .into_iter()
            .find_map(|grid| grid.decode().ok().map(|(_, content)| content)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_decoder_needs_no_model_service_and_validates_empty_input() {
        let decoder = LocalQrDecoder::new();
        assert_eq!(decoder.decode_qr(&RgbaImage::new(0, 0)).unwrap_err().kind, ErrorKind::InvalidInput);
        assert_eq!(decoder.decode_qr(&RgbaImage::new(8, 8)).unwrap(), None);
    }
}
