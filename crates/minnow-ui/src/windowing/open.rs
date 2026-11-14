use gpui::{App, Entity, Render, Window, WindowHandle, WindowOptions};
use minnow_application::{ErrorKind, OperationError, OperationResult};

/// Open the feature's original root and confirm required native effects before
/// reporting a successful handoff. Rejected windows release their feature state.
///
/// Keeping the root type intact is required by gpui-component's root lookup and
/// GPUI's typed window handles; an intermediate root would break both contracts.
pub(crate) fn open_prepared_window<V: Render + 'static>(
    cx: &mut App,
    options: WindowOptions,
    prepare_native: impl FnOnce(&mut Window, &App) -> OperationResult<()>,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V> + 'static,
) -> OperationResult<WindowHandle<V>> {
    let handle = cx
        .open_window(options, build)
        .map_err(|error| OperationError::new(ErrorKind::Native, format!("failed to open native window: {error}")))?;

    let error = match handle.update(cx, |_, window, cx| prepare_native(window, cx)) {
        Ok(Ok(())) => return Ok(handle),
        Ok(Err(error)) => error,
        Err(error) => OperationError::new(ErrorKind::Native, format!("failed to prepare native window: {error}")),
    };
    if let Err(cleanup) = handle.update(cx, |_, window, _| window.remove_window()) {
        return Err(OperationError::new(
            error.kind,
            format!("{error}; failed to close rejected native window: {cleanup}"),
        ));
    }
    Err(error)
}
