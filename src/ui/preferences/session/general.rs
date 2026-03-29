use super::{MutationResult, frame::SelectOption, store};
use crate::core::{
    app,
    appearance::{self, THEME_DARK, THEME_LIGHT, THEME_SYSTEM},
    i18n,
    i18n::SYSTEM_LOCALE,
    io::{fonts::get_system_fonts, storage::get_default_save_path},
    settings::AppSettings,
};
use gpui::{App, SharedString, Window};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneralSnapshot {
    pub(crate) language: SharedString,
    pub(crate) theme: SharedString,
    pub(crate) font_family: SharedString,
    pub(crate) auto_start: bool,
    pub(crate) save_directory_description: SharedString,
    pub(crate) oxipng_enabled: bool,
    pub(crate) language_options: Vec<SelectOption>,
    pub(crate) theme_options: Vec<SelectOption>,
    pub(crate) font_options: Vec<SelectOption>,
}

pub(crate) fn snapshot() -> GeneralSnapshot {
    let settings = store::snapshot();

    GeneralSnapshot {
        language: settings.general.language.clone().into(),
        theme: settings.general.theme.clone().into(),
        font_family: settings.general.font_family.clone().unwrap_or_default().into(),
        auto_start: settings.general.auto_start,
        save_directory_description: save_directory_description(&settings).into(),
        oxipng_enabled: settings.output.oxipng_enabled,
        language_options: language_options(),
        theme_options: theme_options(),
        font_options: font_options(),
    }
}

pub(crate) fn set_auto_start(enabled: bool) -> MutationResult {
    store::with_settings(|settings| {
        settings.set_auto_start(enabled);
        app::set_auto_start(enabled);
    });

    MutationResult::refresh_windows()
}

pub(crate) fn set_language(value: SharedString) -> MutationResult {
    let language = value.to_string();
    store::with_settings(|settings| settings.set_language(language.clone()));
    i18n::init(&language);
    MutationResult::refresh_windows()
}

pub(crate) fn set_theme(value: SharedString, window: &mut Window, cx: &mut App) -> MutationResult {
    let theme_choice = value.to_string();
    store::with_settings(|settings| settings.set_theme(theme_choice.clone()));
    appearance::apply_theme_choice(&theme_choice, Some(window), cx);
    MutationResult::NONE
}

pub(crate) fn set_font(value: SharedString, cx: &mut App) -> MutationResult {
    let font_family = value.to_string();
    store::with_settings(|settings| settings.set_font_family(font_family.clone()));

    let font_ref = if font_family.trim().is_empty() {
        None
    } else {
        Some(font_family.as_str())
    };
    appearance::apply_font_family(font_ref, cx);

    MutationResult::NONE
}

pub(crate) fn set_save_path(save_path: String) -> MutationResult {
    store::with_settings(|settings| settings.set_save_path(save_path));
    MutationResult::refresh_windows().clear_notice()
}

pub(crate) fn set_image_compression(enabled: bool) -> MutationResult {
    store::with_settings(|settings| settings.set_oxipng_enabled(enabled));
    MutationResult::refresh_windows()
}

fn available_font_values() -> Vec<SharedString> {
    std::iter::once(SharedString::from(""))
        .chain(get_system_fonts().into_iter().map(SharedString::from))
        .collect()
}

fn language_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new(SYSTEM_LOCALE, i18n::preferences::follow_system()),
        SelectOption::new("zh_CN", i18n::preferences::language_zh_cn()),
        SelectOption::new("en_US", i18n::preferences::language_en_us()),
    ]
}

fn theme_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new(THEME_SYSTEM, i18n::preferences::follow_system()),
        SelectOption::new(THEME_LIGHT, i18n::preferences::theme_light()),
        SelectOption::new(THEME_DARK, i18n::preferences::theme_dark()),
    ]
}

fn font_options() -> Vec<SelectOption> {
    available_font_values()
        .into_iter()
        .map(|value| {
            if value.is_empty() {
                SelectOption::new(value, i18n::preferences::follow_system())
            } else {
                SelectOption::new(value.clone(), value)
            }
        })
        .collect()
}

fn save_directory_description(settings: &AppSettings) -> String {
    if let Some(path) = settings.output.save_path.clone().filter(|path| !path.trim().is_empty()) {
        path
    } else {
        i18n::preferences::default_path_with_value(get_default_save_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::settings::{AppSettings, OutputSettings};

    #[test]
    fn font_options_include_system_option_first() {
        let snapshot = snapshot();

        assert!(!snapshot.font_options.is_empty());
        assert_eq!(snapshot.font_options[0].value, SharedString::from(""));
        assert_eq!(snapshot.font_options[0].label, SharedString::from(i18n::preferences::follow_system()));
    }

    #[test]
    fn save_directory_description_uses_custom_path_when_present() {
        let settings = AppSettings {
            output: OutputSettings {
                save_path: Some("D:/captures".to_string()),
                ..OutputSettings::default()
            },
            ..AppSettings::default()
        };

        assert_eq!(save_directory_description(&settings), "D:/captures");
    }

    #[test]
    fn save_directory_description_falls_back_to_default_label() {
        let settings = AppSettings::default();
        let description = save_directory_description(&settings);
        let default_path = get_default_save_path();

        assert!(description.starts_with(i18n::preferences::default_path().as_str()));
        assert!(description.contains(default_path.as_str()));
    }
}
