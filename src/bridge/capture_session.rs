use crate::bridge::screen_capture::{is_redo_action, is_undo_action};
use crate::interop::qt_rect_adapter::SelectionRect;
use cxx_qt_lib::{QPointF, QRectF, QUrl};
use std::pin::Pin;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qpointf.h");
        type QPointF = cxx_qt_lib::QPointF;
        include!("cxx-qt-lib/qrectf.h");
        type QRectF = cxx_qt_lib::QRectF;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, action_processing)]
        #[qproperty(bool, annotation_display_ready)]
        #[qproperty(QUrl, background_image_source)]
        #[qproperty(bool, busy)]
        #[qproperty(bool, has_screen_capture)]
        #[qproperty(f64, screen_width)]
        #[qproperty(f64, screen_height)]
        #[qproperty(f64, toolbar_padding)]
        #[qproperty(f64, toolbar_spacing_above)]
        #[qproperty(f64, toolbar_spacing_below)]
        #[qproperty(f64, default_toolbar_y)]
        type CaptureSessionController = super::CaptureSessionControllerRust;

        #[qinvokable]
        fn confirm_action(self: Pin<&mut Self>, action: i32, selection_rect: QRectF, has_annotations: bool);

        #[qinvokable]
        fn cancel_session(self: Pin<&mut Self>, force: bool);

        #[qinvokable]
        fn begin_session(self: Pin<&mut Self>, source: QUrl);

        #[qinvokable]
        fn reset_session(self: Pin<&mut Self>);

        #[qinvokable]
        fn reset_annotation_state(self: Pin<&mut Self>);

        #[qinvokable]
        fn on_locked_state_changed(self: Pin<&mut Self>, is_locked: bool);

        #[qinvokable]
        fn promote_annotation_display_ready(self: Pin<&mut Self>);

        #[qinvokable]
        fn toolbar_position(self: &Self, target_rect: QRectF, item_w: f64, item_h: f64, is_above: bool) -> QPointF;

        #[qsignal]
        fn request_annotation_reset(self: Pin<&mut Self>);

        #[qsignal]
        fn request_compositor_abort(self: Pin<&mut Self>);

        #[qsignal]
        fn request_overlay_hide(self: Pin<&mut Self>);

        #[qsignal]
        fn request_overlay_present(self: Pin<&mut Self>);

        #[qsignal]
        fn request_capture_flag(self: Pin<&mut Self>, value: bool);

        #[qsignal]
        fn request_action_dispatch(self: Pin<&mut Self>, action: i32, selection_rect: QRectF, has_annotations: bool);

        #[qsignal]
        fn request_undo(self: Pin<&mut Self>);

        #[qsignal]
        fn request_redo(self: Pin<&mut Self>);

        #[qsignal]
        fn session_cancelled(self: Pin<&mut Self>);

        #[qsignal]
        fn request_overlay_controller_reset(self: Pin<&mut Self>);
    }
}

pub struct CaptureSessionControllerRust {
    action_processing: bool,
    annotation_display_ready: bool,
    background_image_source: QUrl,
    busy: bool,
    has_screen_capture: bool,
    screen_width: f64,
    screen_height: f64,
    toolbar_padding: f64,
    toolbar_spacing_above: f64,
    toolbar_spacing_below: f64,
    default_toolbar_y: f64,
}

impl Default for CaptureSessionControllerRust {
    fn default() -> Self {
        Self {
            action_processing: false,
            annotation_display_ready: false,
            background_image_source: QUrl::default(),
            busy: false,
            has_screen_capture: false,
            screen_width: 0.0,
            screen_height: 0.0,
            toolbar_padding: 10.0,
            toolbar_spacing_above: 4.0,
            toolbar_spacing_below: 4.0,
            default_toolbar_y: 40.0,
        }
    }
}

impl qobject::CaptureSessionController {
    pub fn confirm_action(mut self: Pin<&mut Self>, action: i32, selection_rect: QRectF, has_annotations: bool) {
        if *self.busy() || !*self.has_screen_capture() {
            return;
        }

        if is_undo_action(action) {
            self.as_mut().request_undo();
            return;
        }
        if is_redo_action(action) {
            self.as_mut().request_redo();
            return;
        }

        let selection = SelectionRect::from_qrect(&selection_rect);
        self.as_mut().set_action_processing(true);
        self.as_mut().request_action_dispatch(action, selection.to_qrect(), has_annotations);
    }

    pub fn cancel_session(mut self: Pin<&mut Self>, force: bool) {
        if !force && *self.busy() {
            return;
        }
        self.as_mut().reset_session();
        self.as_mut().request_overlay_hide();
        self.as_mut().session_cancelled();
    }

    pub fn begin_session(mut self: Pin<&mut Self>, source: QUrl) {
        self.as_mut().reset_session();
        self.as_mut().set_background_image_source(source);
        self.as_mut().request_overlay_hide();
        self.as_mut().request_overlay_present();
    }

    pub fn reset_session(mut self: Pin<&mut Self>) {
        self.as_mut().request_compositor_abort();
        self.as_mut().request_overlay_controller_reset();
        self.as_mut().set_action_processing(false);
        self.as_mut().set_annotation_display_ready(false);
        self.as_mut().request_annotation_reset();
        self.as_mut().set_background_image_source(QUrl::default());
        if *self.has_screen_capture() {
            self.as_mut().request_capture_flag(false);
        }
    }

    pub fn reset_annotation_state(mut self: Pin<&mut Self>) {
        self.as_mut().set_annotation_display_ready(false);
        self.as_mut().request_annotation_reset();
    }

    pub fn on_locked_state_changed(mut self: Pin<&mut Self>, is_locked: bool) {
        if !is_locked {
            self.as_mut().set_annotation_display_ready(false);
        }
    }

    pub fn promote_annotation_display_ready(mut self: Pin<&mut Self>) {
        if !self.annotation_display_ready() {
            self.as_mut().set_annotation_display_ready(true);
        }
    }

    pub fn toolbar_position(&self, target_rect: QRectF, item_w: f64, item_h: f64, is_above: bool) -> QPointF {
        let (x, y) = crate::core::geometry::calculate_toolbar_position(
            target_rect.x(),
            target_rect.y(),
            target_rect.width(),
            target_rect.height(),
            item_w,
            item_h,
            is_above,
            *self.toolbar_padding(),
            *self.toolbar_spacing_above(),
            *self.toolbar_spacing_below(),
            *self.default_toolbar_y(),
            *self.screen_width(),
            *self.screen_height(),
        );

        QPointF::new(x, y)
    }
}
