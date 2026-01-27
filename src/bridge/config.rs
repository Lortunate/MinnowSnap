use crate::core::io::fonts::get_system_fonts;
use crate::core::io::storage::{clean_url_path, get_default_save_path};
use crate::core::settings::SETTINGS;
use cxx_qt_lib::{QString, QStringList};
use std::pin::Pin;

macro_rules! update_prop {
    ($self:expr, $val:expr, $getter:ident, $setter:ident, $rust_setter:ident) => {
        if $self.$getter() == &$val {
            return;
        }
        let val_str = $val.to_string();
        $self.as_mut().$setter($val);
        SETTINGS.lock().unwrap().$rust_setter(val_str);
    };
    ($self:expr, $val:expr, $getter:ident, $setter:ident, $rust_setter:ident, bool) => {
        if *$self.$getter() == $val {
            return;
        }
        $self.as_mut().$setter($val);
        SETTINGS.lock().unwrap().$rust_setter($val);
    };
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, oxipng_enabled, cxx_name = "oxipngEnabled")]
        #[qproperty(bool, auto_start, cxx_name = "autoStart")]
        #[qproperty(bool, enable_ocr, cxx_name = "enableOcr")]
        #[qproperty(QString, save_path, cxx_name = "savePath")]
        #[qproperty(QString, font_family, cxx_name = "fontFamily")]
        #[qproperty(QString, theme)]
        #[qproperty(QString, language)]
        #[qproperty(QString, version)]
        #[qproperty(QString, capture_shortcut, cxx_name = "captureShortcut")]
        #[qproperty(QString, quick_capture_shortcut, cxx_name = "quickCaptureShortcut")]
        type Config = super::ConfigRust;

        #[qinvokable]
        #[cxx_name = "updateTheme"]
        fn update_theme(self: Pin<&mut Self>, theme: QString);

        #[qinvokable]
        #[cxx_name = "updateLanguage"]
        fn update_language(self: Pin<&mut Self>, language: QString);

        #[qinvokable]
        #[cxx_name = "updateOxipngEnabled"]
        fn update_oxipng_enabled(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "updateAutoStart"]
        fn update_auto_start(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "updateEnableOcr"]
        fn update_enable_ocr(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "updateSavePath"]
        fn update_save_path(self: Pin<&mut Self>, path: QString);

        #[qinvokable]
        #[cxx_name = "updateFontFamily"]
        fn update_font_family(self: Pin<&mut Self>, family: QString);

        #[qinvokable]
        #[cxx_name = "updateCaptureShortcut"]
        fn update_capture_shortcut(self: Pin<&mut Self>, shortcut: QString);

        #[qinvokable]
        #[cxx_name = "updateQuickCaptureShortcut"]
        fn update_quick_capture_shortcut(self: Pin<&mut Self>, shortcut: QString);

        #[qinvokable]
        #[cxx_name = "getSystemFonts"]
        fn get_system_fonts(self: &Self) -> QStringList;

        #[qinvokable]
        #[cxx_name = "getDefaultSavePath"]
        fn get_default_save_path(self: &Self) -> QString;

        #[qinvokable]
        #[cxx_name = "getSupportedLanguages"]
        fn get_supported_languages(self: &Self) -> QStringList;

        #[qinvokable]
        #[cxx_name = "loadSettings"]
        fn load_settings(self: Pin<&mut Self>);
    }
}

pub struct ConfigRust {
    oxipng_enabled: bool,
    auto_start: bool,
    enable_ocr: bool,
    save_path: QString,
    font_family: QString,
    theme: QString,
    language: QString,
    version: QString,
    capture_shortcut: QString,
    quick_capture_shortcut: QString,
}

