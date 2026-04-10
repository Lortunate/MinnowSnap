use gpui::{App, Window};
use gpui_component::{Theme, ThemeMode};
use minnow_core::settings::SETTINGS;

pub const THEME_SYSTEM: &str = "System";
pub const THEME_LIGHT: &str = "Light";
pub const THEME_DARK: &str = "Dark";
const DEFAULT_FONT_FAMILY: &str = ".SystemUIFont";

pub fn apply_saved_preferences(window: Option<&mut Window>, cx: &mut App) {
    let settings = SETTINGS.lock().map(|guard| guard.get()).unwrap_or_default();
    apply_theme_choice_inner(&settings.general.theme, window, cx);
    apply_font_family_inner(settings.general.font_family.as_deref(), cx);
    cx.refresh_windows();
}

pub fn apply_theme_choice(theme: &str, window: Option<&mut Window>, cx: &mut App) {
    apply_theme_choice_inner(theme, window, cx);
    cx.refresh_windows();
}

pub fn apply_font_family(font_family: Option<&str>, cx: &mut App) {
    apply_font_family_inner(font_family, cx);
    cx.refresh_windows();
}

fn apply_theme_choice_inner(theme: &str, window: Option<&mut Window>, cx: &mut App) {
    match theme.trim() {
        THEME_LIGHT => Theme::change(ThemeMode::Light, window, cx),
        THEME_DARK => Theme::change(ThemeMode::Dark, window, cx),
        _ => Theme::sync_system_appearance(window, cx),
    }
}

fn apply_font_family_inner(font_family: Option<&str>, cx: &mut App) {
    let family = font_family
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_FONT_FAMILY)
        .to_string();
    Theme::global_mut(cx).font_family = family.into();
}
