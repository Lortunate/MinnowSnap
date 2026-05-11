use std::{borrow::Cow, collections::HashSet};

use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

pub mod asset_bytes {
    pub const LOGO_BYTES: &[u8] = include_bytes!("../../resources/logo.png");
    pub const CAPTURE_SOUND_BYTES: &[u8] = include_bytes!("../../resources/raw/capture.mp3");
}

pub mod asset_paths {
    pub const LOGO_PATH: &str = "resources/logo.png";

    pub mod icons {
        pub const ADD: &str = "resources/icons/add.svg";
        pub const ARROW_DROP_DOWN: &str = "resources/icons/arrow_drop_down.svg";
        pub const ARROW_DROP_UP: &str = "resources/icons/arrow_drop_up.svg";
        pub const ARROW_INSERT: &str = "resources/icons/arrow_insert.svg";
        pub const BLUR_ON: &str = "resources/icons/blur_on.svg";
        pub const CIRCLE: &str = "resources/icons/circle.svg";
        pub const CLOSE: &str = "resources/icons/close.svg";
        pub const COUNTER_1: &str = "resources/icons/counter_1.svg";
        pub const CROP_FREE: &str = "resources/icons/crop_free.svg";
        pub const FILE_COPY: &str = "resources/icons/file_copy.svg";
        pub const GRID_ON: &str = "resources/icons/grid_on.svg";
        pub const KEEP: &str = "resources/icons/keep.svg";
        pub const LENS_BLUR: &str = "resources/icons/lens_blur.svg";
        pub const REDO: &str = "resources/icons/redo.svg";
        pub const SAVE: &str = "resources/icons/save.svg";
        pub const SCROLL: &str = "resources/icons/scroll.svg";
        pub const SQUARE: &str = "resources/icons/square.svg";
        pub const SQUARE_FILL: &str = "resources/icons/square_fill.svg";
        pub const TEXT_FIELDS: &str = "resources/icons/text_fields.svg";
        pub const UNDO: &str = "resources/icons/undo.svg";
    }
}

#[derive(RustEmbed)]
#[folder = "."]
#[include = "resources/icons/**/*.svg"]
#[include = "resources/logo.png"]
struct ProjectAssets;

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        if let Some(file) = ProjectAssets::get(path) {
            return Ok(Some(file.data));
        }

        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = gpui_component_assets::Assets.list(path)?;
        assets.extend(ProjectAssets::iter().filter(|asset| asset.starts_with(path)).map(SharedString::from));

        let mut seen = HashSet::new();
        assets.retain(|asset| seen.insert(asset.clone()));
        Ok(assets)
    }
}
