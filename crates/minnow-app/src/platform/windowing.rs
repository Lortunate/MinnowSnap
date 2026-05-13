use gpui::{App, DisplayId, Pixels, Size, Window, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions};

#[derive(Clone, Copy, Debug)]
pub struct PopupWindowSpec {
    pub window_bounds: Option<WindowBounds>,
    pub kind: WindowKind,
    pub focus: bool,
    pub show: bool,
    pub is_movable: bool,
    pub is_resizable: bool,
    pub is_minimizable: bool,
    pub display_id: Option<DisplayId>,
    pub window_min_size: Option<Size<Pixels>>,
}

pub fn popup_window_options(spec: PopupWindowSpec, app_id: &'static str) -> WindowOptions {
    WindowOptions {
        window_bounds: spec.window_bounds,
        titlebar: None,
        kind: spec.kind,
        focus: spec.focus,
        show: spec.show,
        is_movable: spec.is_movable,
        is_resizable: spec.is_resizable,
        is_minimizable: spec.is_minimizable,
        display_id: spec.display_id,
        window_background: WindowBackgroundAppearance::Transparent,
        window_decorations: None,
        app_id: Some(app_id.to_string()),
        tabbing_identifier: None,
        window_min_size: spec.window_min_size,
    }
}

pub fn configure_window(window: &mut Window, cx: &mut App, focus: bool) {
    if focus {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
    }
}
