use build_tools::icons::{embed_windows_icon, generate_icons};
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=resources/logo.png");
    println!("cargo:rerun-if-changed=resources");

    if let Err(e) = generate_icons(Path::new("resources/logo.png"), Path::new("assets_icons")) {
        println!("cargo:warning=Failed to generate icons: {}", e);
    }

    if let Err(e) = embed_windows_icon(Path::new("assets_icons/icon.ico")) {
        println!("cargo:warning=Failed to embed Windows icon: {}", e);
    }
}
