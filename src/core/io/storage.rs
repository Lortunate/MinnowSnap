use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use log::{error, info};
use std::fs;
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[must_use]
pub fn get_default_save_path() -> String {
    if let Some(mut d) = dirs::picture_dir() {
        d.push("MinnowSnap");
        let path_str = d.to_string_lossy();
        return format!("file://{path_str}");
    }
    String::new()
}

pub fn clean_url_path(path: &str) -> String {
    let mut clean_path = path.strip_prefix("file://").unwrap_or(path).to_string();
    if cfg!(target_os = "windows") && clean_path.starts_with('/') {
        clean_path.remove(0);
    }
    clean_path
}

#[must_use]
pub fn save_image_to_temp(image: &RgbaImage, compress: bool) -> Option<String> {
    let mut path = std::env::temp_dir();
    path.push("minnowsnap_preview.png");

    if compress {
        save_compressed_png(image, &path)
    } else {
        save_uncompressed_png(image, &path)
    }
}

#[must_use]
pub fn save_image_to_unique_temp(image: &RgbaImage, compress: bool) -> Option<String> {
    let mut path = std::env::temp_dir();
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    path.push(format!("minnowsnap_pin_{timestamp}.png"));

    if compress {
        save_compressed_png(image, &path)
    } else {
        save_uncompressed_png(image, &path)
    }
}

#[must_use]
pub fn save_image_to_user_dir(image: &RgbaImage, compress: bool, custom_path: Option<String>) -> Option<String> {
    let mut dir = if let Some(path) = custom_path.filter(|s| !s.is_empty()) {
        PathBuf::from(path)
    } else if let Some(mut d) = dirs::picture_dir() {
        d.push("MinnowSnap");
        d
    } else {
        error!("Could not determine save directory");
        return None;
    };

    if let Err(e) = fs::create_dir_all(&dir) {
        error!("Failed to create directory {:?}: {}", dir, e);
        return None;
    }

    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    dir.push(format!("snap_{timestamp}.png"));

    if compress {
        save_compressed_png(image, &dir)
    } else {
        save_uncompressed_png(image, &dir)
    }
}

fn save_uncompressed_png(image: &RgbaImage, path: &PathBuf) -> Option<String> {
    let file = match fs::File::create(path) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to create output file: {e}");
            return None;
        }
    };

    let writer = BufWriter::with_capacity(64 * 1024, file);
    let encoder = PngEncoder::new(writer);

    if let Err(e) = encoder.write_image(image.as_raw(), image.width(), image.height(), ExtendedColorType::Rgba8) {
        error!("Failed to encode/save image: {e}");
        return None;
    }

    let path_str = path.to_string_lossy().to_string();
    info!("Image saved successfully (buffered): {path_str}");
    Some(path_str)
}

fn save_compressed_png(image: &RgbaImage, path: &PathBuf) -> Option<String> {
    let start_time = Instant::now();

    let mut options = oxipng::Options::from_preset(0);
    options.strip = oxipng::StripChunks::All;
    options.interlace = None;

    let raw = match oxipng::RawImage::new(
        image.width(),
        image.height(),
        oxipng::ColorType::RGBA,
        oxipng::BitDepth::Eight,
        image.as_raw().to_vec(),
    ) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to create raw image: {e}");
            return None;
        }
    };

    let raw_size = image.as_raw().len();

    match raw.create_optimized_png(&options) {
        Ok(optimized) => {
            let optimized_size = optimized.len();
            let duration = start_time.elapsed();
            info!("PNG Compression: Raw RGBA {raw_size} bytes -> Optimized PNG {optimized_size} bytes, Time: {duration:?}");

            if let Err(e) = fs::write(path, optimized) {
                error!("Failed to write optimized image to disk: {e}");
                return None;
            }
            let path_str = path.to_string_lossy().to_string();
            info!("Image saved successfully (optimized): {path_str}");
            Some(path_str)
        }
        Err(e) => {
            error!("Failed to optimize image: {e}");
            save_uncompressed_png(image, path)
        }
    }
}
