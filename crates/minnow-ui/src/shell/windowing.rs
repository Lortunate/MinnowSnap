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
    crate::support::appearance::apply_saved_preferences(Some(window), cx);
    if focus {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{WindowBounds, WindowKind, point, px, size};

    #[test]
    fn popup_window_options_maps_spec_fields() {
        let bounds = WindowBounds::Windowed(gpui::Bounds::new(point(px(12.0), px(24.0)), size(px(320.0), px(200.0))));
        let spec = PopupWindowSpec {
            window_bounds: Some(bounds),
            kind: WindowKind::PopUp,
            focus: true,
            show: true,
            is_movable: false,
            is_resizable: true,
            is_minimizable: false,
            display_id: None,
            window_min_size: Some(size(px(120.0), px(80.0))),
        };

        let options = popup_window_options(spec, "minnowsnap.test");

        assert_eq!(options.window_bounds, Some(bounds));
        assert_eq!(options.kind, WindowKind::PopUp);
        assert!(options.focus);
        assert!(options.show);
        assert!(!options.is_movable);
        assert!(options.is_resizable);
        assert!(!options.is_minimizable);
        assert_eq!(options.window_background, WindowBackgroundAppearance::Transparent);
        assert_eq!(options.app_id.as_deref(), Some("minnowsnap.test"));
        assert_eq!(options.window_min_size, Some(size(px(120.0), px(80.0))));
    }
}
