use anyhow::{Context, Result};
use ico::{IconDir, IconImage, ResourceType};
use image::{
    imageops::{self, FilterType}, DynamicImage,
    RgbaImage,
};
use std::{env, fs, path::Path, process::Command};

fn process_img(img: &DynamicImage, size: u32, scale: f64) -> DynamicImage {
    if (scale - 1.0).abs() < f64::EPSILON {
        return img.resize(size, size, FilterType::Lanczos3);
    }

    let new_size = (size as f64 * scale) as u32;
    let mut bg = RgbaImage::new(size, size);

    let fg = img.resize(new_size, new_size, FilterType::Lanczos3);

    let x = (size - new_size) / 2;
    let y = (size - new_size) / 2;

    imageops::overlay(&mut bg, &fg, x as i64, y as i64);

    DynamicImage::ImageRgba8(bg)
}

pub fn generate_icons(src: &Path, out_dir: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }

    fs::create_dir_all(out_dir)?;

    let img = image::open(src).context("Failed to open source image")?;

    let mut icon_dir = IconDir::new(ResourceType::Icon);
    let ico_sizes = [16, 24, 32, 48, 64, 128, 256];

    for size in ico_sizes {
        let processed = process_img(&img, size, 1.0);
        let rgba = processed.to_rgba8();
        let icon_image = IconImage::from_rgba_data(size, size, rgba.into_vec());
        icon_dir.add_entry(ico::IconDirEntry::encode(&icon_image)?);
    }

    let file = fs::File::create(out_dir.join("icon.ico"))?;
    icon_dir.write(file)?;

    if cfg!(target_os = "macos") && Path::new("/usr/bin/iconutil").exists() {
        let tmp_dir = out_dir.join("icon.iconset");
        fs::create_dir_all(&tmp_dir)?;

        let configs = [
            (16, "icon_16x16.png"),
            (32, "icon_16x16@2x.png"),
            (32, "icon_32x32.png"),
            (64, "icon_32x32@2x.png"),
            (128, "icon_128x128.png"),
            (256, "icon_128x128@2x.png"),
            (256, "icon_256x256.png"),
            (512, "icon_256x256@2x.png"),
            (512, "icon_512x512.png"),
            (1024, "icon_512x512@2x.png"),
        ];

        for (size, name) in configs {
            let processed = process_img(&img, size, 0.82);
            processed.save(tmp_dir.join(name))?;
        }

        let status = Command::new("iconutil")
            .args(&["-c", "icns"])
            .arg(&tmp_dir)
            .arg("-o")
            .arg(out_dir.join("icon.icns"))
            .status()?;

        if status.success() {
            fs::remove_dir_all(tmp_dir)?;
            println!("Done: {}/icon.icns", out_dir.display());
        }
    }

    Ok(())
}

pub fn embed_windows_icon(icon_path: &Path) -> Result<()> {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon(icon_path.to_str().context("Invalid icon path")?);
        res.compile().context("Failed to compile Windows resources")?;
    }
    Ok(())
}
