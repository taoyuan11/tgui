mod builder;
mod color;
mod components;
mod context;
mod mode;
mod motion;
mod resolved_mode;
mod shape;
mod spacing;
mod state;
mod store;
mod theme;
mod typography;

pub use builder::ThemeBuilder;
pub use color::ColorScheme;
pub use components::{ComponentStyle, ComponentThemes};
pub use context::{ControlSize, Density, StyleContext};
pub use mode::ThemeMode;
pub use motion::MotionScale;
pub use resolved_mode::ResolvedThemeMode;
pub use shape::{BorderScale, ElevationScale, RadiusScale, Shadow};
pub use spacing::SpaceScale;
pub use state::{StateValue, WidgetState};
pub use store::{ThemeSet, ThemeStore};
pub use theme::{FocusRingStyle, Theme};
pub use typography::{FontWeight, TextStyle, TypeScale};

use crate::platform::window::Theme as WindowTheme;

impl Theme {
    pub fn from_mode(mode: ThemeMode, system_theme: Option<WindowTheme>) -> Self {
        ThemeSet::default()
            .resolve(mode, system_theme)
            .as_ref()
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{Theme, ThemeMode, ThemeSet, ThemeStore};
    use crate::platform::window::Theme as WindowTheme;

    #[test]
    fn theme_store_increments_version_when_resolution_changes() {
        let mut store = ThemeStore::new(ThemeSet::default(), ThemeMode::Light, None);
        assert_eq!(store.version(), 0);
        assert!(store.set_mode(ThemeMode::Dark));
        assert_eq!(store.version(), 1);
    }

    #[test]
    fn system_mode_defaults_to_dark() {
        let themes = ThemeSet::new(Theme::light(), Theme::dark());
        assert_eq!(
            themes
                .resolve(ThemeMode::System, Some(WindowTheme::Light))
                .name,
            "light"
        );
        assert_eq!(themes.resolve(ThemeMode::System, None).name, "dark");
    }
}
