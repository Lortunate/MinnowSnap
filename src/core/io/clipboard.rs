use arboard::{Clipboard, ImageData};
use image::RgbaImage;
use log::{error, info};
use std::borrow::Cow;
use std::sync::{LazyLock, Mutex};

static CLIPBOARD: LazyLock<Mutex<Option<Clipboard>>> = LazyLock::new(|| Mutex::new(None));

pub fn copy_image_to_clipboard(image: &RgbaImage) -> bool {
    info!("Copying image ({}x{}) to clipboard...", image.width(), image.height());

    let mut guard = CLIPBOARD.lock().unwrap();

    if guard.is_none() {
        match Clipboard::new() {
            Ok(c) => *guard = Some(c),
            Err(e) => {
                error!("Failed to initialize clipboard: {}", e);
                return false;
            }
        }
    }

    if let Some(clipboard) = guard.as_mut() {
        let w = image.width() as usize;
        let h = image.height() as usize;

        let img_data = ImageData {
            width: w,
            height: h,
            bytes: Cow::Borrowed(image.as_raw()),
        };

        if let Err(e) = clipboard.set_image(img_data) {
            error!("Failed to set clipboard image: {}", e);
            *guard = None; // Reset clipboard on failure to force re-initialization next time
            false
        } else {
            info!("Image successfully copied to clipboard");
            true
        }
    } else {
        false
    }
}
