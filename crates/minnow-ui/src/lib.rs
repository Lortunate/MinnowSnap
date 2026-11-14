//! GPUI presentation and native-window integration for MinnowSnap.

mod features;
mod flows;
mod host;
mod navigation;
mod presentation;
mod windowing;

pub use host::{EffectiveShortcuts, UiHost, UiPaths};
pub use presentation::assets::AppAssets as UiAssets;
pub use presentation::i18n;

use gpui::{App, Global};
use minnow_application::ApplicationServices;
use std::rc::Rc;
use std::sync::Arc;

rust_i18n::i18n!("resources/locales", fallback = "en");

#[derive(Clone)]
pub(crate) struct UiServices {
    pub application: Arc<ApplicationServices>,
    pub host: Rc<dyn UiHost>,
    pub native_preferences: Rc<dyn minnow_application::preferences::NativePreferences>,
}

impl Global for UiServices {}

pub(crate) fn application(cx: &App) -> Arc<ApplicationServices> {
    cx.global::<UiServices>().application.clone()
}

pub(crate) fn host(cx: &App) -> Rc<dyn UiHost> {
    cx.global::<UiServices>().host.clone()
}

pub fn install(
    cx: &mut App,
    services: Arc<ApplicationServices>,
    host: Rc<dyn UiHost>,
    native_preferences: Rc<dyn minnow_application::preferences::NativePreferences>,
) {
    cx.set_global(UiServices {
        application: services,
        host,
        native_preferences,
    });
    presentation::locale::apply(&application(cx).settings_snapshot().general.language);
    presentation::appearance::apply_saved_preferences(None, cx);
    navigation::install(cx);
    features::overlay::bind_keys(cx);
    features::pin::bind_keys(cx);
    features::pin::install(cx);
    let overlay = features::overlay::OverlayHandle::new(cx);
    cx.set_global(overlay);
}

pub fn open_capture(cx: &mut App) {
    navigation::open_capture(cx);
}

pub fn open_preferences(cx: &mut App) {
    features::preferences::open_window(cx);
}

pub fn close_all(cx: &mut App) {
    navigation::close_all(cx);
}
