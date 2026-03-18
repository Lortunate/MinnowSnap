use crate::core::io::fonts::get_system_fonts;
use crate::core::io::storage::get_default_save_path;
use crate::core::settings::SETTINGS;
use crate::interop::qt_url_adapter;
use cxx_qt_lib::{QString, QStringList, QUrl};
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
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, oxipng_enabled)]
        #[qproperty(bool, auto_start)]
        #[qproperty(bool, enable_ocr)]
        #[qproperty(bool, notification_enabled)]
        #[qproperty(bool, save_notification)]
        #[qproperty(bool, copy_notification)]
        #[qproperty(bool, qr_code_notification)]
        #[qproperty(bool, shutter_sound)]
        #[qproperty(QString, save_path)]
        #[qproperty(QString, font_family)]
        #[qproperty(QString, theme)]
        #[qproperty(QString, language)]
        #[qproperty(QString, version)]
        #[qproperty(QString, capture_shortcut)]
        #[qproperty(QString, quick_capture_shortcut)]
        #[qproperty(bool, has_shortcut_conflicts)]
        #[qproperty(QString, shortcut_conflict_msg)]
        type Config = super::ConfigRust;

        #[qinvokable]
        fn check_shortcut_conflicts(self: Pin<&mut Self>, capture: QString, quick: QString);

        #[qinvokable]
        fn update_theme(self: Pin<&mut Self>, theme: QString);

        #[qinvokable]
        fn update_language(self: Pin<&mut Self>, language: QString);

        #[qinvokable]
        fn update_oxipng_enabled(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn update_auto_start(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn update_enable_ocr(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn update_notification_enabled(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn update_save_notification(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn update_copy_notification(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn update_qr_code_notification(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn update_shutter_sound(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn update_save_path(self: Pin<&mut Self>, path: QUrl);

        #[qinvokable]
        fn update_font_family(self: Pin<&mut Self>, family: QString);

        #[qinvokable]
        fn update_capture_shortcut(self: Pin<&mut Self>, shortcut: QString);

        #[qinvokable]
        fn update_quick_capture_shortcut(self: Pin<&mut Self>, shortcut: QString);

        #[qinvokable]
        fn get_system_fonts(self: &Self) -> QStringList;

        #[qinvokable]
        fn get_default_save_path(self: &Self) -> QString;

        #[qinvokable]
        fn get_supported_languages(self: &Self) -> QStringList;

        #[qinvokable]
        fn get_log_directory(self: &Self) -> QString;

        #[qinvokable]
        fn load_settings(self: Pin<&mut Self>);
    }
}

pub struct ConfigRust {
    oxipng_enabled: bool,
    auto_start: bool,
    enable_ocr: bool,
    notification_enabled: bool,
    save_notification: bool,
    copy_notification: bool,
    qr_code_notification: bool,
    shutter_sound: bool,
    save_path: QString,
    font_family: QString,
    theme: QString,
    language: QString,
    version: QString,
    capture_shortcut: QString,
    quick_capture_shortcut: QString,
    has_shortcut_conflicts: bool,
    shortcut_conflict_msg: QString,
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
            notification_enabled: settings.notification.enabled,
            save_notification: settings.notification.save_notification,
            copy_notification: settings.notification.copy_notification,
            qr_code_notification: settings.notification.qr_code_notification,
            shutter_sound: settings.notification.shutter_sound,
            save_path: QString::from(settings.output.save_path.as_deref().unwrap_or("")),
            font_family: QString::from(settings.general.font_family.as_deref().unwrap_or("")),
            theme: QString::from(&settings.general.theme),
            language: QString::from(&settings.general.language),
            version: QString::from(env!("CARGO_PKG_VERSION")),
            capture_shortcut: QString::from(&settings.shortcuts.capture),
            quick_capture_shortcut: QString::from(&settings.shortcuts.quick_capture),
            has_shortcut_conflicts: false,
            shortcut_conflict_msg: QString::default(),
        }
    }
}

impl qobject::Config {
    pub fn load_settings(mut self: Pin<&mut Self>) {
        let settings = SETTINGS.lock().unwrap().get();
        self.as_mut().set_oxipng_enabled(settings.output.oxipng_enabled);
        self.as_mut().set_auto_start(settings.general.auto_start);
        self.as_mut().set_enable_ocr(settings.ocr.enabled);
        self.as_mut().set_notification_enabled(settings.notification.enabled);
        self.as_mut().set_save_notification(settings.notification.save_notification);
        self.as_mut().set_copy_notification(settings.notification.copy_notification);
        self.as_mut().set_qr_code_notification(settings.notification.qr_code_notification);
        self.as_mut().set_shutter_sound(settings.notification.shutter_sound);
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

    pub fn update_notification_enabled(mut self: Pin<&mut Self>, enabled: bool) {
        update_prop!(
            self,
            enabled,
            notification_enabled,
            set_notification_enabled,
            set_notification_enabled,
            bool
        );
    }

    pub fn update_save_notification(mut self: Pin<&mut Self>, enabled: bool) {
        update_prop!(self, enabled, save_notification, set_save_notification, set_save_notification, bool);
    }

    pub fn update_copy_notification(mut self: Pin<&mut Self>, enabled: bool) {
        update_prop!(self, enabled, copy_notification, set_copy_notification, set_copy_notification, bool);
    }

    pub fn update_qr_code_notification(mut self: Pin<&mut Self>, enabled: bool) {
        update_prop!(
            self,
            enabled,
            qr_code_notification,
            set_qr_code_notification,
            set_qr_code_notification,
            bool
        );
    }

    pub fn update_shutter_sound(mut self: Pin<&mut Self>, enabled: bool) {
        update_prop!(self, enabled, shutter_sound, set_shutter_sound, set_shutter_sound, bool);
    }

    pub fn update_save_path(mut self: Pin<&mut Self>, path: QUrl) {
        let resolved_path = qt_url_adapter::qurl_to_local_or_uri(&path);

        if self.save_path().to_string() == resolved_path {
            return;
        }

        self.as_mut().set_save_path(QString::from(&resolved_path));
        SETTINGS.lock().unwrap().set_save_path(resolved_path);
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

    pub fn check_shortcut_conflicts(mut self: Pin<&mut Self>, capture: QString, quick: QString) {
        let capture_str = if capture.is_empty() { "F1" } else { &capture.to_string() };
        let quick_str = if quick.is_empty() { "F2" } else { &quick.to_string() };

        if capture_str == quick_str {
            self.as_mut().set_has_shortcut_conflicts(true);
            self.as_mut()
                .set_shortcut_conflict_msg(crate::bridge::app::tr("Preferences", "Shortcuts cannot be identical."));
        } else {
            self.as_mut().set_has_shortcut_conflicts(false);
            self.as_mut().set_shortcut_conflict_msg(QString::default());
        }
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

    pub fn get_log_directory(&self) -> QString {
        let path = crate::core::logging::log_dir(crate::core::app::APP_NAME);
        QString::from(path.to_string_lossy().as_ref())
    }
}
