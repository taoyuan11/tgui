use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::foundation::color::Color;
use crate::ui::widget::Point;

const DEFAULT_DURATION_MS: u64 = 180;
const THEME_DURATION_MS: u64 = 240;
pub(crate) const FRAME_INTERVAL: Duration = Duration::from_nanos(16_666_667);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    Linear,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
}

impl Easing {
    fn sample(self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        match self {
            Self::Linear => progress,
            Self::EaseInCubic => progress * progress * progress,
            Self::EaseOutCubic => 1.0 - (1.0 - progress).powi(3),
            Self::EaseInOutCubic => {
                if progress < 0.5 {
                    4.0 * progress * progress * progress
                } else {
                    1.0 - ((-2.0 * progress + 2.0).powi(3) / 2.0)
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    duration: Duration,
    easing: Easing,
}

impl Transition {
    pub fn linear(duration: Duration) -> Self {
        Self {
            duration,
            easing: Easing::Linear,
        }
    }

    pub fn ease_in(duration: Duration) -> Self {
        Self {
            duration,
            easing: Easing::EaseInCubic,
        }
    }

    pub fn ease_out(duration: Duration) -> Self {
        Self {
            duration,
            easing: Easing::EaseOutCubic,
        }
    }

    pub fn ease_in_out(duration: Duration) -> Self {
        Self {
            duration,
            easing: Easing::EaseInOutCubic,
        }
    }

    pub(crate) fn duration(self) -> Duration {
        self.duration
    }

    pub(crate) fn easing(self) -> Easing {
        self.easing
    }
}

impl Default for Transition {
    fn default() -> Self {
        Self::ease_out(Duration::from_millis(DEFAULT_DURATION_MS))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WidgetProperty {
    Background,
    TextColor,
    Opacity,
    Offset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WindowProperty {
    ClearColor,
    ThemeWindowBackground,
    ThemeSurface,
    ThemeSurfaceMuted,
    ThemeAccent,
    ThemeText,
    ThemeTextMuted,
    ThemeInputBackground,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AnimationKey {
    Widget { id: u64, property: WidgetProperty },
    Window(WindowProperty),
}

pub(crate) trait Animatable: Clone + PartialEq {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self;
}

impl Animatable for Color {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        fn lerp_channel(from: u8, to: u8, progress: f32) -> u8 {
            (from as f32 + (to as f32 - from as f32) * progress)
                .round()
                .clamp(0.0, 255.0) as u8
        }

        Self::rgba(
            lerp_channel(from.r, to.r, progress),
            lerp_channel(from.g, to.g, progress),
            lerp_channel(from.b, to.b, progress),
            lerp_channel(from.a, to.a, progress),
        )
    }
}

impl Animatable for f32 {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        from + (to - from) * progress
    }
}

impl Animatable for Point {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        Self {
            x: f32::interpolate(&from.x, &to.x, progress),
            y: f32::interpolate(&from.y, &to.y, progress),
        }
    }
}

#[derive(Clone)]
struct ActiveAnimation<T> {
    from: T,
    to: T,
    transition: Transition,
    started_at: Instant,
}

struct SlotState<T> {
    displayed: T,
    target: T,
    animation: Option<ActiveAnimation<T>>,
}

impl<T: Animatable> SlotState<T> {
    fn settled(value: T) -> Self {
        Self {
            displayed: value.clone(),
            target: value,
            animation: None,
        }
    }

    fn sample(&mut self, now: Instant) -> T {
        let Some(animation) = self.animation.as_ref() else {
            return self.displayed.clone();
        };

        let duration = animation.transition.duration();
        if duration.is_zero() {
            self.displayed = animation.to.clone();
            self.target = animation.to.clone();
            self.animation = None;
            return self.displayed.clone();
        }

        let elapsed = now.saturating_duration_since(animation.started_at);
        let progress = (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
        let eased = animation.transition.easing().sample(progress);
        self.displayed = T::interpolate(&animation.from, &animation.to, eased);
        if progress >= 1.0 {
            self.displayed = animation.to.clone();
            self.target = animation.to.clone();
            self.animation = None;
        }

        self.displayed.clone()
    }
}

struct AnimationStore<T> {
    slots: HashMap<AnimationKey, SlotState<T>>,
}

impl<T> Default for AnimationStore<T> {
    fn default() -> Self {
        Self {
            slots: HashMap::new(),
        }
    }
}

impl<T: Animatable> AnimationStore<T> {
    fn resolve(
        &mut self,
        key: AnimationKey,
        target: T,
        transition: Option<Transition>,
        now: Instant,
    ) -> T {
        let Some(transition) = transition.filter(|transition| !transition.duration().is_zero())
        else {
            self.slots.insert(key, SlotState::settled(target.clone()));
            return target;
        };

        let state = self
            .slots
            .entry(key)
            .or_insert_with(|| SlotState::settled(target.clone()));

        let current = state.sample(now);
        if state.target != target {
            state.target = target.clone();
            if current != target {
                state.displayed = current.clone();
                state.animation = Some(ActiveAnimation {
                    from: current,
                    to: target,
                    transition,
                    started_at: now,
                });
            } else {
                state.displayed = target.clone();
                state.animation = None;
            }
        }

        state.sample(now)
    }

    fn refresh(&mut self, now: Instant) -> bool {
        let mut changed = false;
        for state in self.slots.values_mut() {
            let before = state.displayed.clone();
            if state.sample(now) != before {
                changed = true;
            }
        }
        changed
    }

    fn has_active(&self) -> bool {
        self.slots.values().any(|state| state.animation.is_some())
    }
}

#[derive(Default)]
pub(crate) struct AnimationEngine {
    colors: AnimationStore<Color>,
    floats: AnimationStore<f32>,
    points: AnimationStore<Point>,
}

impl AnimationEngine {
    pub(crate) fn resolve_color(
        &mut self,
        key: AnimationKey,
        target: Color,
        transition: Option<Transition>,
        now: Instant,
    ) -> Color {
        self.colors.resolve(key, target, transition, now)
    }

    pub(crate) fn resolve_f32(
        &mut self,
        key: AnimationKey,
        target: f32,
        transition: Option<Transition>,
        now: Instant,
    ) -> f32 {
        self.floats.resolve(key, target, transition, now)
    }

    pub(crate) fn resolve_point(
        &mut self,
        key: AnimationKey,
        target: Point,
        transition: Option<Transition>,
        now: Instant,
    ) -> Point {
        self.points.resolve(key, target, transition, now)
    }

    pub(crate) fn refresh(&mut self, now: Instant) -> bool {
        self.colors.refresh(now) || self.floats.refresh(now) || self.points.refresh(now)
    }

    pub(crate) fn has_active_animations(&self) -> bool {
        self.colors.has_active() || self.floats.has_active() || self.points.has_active()
    }

    pub(crate) fn next_frame_deadline(&self, now: Instant) -> Option<Instant> {
        self.has_active_animations().then_some(now + FRAME_INTERVAL)
    }
}

pub(crate) fn default_theme_transition() -> Transition {
    Transition::ease_in_out(Duration::from_millis(THEME_DURATION_MS))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{Animatable, AnimationEngine, AnimationKey, Transition, WidgetProperty};
    use crate::foundation::color::Color;
    use crate::ui::widget::Point;

    fn key(property: WidgetProperty) -> AnimationKey {
        AnimationKey::Widget { id: 1, property }
    }

    #[test]
    fn color_interpolation_blends_channels() {
        let start = Color::rgba(0, 20, 40, 60);
        let end = Color::rgba(100, 120, 140, 160);
        assert_eq!(
            Color::interpolate(&start, &end, 0.5),
            Color::rgba(50, 70, 90, 110)
        );
    }

    #[test]
    fn point_interpolation_blends_coordinates() {
        let interpolated = Point::interpolate(
            &Point { x: 0.0, y: 10.0 },
            &Point { x: 20.0, y: 30.0 },
            0.25,
        );
        assert_eq!(interpolated, Point { x: 5.0, y: 15.0 });
    }

    #[test]
    fn unchanged_target_does_not_restart_animation() {
        let mut engine = AnimationEngine::default();
        let transition = Transition::ease_out(Duration::from_millis(100));
        let start = Instant::now();

        assert_eq!(
            engine.resolve_f32(key(WidgetProperty::Opacity), 0.0, Some(transition), start),
            0.0
        );
        let mid = start + Duration::from_millis(50);
        let animated = engine.resolve_f32(key(WidgetProperty::Opacity), 1.0, Some(transition), mid);
        let repeated = engine.resolve_f32(key(WidgetProperty::Opacity), 1.0, Some(transition), mid);

        assert_eq!(animated, repeated);
        assert!(engine.has_active_animations());
    }

    #[test]
    fn target_change_continues_from_current_value() {
        let mut engine = AnimationEngine::default();
        let transition = Transition::linear(Duration::from_millis(100));
        let start = Instant::now();

        engine.resolve_f32(key(WidgetProperty::Opacity), 0.0, Some(transition), start);
        engine.resolve_f32(
            key(WidgetProperty::Opacity),
            10.0,
            Some(transition),
            start + Duration::from_millis(1),
        );
        let mid = start + Duration::from_millis(51);
        let current = engine.resolve_f32(key(WidgetProperty::Opacity), 10.0, Some(transition), mid);
        let redirected =
            engine.resolve_f32(key(WidgetProperty::Opacity), 20.0, Some(transition), mid);

        assert_eq!(current, redirected);
    }

    #[test]
    fn finished_animation_lands_exactly_on_target() {
        let mut engine = AnimationEngine::default();
        let transition = Transition::linear(Duration::from_millis(100));
        let start = Instant::now();

        engine.resolve_color(
            key(WidgetProperty::Background),
            Color::BLACK,
            Some(transition),
            start,
        );
        engine.resolve_color(
            key(WidgetProperty::Background),
            Color::WHITE,
            Some(transition),
            start + Duration::from_millis(1),
        );

        let end = start + Duration::from_millis(200);
        assert_eq!(
            engine.resolve_color(
                key(WidgetProperty::Background),
                Color::WHITE,
                Some(transition),
                end,
            ),
            Color::WHITE
        );
        assert!(!engine.has_active_animations());
    }

    #[test]
    fn refresh_reports_when_animated_values_advance() {
        let mut engine = AnimationEngine::default();
        let transition = Transition::linear(Duration::from_millis(100));
        let start = Instant::now();

        engine.resolve_point(
            key(WidgetProperty::Offset),
            Point { x: 0.0, y: 0.0 },
            Some(transition),
            start,
        );
        engine.resolve_point(
            key(WidgetProperty::Offset),
            Point { x: 20.0, y: 0.0 },
            Some(transition),
            start + Duration::from_millis(1),
        );

        assert!(engine.refresh(start + Duration::from_millis(50)));
    }
}
