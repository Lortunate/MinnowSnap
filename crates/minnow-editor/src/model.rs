use image::RgbaImage;
use std::sync::Arc;

use minnow_imaging::geometry::RectF;

pub const MIN_DRAW_LENGTH: f64 = 8.0;
pub const TEXT_DEFAULT: &str = "Text";
pub const COLOR_PRESETS: [u32; 6] = [0xf44336ff, 0x2196f3ff, 0x4caf50ff, 0xff9800ff, 0xffffffff, 0x111111ff];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum AnnotationTool {
    Arrow,
    #[default]
    Rectangle,
    Circle,
    Counter,
    Text,
    Mosaic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum MosaicMode {
    #[default]
    Pixelate,
    Blur,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnnotationKindTag {
    Arrow,
    Rectangle,
    Circle,
    Counter,
    Text,
    Mosaic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnotationStyleState {
    pub stroke_color: u32,
    pub fill_color: u32,
    pub fill_enabled: bool,
    pub stroke_width: f64,
    pub text_size: f64,
    pub counter_radius: f64,
    pub mosaic_intensity: f64,
    pub mosaic_mode: MosaicMode,
}

impl Default for AnnotationStyleState {
    fn default() -> Self {
        Self {
            stroke_color: COLOR_PRESETS[0],
            fill_color: 0x00000033,
            fill_enabled: false,
            stroke_width: 3.0,
            text_size: 22.0,
            counter_radius: 18.0,
            mosaic_intensity: 10.0,
            mosaic_mode: MosaicMode::Pixelate,
        }
    }
}

impl AnnotationStyleState {
    /// Logical size limits used by editor tools. NaN and infinities are outside
    /// every range, so caller-constructed styles cannot introduce them.
    pub fn is_valid(&self) -> bool {
        (1.0..=18.0).contains(&self.stroke_width)
            && (12.0..=96.0).contains(&self.text_size)
            && (10.0..=64.0).contains(&self.counter_radius)
            && (2.0..=64.0).contains(&self.mosaic_intensity)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnnotationKind {
    Arrow { start: (f64, f64), end: (f64, f64) },
    Rectangle { rect: RectF },
    Circle { rect: RectF },
    Counter { center: (f64, f64), number: u32 },
    Text { origin: (f64, f64), text: String },
    Mosaic { rect: RectF, mode: MosaicMode, intensity: f64 },
}

impl AnnotationKind {
    pub const fn tag(&self) -> AnnotationKindTag {
        match self {
            Self::Arrow { .. } => AnnotationKindTag::Arrow,
            Self::Rectangle { .. } => AnnotationKindTag::Rectangle,
            Self::Circle { .. } => AnnotationKindTag::Circle,
            Self::Counter { .. } => AnnotationKindTag::Counter,
            Self::Text { .. } => AnnotationKindTag::Text,
            Self::Mosaic { .. } => AnnotationKindTag::Mosaic,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnnotationItem {
    pub id: u64,
    pub style: AnnotationStyleState,
    pub kind: AnnotationKind,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum AnnotationInteractionState {
    #[default]
    Idle,
    Drawing {
        tool: AnnotationTool,
        start: (f64, f64),
        current: (f64, f64),
        style: AnnotationStyleState,
    },
    Moving {
        id: u64,
        start: (f64, f64),
        current: (f64, f64),
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextEditState {
    pub id: u64,
    pub draft: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnotationSelectionInfo {
    pub id: u64,
    pub kind: AnnotationKindTag,
    pub metric: f64,
    pub mosaic_mode: Option<MosaicMode>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnotationOutline {
    pub id: u64,
    pub bounds: RectF,
    pub selected: bool,
    pub transient: bool,
}

#[derive(Clone, Debug, Default)]
pub struct AnnotationLayerState {
    pub raster_error: Option<minnow_imaging::GeometryError>,
    /// Actual logical bounds of the floor/ceil resolved pixel selection.
    pub image_bounds: Option<RectF>,
    pub image: Option<Arc<RgbaImage>>,
    pub outlines: Vec<AnnotationOutline>,
}

#[derive(Clone, Debug)]
pub struct AnnotationUiState {
    pub layer: AnnotationLayerState,
    pub selected: Option<AnnotationSelectionInfo>,
    pub tool: Option<AnnotationTool>,
    pub style: AnnotationStyleState,
    pub can_undo: bool,
    pub can_redo: bool,
    pub text_editing: bool,
}
