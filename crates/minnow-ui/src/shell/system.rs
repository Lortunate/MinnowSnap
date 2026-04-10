use gpui::{App, Global};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct UiSystemActions {
    set_auto_start: Arc<dyn Fn(bool) + Send + Sync>,
}

impl Global for UiSystemActions {}

impl UiSystemActions {
    pub fn new<F>(set_auto_start: F) -> Self
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        Self {
            set_auto_start: Arc::new(set_auto_start),
        }
    }

    pub fn set_auto_start(&self, enabled: bool) {
        (self.set_auto_start)(enabled);
    }
}

pub fn install_ui_system_actions<F>(cx: &mut App, set_auto_start: F)
where
    F: Fn(bool) + Send + Sync + 'static,
{
    cx.set_global(UiSystemActions::new(set_auto_start));
}

pub fn open_external_url(app: &mut App, url: &str) {
    app.open_url(url);
}

pub fn open_in_file_manager(app: &mut App, path: impl AsRef<Path>) {
    app.open_with_system(path.as_ref());
}
