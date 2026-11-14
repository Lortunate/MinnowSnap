use super::frame::SelectOption;
use crate::presentation::i18n::{self, SYSTEM_LOCALE};
use gpui::SharedString;
use minnow_application::settings::{AppSettings, THEME_DARK, THEME_LIGHT, THEME_SYSTEM};
use std::path::PathBuf;

pub(super) fn language_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new(SYSTEM_LOCALE, i18n::preferences::follow_system()),
        SelectOption::new("zh_CN", i18n::preferences::language_zh_cn()),
        SelectOption::new("en_US", i18n::preferences::language_en_us()),
    ]
}

pub(super) fn theme_options() -> Vec<SelectOption> {
    vec![
        SelectOption::new(THEME_SYSTEM, i18n::preferences::follow_system()),
        SelectOption::new(THEME_LIGHT, i18n::preferences::theme_light()),
        SelectOption::new(THEME_DARK, i18n::preferences::theme_dark()),
    ]
}

pub(super) fn font_options(fonts: &[SharedString]) -> Vec<SelectOption> {
    std::iter::once(SelectOption::new("", i18n::preferences::follow_system()))
        .chain(fonts.iter().map(|value| {
            if value.is_empty() {
                SelectOption::new(value.clone(), i18n::preferences::follow_system())
            } else {
                SelectOption::new(value.clone(), value.clone())
            }
        }))
        .collect()
}

pub(super) fn save_directory_description(settings: &AppSettings, default_path: &Result<PathBuf, String>) -> String {
    match settings.output.save_path.as_custom_str() {
        Some(path) => path.to_owned(),
        None => match default_path {
            Ok(path) => match path.to_str() {
                Some(path) => i18n::preferences::default_path_with_value(path),
                None => i18n::preferences::default_path_not_displayable(),
            },
            Err(message) => i18n::preferences::default_path_unavailable(message),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minnow_application::settings::{AppSettings, OutputSettings, SaveDirectory};

    #[test]
    fn save_directory_description_uses_custom_path_when_present() {
        let settings = AppSettings {
            output: OutputSettings {
                save_path: SaveDirectory::new("  D:/captures  ").expect("valid directory"),
                ..OutputSettings::default()
            },
            ..AppSettings::default()
        };

        assert_eq!(save_directory_description(&settings, &Ok("/captures".into())), "  D:/captures  ");
    }

    #[test]
    fn save_directory_description_falls_back_to_default_label() {
        let settings = AppSettings::default();
        let default_path = "/captures";
        let description = save_directory_description(&settings, &Ok(default_path.into()));

        assert_eq!(description, i18n::preferences::default_path_with_value(default_path));
    }

    #[test]
    fn default_directory_description_reports_unavailable_or_undisplayable_paths() {
        let settings = AppSettings::default();
        assert_eq!(
            save_directory_description(&settings, &Err("Pictures directory unavailable".into())),
            i18n::preferences::default_path_unavailable("Pictures directory unavailable")
        );
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let native = PathBuf::from(std::ffi::OsString::from_vec(b"/captures/\xff".to_vec()));
            let description = save_directory_description(&settings, &Ok(native));
            assert_eq!(description, i18n::preferences::default_path_not_displayable());
            assert!(
                !description.contains('\u{fffd}'),
                "a path display must not pretend replacement characters are the native name"
            );
        }
    }
}
