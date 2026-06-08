use super::color::ColorScheme;
use super::components::ComponentThemes;
use super::context::Density;
use super::motion::MotionScale;
use super::resolved_mode::ResolvedThemeMode;
use super::shape::{BorderScale, ElevationScale, RadiusScale};
use super::spacing::SpaceScale;
use super::typography::TypeScale;
use crate::foundation::color::Color;
use crate::ui::unit::Dp;

#[derive(Clone, Debug, PartialEq)]
pub struct FocusRingStyle {
    pub enabled: bool,
    pub color: Color,
    pub width: Dp,
    pub gap: Dp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub colors: ColorScheme,
    pub typography: TypeScale,
    pub spacing: SpaceScale,
    pub radius: RadiusScale,
    pub border: BorderScale,
    pub focus_ring: FocusRingStyle,
    pub elevation: ElevationScale,
    pub motion: MotionScale,
    pub density: Density,
    pub components: ComponentThemes,
    pub mode: ResolvedThemeMode,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    pub fn light() -> Self {
        super::ThemeBuilder::light("light").build()
    }

    pub fn dark() -> Self {
        super::ThemeBuilder::dark("dark").build()
    }

    pub fn builder(name: impl Into<String>) -> super::ThemeBuilder {
        super::ThemeBuilder::light(name)
    }

    pub(crate) fn new(name: impl Into<String>, colors: ColorScheme) -> Self {
        let typography = TypeScale::default();
        let spacing = SpaceScale::default();
        let radius = RadiusScale::default();
        let border = BorderScale::default();
        let focus_ring = FocusRingStyle {
            enabled: true,
            color: colors.focus_ring,
            width: border.normal,
            gap: spacing.xxs,
        };
        let elevation = ElevationScale::default();
        let motion = MotionScale::default();
        let density = Density::default();
        let components = ComponentThemes::default();
        let mode = ResolvedThemeMode::Dark;
        Self {
            name: name.into(),
            colors,
            typography,
            spacing,
            radius,
            border,
            focus_ring,
            elevation,
            motion,
            density,
            components,
            mode,
        }
    }
}
