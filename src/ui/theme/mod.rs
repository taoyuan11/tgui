mod color;
mod component;
mod mode;
mod motion;
mod shape;
mod spacing;
mod state;
mod store;
mod theme;
mod typography;

pub use color::ColorScheme;
#[allow(unused_imports)]
pub use component::{
    ButtonStyle, ButtonTheme, ButtonVariant, ComponentTheme, DialogTheme, InputStyle, InputTheme,
    PanelTheme, ScrollbarTheme, SwitchStyle, SwitchTheme, TextTheme, TooltipTheme,
};
pub use mode::ThemeMode;
pub use motion::MotionScale;
pub use shape::{BorderScale, ElevationScale, RadiusScale, Shadow};
pub use spacing::SpaceScale;
pub use state::{Stateful, WidgetState};
pub use store::{ThemeSet, ThemeStore};
pub use theme::Theme;
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
    use super::{Theme, ThemeMode, ThemeSet, ThemeStore, WidgetState};
    use crate::platform::window::Theme as WindowTheme;

    #[test]
    fn default_theme_set_uses_builtin_light_and_dark_themes() {
        let themes = ThemeSet::default();
        assert_eq!(themes.light.as_ref(), &Theme::light());
        assert_eq!(themes.dark.as_ref(), &Theme::dark());
    }

    #[test]
    fn theme_set_resolves_explicit_light_and_dark_modes() {
        let themes = ThemeSet::new(Theme::light(), Theme::dark());
        assert_eq!(themes.resolve(ThemeMode::Light, None).name, "light");
        assert_eq!(themes.resolve(ThemeMode::Dark, None).name, "dark");
    }

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

    #[test]
    fn stateful_resolution_prefers_disabled_then_pressed_then_hovered_then_focused() {
        let resolved = Theme::dark()
            .components
            .button
            .primary
            .container
            .resolve(WidgetState {
                disabled: true,
                hovered: true,
                pressed: true,
                focused: true,
                selected: false,
            });
        assert_eq!(resolved, Theme::dark().colors.disabled);
    }

    #[test]
    fn refresh_components_rebuilds_button_tokens_after_color_mutation() {
        let mut theme = Theme::dark();
        theme.colors.primary = crate::foundation::color::Color::WHITE;
        assert_ne!(theme.components.button.primary.container.normal, theme.colors.primary);

        theme.refresh_components();

        assert_eq!(theme.components.button.primary.container.normal, theme.colors.primary);
    }
}
