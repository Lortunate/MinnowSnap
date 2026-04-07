use crate::core::capture::service::CaptureService;
use crate::core::geometry::Rect;
use crate::core::i18n;
use std::str::FromStr;
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
    PinRequested(String, Rect, bool),
    OcrResult(String),
    NoOp,
    Error(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CaptureInputMode {
    CropSelection,
    FullImage,
}

pub struct ActionContext {
    pub path: String,
    pub rect: Rect,
    pub input_mode: CaptureInputMode,
    pub save_path_override: Option<String>,
}

impl ActionContext {
    pub fn crop_selection(path: String, rect: Rect) -> Self {
        Self {
            path,
            rect,
            input_mode: CaptureInputMode::CropSelection,
            save_path_override: None,
        }
    }

    pub fn full_image(path: String) -> Self {
        Self {
            path,
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
        info!("Executing action: {:?} for path: {}", self, ctx.path);

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

    fn handle_copy(ctx: ActionContext) -> ActionResult {
        if CaptureService::copy_image(&ctx.path, ctx.rect, ctx.input_mode) {
            ActionResult::Copied
        } else {
            ActionResult::Error(i18n::capture::copy_failed())
        }
    }

    fn handle_save(ctx: ActionContext) -> ActionResult {
        match CaptureService::save_region_to_user_dir(&ctx.path, ctx.rect, ctx.input_mode, ctx.save_path_override) {
            Ok(path) => ActionResult::Saved(path),
            Err(e) => ActionResult::Error(e),
        }
    }

    fn handle_pin_ocr(ctx: ActionContext, auto_ocr: bool) -> ActionResult {
        if let Some(cropped) = CaptureService::resolve_image(&ctx.path, ctx.rect, ctx.input_mode)
            && let Some(temp_path) = CaptureService::save_temp(&cropped)
        {
            let source_rect = if ctx.rect.has_area() {
                ctx.rect
            } else {
                Rect::new(0, 0, cropped.width() as i32, cropped.height() as i32)
            };
            return ActionResult::PinRequested(temp_path, source_rect, auto_ocr);
        }
        ActionResult::Error(i18n::capture::pin_failed())
    }

    fn handle_qrcode(ctx: ActionContext) -> ActionResult {
        if let Some(content) = CaptureService::detect_qrcode(&ctx.path, ctx.rect, ctx.input_mode) {
            ActionResult::OcrResult(content)
        } else {
            ActionResult::Error(i18n::overlay::qr_not_found())
        }
    }

    fn handle_pick_color(ctx: ActionContext) -> ActionResult {
        let Some(img) = CaptureService::resolve_image(&ctx.path, ctx.rect, ctx.input_mode) else {
            return ActionResult::Error(i18n::capture::copy_failed());
        };
        if img.width() == 0 || img.height() == 0 {
            return ActionResult::NoOp;
        }
        let center_x = img.width() / 2;
        let center_y = img.height() / 2;
        let pixel = img.get_pixel(center_x, center_y);
        ActionResult::ColorPicked(format!("#{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2]))
    }
}
