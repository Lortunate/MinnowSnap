use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use std::pin::Pin;

struct AnnotationState {
    id: i32,
}

const LINE_WIDTH_MIN: i32 = 1;
const LINE_WIDTH_MAX: i32 = 50;
const INTENSITY_MIN: i32 = 2;
const INTENSITY_MAX: i32 = 64;
const COUNTER_SIZE_MIN: i32 = 16;
const COUNTER_SIZE_MAX: i32 = 64;
const FONT_SIZE_MIN: i32 = 12;
const FONT_SIZE_MAX: i32 = 96;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, active_tool)]
        #[qproperty(QString, active_color)]
        #[qproperty(bool, active_has_outline)]
        #[qproperty(bool, active_has_stroke)]
        #[qproperty(i32, active_line_width)]
        #[qproperty(i32, active_intensity)]
        #[qproperty(QString, active_mosaic_type)]
        #[qproperty(i32, active_counter_size)]
        #[qproperty(i32, active_font_size)]
        #[qproperty(i32, next_counter_value)]
        #[qproperty(bool, has_annotations)]
        #[qproperty(bool, has_selected_annotation)]
        #[qproperty(QString, selected_annotation_type)]
        type AnnotationController = super::AnnotationControllerRust;

        #[qinvokable]
        fn initialize_defaults(self: Pin<&mut Self>, active_color: QString);

        #[qinvokable]
        fn update_active_tool(self: Pin<&mut Self>, tool: QString);

        #[qinvokable]
        fn update_active_color(self: Pin<&mut Self>, color: QString);

        #[qinvokable]
        fn update_active_has_outline(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn update_active_has_stroke(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn update_active_line_width(self: Pin<&mut Self>, value: i32);

        #[qinvokable]
        fn update_active_intensity(self: Pin<&mut Self>, value: i32);

        #[qinvokable]
        fn update_active_mosaic_type(self: Pin<&mut Self>, value: QString);

        #[qinvokable]
        fn update_active_counter_size(self: Pin<&mut Self>, value: i32);

        #[qinvokable]
        fn update_active_font_size(self: Pin<&mut Self>, value: i32);

        #[qinvokable]
        fn begin_create_annotation(self: Pin<&mut Self>) -> i32;

        #[qinvokable]
        fn register_created_annotation(self: Pin<&mut Self>, id: i32, kind: QString);

        #[qinvokable]
        fn cancel_created_annotation(self: Pin<&mut Self>, id: i32);

        #[qinvokable]
        fn on_annotation_selected(
            self: Pin<&mut Self>,
            id: i32,
            kind: QString,
            color: QString,
            has_outline: bool,
            has_stroke: bool,
            line_width: i32,
            intensity: i32,
            mosaic_type: QString,
            counter_size: i32,
            font_size: i32,
            deactivate_tool: bool,
        );

        #[qinvokable]
        fn remove_annotation(self: Pin<&mut Self>, id: i32);

        #[qinvokable]
        fn clear_all(self: Pin<&mut Self>);

        #[qinvokable]
        fn undo(self: Pin<&mut Self>);

        #[qinvokable]
        fn redo(self: Pin<&mut Self>);

        #[qinvokable]
        fn clear_selection(self: Pin<&mut Self>);

        #[qsignal]
        fn request_set_tool(self: Pin<&mut Self>, tool: QString);

        #[qsignal]
        fn request_clear_selection(self: Pin<&mut Self>);

        #[qsignal]
        fn request_select_annotation(self: Pin<&mut Self>, id: i32);

        #[qsignal]
        fn request_bring_to_front(self: Pin<&mut Self>, id: i32);

        #[qsignal]
        fn request_remove_annotation(self: Pin<&mut Self>, id: i32);

        #[qsignal]
        fn request_set_annotation_visible(self: Pin<&mut Self>, id: i32, visible: bool);
    }
}

pub struct AnnotationControllerRust {
    active_tool: QString,
    active_color: QString,
    active_has_outline: bool,
    active_has_stroke: bool,
    active_line_width: i32,
    active_intensity: i32,
    active_mosaic_type: QString,
    active_counter_size: i32,
    active_font_size: i32,
    next_counter_value: i32,
    has_annotations: bool,
    has_selected_annotation: bool,
    selected_annotation_type: QString,
    annotations: Vec<AnnotationState>,
    selected_id: Option<i32>,
    next_id: i32,
    visible_len: usize,
}

