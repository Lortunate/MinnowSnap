use cxx_qt_lib::{QPointF, QRectF, QString};
use std::pin::Pin;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qpointf.h");
        type QPointF = cxx_qt_lib::QPointF;
        include!("cxx-qt-lib/qrectf.h");
        type QRectF = cxx_qt_lib::QRectF;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, action_processing, cxx_name = "actionProcessing")]
        #[qproperty(bool, annotation_display_ready, cxx_name = "annotationDisplayReady")]
        #[qproperty(QString, background_image_source, cxx_name = "backgroundImageSource")]
        #[qproperty(bool, busy, cxx_name = "busy")]
        #[qproperty(bool, has_screen_capture, cxx_name = "hasScreenCapture")]
        #[qproperty(f64, screen_width, cxx_name = "screenWidth")]
        #[qproperty(f64, screen_height, cxx_name = "screenHeight")]
        #[qproperty(f64, toolbar_padding, cxx_name = "toolbarPadding")]
        #[qproperty(f64, toolbar_spacing_above, cxx_name = "toolbarSpacingAbove")]
        #[qproperty(f64, toolbar_spacing_below, cxx_name = "toolbarSpacingBelow")]
        #[qproperty(f64, default_toolbar_y, cxx_name = "defaultToolbarY")]
        type CaptureSessionController = super::CaptureSessionControllerRust;

        #[qinvokable]
        #[cxx_name = "confirmAction"]
        fn confirm_action(self: Pin<&mut Self>, action: QString, selection_rect: QRectF, has_annotations: bool);

        #[qinvokable]
        #[cxx_name = "cancelSession"]
        fn cancel_session(self: Pin<&mut Self>, force: bool);

        #[qinvokable]
        #[cxx_name = "beginSession"]
        fn begin_session(self: Pin<&mut Self>, source: QString);

        #[qinvokable]
        #[cxx_name = "resetSession"]
        fn reset_session(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "resetAnnotationState"]
        fn reset_annotation_state(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "onLockedStateChanged"]
        fn on_locked_state_changed(self: Pin<&mut Self>, is_locked: bool);

        #[qinvokable]
        #[cxx_name = "promoteAnnotationDisplayReady"]
        fn promote_annotation_display_ready(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "toolbarPosition"]
        fn toolbar_position(self: &Self, target_rect: QRectF, item_w: f64, item_h: f64, is_above: bool) -> QPointF;

        #[qsignal]
        #[cxx_name = "requestAnnotationReset"]
        fn request_annotation_reset(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "requestCompositorAbort"]
        fn request_compositor_abort(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "requestOverlayHide"]
        fn request_overlay_hide(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "requestOverlayPresent"]
        fn request_overlay_present(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "requestCaptureFlag"]
        fn request_capture_flag(self: Pin<&mut Self>, value: bool);

        #[qsignal]
        #[cxx_name = "requestActionDispatch"]
        fn request_action_dispatch(self: Pin<&mut Self>, action: QString, x: i32, y: i32, width: i32, height: i32, has_annotations: bool);

        #[qsignal]
        #[cxx_name = "requestUndo"]
        fn request_undo(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "requestRedo"]
        fn request_redo(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "sessionCancelled"]
        fn session_cancelled(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "requestOverlayControllerReset"]
        fn request_overlay_controller_reset(self: Pin<&mut Self>);
    }
}

pub struct CaptureSessionControllerRust {
    action_processing: bool,
    annotation_display_ready: bool,
    background_image_source: QString,
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
            background_image_source: QString::default(),
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

fn normalize_selection_rect(rect: &QRectF) -> (i32, i32, i32, i32) {
    let x = rect.x().floor() as i32;
    let y = rect.y().floor() as i32;
    let width = rect.width().ceil().max(1.0) as i32;
    let height = rect.height().ceil().max(1.0) as i32;
    (x, y, width, height)
}

impl qobject::CaptureSessionController {
    pub fn confirm_action(mut self: Pin<&mut Self>, action: QString, selection_rect: QRectF, has_annotations: bool) {
        if *self.busy() || !*self.has_screen_capture() {
            return;
        }

        match action.to_string().as_str() {
            "undo" => {
                self.as_mut().request_undo();
                return;
            }
            "redo" => {
                self.as_mut().request_redo();
                return;
            }
            _ => {}
        }

        let (x, y, width, height) = normalize_selection_rect(&selection_rect);
        self.as_mut().set_action_processing(true);
        self.as_mut().request_action_dispatch(action, x, y, width, height, has_annotations);
    }

    pub fn cancel_session(mut self: Pin<&mut Self>, force: bool) {
        if !force && *self.busy() {
            return;
        }
        self.as_mut().reset_session();
        self.as_mut().request_overlay_hide();
        self.as_mut().session_cancelled();
    }

    pub fn begin_session(mut self: Pin<&mut Self>, source: QString) {
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
        self.as_mut().set_background_image_source(QString::default());
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
        let target_x = target_rect.x();
        let target_y = target_rect.y();
        let target_w = target_rect.width();
        let target_h = target_rect.height();
        let padding = *self.toolbar_padding();
        let spacing_above = *self.toolbar_spacing_above();
        let spacing_below = *self.toolbar_spacing_below();
        let default_y = *self.default_toolbar_y();
        let screen_w = *self.screen_width();
        let screen_h = *self.screen_height();
        let desired_x = target_x + target_w - item_w;
        let max_x = (screen_w - item_w - padding).max(padding);
        let x = desired_x.clamp(padding, max_x);

        let y = if is_above {
            let above_y = target_y - item_h - spacing_above;
            if above_y >= 0.0 { above_y } else { target_y + target_h + spacing_above }
        } else {
            let below_y = target_y + target_h + spacing_below;
            let above_y = target_y - item_h - spacing_below;
            if below_y + item_h <= screen_h {
                below_y
            } else if above_y >= 0.0 {
                above_y
            } else {
                default_y
            }
        };

        QPointF::new(x, y)
    }
}
