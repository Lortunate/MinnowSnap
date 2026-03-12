use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use std::pin::Pin;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(f64, popup_x)]
        #[qproperty(f64, popup_y)]
        #[qproperty(bool, popup_visible)]
        #[qproperty(QString, preferences_text)]
        #[qproperty(QString, screen_capture_text)]
        #[qproperty(QString, quick_capture_text)]
        #[qproperty(QString, quit_text)]
        #[qproperty(QString, tooltip_text)]
        type TrayMenuController = super::TrayMenuControllerRust;

        #[qinvokable]
        fn popup(self: Pin<&mut Self>, geometry_json: QString, screens_json: QString, platform_os: QString, menu_width: f64, menu_height: f64);

        #[qinvokable]
        fn hide_menu(self: Pin<&mut Self>);

        #[qinvokable]
        fn sync_window_active(self: Pin<&mut Self>, active: bool);
    }
}

pub struct TrayMenuControllerRust {
    popup_x: f64,
    popup_y: f64,
    popup_visible: bool,
    preferences_text: QString,
    screen_capture_text: QString,
    quick_capture_text: QString,
    quit_text: QString,
    tooltip_text: QString,
    activation_pending: bool,
}

impl Default for TrayMenuControllerRust {
    fn default() -> Self {
        Self {
            popup_x: 0.0,
            popup_y: 0.0,
            popup_visible: false,
            preferences_text: crate::bridge::app::tr("main", "Preferences"),
            screen_capture_text: crate::bridge::app::tr("main", "Capture"),
            quick_capture_text: crate::bridge::app::tr("main", "Quick Capture"),
            quit_text: crate::bridge::app::tr("main", "Exit"),
            tooltip_text: crate::bridge::app::tr("main", "MinnowSnap - Screen Capture Tool"),
            activation_pending: false,
        }
    }
}

impl qobject::TrayMenuController {
    pub fn popup(mut self: Pin<&mut Self>, geometry_json: QString, screens_json: QString, platform_os: QString, menu_width: f64, menu_height: f64) {
        let screens = crate::core::tray::parse_screens(&screens_json.to_string());
        if screens.is_empty() {
            return;
        }

        let width = menu_width.max(1.0);
        let height = menu_height.max(1.0);
        let fallback = screens[0];
        let geometry = crate::core::tray::parse_geometry(&geometry_json.to_string());
        let (popup_x, popup_y) = match geometry {
            Some(rect) => {
                let normalized = crate::core::tray::normalize_geometry(rect, &screens, &platform_os.to_string());
                let target = crate::core::tray::find_screen(normalized, &screens).unwrap_or(fallback);
                crate::core::tray::calculate_position(normalized, target, width, height)
            }
            None => crate::core::tray::centered_position(fallback, width, height),
        };
        self.as_mut().show_at(popup_x, popup_y);
    }

    pub fn hide_menu(mut self: Pin<&mut Self>) {
        if *self.popup_visible() {
            self.as_mut().set_popup_visible(false);
        }
        self.as_mut().rust_mut().activation_pending = false;
    }

    pub fn sync_window_active(mut self: Pin<&mut Self>, active: bool) {
        if active {
            self.as_mut().rust_mut().activation_pending = false;
            return;
        }

        if self.rust().activation_pending {
            return;
        }

        if *self.popup_visible() {
            self.as_mut().set_popup_visible(false);
        }
    }

    fn show_at(mut self: Pin<&mut Self>, x: f64, y: f64) {
        if (*self.popup_x() - x).abs() > f64::EPSILON {
            self.as_mut().set_popup_x(x);
        }
        if (*self.popup_y() - y).abs() > f64::EPSILON {
            self.as_mut().set_popup_y(y);
        }
        if !*self.popup_visible() {
            self.as_mut().set_popup_visible(true);
        }
        self.as_mut().rust_mut().activation_pending = true;
    }
}