impl Default for AnnotationControllerRust {
    fn default() -> Self {
        Self {
            active_tool: QString::from(""),
            active_color: QString::from("#FF3B30"),
            active_has_outline: true,
            active_has_stroke: false,
            active_line_width: 4,
            active_intensity: 10,
            active_mosaic_type: QString::from("mosaic"),
            active_counter_size: 32,
            active_font_size: 24,
            next_counter_value: 1,
            has_annotations: false,
            has_selected_annotation: false,
            selected_annotation_type: QString::from(""),
            annotations: Vec::new(),
            selected_id: None,
            next_id: 1,
            visible_len: 0,
        }
    }
}

macro_rules! forward_update {
    ($fn_name:ident, $setter:ident, $ty:ty) => {
        pub fn $fn_name(mut self: Pin<&mut Self>, value: $ty) {
            self.as_mut().$setter(value);
        }
    };
}

macro_rules! clamped_update {
    ($fn_name:ident, $setter:ident, $min:expr, $max:expr) => {
        pub fn $fn_name(mut self: Pin<&mut Self>, value: i32) {
            self.as_mut().$setter(value.clamp($min, $max));
        }
    };
}

impl qobject::AnnotationController {
    fn set_selected_annotation(mut self: Pin<&mut Self>, id: i32, kind: QString) {
        self.as_mut().rust_mut().selected_id = Some(id);
        self.as_mut().set_has_selected_annotation(true);
        self.as_mut().set_selected_annotation_type(kind);
    }

    fn clear_selected_annotation(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().selected_id = None;
        self.as_mut().set_has_selected_annotation(false);
        self.as_mut().set_selected_annotation_type(QString::from(""));
    }

    pub fn initialize_defaults(mut self: Pin<&mut Self>, active_color: QString) {
        if !active_color.is_empty() {
            self.as_mut().set_active_color(active_color);
        }
    }

    pub fn update_active_tool(mut self: Pin<&mut Self>, tool: QString) {
        self.as_mut().set_active_tool(tool.clone());
        let tool_name = tool.to_string();
        if !tool_name.is_empty() {
            self.as_mut().clear_selection();
            if tool_name == "counter" || tool_name == "arrow" {
                self.as_mut().set_active_has_outline(false);
            }
        }
    }

    forward_update!(update_active_color, set_active_color, QString);
    forward_update!(update_active_has_outline, set_active_has_outline, bool);
    forward_update!(update_active_has_stroke, set_active_has_stroke, bool);
    forward_update!(update_active_mosaic_type, set_active_mosaic_type, QString);
    clamped_update!(update_active_line_width, set_active_line_width, LINE_WIDTH_MIN, LINE_WIDTH_MAX);
    clamped_update!(update_active_intensity, set_active_intensity, INTENSITY_MIN, INTENSITY_MAX);
    clamped_update!(update_active_counter_size, set_active_counter_size, COUNTER_SIZE_MIN, COUNTER_SIZE_MAX);
    clamped_update!(update_active_font_size, set_active_font_size, FONT_SIZE_MIN, FONT_SIZE_MAX);

    fn set_visible_len(mut self: Pin<&mut Self>, visible_len: usize) {
        self.as_mut().rust_mut().visible_len = visible_len;
        self.as_mut().set_has_annotations(visible_len > 0);
    }

    fn trim_redo_history(mut self: Pin<&mut Self>) -> Vec<i32> {
        let removed_ids = {
            let rust = self.rust();
            rust.annotations[rust.visible_len..].iter().map(|item| item.id).collect::<Vec<_>>()
        };

        if !removed_ids.is_empty() {
            let visible_len = self.rust().visible_len;
            self.as_mut().rust_mut().annotations.truncate(visible_len);
        }

        removed_ids
    }

    fn select_with_kind(mut self: Pin<&mut Self>, id: i32, kind: &str) {
        self.as_mut().set_selected_annotation(id, QString::from(kind));
        self.as_mut().request_select_annotation(id);
    }

    fn apply_active_values(
        mut self: Pin<&mut Self>,
        color: QString,
        has_outline: bool,
        has_stroke: bool,
        line_width: i32,
        intensity: i32,
        mosaic_type: QString,
        counter_size: i32,
        font_size: i32,
    ) {
        self.as_mut().set_active_color(color);
        self.as_mut().set_active_has_outline(has_outline);
        self.as_mut().set_active_has_stroke(has_stroke);
        self.as_mut().set_active_line_width(line_width.clamp(LINE_WIDTH_MIN, LINE_WIDTH_MAX));
        self.as_mut().set_active_intensity(intensity.clamp(INTENSITY_MIN, INTENSITY_MAX));
        self.as_mut().set_active_mosaic_type(mosaic_type);
        self.as_mut()
            .set_active_counter_size(counter_size.clamp(COUNTER_SIZE_MIN, COUNTER_SIZE_MAX));
        self.as_mut().set_active_font_size(font_size.clamp(FONT_SIZE_MIN, FONT_SIZE_MAX));
    }

