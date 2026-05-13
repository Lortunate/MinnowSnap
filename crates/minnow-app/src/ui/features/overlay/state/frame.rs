use super::{HudVm, OverlayFrame, OverlaySession, PickerVm, SelectionVm};

impl OverlaySession {
    pub(super) fn frame(&mut self) -> OverlayFrame {
        OverlayFrame {
            background_image: self.background_image.clone(),
            selection: SelectionVm {
                selection: self.viewport.selection,
                target: self.viewport.target,
                drag_mode: self.viewport.mode,
            },
            selection_move_delta: self.viewport.selection_move_delta,
            picker: self.picker_visible().then(|| PickerVm {
                cursor: self.picker_cursor,
                sample: self.picker_sample.clone(),
                neighborhood: self.picker_neighborhood.clone(),
                format: self.picker_format,
            }),
            annotation: self.annotation_ui_state(),
            hud: HudVm {
                hovered_window: self.hovered_window.clone(),
            },
            #[cfg(feature = "overlay-diagnostics")]
            diagnostics: self.diagnostics_snapshot(),
        }
    }
}
