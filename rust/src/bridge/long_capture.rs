use crate::bridge::screen_capture::is_cancel_action;
use cxx_qt_lib::{QRectF, QString};
use std::pin::Pin;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qrectf.h");
        type QRectF = cxx_qt_lib::QRectF;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, frame_visible)]
        #[qproperty(bool, toolbar_visible)]
        #[qproperty(bool, preview_visible)]
        #[qproperty(bool, toolbar_busy)]
        #[qproperty(QString, warning_text)]
        #[qproperty(QRectF, selection_rect)]
        type LongCaptureController = super::LongCaptureControllerRust;

        #[qinvokable]
        fn start(self: Pin<&mut Self>, x: i32, y: i32, width: i32, height: i32);

        #[qinvokable]
        fn finish(self: Pin<&mut Self>);

        #[qinvokable]
        fn handle_toolbar_action(self: Pin<&mut Self>, action: i32);

        #[qinvokable]
        fn on_capture_ready(self: Pin<&mut Self>);

        #[qinvokable]
        fn on_scroll_capture_finished(self: Pin<&mut Self>);

        #[qinvokable]
        fn on_scroll_capture_updated(self: Pin<&mut Self>, height: i32);

        #[qinvokable]
        fn on_scroll_capture_warning(self: Pin<&mut Self>, message: QString);

        #[qsignal]
        fn request_hide_overlay(self: Pin<&mut Self>);

        #[qsignal]
        fn request_reset_overlay(self: Pin<&mut Self>);

        #[qsignal]
        fn request_cancel_scroll_capture(self: Pin<&mut Self>);

        #[qsignal]
        fn request_scroll_action(self: Pin<&mut Self>, action: i32);

        #[qsignal]
        fn request_preview_refresh(self: Pin<&mut Self>, height: i32);

        #[qsignal]
        fn request_frame_flash(self: Pin<&mut Self>);
    }
}

#[derive(Default)]
pub struct LongCaptureControllerRust {
    frame_visible: bool,
    toolbar_visible: bool,
    preview_visible: bool,
    toolbar_busy: bool,
    warning_text: QString,
    selection_rect: QRectF,
}

impl qobject::LongCaptureController {
    pub fn start(mut self: Pin<&mut Self>, x: i32, y: i32, width: i32, height: i32) {
        self.as_mut()
            .set_selection_rect(QRectF::new(f64::from(x), f64::from(y), f64::from(width), f64::from(height)));
        self.as_mut().set_frame_visible(true);
        self.as_mut().set_toolbar_visible(true);
        self.as_mut().set_preview_visible(true);
        self.as_mut().set_toolbar_busy(false);
        self.as_mut().set_warning_text(QString::default());
        self.as_mut().request_hide_overlay();
    }

    pub fn finish(mut self: Pin<&mut Self>) {
        self.as_mut().set_toolbar_busy(false);
        self.as_mut().set_frame_visible(false);
        self.as_mut().set_toolbar_visible(false);
        self.as_mut().set_preview_visible(false);
        self.as_mut().set_warning_text(QString::default());
        self.as_mut().request_reset_overlay();
    }

    pub fn handle_toolbar_action(mut self: Pin<&mut Self>, action: i32) {
        if is_cancel_action(action) {
            self.as_mut().set_toolbar_busy(false);
            self.as_mut().set_frame_visible(false);
            self.as_mut().set_toolbar_visible(false);
            self.as_mut().set_preview_visible(false);
            self.as_mut().request_cancel_scroll_capture();
            self.as_mut().request_reset_overlay();
            return;
        }

        self.as_mut().set_toolbar_busy(true);
        self.as_mut().set_frame_visible(false);
        self.as_mut().request_scroll_action(action);
    }

    pub fn on_capture_ready(mut self: Pin<&mut Self>) {
        if *self.toolbar_visible() {
            self.as_mut().finish();
        }
    }

    pub fn on_scroll_capture_finished(mut self: Pin<&mut Self>) {
        self.as_mut().finish();
    }

    pub fn on_scroll_capture_updated(mut self: Pin<&mut Self>, height: i32) {
        self.as_mut().request_preview_refresh(height);
        self.as_mut().request_frame_flash();
        self.as_mut().set_warning_text(QString::default());
    }

    pub fn on_scroll_capture_warning(mut self: Pin<&mut Self>, message: QString) {
        self.as_mut().set_warning_text(message);
    }
}
