use crate::core::geometry::{clamp_point, clamp_rect_move, clamp_rect_resize};
use crate::core::window::{find_window_at, WindowInfo};
use cxx_qt::{CxxQtType, QObject};
use cxx_qt_lib::{QPointF, QRectF, QString};
use std::pin::Pin;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qrectf.h");
        type QRectF = cxx_qt_lib::QRectF;
        include!("cxx-qt-lib/qpointf.h");
        type QPointF = cxx_qt_lib::QPointF;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QRectF, target_rect, cxx_name = "targetRect")]
        #[qproperty(QString, target_info, cxx_name = "targetInfo")]
        #[qproperty(bool, has_target, cxx_name = "hasTarget")]
        #[qproperty(QRectF, selection_rect, cxx_name = "selectionRect")]
        #[qproperty(QString, state, cxx_name = "state")]
        #[qproperty(f64, screen_width, cxx_name = "screenWidth")]
        #[qproperty(f64, screen_height, cxx_name = "screenHeight")]
        type OverlayController = super::OverlayControllerRust;

        #[qinvokable]
        #[cxx_name = "updateWindowList"]
        fn update_window_list(self: Pin<&mut Self>, json: QString);

        #[qinvokable]
        #[cxx_name = "updateHover"]
        fn update_hover(self: Pin<&mut Self>, x: f64, y: f64);

        #[qinvokable]
        #[cxx_name = "clearTarget"]
        fn clear_target(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "startSelection"]
        fn start_selection(self: Pin<&mut Self>, x: f64, y: f64);

        #[qinvokable]
        #[cxx_name = "updateSelection"]
        fn update_selection(self: Pin<&mut Self>, x: f64, y: f64);

        #[qinvokable]
        #[cxx_name = "endSelection"]
        fn end_selection(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "startMove"]
        fn start_move(self: Pin<&mut Self>, x: f64, y: f64);

        #[qinvokable]
        #[cxx_name = "updateMove"]
        fn update_move(self: Pin<&mut Self>, x: f64, y: f64);

        #[qinvokable]
        #[cxx_name = "endMove"]
        fn end_move(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "startResize"]
        fn start_resize(self: Pin<&mut Self>, corner: QString, x: f64, y: f64);

        #[qinvokable]
        #[cxx_name = "updateResize"]
        fn update_resize(self: Pin<&mut Self>, x: f64, y: f64);

        #[qinvokable]
        #[cxx_name = "endResize"]
        fn end_resize(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "reset"]
        fn reset(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "selectTarget"]
        fn select_target(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "setupWindow"]
        unsafe fn setup_window(self: Pin<&mut Self>, window: *mut QObject);
    }
}

pub struct OverlayControllerRust {
    target_rect: QRectF,
    target_info: QString,
    has_target: bool,
    selection_rect: QRectF,
    state: QString,
    screen_width: f64,
    screen_height: f64,

    window_list: Vec<WindowInfo>,
    drag_start_point: QPointF,
    drag_start_rect: QRectF,
    resize_corner: String,
}

impl Default for OverlayControllerRust {
    fn default() -> Self {
        Self {
            target_rect: QRectF::default(),
            target_info: QString::default(),
            has_target: false,
            selection_rect: QRectF::default(),
            state: QString::from("BROWSING"),
            screen_width: 0.0,
            screen_height: 0.0,
            window_list: Vec::new(),
            drag_start_point: QPointF::default(),
            drag_start_rect: QRectF::default(),
            resize_corner: String::new(),
        }
    }
}

impl qobject::OverlayController {
    pub fn update_window_list(mut self: Pin<&mut Self>, json: QString) {
        let json_str = json.to_string();
        let windows: Vec<WindowInfo> = serde_json::from_str(&json_str).unwrap_or_default();
        self.as_mut().rust_mut().window_list = windows;
    }

    pub fn update_hover(mut self: Pin<&mut Self>, x: f64, y: f64) {
        if self.state().to_string() != "BROWSING" {
            return;
        }

        let best_candidate_idx = {
            let q_obj = self.as_ref();
            let windows = &q_obj.rust().window_list;
            find_window_at(windows, x, y)
        };

        if let Some(idx) = best_candidate_idx {
            let q_obj = self.as_ref();
            let target = &q_obj.rust().window_list[idx];
            let rect = QRectF::new(
                f64::from(target.x),
                f64::from(target.y),
                f64::from(target.width),
                f64::from(target.height),
            );

            if *self.has_target() && *self.target_rect() == rect {
                return;
            }

            let info = format!("{}: {} ({} x {})", target.app_name, target.title, target.width, target.height);

            self.as_mut().set_target_rect(rect);
            self.as_mut().set_target_info(QString::from(&info));
            self.as_mut().set_has_target(true);
        } else if *self.has_target() {
            self.as_mut().set_has_target(false);
        }
    }

    pub fn clear_target(mut self: Pin<&mut Self>) {
        if *self.has_target() {
            self.as_mut().set_has_target(false);
        }
    }

    pub fn start_selection(mut self: Pin<&mut Self>, x: f64, y: f64) {
        self.as_mut().set_state(QString::from("DRAGGING"));
        self.as_mut().rust_mut().drag_start_point = QPointF::new(x, y);
        self.as_mut().set_selection_rect(QRectF::new(x, y, 0.0, 0.0));
    }

