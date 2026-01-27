use arboard::{Clipboard, ImageData};
use image::RgbaImage;
use log::{error, info};
use std::borrow::Cow;

pub fn copy_image_to_clipboard(image: &RgbaImage) -> bool {
    info!("Copying image ({}x{}) to clipboard...", image.width(), image.height());

    let mut clipboard = match Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to initialize clipboard: {}", e);
            return false;
        }
    };

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

pub fn copy_text_to_clipboard(text: String) -> bool {
    info!("Copying text to clipboard (len: {})...", text.len());

    let mut clipboard = match Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to initialize clipboard: {}", e);
            return false;
        }
    };

    if let Err(e) = clipboard.set_text(text) {
        error!("Failed to set clipboard text: {}", e);
        false
    } else {
        info!("Text successfully copied to clipboard");
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Depends on system clipboard availability"]
    fn test_clipboard_text_copy() {
        let text = "MinnowSnap Test Text";
        let result = copy_text_to_clipboard(text.to_string());

        if std::env::var("CI").is_err() {
            assert!(result, "Clipboard copy should succeed in non-CI environment");
        }
    }
}
