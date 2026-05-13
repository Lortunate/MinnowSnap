use gpui::RenderImage;
use image::{Frame, RgbaImage};
use std::sync::Arc;

pub fn from_rgba(mut image: RgbaImage) -> Arc<RenderImage> {
    for pixel in image.chunks_exact_mut(4) {
        pixel[3] = 255;
        pixel.swap(0, 2);
    }

    Arc::new(RenderImage::new([Frame::new(image)]))
}
