use super::color::ColorScheme;
use super::component::ComponentTheme;
use super::motion::MotionScale;
use super::shape::{BorderScale, ElevationScale, RadiusScale};
use super::spacing::SpaceScale;
use super::typography::TypeScale;

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub colors: ColorScheme,
    pub typography: TypeScale,
    pub spacing: SpaceScale,
    pub radius: RadiusScale,
    pub border: BorderScale,
    pub elevation: ElevationScale,
    pub motion: MotionScale,
    pub components: ComponentTheme,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    pub fn light() -> Self {
        Self::new("light", ColorScheme::light())
    }

    pub fn dark() -> Self {
        Self::new("dark", ColorScheme::dark())
    }

    /// Rebuilds component styles from the current theme tokens.
    ///
    /// Call this after mutating `colors`, `typography`, `spacing`, `radius`,
    /// `border`, `elevation`, or `motion` directly so derived component styles
    /// stay in sync with the updated token values.
    pub fn refresh_components(&mut self) {
        self.components = ComponentTheme::from_tokens(
            &self.colors,
            &self.typography,
            &self.spacing,
            &self.radius,
            &self.border,
            &self.elevation,
            &self.motion,
        );
    }

    pub(crate) fn new(name: impl Into<String>, colors: ColorScheme) -> Self {
        let typography = TypeScale::default();
        let spacing = SpaceScale::default();
        let radius = RadiusScale::default();
        let border = BorderScale::default();
        let elevation = ElevationScale::default();
        let motion = MotionScale::default();
        let components = ComponentTheme::from_tokens(
            &colors,
            &typography,
            &spacing,
            &radius,
            &border,
            &elevation,
            &motion,
        );
        Self {
            name: name.into(),
            colors,
            typography,
            spacing,
            radius,
            border,
            elevation,
            motion,
            components,
        }
    }
}
