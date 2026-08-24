use super::{ResolvedThemeMode, Theme};
use crate::animation::Transition;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Density {
    /// The default density for built-in themes and components.
    #[default]
    Compact,
    Comfortable,
    Spacious,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ControlSize {
    Small,
    #[default]
    Medium,
    Large,
}

#[derive(Clone, Copy)]
pub struct StyleContext<'a> {
    pub theme: &'a Theme,
    pub mode: ResolvedThemeMode,
    pub density: Density,
    pub reduced_motion: bool,
    pub text_scale: f32,
}

impl<'a> StyleContext<'a> {
    pub fn new(theme: &'a Theme, mode: ResolvedThemeMode) -> Self {
        Self {
            theme,
            mode,
            density: theme.density,
            reduced_motion: false,
            text_scale: 1.0,
        }
    }

    pub fn from_theme(theme: &'a Theme) -> Self {
        Self::new(theme, theme.mode)
    }

    pub fn with_reduced_motion(mut self, reduced_motion: bool) -> Self {
        self.reduced_motion = reduced_motion;
        self
    }

    pub fn with_text_scale(mut self, text_scale: f32) -> Self {
        self.text_scale = text_scale.max(0.1);
        self
    }

    pub fn with_density(mut self, density: Density) -> Self {
        self.density = density;
        self
    }

    /// A short, decelerating transition for state changes and small overlays.
    ///
    /// Component code resolves motion through the live style context instead of
    /// baking durations while the widget tree is built. Reduced motion and a
    /// zero-duration theme both intentionally return `None`: feeding that into
    /// the animation engine lands on the target immediately and leaves no
    /// animation deadline behind.
    pub(crate) fn motion_fast_transition(self) -> Option<Transition> {
        self.motion_transition(self.theme.motion.fast_ms)
    }

    /// The default transition for compact component entrances and disclosure.
    pub(crate) fn motion_normal_transition(self) -> Option<Transition> {
        self.motion_transition(self.theme.motion.normal_ms)
    }

    /// A restrained transition for large spatial movement such as drawers.
    pub(crate) fn motion_slow_transition(self) -> Option<Transition> {
        self.motion_transition(self.theme.motion.slow_ms)
    }

    #[inline]
    fn motion_transition(self, duration_ms: u64) -> Option<Transition> {
        (!self.reduced_motion && duration_ms > 0)
            .then(|| Transition::ease_out(Duration::from_millis(duration_ms)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::AnimationCurve;

    #[test]
    fn motion_transitions_follow_live_theme_tokens() {
        let mut theme = Theme::light();
        theme.motion.fast_ms = 73;
        theme.motion.normal_ms = 149;
        theme.motion.slow_ms = 271;
        let context = StyleContext::from_theme(&theme);

        for (transition, expected_ms) in [
            (context.motion_fast_transition(), 73),
            (context.motion_normal_transition(), 149),
            (context.motion_slow_transition(), 271),
        ] {
            let transition = transition.expect("motion should be enabled");
            assert_eq!(transition.duration(), Duration::from_millis(expected_ms));
            assert_eq!(transition.curve_mode(), AnimationCurve::EaseOutCubic);
        }
    }

    #[test]
    fn reduced_motion_and_zero_duration_have_no_transition() {
        let mut theme = Theme::light();
        theme.motion.fast_ms = 0;
        assert!(StyleContext::from_theme(&theme)
            .motion_fast_transition()
            .is_none());
        assert!(StyleContext::from_theme(&theme)
            .with_reduced_motion(true)
            .motion_slow_transition()
            .is_none());
    }
}
