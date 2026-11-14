use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub(crate) const LOGO_BYTES: &[u8] = include_bytes!("../../resources/logo.png");
pub(crate) const CAPTURE_SOUND_BYTES: &[u8] = include_bytes!("../../resources/sounds/capture.mp3");

pub(crate) struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == "resources/logo.png" {
            return Ok(Some(Cow::Borrowed(LOGO_BYTES)));
        }
        minnow_ui::UiAssets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = minnow_ui::UiAssets.list(path)?;
        if "resources/logo.png".starts_with(path) {
            assets.push("resources/logo.png".into());
        }
        Ok(assets)
    }
}
