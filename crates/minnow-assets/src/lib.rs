use std::{borrow::Cow, collections::HashSet};

use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

pub mod asset_bytes;
pub mod asset_paths;

#[derive(RustEmbed)]
#[folder = "."]
// Keep these include patterns aligned with asset_paths::{LOGO_PATH, icons::*}.
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

#[cfg(test)]
mod tests {
    use super::AppAssets;
    use crate::{asset_bytes, asset_paths};
    use gpui::AssetSource;

    const ICONS_PREFIX: &str = "resources/icons/";

    #[test]
    fn app_assets_load_logo_and_icon() {
        let assets = AppAssets;
        let logo = assets
            .load(asset_paths::LOGO_PATH)
            .expect("failed to load logo")
            .expect("missing embedded logo");
        assert!(!logo.is_empty(), "embedded logo bytes should not be empty");

        let icon = assets
            .load(asset_paths::icons::CLOSE)
            .expect("failed to load close icon")
            .expect("missing embedded close icon");
        assert!(!icon.is_empty(), "embedded icon bytes should not be empty");
    }

    #[test]
    fn app_assets_list_icons_prefix() {
        let assets = AppAssets;
        let list = assets.list(ICONS_PREFIX).expect("failed to list icon assets");
        assert!(
            list.iter().any(|item| item.as_ref().starts_with(ICONS_PREFIX)),
            "icon list should include items under {}",
            ICONS_PREFIX
        );
        assert!(
            list.iter().any(|item| item.as_ref() == asset_paths::icons::CLOSE),
            "icon list should include {}",
            asset_paths::icons::CLOSE
        );
    }

    #[test]
    fn embedded_bytes_are_usable() {
        assert!(!asset_bytes::CAPTURE_SOUND_BYTES.is_empty(), "capture sound bytes should not be empty");
        let image = image::load_from_memory(asset_bytes::LOGO_BYTES).expect("failed to decode embedded logo");
        assert!(image.width() > 0 && image.height() > 0, "embedded logo dimensions must be positive");
    }
}
