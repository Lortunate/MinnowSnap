pub(crate) use super::image_dimensions::display_dimensions;
pub(crate) use super::image_dimensions::preview_is_reduced;
use gpui::RenderImage;
use image::{Frame, RgbaImage, imageops};
use std::sync::Arc;

pub fn from_rgba(image: RgbaImage) -> Arc<RenderImage> {
    let dimensions = display_dimensions(image.width(), image.height()).expect("display images must have a positive area");
    let image = if dimensions == image.dimensions() {
        image
    } else {
        imageops::thumbnail(&image, dimensions.0, dimensions.1)
    };
    upload(image)
}

pub fn from_rgba_copy(image: &RgbaImage) -> Arc<RenderImage> {
    let dimensions = display_dimensions(image.width(), image.height()).expect("display images must have a positive area");
    // Resize before copying so a very large original does not create a second full-size buffer.
    let image = if dimensions == image.dimensions() {
        image.clone()
    } else {
        imageops::thumbnail(image, dimensions.0, dimensions.1)
    };
    from_rgba(image)
}

fn upload(mut image: RgbaImage) -> Arc<RenderImage> {
    for pixel in image.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
        pixel.swap(0, 2);
    }
    Arc::new(RenderImage::new([Frame::new(image)]))
}
