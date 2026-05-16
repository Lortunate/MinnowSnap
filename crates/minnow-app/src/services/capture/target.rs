use crate::services::geometry::Rect;
use xcap::Monitor;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureMonitorTarget {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale_factor: f32,
}

impl CaptureMonitorTarget {
    #[must_use]
    pub fn from_monitor(monitor: &Monitor) -> Option<Self> {
        let width = i32::try_from(monitor.width().ok()?).ok()?;
        let height = i32::try_from(monitor.height().ok()?).ok()?;
        Some(Self {
            id: monitor.id().ok()?,
            x: monitor.x().ok()?,
            y: monitor.y().ok()?,
            width,
            height,
            scale_factor: monitor.scale_factor().ok().unwrap_or(1.0),
        })
    }

    #[must_use]
    pub fn effective_scale(self) -> f32 {
        if self.scale_factor <= 0.0 { 1.0 } else { self.scale_factor }
    }

    #[must_use]
    pub fn logical_geometry(self) -> (f64, f64, f64, f64) {
        let scale = f64::from(self.effective_scale());
        (
            f64::from(self.x) / scale,
            f64::from(self.y) / scale,
            f64::from(self.width) / scale,
            f64::from(self.height) / scale,
        )
    }

    #[must_use]
    pub fn center(self) -> (i32, i32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    #[must_use]
    pub fn rect(self) -> Rect {
        Rect::new(self.x, self.y, self.width, self.height)
    }
}
