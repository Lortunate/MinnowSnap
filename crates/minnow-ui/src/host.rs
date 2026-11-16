use ab_glyph::FontArc;
use gpui::Window;
use minnow_application::OperationResult;
pub use minnow_application::preferences::EffectiveShortcuts;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct UiPaths {
    pub config_file: PathBuf,
    pub logs_dir: PathBuf,
    pub models_dir: PathBuf,
}

/// Main-thread native integrations supplied by the composition root.
pub trait UiHost {
    fn exclude_from_capture(&self, window: &Window) -> OperationResult<()>;
    fn set_always_on_top(&self, window: &Window) -> OperationResult<()>;
    fn set_click_through(&self, window: &Window, enabled: bool) -> OperationResult<()>;
    fn start_window_drag(&self, window: &Window) -> OperationResult<()>;
    fn system_fonts(&self) -> Vec<String>;
    fn annotation_font(&self) -> OperationResult<FontArc>;
    fn paths(&self) -> &UiPaths;
}
