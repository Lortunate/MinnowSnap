#[cfg(target_os = "windows")]
pub(crate) fn install(cx: &mut gpui::App) -> gpui::Result<()> {
    use gpui::{AppContext, Bounds, Empty, WindowBounds, WindowKind, WindowOptions, point, px, size};
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(-10_000.0), px(-10_000.0)),
                size(px(1.0), px(1.0)),
            ))),
            kind: WindowKind::PopUp,
            titlebar: None,
            focus: false,
            show: false,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_min_size: Some(size(px(1.0), px(1.0))),
            ..WindowOptions::default()
        },
        |_, cx| cx.new(|_| Empty),
    )?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn install(_: &mut gpui::App) -> gpui::Result<()> {
    Ok(())
}