impl Default for ConfigRust {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigRust {
    pub fn new() -> Self {
        let settings = SETTINGS.lock().unwrap().get();
        Self {
            oxipng_enabled: settings.output.oxipng_enabled,
            auto_start: settings.general.auto_start,
            enable_ocr: settings.ocr.enabled,
            save_path: QString::from(settings.output.save_path.as_deref().unwrap_or("")),
            font_family: QString::from(settings.general.font_family.as_deref().unwrap_or("")),
            theme: QString::from(&settings.general.theme),
            language: QString::from(&settings.general.language),
            version: QString::from(env!("CARGO_PKG_VERSION")),
            capture_shortcut: QString::from(&settings.shortcuts.capture),
            quick_capture_shortcut: QString::from(&settings.shortcuts.quick_capture),
        }
    }
}

impl qobject::Config {
    pub fn load_settings(mut self: Pin<&mut Self>) {
        let settings = SETTINGS.lock().unwrap().get();
        self.as_mut().set_oxipng_enabled(settings.output.oxipng_enabled);
        self.as_mut().set_auto_start(settings.general.auto_start);
        self.as_mut().set_enable_ocr(settings.ocr.enabled);
        self.as_mut().set_theme(QString::from(&settings.general.theme));
        self.as_mut().set_language(QString::from(&settings.general.language));
        self.as_mut().set_version(QString::from(env!("CARGO_PKG_VERSION")));
        self.as_mut().set_capture_shortcut(QString::from(&settings.shortcuts.capture));
        self.as_mut().set_quick_capture_shortcut(QString::from(&settings.shortcuts.quick_capture));

        if let Some(path) = settings.output.save_path {
            self.as_mut().set_save_path(QString::from(&path));
        }
        if let Some(font) = settings.general.font_family {
            self.as_mut().set_font_family(QString::from(&font));
        }
    }

    pub fn update_oxipng_enabled(mut self: Pin<&mut Self>, enabled: bool) {
        update_prop!(self, enabled, oxipng_enabled, set_oxipng_enabled, set_oxipng_enabled, bool);
    }

    pub fn update_auto_start(mut self: Pin<&mut Self>, enabled: bool) {
        if *self.auto_start() == enabled {
            return;
        }
        self.as_mut().set_auto_start(enabled);
        SETTINGS.lock().unwrap().set_auto_start(enabled);
        crate::core::app::set_auto_start(enabled);
    }

    pub fn update_enable_ocr(mut self: Pin<&mut Self>, enabled: bool) {
        update_prop!(self, enabled, enable_ocr, set_enable_ocr, set_ocr_enabled, bool);
    }

    pub fn update_save_path(mut self: Pin<&mut Self>, path: QString) {
        if self.save_path() == &path {
            return;
        }
        let path_str = clean_url_path(&path.to_string());
        self.as_mut().set_save_path(path);
        SETTINGS.lock().unwrap().set_save_path(path_str);
    }

    pub fn update_font_family(mut self: Pin<&mut Self>, family: QString) {
        update_prop!(self, family, font_family, set_font_family, set_font_family);
    }

    pub fn update_theme(mut self: Pin<&mut Self>, theme: QString) {
        update_prop!(self, theme, theme, set_theme, set_theme);
    }

    pub fn update_language(mut self: Pin<&mut Self>, language: QString) {
        let language_str = language.to_string();
        if self.language().to_string() == language_str {
            return;
        }
        SETTINGS.lock().unwrap().set_language(language_str.clone());
        crate::bridge::app::install_translator(&language_str);
        self.as_mut().set_language(language);
        crate::bridge::app::retranslate();
    }

    pub fn update_capture_shortcut(mut self: Pin<&mut Self>, shortcut: QString) {
        update_prop!(self, shortcut, capture_shortcut, set_capture_shortcut, set_capture_shortcut);
    }

    pub fn update_quick_capture_shortcut(mut self: Pin<&mut Self>, shortcut: QString) {
        update_prop!(
            self,
            shortcut,
            quick_capture_shortcut,
            set_quick_capture_shortcut,
            set_quick_capture_shortcut
        );
    }

    pub fn get_system_fonts(&self) -> QStringList {
        let mut list = QStringList::default();
        for font in get_system_fonts() {
            list.append(QString::from(&font));
        }
        list
    }

    pub fn get_default_save_path(&self) -> QString {
        QString::from(&get_default_save_path())
    }

    pub fn get_supported_languages(&self) -> QStringList {
        let mut list = QStringList::default();
        for lang in ["System", "zh_CN", "en_US"] {
            list.append(QString::from(lang));
        }
        list
    }
}
