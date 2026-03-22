use build_tools::i18n::compile_all_translations;
use build_tools::icons::{embed_windows_icon, generate_icons};
use build_tools::resources::update_resources;
use build_tools::utils::{collect_bridge_files, collect_qml_files};
use cxx_qt_build::{CxxQtBuilder, QmlModule};
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=resources/logo.png");
    println!("cargo:rerun-if-changed=resources");
    println!("cargo:rerun-if-changed=qml");
    println!("cargo:rerun-if-changed=src/bridge");
    println!("cargo:rerun-if-changed=resources/i18n");

    if let Err(e) = generate_icons(Path::new("resources/logo.png"), Path::new("assets_icons")) {
        println!("cargo:warning=Failed to generate icons: {}", e);
    }

    if let Err(e) = embed_windows_icon(Path::new("assets_icons/icon.ico")) {
        println!("cargo:warning=Failed to embed Windows icon: {}", e);
    }

    if let Err(e) = compile_all_translations(Path::new("resources/i18n")) {
        println!("cargo:warning=Failed to compile translations: {}", e);
    }

    if let Err(e) = update_resources(Path::new("resources.qrc"), Path::new("resources")) {
        println!("cargo:warning=Failed to update resources: {}", e);
    }

    let qml_files = collect_qml_files(Path::new("qml"));
    let bridge_files = collect_bridge_files(Path::new("src/bridge"));

    let mut builder = CxxQtBuilder::new_qml_module(QmlModule::new("com.lortunate.minnow").qml_files(qml_files))
        .qrc("resources.qrc")
        .files(bridge_files)
        .qt_module("Quick");

    unsafe {
        builder = builder.cc_builder(|cc| {
            cc.file("cpp/qt_logging.cpp");
        });
    }

    if cfg!(target_os = "macos") {
        unsafe {
            builder = builder.cc_builder(|cc| {
                cc.file("cpp/macos_window_utils.mm").flag("-fobjc-arc");
            });
        }
        println!("cargo:rustc-link-lib=framework=Cocoa");
    }

    builder.build();
}
