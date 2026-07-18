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
        // Focus and selection are the low-emphasis expressions of the accent color. Keeping the
        // baked blue values after a caller changes `primary` makes an otherwise custom theme look
        // inconsistent. Explicit `focus_ring(...)` still wins during `build`.
        let (focus_alpha, selection_alpha) = match self.mode {
            ResolvedThemeMode::Light => (0.75, 0.18),
            ResolvedThemeMode::Dark => (0.65, 0.24),
        };
        self.colors.focus_ring = color.with_alpha_factor(focus_alpha);
        self.colors.selection = color.with_alpha_factor(selection_alpha);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::unit::dp;

    #[test]
    fn custom_primary_keeps_focus_and_selection_accents_coherent() {
        let accent = Color::hexa(0x7C3AEDFF);
        let light = ThemeBuilder::light("custom-light").primary(accent).build();
        assert_eq!(light.colors.primary, accent);
        assert_eq!(light.colors.focus_ring, accent.with_alpha_factor(0.75));
        assert_eq!(light.colors.selection, accent.with_alpha_factor(0.18));
        assert_eq!(light.focus_ring.color, light.colors.focus_ring);

        let dark = ThemeBuilder::dark("custom-dark").primary(accent).build();
        assert_eq!(dark.colors.primary, accent);
        assert_eq!(dark.colors.focus_ring, accent.with_alpha_factor(0.65));
        assert_eq!(dark.colors.selection, accent.with_alpha_factor(0.24));
    }

    #[test]
    fn explicit_focus_ring_still_overrides_derived_primary_accent() {
        let explicit = FocusRingStyle {
            enabled: true,
            color: Color::hexa(0x22C55EFF),
            width: dp(3.0),
            gap: dp(1.0),
        };
        let theme = ThemeBuilder::light("custom")
            .focus_ring(explicit.clone())
            .primary(Color::hexa(0x7C3AEDFF))
            .build();
        assert_eq!(theme.focus_ring, explicit);
    }
}
