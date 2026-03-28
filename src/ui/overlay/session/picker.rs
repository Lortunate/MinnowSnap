use gpui::{Pixels, Point};

use super::{DragMode, OverlaySession};

const PICKER_POINTER_LOCK_EPSILON: f64 = 0.5;
const PICKER_NEIGHBORHOOD_SIZE: usize = 13;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum PickerFormat {
    #[default]
    Hex,
    Rgb,
    Hsl,
}

impl PickerFormat {
    pub fn cycle(self) -> Self {
        match self {
            Self::Hex => Self::Rgb,
            Self::Rgb => Self::Hsl,
            Self::Hsl => Self::Hex,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Hex => "HEX",
            Self::Rgb => "RGB",
            Self::Hsl => "HSL",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PickerSample {
    pub x: i32,
    pub y: i32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PickerNeighborhood {
    pub center_x: i32,
    pub center_y: i32,
    pub size: usize,
    pub pixels: Vec<[u8; 3]>,
}

impl PickerSample {
    pub fn formatted(&self, format: PickerFormat) -> String {
        match format {
            PickerFormat::Hex => self.hex(),
            PickerFormat::Rgb => self.rgb_text(),
            PickerFormat::Hsl => self.hsl_text(),
        }
    }

    pub fn hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    pub fn rgb_text(&self) -> String {
        format!("{}, {}, {}", self.r, self.g, self.b)
    }

    pub fn hsl_text(&self) -> String {
        let (h, s, l) = rgb_to_hsl(self.r, self.g, self.b);
        format!("{h}°, {s}%, {l}%")
    }
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (u16, u8, u8) {
    let r = f64::from(r) / 255.0;
    let g = f64::from(g) / 255.0;
    let b = f64::from(b) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < f64::EPSILON {
        return (0, 0, (l * 100.0).round() as u8);
    }

    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let mut h = if (max - r).abs() < f64::EPSILON {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < f64::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h /= 6.0;

    ((h * 360.0).round() as u16, (s * 100.0).round() as u8, (l * 100.0).round() as u8)
}

impl OverlaySession {
    pub(crate) fn picker_visible(&self) -> bool {
        self.viewport.mode == DragMode::Idle && self.viewport.selection.is_none() && self.background_pixels.is_some()
    }

    pub(crate) fn picker_text(&self) -> Option<String> {
        let sample = self.picker_sample.as_ref()?;
        Some(sample.formatted(self.picker_format))
    }

    pub(crate) fn cycle_picker_format(&mut self) -> bool {
        if !self.picker_visible() {
            return false;
        }
        self.picker_format = self.picker_format.cycle();
        true
    }

    pub(crate) fn update_pointer(&mut self, point: Point<Pixels>) -> bool {
        let (x, y) = self.clamp_point_to_viewport(point);

        if self.pointer_lock_matches(x, y) {
            return false;
        }
        self.picker_pointer_lock = None;

        let next_cursor = (x, y);
        self.picker_last_pointer = Some(next_cursor);
        self.set_picker_cursor(next_cursor)
    }

    pub(crate) fn move_picker_by_pixel(&mut self, delta_x: i32, delta_y: i32) -> bool {
        if !self.picker_visible() {
            return false;
        }
        let Some((image_w, image_h)) = self.picker_image_size() else {
            return false;
        };
        let (current_x, current_y) = self.current_picker_pixel(image_w, image_h);

        let next_x = (current_x + delta_x).clamp(0, image_w.saturating_sub(1));
        let next_y = (current_y + delta_y).clamp(0, image_h.saturating_sub(1));
        if next_x == current_x && next_y == current_y {
            return false;
        }

        let viewport_w = self.viewport.viewport_w.max(1.0);
        let viewport_h = self.viewport.viewport_h.max(1.0);
        let cursor_x = ((f64::from(next_x) + 0.5) / f64::from(image_w)) * viewport_w;
        let cursor_y = ((f64::from(next_y) + 0.5) / f64::from(image_h)) * viewport_h;

        self.viewport.pending_pointer = None;
        if self.picker_pointer_lock.is_none() {
            self.picker_pointer_lock = self.picker_last_pointer.or(self.picker_cursor);
        }

        self.set_picker_cursor((cursor_x, cursor_y))
    }

    fn pointer_lock_matches(&self, pointer_x: f64, pointer_y: f64) -> bool {
        let Some((locked_x, locked_y)) = self.picker_pointer_lock else {
            return false;
        };
        (pointer_x - locked_x).abs() < PICKER_POINTER_LOCK_EPSILON && (pointer_y - locked_y).abs() < PICKER_POINTER_LOCK_EPSILON
    }

    fn picker_image_size(&self) -> Option<(i32, i32)> {
        let image = self.background_pixels.as_ref()?;
        let image_w = i32::try_from(image.width().max(1)).ok()?;
        let image_h = i32::try_from(image.height().max(1)).ok()?;
        Some((image_w, image_h))
    }

    fn current_picker_pixel(&self, image_w: i32, image_h: i32) -> (i32, i32) {
        if let Some(sample) = self.picker_sample.as_ref() {
            return (sample.x, sample.y);
        }
        if let Some((cursor_x, cursor_y)) = self.picker_cursor {
            return self.cursor_to_image_pixel(cursor_x, cursor_y, image_w, image_h);
        }
        (image_w / 2, image_h / 2)
    }

    fn cursor_to_image_pixel(&self, x: f64, y: f64, image_w: i32, image_h: i32) -> (i32, i32) {
        let viewport_w = self.viewport.viewport_w.max(1.0);
        let viewport_h = self.viewport.viewport_h.max(1.0);
        let sample_x = ((x / viewport_w) * f64::from(image_w)).floor() as i32;
        let sample_y = ((y / viewport_h) * f64::from(image_h)).floor() as i32;
        (sample_x.clamp(0, image_w.saturating_sub(1)), sample_y.clamp(0, image_h.saturating_sub(1)))
    }

    fn set_picker_cursor(&mut self, cursor: (f64, f64)) -> bool {
        if self.picker_cursor == Some(cursor) {
            return false;
        }
        self.picker_cursor = Some(cursor);
        self.refresh_picker_sample();
        true
    }

    pub(super) fn clear_picker_sample_data(&mut self) {
        self.picker_sample = None;
        self.picker_neighborhood = None;
    }

    pub(super) fn refresh_picker_sample(&mut self) {
        if !self.picker_visible() {
            self.clear_picker_sample_data();
            return;
        }
        let Some((x, y)) = self.picker_cursor else {
            self.clear_picker_sample_data();
            return;
        };
        let Some((image_w, image_h)) = self.picker_image_size() else {
            self.clear_picker_sample_data();
            return;
        };
        let (clamped_x, clamped_y) = self.cursor_to_image_pixel(x, y, image_w, image_h);
        if self
            .picker_sample
            .as_ref()
            .is_some_and(|sample| sample.x == clamped_x && sample.y == clamped_y)
        {
            return;
        }
        let Some((sample, neighborhood)) = self.sample_picker_data_at(clamped_x, clamped_y) else {
            self.clear_picker_sample_data();
            return;
        };
        self.picker_sample = Some(sample);
        self.picker_neighborhood = Some(neighborhood);
    }

    fn sample_picker_data_at(&self, clamped_x: i32, clamped_y: i32) -> Option<(PickerSample, PickerNeighborhood)> {
        let image = self.background_pixels.as_ref()?;
        let image_w = usize::try_from(image.width().max(1)).ok()?;
        let image_h = usize::try_from(image.height().max(1)).ok()?;
        let image_w_i32 = i32::try_from(image_w).ok()?;
        let image_h_i32 = i32::try_from(image_h).ok()?;
        let max_x = image_w_i32.saturating_sub(1);
        let max_y = image_h_i32.saturating_sub(1);
        let clamped_x = clamped_x.clamp(0, max_x);
        let clamped_y = clamped_y.clamp(0, max_y);
        let row_stride = image_w.saturating_mul(4);
        let raw = image.as_raw();

        let read_rgb = |px: i32, py: i32| -> Option<[u8; 3]> {
            let px = usize::try_from(px).ok()?;
            let py = usize::try_from(py).ok()?;
            let offset = py.checked_mul(row_stride)?.checked_add(px.checked_mul(4)?)?;
            Some([*raw.get(offset)?, *raw.get(offset + 1)?, *raw.get(offset + 2)?])
        };

        let center_pixel = read_rgb(clamped_x, clamped_y)?;
        let sample = PickerSample {
            x: clamped_x,
            y: clamped_y,
            r: center_pixel[0],
            g: center_pixel[1],
            b: center_pixel[2],
        };

        let radius = i32::try_from(PICKER_NEIGHBORHOOD_SIZE / 2).ok()?;
        let mut pixels = Vec::with_capacity(PICKER_NEIGHBORHOOD_SIZE * PICKER_NEIGHBORHOOD_SIZE);

        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let nx = (clamped_x + dx).clamp(0, max_x);
                let ny = (clamped_y + dy).clamp(0, max_y);
                pixels.push(read_rgb(nx, ny)?);
            }
        }

        Some((
            sample,
            PickerNeighborhood {
                center_x: clamped_x,
                center_y: clamped_y,
                size: PICKER_NEIGHBORHOOD_SIZE,
                pixels,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::overlay::session::{OverlaySession, OverlaySurface};

    fn session_with_pixels() -> OverlaySession {
        let mut session = OverlaySession::default();
        session.set_viewport_size(100.0, 100.0);
        session.prepare_surface(OverlaySurface {
            background_pixels: Some(std::sync::Arc::new(image::RgbaImage::from_pixel(
                3,
                3,
                image::Rgba([0x10, 0x20, 0x30, 0xff]),
            ))),
            ..OverlaySurface::default()
        });
        session
    }

    #[test]
    fn picker_move_clamps_within_image_bounds() {
        let mut session = session_with_pixels();
        assert!(session.update_pointer(Point::new(gpui::px(10.0), gpui::px(10.0))));
        assert!(session.move_picker_by_pixel(50, 50));

        let sample = session.picker_sample.as_ref().unwrap();
        assert_eq!((sample.x, sample.y), (2, 2));
    }

    #[test]
    fn picker_clear_hides_samples_when_selection_exists() {
        let mut session = session_with_pixels();
        session.update_pointer(Point::new(gpui::px(10.0), gpui::px(10.0)));
        assert!(session.picker_sample.is_some());

        session.viewport.selection = Some(crate::core::geometry::RectF::new(0.0, 0.0, 10.0, 10.0));
        session.refresh_picker_sample();

        assert!(session.picker_sample.is_none());
        assert!(session.picker_neighborhood.is_none());
    }
}
