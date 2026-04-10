pub fn apply(language: &str) -> String {
    let locale = minnow_core::i18n::init(language);
    gpui_component::set_locale(&locale);
    locale
}
