use crate::application::ThemeSelection;
use crate::platform::backend::window::Window;
use crate::platform::window::Theme as WindowTheme;
use crate::ui::theme::{Theme, ThemeSet};

pub(super) fn resolve_theme(
    selection: &ThemeSelection,
    theme_set: &ThemeSet,
    window_theme: Option<WindowTheme>,
) -> Theme {
    match selection {
        ThemeSelection::System => theme_set
            .resolve_window_theme(window_theme)
            .as_ref()
            .clone(),
        ThemeSelection::Mode(mode) => theme_set.resolve(*mode, window_theme).as_ref().clone(),
    }
}

pub(super) fn resolve_window_theme(window: Option<&dyn Window>) -> Option<WindowTheme> {
    window.and_then(|window| window.theme())
}
