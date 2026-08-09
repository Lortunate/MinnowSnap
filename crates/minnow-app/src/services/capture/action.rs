use crate::services::capture::service::{CaptureService, ResolvedCaptureImage};
use crate::services::geometry::Rect;
use crate::services::i18n;
use image::RgbaImage;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CaptureAction {
    Copy,
    Save,
    Pin,
    Ocr,
    Scroll,
    QrCode,
    PickColor,
    Unknown,
}

impl FromStr for CaptureAction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "copy" => Ok(CaptureAction::Copy),
            "save" => Ok(CaptureAction::Save),
            "pin" => Ok(CaptureAction::Pin),
            "ocr" => Ok(CaptureAction::Ocr),
            "scroll" => Ok(CaptureAction::Scroll),
            "qrcode" => Ok(CaptureAction::QrCode),
            "pick-color" => Ok(CaptureAction::PickColor),
            _ => Ok(CaptureAction::Unknown),
        }
    }
}

#[derive(Debug)]
pub enum ActionResult {
    Copied,
    ColorPicked(String),
    Saved(String),
    PinRequested(PinCaptureRequest),
    OcrResult(String),
    NoOp,
    Error(String),
}

/// Domain output for a capture request. The application workflow owns the
/// platform effects required to turn a plan into an [`ActionResult`].
#[derive(Debug)]
pub(crate) enum CaptureActionPlan {
    CopyImage(Arc<RgbaImage>),
    SaveImage {
        image: Arc<RgbaImage>,
        save_path_override: Option<String>,
    },
    Pin {
        image: Arc<RgbaImage>,
        source_bounds: Rect,
        auto_ocr: bool,
    },
    Text(String),
    ColorPicked(String),
    NoOp,
    Error(String),
}

#[derive(Debug)]
pub struct PinCaptureRequest {
    pub image_path: String,
    pub source_bounds: Rect,
    pub auto_ocr: bool,
    pub ocr_image: Arc<RgbaImage>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CaptureInputMode {
    CropSelection,
    FullImage,
}

pub struct ActionContext {
    source: ActionImageSource,
    pub rect: Rect,
    pub input_mode: CaptureInputMode,
    pub save_path_override: Option<String>,
}

enum ActionImageSource {
    Path(String),
    Rgba(Arc<RgbaImage>),
}

impl ActionContext {
    pub fn crop_selection(path: String, rect: Rect) -> Self {
        Self {
            source: ActionImageSource::Path(path),
            rect,
            input_mode: CaptureInputMode::CropSelection,
            save_path_override: None,
        }
    }

    pub fn full_image(path: String) -> Self {
        Self {
            source: ActionImageSource::Path(path),
            rect: Rect::empty(),
            input_mode: CaptureInputMode::FullImage,
            save_path_override: None,
        }
    }

    pub(crate) fn full_image_data(image: Arc<RgbaImage>) -> Self {
        Self {
            source: ActionImageSource::Rgba(image),
            rect: Rect::empty(),
            input_mode: CaptureInputMode::FullImage,
            save_path_override: None,
        }
    }

    pub fn with_save_path_override(mut self, save_path_override: String) -> Self {
        self.save_path_override = Some(save_path_override);
        self
    }
}

impl CaptureAction {
    pub(crate) fn plan(&self, ctx: ActionContext) -> CaptureActionPlan {
        match self {
            CaptureAction::Copy => Self::plan_image(ctx, false),
            CaptureAction::Save => Self::plan_image(ctx, true),
            CaptureAction::Pin => Self::plan_pin_ocr(ctx, false),
            CaptureAction::Ocr => Self::plan_pin_ocr(ctx, true),
            CaptureAction::QrCode => Self::plan_qrcode(ctx),
            CaptureAction::PickColor => Self::plan_pick_color(ctx),
            CaptureAction::Scroll | CaptureAction::Unknown => CaptureActionPlan::NoOp,
        }
    }

    fn resolve_image(ctx: &ActionContext) -> Option<ResolvedCaptureImage> {
        match &ctx.source {
            ActionImageSource::Path(path) => CaptureService::resolve_capture_image(path, ctx.rect, ctx.input_mode),
            ActionImageSource::Rgba(image) => CaptureService::resolve_rgba_image(image.clone(), ctx.rect, ctx.input_mode),
        }
    }

    fn plan_image(ctx: ActionContext, save: bool) -> CaptureActionPlan {
        let Some(image) = Self::resolve_image(&ctx) else {
            let message = if save {
                "Failed to resolve or crop image for saving".to_string()
            } else {
                i18n::capture::copy_failed()
            };
            return CaptureActionPlan::Error(message);
        };

        let image = image.into_arc();
        if save {
            CaptureActionPlan::SaveImage {
                image,
                save_path_override: ctx.save_path_override,
            }
        } else {
            CaptureActionPlan::CopyImage(image)
        }
    }

    fn plan_pin_ocr(ctx: ActionContext, auto_ocr: bool) -> CaptureActionPlan {
        if let Some(image) = Self::resolve_image(&ctx) {
            let source_rect = if ctx.rect.has_area() {
                ctx.rect
            } else {
                Rect::new(0, 0, image.as_rgba().width() as i32, image.as_rgba().height() as i32)
            };
            return CaptureActionPlan::Pin {
                image: image.into_arc(),
                source_bounds: source_rect,
                auto_ocr,
            };
        }
        CaptureActionPlan::Error(i18n::capture::pin_failed())
    }

    fn plan_qrcode(ctx: ActionContext) -> CaptureActionPlan {
        if let Some(image) = Self::resolve_image(&ctx)
            && let Some(content) = CaptureService::decode_qrcode(image.as_rgba())
        {
            CaptureActionPlan::Text(content)
        } else {
            CaptureActionPlan::Error(i18n::overlay::qr_not_found())
        }
    }

    fn plan_pick_color(ctx: ActionContext) -> CaptureActionPlan {
        let Some(img) = Self::resolve_image(&ctx) else {
            return CaptureActionPlan::Error(i18n::capture::copy_failed());
        };
        let img = img.as_rgba();
        if img.width() == 0 || img.height() == 0 {
            return CaptureActionPlan::NoOp;
        }
        let center_x = img.width() / 2;
        let center_y = img.height() / 2;
        let pixel = img.get_pixel(center_x, center_y);
        CaptureActionPlan::ColorPicked(format!("#{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn pin_action_preserves_in_memory_image_for_ocr() {
        let image = Arc::new(RgbaImage::from_pixel(8, 6, Rgba([255, 0, 0, 255])));

        let CaptureActionPlan::Pin {
            image: planned,
            source_bounds,
            auto_ocr,
        } = CaptureAction::Pin.plan(ActionContext::full_image_data(image.clone()))
        else {
            panic!("pin action should produce a pin plan");
        };

        assert!(Arc::ptr_eq(&planned, &image));
        assert_eq!(source_bounds, Rect::new(0, 0, 8, 6));
        assert!(!auto_ocr);
    }
}
