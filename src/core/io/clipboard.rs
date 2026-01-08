use arboard::{Clipboard, ImageData};
use image::RgbaImage;
use log::{error, info};
use std::borrow::Cow;

pub fn copy_image_to_clipboard(image: &RgbaImage) -> bool {
    info!("Copying image ({}x{}) to clipboard...", image.width(), image.height());
    match Clipboard::new() {
        Ok(mut clipboard) => {
            let w = image.width() as usize;
            let h = image.height() as usize;

            let img_data = ImageData {
                width: w,
                height: h,
                bytes: Cow::Borrowed(image.as_raw()),
            };

            if let Err(e) = clipboard.set_image(img_data) {
                error!("Failed to set clipboard image: {}", e);
                false
            } else {
                info!("Image successfully copied to clipboard");
                true
            }
        }
        Err(e) => {
            error!("Failed to initialize clipboard: {}", e);
            false
        }
    }
}
