use std::{borrow::Cow, collections::HashSet};

use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

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
