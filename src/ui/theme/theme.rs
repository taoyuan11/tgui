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
