#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct WidgetState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub focus_visible: bool,
    pub disabled: bool,
    pub selected: bool,
    pub checked: bool,
    pub open: bool,
    pub invalid: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StateValue<T> {
    pub normal: T,
    pub hovered: T,
    pub pressed: T,
    pub disabled: T,
    pub focused: Option<T>,
    pub focus_visible: Option<T>,
    pub selected: Option<T>,
    pub checked: Option<T>,
    pub open: Option<T>,
    pub invalid: Option<T>,
}

impl<T: Clone> StateValue<T> {
    pub fn new(normal: T) -> Self {
        Self {
            normal: normal.clone(),
            hovered: normal.clone(),
            pressed: normal.clone(),
            disabled: normal,
            focused: None,
            focus_visible: None,
            selected: None,
            checked: None,
            open: None,
            invalid: None,
        }
    }

    pub fn interactive(normal: T, hovered: T, pressed: T, disabled: T) -> Self {
        Self {
            normal,
            hovered,
            pressed,
            disabled,
            focused: None,
            focus_visible: None,
            selected: None,
            checked: None,
            open: None,
            invalid: None,
        }
    }

    pub fn resolve(&self, state: WidgetState) -> T {
        if state.disabled {
            return self.disabled.clone();
        }
        if state.invalid {
            if let Some(invalid) = &self.invalid {
                return invalid.clone();
            }
        }
        if state.pressed {
            return self.pressed.clone();
        }
        if state.hovered {
            return self.hovered.clone();
        }
        if state.focus_visible {
            if let Some(focus_visible) = &self.focus_visible {
                return focus_visible.clone();
            }
        }
        if state.focused {
            if let Some(focused) = &self.focused {
                return focused.clone();
            }
        }
        if state.selected {
            if let Some(selected) = &self.selected {
                return selected.clone();
            }
        }
        if state.checked {
            if let Some(checked) = &self.checked {
                return checked.clone();
            }
        }
        if state.open {
            if let Some(open) = &self.open {
                return open.clone();
            }
        }
        self.normal.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{StateValue, WidgetState};

    #[test]
    fn state_value_resolves_only_active_states_in_priority_order() {
        let mut value = StateValue::interactive("normal", "hovered", "pressed", "disabled");
        value.focused = Some("focused");
        value.focus_visible = Some("focus_visible");
        value.selected = Some("selected");
        value.checked = Some("checked");
        value.open = Some("open");
        value.invalid = Some("invalid");

        assert_eq!(value.resolve(WidgetState::default()), "normal");
        assert_eq!(
            value.resolve(WidgetState {
                checked: true,
                ..Default::default()
            }),
            "checked"
        );
        assert_eq!(
            value.resolve(WidgetState {
                open: true,
                ..Default::default()
            }),
            "open"
        );
        assert_eq!(
            value.resolve(WidgetState {
                selected: true,
                checked: true,
                open: true,
                ..Default::default()
            }),
            "selected"
        );
        assert_eq!(
            value.resolve(WidgetState {
                focused: true,
                selected: true,
                checked: true,
                open: true,
                ..Default::default()
            }),
            "focused"
        );
        assert_eq!(
            value.resolve(WidgetState {
                focused: true,
                focus_visible: true,
                selected: true,
                ..Default::default()
            }),
            "focus_visible"
        );
        assert_eq!(
            value.resolve(WidgetState {
                hovered: true,
                focus_visible: true,
                ..Default::default()
            }),
            "hovered"
        );
        assert_eq!(
            value.resolve(WidgetState {
                pressed: true,
                hovered: true,
                ..Default::default()
            }),
            "pressed"
        );
        assert_eq!(
            value.resolve(WidgetState {
                invalid: true,
                pressed: true,
                ..Default::default()
            }),
            "invalid"
        );
        assert_eq!(
            value.resolve(WidgetState {
                disabled: true,
                invalid: true,
                pressed: true,
                ..Default::default()
            }),
            "disabled"
        );
    }

    #[test]
    fn state_value_ignores_inactive_optional_states() {
        let mut value = StateValue::interactive("normal", "hovered", "pressed", "disabled");
        value.invalid = Some("invalid");
        value.focus_visible = Some("focus_visible");
        value.selected = Some("selected");

        assert_eq!(value.resolve(WidgetState::default()), "normal");
        assert_eq!(
            value.resolve(WidgetState {
                hovered: true,
                ..Default::default()
            }),
            "hovered"
        );
    }
}