    pub fn update_selection(mut self: Pin<&mut Self>, x: f64, y: f64) {
        if self.state().to_string() != "DRAGGING" {
            return;
        }

        let (clamped_x, clamped_y) = clamp_point(x, y, *self.screen_width(), *self.screen_height());

        let start = self.rust().drag_start_point.clone();
        let dx = clamped_x - start.x();
        let dy = clamped_y - start.y();

        let new_x = if dx < 0.0 { clamped_x } else { start.x() };
        let new_y = if dy < 0.0 { clamped_y } else { start.y() };
        let w = dx.abs();
        let h = dy.abs();

        self.as_mut().set_selection_rect(QRectF::new(new_x, new_y, w, h));
    }

    pub fn end_selection(mut self: Pin<&mut Self>) {
        if self.state().to_string() != "DRAGGING" {
            return;
        }

        let rect = self.selection_rect().clone();
        if rect.width() > 5.0 && rect.height() > 5.0 {
            self.as_mut().set_state(QString::from("LOCKED"));
        } else {
            if *self.has_target() {
                let target = self.target_rect().clone();
                self.as_mut().set_selection_rect(target);
                self.as_mut().set_state(QString::from("LOCKED"));
            } else {
                self.as_mut().set_state(QString::from("BROWSING"));
                self.as_mut().set_selection_rect(QRectF::default());
            }
        }
    }

    pub fn select_target(mut self: Pin<&mut Self>) {
        if *self.has_target() {
            let target = self.target_rect().clone();
            self.as_mut().set_selection_rect(target);
            self.as_mut().set_state(QString::from("LOCKED"));
        }
    }

    pub fn start_move(mut self: Pin<&mut Self>, x: f64, y: f64) {
        if self.state().to_string() != "LOCKED" {
            return;
        }

        let current_rect = self.selection_rect().clone();
        self.as_mut().set_state(QString::from("MOVING"));
        self.as_mut().rust_mut().drag_start_point = QPointF::new(x, y);
        self.as_mut().rust_mut().drag_start_rect = current_rect;
    }

    pub fn update_move(mut self: Pin<&mut Self>, x: f64, y: f64) {
        if self.state().to_string() != "MOVING" {
            return;
        }

        let start_pt = self.rust().drag_start_point.clone();
        let start_rect = self.rust().drag_start_rect.clone();

        let dx = x - start_pt.x();
        let dy = y - start_pt.y();

        let raw_new_x = start_rect.x() + dx;
        let raw_new_y = start_rect.y() + dy;

        let (new_x, new_y) = clamp_rect_move(
            raw_new_x,
            raw_new_y,
            start_rect.width(),
            start_rect.height(),
            *self.screen_width(),
            *self.screen_height(),
        );

        self.as_mut()
            .set_selection_rect(QRectF::new(new_x, new_y, start_rect.width(), start_rect.height()));
    }

    pub fn end_move(mut self: Pin<&mut Self>) {
        if self.state().to_string() == "MOVING" {
            self.as_mut().set_state(QString::from("LOCKED"));
        }
    }

    pub fn start_resize(mut self: Pin<&mut Self>, corner: QString, x: f64, y: f64) {
        if self.state().to_string() != "LOCKED" {
            return;
        }

        let current_rect = self.selection_rect().clone();
        self.as_mut().set_state(QString::from("RESIZING"));
        self.as_mut().rust_mut().resize_corner = corner.to_string();
        self.as_mut().rust_mut().drag_start_point = QPointF::new(x, y);
        self.as_mut().rust_mut().drag_start_rect = current_rect;
    }

    pub fn update_resize(mut self: Pin<&mut Self>, x: f64, y: f64) {
        if self.state().to_string() != "RESIZING" {
            return;
        }

        let start_pt = self.rust().drag_start_point.clone();
        let start_rect = self.rust().drag_start_rect.clone();
        let corner = self.rust().resize_corner.clone();

        let dx = x - start_pt.x();
        let dy = y - start_pt.y();

        let mut new_x = start_rect.x();
        let mut new_y = start_rect.y();
        let mut new_w = start_rect.width();
        let mut new_h = start_rect.height();

        if corner.contains("right") {
            new_w += dx;
        } else if corner.contains("left") {
            new_x += dx;
            new_w -= dx;
        }

        if corner.contains("bottom") {
            new_h += dy;
        } else if corner.contains("top") {
            new_y += dy;
            new_h -= dy;
        }

        if new_w < 10.0 {
            if corner.contains("left") {
                new_x = start_rect.x() + start_rect.width() - 10.0;
            }
            new_w = 10.0;
        }
        if new_h < 10.0 {
            if corner.contains("top") {
                new_y = start_rect.y() + start_rect.height() - 10.0;
            }
            new_h = 10.0;
        }

        let (final_x, final_y, final_w, final_h) = clamp_rect_resize(new_x, new_y, new_w, new_h, *self.screen_width(), *self.screen_height());

        self.as_mut().set_selection_rect(QRectF::new(final_x, final_y, final_w, final_h));
    }

    pub fn end_resize(mut self: Pin<&mut Self>) {
        if self.state().to_string() == "RESIZING" {
            self.as_mut().set_state(QString::from("LOCKED"));
        }
    }

    pub fn reset(mut self: Pin<&mut Self>) {
        self.as_mut().set_state(QString::from("BROWSING"));
        self.as_mut().set_selection_rect(QRectF::default());
        self.as_mut().set_has_target(false);
    }

    pub unsafe fn setup_window(self: Pin<&mut Self>, window: *mut QObject) {
        #[cfg(target_os = "macos")]
        {
            let ptr = window as usize;
            crate::bridge::window::setup_macos_window(ptr);
        }
    }
}
