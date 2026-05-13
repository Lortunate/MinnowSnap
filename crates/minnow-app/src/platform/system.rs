use gpui::{App, Global};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct UiSystemActions {
    set_auto_start: Arc<dyn Fn(bool) + Send + Sync>,
}

impl Global for UiSystemActions {}

impl UiSystemActions {
    fn new<F>(set_auto_start: F) -> Self
    where
        F: Fn(bool) + Send + Sync + 'static,
    {
        Self {
            set_auto_start: Arc::new(set_auto_start),
        }
    }

    pub(crate) fn set_auto_start(&self, enabled: bool) {
        (self.set_auto_start)(enabled);
    }
}

pub(crate) fn install_ui_system_actions<F>(cx: &mut App, set_auto_start: F)
where
    F: Fn(bool) + Send + Sync + 'static,
{
    cx.set_global(UiSystemActions::new(set_auto_start));
}
