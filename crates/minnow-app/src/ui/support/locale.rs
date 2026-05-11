pub fn apply(language: &str) -> String {
    let locale = crate::services::i18n::init(language);
    gpui_component::set_locale(&locale);
    locale
}


