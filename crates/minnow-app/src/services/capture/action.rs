use crate::services::capture::service::{CaptureService, ResolvedCaptureImage};
use crate::services::geometry::Rect;
use crate::services::i18n;
use image::RgbaImage;
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

#[derive(Debug, PartialEq, Clone)]
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

impl ActionImageSource {
    fn label(&self) -> &str {
        match self {
            Self::Path(path) => path,
            Self::Rgba(_) => "in-memory image",
        }
    }
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
    pub fn execute(&self, ctx: ActionContext) -> ActionResult {
        info!("Executing action: {:?} for source: {}", self, ctx.source.label());

        match self {
            CaptureAction::Copy => Self::handle_copy(ctx),
            CaptureAction::Save => Self::handle_save(ctx),
            CaptureAction::Pin => Self::handle_pin_ocr(ctx, false),
            CaptureAction::Ocr => Self::handle_pin_ocr(ctx, true),
            CaptureAction::QrCode => Self::handle_qrcode(ctx),
            CaptureAction::PickColor => Self::handle_pick_color(ctx),
            CaptureAction::Scroll | CaptureAction::Unknown => ActionResult::NoOp,
        }
    }

    fn resolve_image(ctx: &ActionContext) -> Option<ResolvedCaptureImage> {
        match &ctx.source {
            ActionImageSource::Path(path) => CaptureService::resolve_capture_image(path, ctx.rect, ctx.input_mode),
            ActionImageSource::Rgba(image) => CaptureService::resolve_rgba_image(image.clone(), ctx.rect, ctx.input_mode),
        }
    }

    fn handle_copy(ctx: ActionContext) -> ActionResult {
        if let Some(image) = Self::resolve_image(&ctx)
            && CaptureService::copy_rgba(image.as_rgba())
        {
            ActionResult::Copied
        } else {
            ActionResult::Error(i18n::capture::copy_failed())
        }
    }

    fn handle_save(ctx: ActionContext) -> ActionResult {
        let Some(image) = Self::resolve_image(&ctx) else {
            return ActionResult::Error("Failed to resolve or crop image for saving".to_string());
        };

        match CaptureService::save_rgba_to_user_dir(image.as_rgba(), ctx.save_path_override) {
            Ok(path) => ActionResult::Saved(path),
            Err(e) => ActionResult::Error(e),
        }
    }

    fn handle_pin_ocr(ctx: ActionContext, auto_ocr: bool) -> ActionResult {
        if let Some(image) = Self::resolve_image(&ctx)
            && let Some(temp_path) = CaptureService::save_temp(image.as_rgba())
        {
            let source_rect = if ctx.rect.has_area() {
                ctx.rect
            } else {
                Rect::new(0, 0, image.as_rgba().width() as i32, image.as_rgba().height() as i32)
            };
            return ActionResult::PinRequested(PinCaptureRequest {
                image_path: temp_path,
                source_bounds: source_rect,
                auto_ocr,
                ocr_image: image.into_arc(),
            });
        }
        ActionResult::Error(i18n::capture::pin_failed())
    }

    fn handle_qrcode(ctx: ActionContext) -> ActionResult {
        if let Some(image) = Self::resolve_image(&ctx)
            && let Some(content) = CaptureService::decode_qrcode(image.as_rgba())
        {
            ActionResult::OcrResult(content)
        } else {
            ActionResult::Error(i18n::overlay::qr_not_found())
        }
    }

    fn handle_pick_color(ctx: ActionContext) -> ActionResult {
        let Some(img) = Self::resolve_image(&ctx) else {
            return ActionResult::Error(i18n::capture::copy_failed());
        };
        let img = img.as_rgba();
        if img.width() == 0 || img.height() == 0 {
            return ActionResult::NoOp;
        }
        let center_x = img.width() / 2;
        let center_y = img.height() / 2;
        let pixel = img.get_pixel(center_x, center_y);
        ActionResult::ColorPicked(format!("#{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn pin_action_preserves_in_memory_image_for_ocr() {
        let image = Arc::new(RgbaImage::from_pixel(8, 6, Rgba([255, 0, 0, 255])));

        let ActionResult::PinRequested(request) = CaptureAction::Pin.execute(ActionContext::full_image_data(image.clone())) else {
            panic!("pin action should request a pin window");
        };

        assert!(Arc::ptr_eq(&request.ocr_image, &image));
        assert_eq!(request.source_bounds, Rect::new(0, 0, 8, 6));
        let _ = std::fs::remove_file(request.image_path);
    }
}
