use minnow_capture::{CaptureBackend, CaptureError, CaptureTarget, DisplayInfo, WindowInfo};
use minnow_imaging::{CaptureGeometry, CapturedFrame, DisplayId, ImageAsset, RectF, ScreenPoint};
use xcap::Monitor;

/// Stateless native acquisition. A frame carries the exact display transform
/// used for that acquisition; later processing never re-queries desktop scale.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeCapture;

impl CaptureBackend for NativeCapture {
    fn capture(&self, target: &CaptureTarget) -> Result<CapturedFrame, CaptureError> {
        let monitor = select_monitor(target)?;
        let mut geometry = monitor_geometry(&monitor)?;
        let image = monitor.capture_image().map_err(backend_error)?;
        geometry.pixel_size = image.dimensions();
        CapturedFrame::new(ImageAsset::new(image), geometry).map_err(Into::into)
    }

    fn displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
        Monitor::all()
            .map_err(backend_error)?
            .into_iter()
            .map(|monitor| {
                Ok(DisplayInfo {
                    geometry: monitor_geometry(&monitor)?,
                    name: monitor.friendly_name().or_else(|_| monitor.name()).map_err(backend_error)?,
                    is_primary: monitor.is_primary().map_err(backend_error)?,
                })
            })
            .collect()
    }

    fn windows(&self, display: &DisplayId) -> Result<Vec<WindowInfo>, CaptureError> {
        let monitor = select_monitor(&CaptureTarget::Display(display.clone()))?;
        let geometry = monitor_geometry(&monitor)?;
        let raw_origin = (
            f64::from(monitor.x().map_err(backend_error)?),
            f64::from(monitor.y().map_err(backend_error)?),
        );
        let units_per_logical = native_units_per_logical(geometry.scale_factor);
        let viewport = geometry.viewport_bounds();
        let mut occluders = Vec::new();
        let mut result = Vec::new();

        for window in xcap::Window::all().map_err(backend_error)? {
            if window.is_minimized().unwrap_or(true) {
                continue;
            }
            let (Ok(x), Ok(y), Ok(width), Ok(height)) = (window.x(), window.y(), window.width(), window.height()) else {
                // A window may close between enumeration and reading bounds.
                continue;
            };
            let Some(bounds) = local_window_bounds(
                RawBounds {
                    x: f64::from(x),
                    y: f64::from(y),
                    width: f64::from(width),
                    height: f64::from(height),
                },
                raw_origin,
                units_per_logical,
                viewport,
            ) else {
                continue;
            };
            if occluders.iter().any(|occluder| is_inside(bounds, *occluder)) {
                continue;
            }
            let app_name = window.app_name().unwrap_or_default();
            if !is_system_overlay(&app_name) {
                occluders.push(bounds);
            }
            result.push(WindowInfo {
                title: window.title().unwrap_or_default(),
                app_name,
                bounds,
            });
        }
        Ok(result)
    }
}

fn select_monitor(target: &CaptureTarget) -> Result<Monitor, CaptureError> {
    let monitors = Monitor::all().map_err(backend_error)?;
    if monitors.is_empty() {
        return Err(CaptureError::Backend("no displays are available".into()));
    }
    match target {
        CaptureTarget::Primary => {
            for monitor in &monitors {
                if monitor.is_primary().map_err(backend_error)? {
                    return Ok(monitor.clone());
                }
            }
            // A single display is unambiguous even on backends without a
            // configured primary monitor. Never guess from enumeration order.
            if monitors.len() == 1 {
                Ok(monitors.into_iter().next().expect("one monitor"))
            } else {
                Err(CaptureError::Backend(
                    "no primary display is configured; select an explicit display".into(),
                ))
            }
        }
        CaptureTarget::Display(id) => {
            for monitor in monitors {
                if monitor_id(&monitor)? == *id {
                    return Ok(monitor);
                }
            }
            Err(CaptureError::Backend(format!("display {} is no longer available", id.0)))
        }
    }
}

fn monitor_id(monitor: &Monitor) -> Result<DisplayId, CaptureError> {
    Ok(DisplayId(format!("xcap:{}", monitor.id().map_err(backend_error)?)))
}

