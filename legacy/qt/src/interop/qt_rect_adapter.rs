use crate::core::geometry::Rect;
use cxx_qt_lib::QRectF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRect(Rect);

impl SelectionRect {
    #[must_use]
    pub fn from_qrect(rect: &QRectF) -> Self {
        Self(crate::core::geometry::normalize_rect(rect.x(), rect.y(), rect.width(), rect.height()))
    }

    #[must_use]
    pub const fn rect(self) -> Rect {
        self.0
    }

    #[must_use]
    pub fn to_qrect(self) -> QRectF {
        rect_to_qrect(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRequestRect(Rect);

impl CaptureRequestRect {
    #[must_use]
    pub fn from_qrect(rect: &QRectF) -> Self {
        let x = rect.x().floor() as i32;
        let y = rect.y().floor() as i32;
        let width = rect.width().ceil().max(0.0) as i32;
        let height = rect.height().ceil().max(0.0) as i32;
        Self(Rect::new(x, y, width, height))
    }

    #[must_use]
    pub const fn rect(self) -> Rect {
        self.0
    }
}

#[must_use]
pub fn rect_to_qrect(rect: Rect) -> QRectF {
    QRectF::new(f64::from(rect.x), f64::from(rect.y), f64::from(rect.width), f64::from(rect.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_empty_keeps_zero_sized_rect() {
        let qrect = QRectF::new(0.0, 0.0, 0.0, 0.0);
        let rect = CaptureRequestRect::from_qrect(&qrect).rect();
        assert_eq!(rect, Rect::new(0, 0, 0, 0));
    }

    #[test]
    fn normalize_qrect_enforces_minimum_size() {
        let qrect = QRectF::new(10.4, 20.6, 0.2, 0.3);
        let rect = SelectionRect::from_qrect(&qrect).rect();
        assert_eq!(rect, Rect::new(10, 20, 1, 1));
    }
}
