use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde::Deserialize;
use std::pin::Pin;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(f64, popup_x, cxx_name = "popupX")]
        #[qproperty(f64, popup_y, cxx_name = "popupY")]
        #[qproperty(bool, popup_visible, cxx_name = "popupVisible")]
        #[qproperty(QString, preferences_text, cxx_name = "preferencesText")]
        #[qproperty(QString, screen_capture_text, cxx_name = "screenCaptureText")]
        #[qproperty(QString, quick_capture_text, cxx_name = "quickCaptureText")]
        #[qproperty(QString, quit_text, cxx_name = "quitText")]
        #[qproperty(QString, tooltip_text, cxx_name = "tooltipText")]
        type TrayMenuController = super::TrayMenuControllerRust;

        #[qinvokable]
        #[cxx_name = "popup"]
        fn popup(self: Pin<&mut Self>, geometry_json: QString, screens_json: QString, platform_os: QString, menu_width: f64, menu_height: f64);

        #[qinvokable]
        #[cxx_name = "hideMenu"]
        fn hide_menu(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "syncWindowActive"]
        fn sync_window_active(self: Pin<&mut Self>, active: bool);
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RectData {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScreenData {
    virtual_x: f64,
    virtual_y: f64,
    width: f64,
    height: f64,
    #[serde(default = "default_device_pixel_ratio")]
    device_pixel_ratio: f64,
}

fn default_device_pixel_ratio() -> f64 {
    1.0
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
        let screens = parse_screens(screens_json.to_string());
        if screens.is_empty() {
            return;
        }

        let width = menu_width.max(1.0);
        let height = menu_height.max(1.0);
        let fallback = screens[0];
        let geometry = parse_geometry(geometry_json.to_string());
        let (popup_x, popup_y) = match geometry {
            Some(rect) => {
                let normalized = normalize_geometry(rect, &screens, platform_os.to_string().as_str());
                let target = find_screen(normalized, &screens).unwrap_or(fallback);
                calculate_position(normalized, target, width, height)
            }
            None => centered_position(fallback, width, height),
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
}

impl qobject::TrayMenuController {
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

fn parse_screens(screens_json: String) -> Vec<ScreenData> {
    serde_json::from_str::<Vec<ScreenData>>(&screens_json).unwrap_or_default()
}

fn parse_geometry(geometry_json: String) -> Option<RectData> {
    let trimmed = geometry_json.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return None;
    }
    serde_json::from_str::<RectData>(trimmed).ok()
}

fn centered_position(screen: ScreenData, menu_width: f64, menu_height: f64) -> (f64, f64) {
    (
        screen.virtual_x + (screen.width - menu_width) / 2.0,
        screen.virtual_y + (screen.height - menu_height) / 2.0,
    )
}

fn normalize_geometry(mut rect: RectData, screens: &[ScreenData], platform_os: &str) -> RectData {
    if platform_os != "windows" {
        return rect;
    }

    let mut is_logical = false;
    for screen in screens {
        if rect.x >= screen.virtual_x
            && rect.x < screen.virtual_x + screen.width
            && rect.y >= screen.virtual_y
            && rect.y < screen.virtual_y + screen.height
        {
            is_logical = true;
            break;
        }
    }

    if is_logical {
        return rect;
    }

    for screen in screens {
        let dpr = if screen.device_pixel_ratio <= 0.0 {
            1.0
        } else {
            screen.device_pixel_ratio
        };
        let px = rect.x / dpr;
        let py = rect.y / dpr;
        if px >= screen.virtual_x && px < screen.virtual_x + screen.width && py >= screen.virtual_y && py < screen.virtual_y + screen.height {
            rect.x = px;
            rect.y = py;
            rect.width /= dpr;
            rect.height /= dpr;
            break;
        }
    }

    rect
}

fn find_screen(rect: RectData, screens: &[ScreenData]) -> Option<ScreenData> {
    let cx = rect.x + rect.width / 2.0;
    let cy = rect.y + rect.height / 2.0;
    for screen in screens {
        if cx >= screen.virtual_x && cx < screen.virtual_x + screen.width && cy >= screen.virtual_y && cy < screen.virtual_y + screen.height {
            return Some(*screen);
        }
    }
    screens.first().copied()
}

fn calculate_position(rect: RectData, screen: ScreenData, menu_width: f64, menu_height: f64) -> (f64, f64) {
    let cy = rect.y + rect.height / 2.0;

    let mut target_x = rect.x;
    let mut target_y = rect.y + rect.height + 5.0;

    if cy > screen.virtual_y + screen.height / 2.0 {
        target_y = rect.y - menu_height - 5.0;
    }

    let padding = 6.0;
    if target_x + menu_width > screen.virtual_x + screen.width - padding {
        target_x = screen.virtual_x + screen.width - menu_width - padding;
    }
    if target_x < screen.virtual_x + padding {
        target_x = screen.virtual_x + padding;
    }

    if target_y + menu_height > screen.virtual_y + screen.height - padding {
        target_y = screen.virtual_y + screen.height - menu_height - padding;
    }
    if target_y < screen.virtual_y + padding {
        target_y = screen.virtual_y + padding;
    }

    (target_x, target_y)
}
