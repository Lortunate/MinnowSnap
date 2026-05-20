use gpui::RenderImage;
use image::{Frame, RgbaImage};
use std::sync::Arc;

pub fn from_rgba(mut image: RgbaImage) -> Arc<RenderImage> {
    normalize_for_gpui(&mut image);
    Arc::new(RenderImage::new([Frame::new(image)]))
}

pub fn from_rgba_copy(image: &RgbaImage) -> Arc<RenderImage> {
    from_rgba(image.clone())
}

fn normalize_for_gpui(image: &mut RgbaImage) {
    for pixel in image.chunks_exact_mut(4) {
        pixel[3] = 255;
        pixel.swap(0, 2);
    }
}
