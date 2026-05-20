use super::super::PreferencesPage;
use gpui::SharedString;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SelectOption {
    pub(crate) value: SharedString,
    pub(crate) label: SharedString,
}

impl SelectOption {
    pub(crate) fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }

    pub(crate) fn label_for(current: &str, options: &[Self]) -> SharedString {
        options
            .iter()
            .find(|option| option.value.as_ref() == current)
            .map(|option| option.label.clone())
            .unwrap_or_else(|| SharedString::from(current.to_owned()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SidebarItemProps {
    pub(crate) page: PreferencesPage,
    pub(crate) title: SharedString,
    pub(crate) is_active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ToggleRowProps {
    pub(crate) id: &'static str,
    pub(crate) title: SharedString,
    pub(crate) description: SharedString,
    pub(crate) checked: bool,
    pub(crate) disabled: bool,
}

impl ToggleRowProps {
    pub(super) fn new(id: &'static str, title: impl Into<SharedString>, description: impl Into<SharedString>, checked: bool) -> Self {
        Self {
            id,
            title: title.into(),
            description: description.into(),
            checked,
            disabled: false,
        }
    }

    pub(super) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SelectRowProps {
    pub(crate) id: &'static str,
    pub(crate) title: SharedString,
    pub(crate) description: SharedString,
    pub(crate) current_value: SharedString,
    pub(crate) disabled: bool,
    pub(crate) options: Vec<SelectOption>,
}

impl SelectRowProps {
    pub(super) fn new(
        id: &'static str,
        title: impl Into<SharedString>,
        description: impl Into<SharedString>,
        current_value: impl Into<SharedString>,
        options: Vec<SelectOption>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            description: description.into(),
            current_value: current_value.into(),
            disabled: false,
            options,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActionRowProps {
    pub(crate) id: &'static str,
    pub(crate) title: SharedString,
    pub(crate) description: SharedString,
    pub(crate) button_label: SharedString,
    pub(crate) disabled: bool,
}

impl ActionRowProps {
    pub(super) fn new(
        id: &'static str,
        title: impl Into<SharedString>,
        description: impl Into<SharedString>,
        button_label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            description: description.into(),
            button_label: button_label.into(),
            disabled: false,
        }
    }

    pub(crate) fn button(&self) -> ButtonProps {
        ButtonProps::new(self.id, self.button_label.clone()).disabled(self.disabled)
    }

    pub(super) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ButtonProps {
    pub(crate) id: &'static str,
    pub(crate) label: SharedString,
    pub(crate) disabled: bool,
}

impl ButtonProps {
    pub(crate) fn new(id: &'static str, label: impl Into<SharedString>) -> Self {
        Self {
            id,
            label: label.into(),
            disabled: false,
        }
    }

    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_option_label_uses_matching_label() {
        let options = vec![SelectOption::new("system", "Follow System"), SelectOption::new("dark", "Dark")];

        assert_eq!(SelectOption::label_for("dark", &options), SharedString::from("Dark"));
        assert_eq!(SelectOption::label_for("missing", &options), SharedString::from("missing"));
    }
}
