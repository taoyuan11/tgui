use super::{
    BorderScale, ColorScheme, ComponentThemes, Density, ElevationScale, FocusRingStyle,
    MotionScale, RadiusScale, ResolvedThemeMode, SpaceScale, Theme, TypeScale,
};
use crate::foundation::color::Color;

#[derive(Clone, Debug)]
pub struct ThemeBuilder {
    name: String,
    colors: ColorScheme,
    typography: TypeScale,
    spacing: SpaceScale,
    radius: RadiusScale,
    border: BorderScale,
    focus_ring: Option<FocusRingStyle>,
    elevation: ElevationScale,
    motion: MotionScale,
    density: Density,
    components: ComponentThemes,
    mode: ResolvedThemeMode,
}

impl ThemeBuilder {
    pub fn light(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            colors: ColorScheme::light(),
            typography: TypeScale::default(),
            spacing: SpaceScale::default(),
            radius: RadiusScale::default(),
            border: BorderScale::default(),
            focus_ring: None,
            elevation: ElevationScale::light(),
            motion: MotionScale::default(),
            density: Density::default(),
            components: ComponentThemes::default(),
            mode: ResolvedThemeMode::Light,
        }
    }

    pub fn dark(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            colors: ColorScheme::dark(),
            typography: TypeScale::default(),
            spacing: SpaceScale::default(),
            radius: RadiusScale::default(),
            border: BorderScale::default(),
            focus_ring: None,
            elevation: ElevationScale::dark(),
            motion: MotionScale::default(),
            density: Density::default(),
            components: ComponentThemes::default(),
            mode: ResolvedThemeMode::Dark,
        }
    }

    pub fn primary(mut self, color: Color) -> Self {
        self.colors.primary = color;
        self
    }

    pub fn colors(mut self, colors: ColorScheme) -> Self {
        self.colors = colors;
        self
    }

    pub fn typography(mut self, typography: TypeScale) -> Self {
        self.typography = typography;
        self
    }

    pub fn spacing(mut self, spacing: SpaceScale) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn radius(mut self, radius: RadiusScale) -> Self {
        self.radius = radius;
        self
    }

    pub fn border(mut self, border: BorderScale) -> Self {
        self.border = border;
        self
    }

    pub fn focus_ring(mut self, focus_ring: FocusRingStyle) -> Self {
        self.focus_ring = Some(focus_ring);
        self
    }

    pub fn elevation(mut self, elevation: ElevationScale) -> Self {
        self.elevation = elevation;
        self
    }

    pub fn motion(mut self, motion: MotionScale) -> Self {
        self.motion = motion;
        self
    }

    pub fn density(mut self, density: Density) -> Self {
        self.density = density;
        self
    }

    pub fn components(mut self, components: ComponentThemes) -> Self {
        self.components = components;
        self
    }

    pub fn build(self) -> Theme {
        let mut theme = Theme::new(self.name, self.colors);
        theme.mode = self.mode;
        theme.typography = self.typography;
        theme.spacing = self.spacing;
        theme.radius = self.radius;
        theme.border = self.border;
        if let Some(focus_ring) = self.focus_ring {
            theme.focus_ring = focus_ring;
        }
        theme.elevation = self.elevation;
        theme.density = self.density;
        theme.components = self.components;
        theme.motion = self.motion;
        theme
    }
}