fn monitor_geometry(monitor: &Monitor) -> Result<CaptureGeometry, CaptureError> {
    let scale = f64::from(monitor.scale_factor().map_err(backend_error)?);
    let raw_x = f64::from(monitor.x().map_err(backend_error)?);
    let raw_y = f64::from(monitor.y().map_err(backend_error)?);
    let width = monitor.width().map_err(backend_error)?;
    let height = monitor.height().map_err(backend_error)?;
    let units_per_logical = native_units_per_logical(scale);
    let physical_factor = scale / units_per_logical;
    let geometry = CaptureGeometry {
        display_id: monitor_id(monitor)?,
        screen_origin: ScreenPoint {
            x: raw_x / units_per_logical,
            y: raw_y / units_per_logical,
        },
        scale_factor: scale,
        pixel_size: (
            (f64::from(width) * physical_factor).round() as u32,
            (f64::from(height) * physical_factor).round() as u32,
        ),
    };
    geometry.validate()?;
    Ok(geometry)
}

fn native_units_per_logical(scale: f64) -> f64 {
    // CoreGraphics bounds are already logical points. Windows DEVMODE and X11
    // bounds are physical pixels, and must be converted using the target display.
    #[cfg(target_os = "macos")]
    {
        let _ = scale;
        1.0
    }
    #[cfg(not(target_os = "macos"))]
    {
        scale
    }
}

#[derive(Clone, Copy)]
struct RawBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn local_window_bounds(raw: RawBounds, origin: (f64, f64), units_per_logical: f64, viewport: RectF) -> Option<RectF> {
    if !units_per_logical.is_finite() || units_per_logical <= 0.0 || raw.width <= 0.0 || raw.height <= 0.0 {
        return None;
    }
    let x = (raw.x - origin.0) / units_per_logical;
    let y = (raw.y - origin.1) / units_per_logical;
    let left = x.max(viewport.x);
    let top = y.max(viewport.y);
    let right = (x + raw.width / units_per_logical).min(viewport.x + viewport.width);
    let bottom = (y + raw.height / units_per_logical).min(viewport.y + viewport.height);
    (right > left && bottom > top).then(|| RectF::new(left, top, right - left, bottom - top))
}

fn is_inside(rect: RectF, outer: RectF) -> bool {
    rect.x >= outer.x && rect.y >= outer.y && rect.x + rect.width <= outer.x + outer.width && rect.y + rect.height <= outer.y + outer.height
}

fn is_system_overlay(app_name: &str) -> bool {
    ["程序坞", "Dock", "Window Server", "Control Center", "Notification Center", "Spotlight"].contains(&app_name)
}

fn backend_error(error: impl std::fmt::Display) -> CaptureError {
    CaptureError::Backend(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_window_bounds_are_relative_to_the_selected_display() {
        let bounds = local_window_bounds(
            RawBounds {
                x: -1800.0,
                y: -150.0,
                width: 900.0,
                height: 600.0,
            },
            (-1920.0, -300.0),
            1.5,
            RectF::new(0.0, 0.0, 1280.0, 720.0),
        )
        .unwrap();
        assert_eq!(bounds, RectF::new(80.0, 100.0, 600.0, 400.0));
    }

    #[test]
    fn logical_window_bounds_are_not_scaled_twice() {
        let bounds = local_window_bounds(
            RawBounds {
                x: -1400.0,
                y: 10.0,
                width: 500.0,
                height: 400.0,
            },
            (-1440.0, 0.0),
            1.0,
            RectF::new(0.0, 0.0, 1440.0, 900.0),
        )
        .unwrap();
        assert_eq!(bounds, RectF::new(40.0, 10.0, 500.0, 400.0));
    }

    #[test]
    fn off_display_windows_are_clipped_or_rejected() {
        let viewport = RectF::new(0.0, 0.0, 100.0, 80.0);
        let raw = RawBounds {
            x: -10.0,
            y: -20.0,
            width: 50.0,
            height: 50.0,
        };
        assert_eq!(
            local_window_bounds(raw, (0.0, 0.0), 1.0, viewport),
            Some(RectF::new(0.0, 0.0, 40.0, 30.0))
        );
        assert!(local_window_bounds(raw, (500.0, 0.0), 1.0, viewport).is_none());
    }
}
