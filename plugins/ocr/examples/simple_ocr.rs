use ab_glyph::FontRef;
use anyhow::{Context, Result};
use log::info;
use ocr::{visualization, OcrContext, OcrModelType};
use std::env;
use std::path::Path;

const IMAGE_URL: &str = "https://raw.githubusercontent.com/RapidAI/RapidOCR/main/python/tests/test_files/ch_en_num.jpg";
const IMAGE_PATH: &str = "./er/test_image.jpg";
const FONT_URL: &str = "https://github.com/StellarCN/scp_zh/raw/master/fonts/SimHei.ttf";
const FONT_PATH: &str = "./er/SimHei.ttf";

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().collect();
    let model_type = if args.len() > 1 && args[1] == "mobile" {
        OcrModelType::Mobile
    } else {
        OcrModelType::Server
    };

    let mut ocr = OcrContext::new::<&Path>(None, Some(model_type), None, None, None, None).await?;

    let image_path = Path::new(IMAGE_PATH);
    ensure_file(IMAGE_URL, image_path).await?;

    let font_path = Path::new(FONT_PATH);
    ensure_file(FONT_URL, font_path).await?;

    let image = image::open(image_path).context("Failed to open image")?;

    let start = std::time::Instant::now();
    let results = ocr.recognize(&image)?;
    let duration = start.elapsed();

    info!("OCR completed in {:?}", duration);

    for (i, res) in results.iter().enumerate() {
        info!("Result {}: Text='{}', Conf={:.4}, Box={:?}", i, res.text, res.confidence, res.box_points);
    }

    let font_bytes = std::fs::read(font_path).context("Failed to read font file")?;
    let font = FontRef::try_from_slice(&font_bytes).context("Failed to parse font")?;

    let output_image = visualization::draw_ocr_results(&image, &results, &font);

    let output_dir = Path::new("./output");
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir).context("Failed to create output directory")?;
    }
    let output_path = output_dir.join("ocr_result.jpg");

    output_image.save(&output_path).context("Failed to save output image")?;
    info!("Visualization saved to {:?}", output_path);

    Ok(())
}

async fn ensure_file(url: &str, path: &Path) -> Result<()> {
    if !path.exists() {
        info!("Downloading {}...", url);
        let bytes = reqwest::get(url).await?.bytes().await?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create directory")?;
        }
        std::fs::write(path, bytes).context("Failed to write file")?;
        info!("Saved to {:?}", path);
    }
    Ok(())
}