    fn deactivate_tool(mut self: Pin<&mut Self>) {
        if self.active_tool().is_empty() {
            return;
        }
        self.as_mut().set_active_tool(QString::from(""));
        self.as_mut().request_set_tool(QString::from(""));
    }

    pub fn begin_create_annotation(mut self: Pin<&mut Self>) -> i32 {
        self.as_mut().clear_selection();
        let removed_ids = self.as_mut().trim_redo_history();

        for id in removed_ids {
            self.as_mut().request_remove_annotation(id);
        }

        let id = self.rust().next_id;
        self.as_mut().rust_mut().next_id += 1;

        id
    }

    pub fn register_created_annotation(mut self: Pin<&mut Self>, id: i32, kind: QString) {
        self.as_mut().rust_mut().annotations.push(AnnotationState { id });
        let visible_len = self.rust().annotations.len();
        self.as_mut().set_visible_len(visible_len);
        self.as_mut().request_bring_to_front(id);

        let kind_str = kind.to_string();
        if kind_str == "counter" {
            self.as_mut().select_with_kind(id, "counter");
            let next_counter_value = *self.next_counter_value() + 1;
            self.as_mut().set_next_counter_value(next_counter_value);
        } else if kind_str == "text" {
            self.as_mut().select_with_kind(id, "text");
        }
    }

    pub fn cancel_created_annotation(mut self: Pin<&mut Self>, id: i32) {
        self.as_mut().remove_annotation(id);
    }

    pub fn on_annotation_selected(
        mut self: Pin<&mut Self>,
        id: i32,
        kind: QString,
        color: QString,
        has_outline: bool,
        has_stroke: bool,
        line_width: i32,
        intensity: i32,
        mosaic_type: QString,
        counter_size: i32,
        font_size: i32,
        deactivate_tool: bool,
    ) {
        if deactivate_tool {
            self.as_mut().deactivate_tool();
        }

        self.as_mut().request_clear_selection();
        self.as_mut().set_selected_annotation(id, kind);
        self.as_mut().apply_active_values(
            color,
            has_outline,
            has_stroke,
            line_width,
            intensity,
            mosaic_type,
            counter_size,
            font_size,
        );
        self.as_mut().request_select_annotation(id);
        self.as_mut().request_bring_to_front(id);
    }

    pub fn remove_annotation(mut self: Pin<&mut Self>, id: i32) {
        if let Some(index) = self.rust().annotations.iter().position(|item| item.id == id) {
            let selected = self.rust().selected_id == Some(id);
            let visible_len = self.rust().visible_len;
            let new_visible_len = if index < visible_len {
                visible_len.saturating_sub(1)
            } else {
                visible_len
            };

            self.as_mut().rust_mut().annotations.remove(index);
            self.as_mut().set_visible_len(new_visible_len);

            if selected {
                self.as_mut().clear_selected_annotation();
            }
            self.as_mut().request_remove_annotation(id);
        }
    }

    pub fn clear_all(mut self: Pin<&mut Self>) {
        let mut ids: Vec<i32> = Vec::with_capacity(self.rust().annotations.len());
        for item in &self.rust().annotations {
            ids.push(item.id);
        }
        for id in ids {
            self.as_mut().request_remove_annotation(id);
        }

        self.as_mut().rust_mut().annotations.clear();
        self.as_mut().set_visible_len(0);
        self.as_mut().clear_selected_annotation();
        self.as_mut().set_next_counter_value(1);
    }

    pub fn undo(mut self: Pin<&mut Self>) {
        if self.rust().visible_len == 0 {
            return;
        }

        let target_index = self.rust().visible_len - 1;
        let target_id = self.rust().annotations[target_index].id;
        self.as_mut().set_visible_len(target_index);

        if self.rust().selected_id == Some(target_id) {
            self.as_mut().clear_selected_annotation();
        }

        self.as_mut().request_set_annotation_visible(target_id, false);
    }

    pub fn redo(mut self: Pin<&mut Self>) {
        let visible_len = self.rust().visible_len;
        if visible_len >= self.rust().annotations.len() {
            return;
        }

        let id = self.rust().annotations[visible_len].id;
        self.as_mut().set_visible_len(visible_len + 1);
        self.as_mut().request_set_annotation_visible(id, true);
    }

    pub fn clear_selection(mut self: Pin<&mut Self>) {
        self.as_mut().clear_selected_annotation();
        self.as_mut().request_clear_selection();
    }
}
