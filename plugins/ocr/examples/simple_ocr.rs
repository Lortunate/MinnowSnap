use anyhow::{Context, Result};
use log::info;
use ocr::{OcrContext, OcrModelType};
use std::env;
use std::path::Path;

const IMAGE_URL: &str = "https://raw.githubusercontent.com/RapidAI/RapidOCR/main/python/tests/test_files/ch_en_num.jpg";
const IMAGE_PATH: &str = "./imgs/test_image.jpg";

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().collect();
    let model_type = if args.len() > 1 && args[1] == "mobile" {
        info!("Using Mobile models");
        OcrModelType::Mobile
    } else {
        info!("Using Server models (default). Pass 'mobile' as argument to use mobile models.");
        OcrModelType::Server
    };

    info!("Initializing OCR Context...");
    let mut ocr = OcrContext::new::<&Path>(None, Some(model_type), None, None, None, None).await?;

    let path = Path::new(IMAGE_PATH);
    if !path.exists() {
        info!("Downloading test image from {}...", IMAGE_URL);
        let bytes = reqwest::get(IMAGE_URL).await?.bytes().await?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create directory")?;
        }
        std::fs::write(path, bytes).context("Failed to write image file")?;
        info!("Image saved to {:?}", path);
    }

    info!("Loading image...");
    let image = image::open(path).context("Failed to open image")?;

    info!("Recognizing...");
    let start = std::time::Instant::now();
    let results = ocr.recognize(&image)?;
    let duration = start.elapsed();

    info!("OCR completed in {:?}", duration);

    for (i, res) in results.iter().enumerate() {
        info!("Result {}: Text='{}', Conf={:.4}, Box={:?}", i, res.text, res.confidence, res.box_points);
    }

    Ok(())
}
