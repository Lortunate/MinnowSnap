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

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, frame_visible, cxx_name = "frameVisible")]
        #[qproperty(bool, toolbar_visible, cxx_name = "toolbarVisible")]
        #[qproperty(bool, preview_visible, cxx_name = "previewVisible")]
        #[qproperty(bool, toolbar_busy, cxx_name = "toolbarBusy")]
        #[qproperty(QString, warning_text, cxx_name = "warningText")]
        #[qproperty(QRectF, selection_rect, cxx_name = "selectionRect")]
        type LongCaptureController = super::LongCaptureControllerRust;

        #[qinvokable]
        #[cxx_name = "start"]
        fn start(self: Pin<&mut Self>, x: i32, y: i32, width: i32, height: i32);

        #[qinvokable]
        #[cxx_name = "finish"]
        fn finish(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "handleToolbarAction"]
        fn handle_toolbar_action(self: Pin<&mut Self>, action: QString);

        #[qinvokable]
        #[cxx_name = "onCaptureReady"]
        fn on_capture_ready(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "onScrollCaptureFinished"]
        fn on_scroll_capture_finished(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "onScrollCaptureUpdated"]
        fn on_scroll_capture_updated(self: Pin<&mut Self>, height: i32);

        #[qinvokable]
        #[cxx_name = "onScrollCaptureWarning"]
        fn on_scroll_capture_warning(self: Pin<&mut Self>, message: QString);

        #[qsignal]
        #[cxx_name = "requestHideOverlay"]
        fn request_hide_overlay(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "requestResetOverlay"]
        fn request_reset_overlay(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "requestCancelScrollCapture"]
        fn request_cancel_scroll_capture(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "requestScrollAction"]
        fn request_scroll_action(self: Pin<&mut Self>, action: QString);

        #[qsignal]
        #[cxx_name = "requestPreviewRefresh"]
        fn request_preview_refresh(self: Pin<&mut Self>, height: i32);

        #[qsignal]
        #[cxx_name = "requestFrameFlash"]
        fn request_frame_flash(self: Pin<&mut Self>);
    }
}

pub struct LongCaptureControllerRust {
    frame_visible: bool,
    toolbar_visible: bool,
    preview_visible: bool,
    toolbar_busy: bool,
    warning_text: QString,
    selection_rect: QRectF,
}

impl Default for LongCaptureControllerRust {
    fn default() -> Self {
        Self {
            frame_visible: false,
            toolbar_visible: false,
            preview_visible: false,
            toolbar_busy: false,
            warning_text: QString::default(),
            selection_rect: QRectF::default(),
        }
    }
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

    pub fn handle_toolbar_action(mut self: Pin<&mut Self>, action: QString) {
        if action.to_string() == "cancel" {
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
