use cxx_qt::CxxQtType;
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
        #[qproperty(bool, processing)]
        type CaptureCompositorController = super::CaptureCompositorControllerRust;

        #[qinvokable]
        fn start(
            self: Pin<&mut Self>,
            action: QString,
            selection_rect: QRectF,
            has_annotations: bool,
            source_pixel_width: f64,
            source_logical_width: f64,
            fallback_dpr: f64,
        );

        #[qinvokable]
        fn abort(self: Pin<&mut Self>);

        #[qinvokable]
        fn handle_grab_result(self: Pin<&mut Self>, composition_id: i32, save_path: QString, save_succeeded: bool);

        #[qinvokable]
        fn is_active(self: Pin<&mut Self>, composition_id: i32) -> bool;

        #[qsignal]
        fn request_prepare_composition(self: Pin<&mut Self>, composition_id: i32, selection_rect: QRectF, out_width: i32, out_height: i32);

        #[qsignal]
        fn request_submit_direct(self: Pin<&mut Self>, action: QString, selection_rect: QRectF);

        #[qsignal]
        fn request_submit_composited(self: Pin<&mut Self>, path: QString, action: QString, selection_rect: QRectF);

        #[qsignal]
        fn request_restore_annotation(self: Pin<&mut Self>);

        #[qsignal]
        fn request_composition_failed(self: Pin<&mut Self>);

        #[qsignal]
        fn request_reset_surface(self: Pin<&mut Self>);
    }
}

pub struct CaptureCompositorControllerRust {
    processing: bool,
    composition_version: i32,
    active_composition_id: i32,
    pending_action: QString,
    selection_rect: QRectF,
}

impl Default for CaptureCompositorControllerRust {
    fn default() -> Self {
        Self {
            processing: false,
            composition_version: 0,
            active_composition_id: -1,
            pending_action: QString::default(),
            selection_rect: QRectF::default(),
        }
    }
}

fn normalize_rect(rect: &QRectF) -> QRectF {
    let (x, y, width, height) = crate::core::geometry::normalize_rect(rect.x(), rect.y(), rect.width(), rect.height());
    QRectF::new(f64::from(x), f64::from(y), f64::from(width), f64::from(height))
}

impl qobject::CaptureCompositorController {
    pub fn start(
        mut self: Pin<&mut Self>,
        action: QString,
        selection_rect: QRectF,
        has_annotations: bool,
        source_pixel_width: f64,
        source_logical_width: f64,
        fallback_dpr: f64,
    ) {
        let normalized = normalize_rect(&selection_rect);
        let width = normalized.width().max(1.0);
        let height = normalized.height().max(1.0);
        if !has_annotations {
            self.as_mut().set_processing(false);
            self.as_mut().request_reset_surface();
            self.as_mut().request_submit_direct(action, normalized);
            return;
        }

        {
            let mut rust = self.as_mut().rust_mut();
            rust.composition_version += 1;
            rust.active_composition_id = rust.composition_version;
            rust.pending_action = QString::from(&action.to_string());
            rust.selection_rect = normalized.clone();
        }

        self.as_mut().set_processing(true);

        let dpr = if source_pixel_width > 0.0 && source_logical_width > 0.0 {
            source_pixel_width / source_logical_width
        } else {
            fallback_dpr
        }
        .max(1.0);

        let out_width = (width * dpr).ceil().max(1.0) as i32;
        let out_height = (height * dpr).ceil().max(1.0) as i32;
        let composition_id = self.rust().active_composition_id;
        self.as_mut()
            .request_prepare_composition(composition_id, normalized, out_width, out_height);
    }

    pub fn abort(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.active_composition_id = -1;
        }
        self.as_mut().set_processing(false);
        self.as_mut().request_restore_annotation();
        self.as_mut().request_reset_surface();
    }

    pub fn handle_grab_result(mut self: Pin<&mut Self>, composition_id: i32, save_path: QString, save_succeeded: bool) {
        if self.rust().active_composition_id != composition_id {
            return;
        }

        self.as_mut().request_restore_annotation();

        let (action, rect) = {
            let rust = self.rust();
            (rust.pending_action.to_string(), rust.selection_rect.clone())
        };

        {
            let mut rust = self.as_mut().rust_mut();
            rust.active_composition_id = -1;
        }

        self.as_mut().set_processing(false);
        self.as_mut().request_reset_surface();

        if save_succeeded && !save_path.is_empty() {
            self.as_mut().request_submit_composited(save_path, QString::from(&action), rect);
        } else {
            self.as_mut().request_composition_failed();
        }
    }

    pub fn is_active(self: Pin<&mut Self>, composition_id: i32) -> bool {
        self.rust().active_composition_id == composition_id
    }
}
