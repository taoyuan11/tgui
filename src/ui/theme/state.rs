#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct WidgetState {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub disabled: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stateful<T> {
    pub normal: T,
    pub hovered: T,
    pub pressed: T,
    pub focused: T,
    pub disabled: T,
}

impl<T: Clone> Stateful<T> {
    pub fn resolve(&self, state: WidgetState) -> T {
        if state.disabled {
            return self.disabled.clone();
        }
        if state.pressed {
            return self.pressed.clone();
        }
        if state.hovered {
            return self.hovered.clone();
        }
        if state.focused {
            return self.focused.clone();
        }
        self.normal.clone()
    }
}
