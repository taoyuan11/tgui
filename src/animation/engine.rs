use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::foundation::color::Color;
use crate::ui::layout::Insets;
use crate::ui::unit::{Dp, Sp};
use crate::ui::widget::Point;
use smallvec::SmallVec;

use super::controller::{sample_timeline, FRAME_INTERVAL};
use super::spec::{Keyframes, Transition};

const THEME_DURATION_MS: u64 = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WidgetProperty {
    Background,
    BackgroundAlt,
    BackgroundBlur,
    BorderColor,
    BorderRadius,
    BorderWidth,
    TextColor,
    Opacity,
    Offset,
    TooltipVisibility,
    ModalVisibility,
    SwitchThumbColor,
    SwitchThumbOffset,
    SelectMenuOpen,
    Width,
    Height,
    Margin,
    Padding,
    Gap,
    Grow,
    ProgressIndeterminatePhase,
    SpinnerPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WindowProperty {
    ClearColor,
    ThemeBackground,
    ThemeSurface,
    ThemeSurfaceLow,
    ThemeSurfaceHigh,
    ThemePrimary,
    ThemeOnSurface,
    ThemeOnSurfaceMuted,
    ThemePrimaryContainer,
    ThemeFocusRing,
    ThemeSelection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AnimationKey {
    Widget { id: u64, property: WidgetProperty },
    Window(WindowProperty),
}

impl WidgetProperty {
    pub(crate) const fn affects_layout(self) -> bool {
        matches!(
            self,
            Self::Width | Self::Height | Self::Margin | Self::Padding | Self::Gap | Self::Grow
        )
    }

    pub(crate) const fn affects_scene(self) -> bool {
        !self.affects_layout()
    }
}

impl AnimationKey {
    pub(crate) const fn affects_layout(self) -> bool {
        match self {
            Self::Widget { property, .. } => property.affects_layout(),
            Self::Window(WindowProperty::ClearColor) => false,
            Self::Window(_) => true,
        }
    }
}

/// 表示可由动画系统插值的类型。
pub trait Animatable: Clone + PartialEq + Send + Sync + 'static {
    /// 在 `from` 和 `to` 之间按进度插值。
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self;
}

impl Animatable for Color {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        fn lerp_channel(from: u8, to: u8, progress: f32) -> u8 {
            (from as f32 + (to as f32 - from as f32) * progress)
                .round()
                .clamp(0.0, 255.0) as u8
        }

        fn lerp_unit(from: f32, to: f32, progress: f32) -> f32 {
            from + (to - from) * progress
        }

        let from_alpha = from.a as f32 / 255.0;
        let to_alpha = to.a as f32 / 255.0;
        let alpha = lerp_unit(from_alpha, to_alpha, progress).clamp(0.0, 1.0);
        if alpha > f32::EPSILON {
            let channel = |from_channel: u8, to_channel: u8| {
                let from_premultiplied = (from_channel as f32 / 255.0) * from_alpha;
                let to_premultiplied = (to_channel as f32 / 255.0) * to_alpha;
                ((lerp_unit(from_premultiplied, to_premultiplied, progress) / alpha) * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8
            };

            return Self::rgba(
                channel(from.r, to.r),
                channel(from.g, to.g),
                channel(from.b, to.b),
                (alpha * 255.0).round().clamp(0.0, 255.0) as u8,
            );
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

impl Animatable for Dp {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        Self(f32::interpolate(&from.0, &to.0, progress))
    }
}

impl Animatable for Sp {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        Self(f32::interpolate(&from.0, &to.0, progress))
    }
}

impl Animatable for Point {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        Self {
            x: Dp::interpolate(&from.x, &to.x, progress),
            y: Dp::interpolate(&from.y, &to.y, progress),
        }
    }
}

impl Animatable for Insets {
    fn interpolate(from: &Self, to: &Self, progress: f32) -> Self {
        Self {
            left: Dp::interpolate(&from.left, &to.left, progress),
            top: Dp::interpolate(&from.top, &to.top, progress),
            right: Dp::interpolate(&from.right, &to.right, progress),
            bottom: Dp::interpolate(&from.bottom, &to.bottom, progress),
        }
    }
}

impl<T: Animatable> Keyframes<T> {
    /// 在指定时间点采样关键帧值。
    pub fn sample_at(&self, time: Duration) -> Option<T> {
        let frames = self.sorted_frames();
        let first = frames.first()?;
        let last = frames.last()?;
        if frames.len() == 1 || time <= first.offset() {
            return Some(first.value().clone());
        }
        if time >= last.offset() {
            return Some(last.value().clone());
        }

        for window in frames.windows(2) {
            let from = window[0];
            let to = window[1];
            if time >= from.offset() && time <= to.offset() {
                let span = to.offset().saturating_sub(from.offset());
                if span.is_zero() {
                    return Some(to.value().clone());
                }
                let elapsed = time.saturating_sub(from.offset());
                let progress = (elapsed.as_secs_f32() / span.as_secs_f32()).clamp(0.0, 1.0);
                let eased = self.curve_mode().sample(progress);
                return Some(T::interpolate(from.value(), to.value(), eased));
            }
        }

        Some(last.value().clone())
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

        let Some(sample) = sample_timeline(
            animation.transition.duration(),
            animation.transition.playback_mode(),
            now.saturating_duration_since(animation.started_at),
        ) else {
            self.displayed = animation.from.clone();
            return self.displayed.clone();
        };

        let progress = if animation.transition.duration().is_zero() {
            1.0
        } else {
            animation.transition.curve_mode().sample(
                sample.local_time.as_secs_f32() / animation.transition.duration().as_secs_f32(),
            )
        };
        self.displayed = if sample.reversed {
            T::interpolate(&animation.to, &animation.from, progress)
        } else {
            T::interpolate(&animation.from, &animation.to, progress)
        };
        if sample.completed {
            self.displayed = if sample.reversed {
                animation.from.clone()
            } else {
                animation.to.clone()
            };
            self.target = self.displayed.clone();
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
    fn contains(&self, key: AnimationKey) -> bool {
        self.slots.contains_key(&key)
    }

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

    fn refresh(&mut self, now: Instant) -> AnimationRefresh {
        let mut refresh = AnimationRefresh::default();
        for (key, state) in self.slots.iter_mut() {
            let before = state.displayed.clone();
            if state.sample(now) != before {
                refresh.changed = true;
                if key.affects_layout() {
                    refresh.layout_changed = true;
                    if let AnimationKey::Widget { id, .. } = key {
                        refresh.push_layout_widget(*id);
                    }
                } else if let AnimationKey::Widget { id, property } = key {
                    if property.affects_scene() {
                        refresh.push_scene_widget(*id);
                    }
                }
            }
        }
        refresh
    }

    fn has_active(&self) -> bool {
        self.slots.values().any(|state| state.animation.is_some())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AnimationRefresh {
    pub(crate) changed: bool,
    pub(crate) layout_changed: bool,
    pub(crate) layout_widget_ids: SmallVec<[u64; 16]>,
    pub(crate) scene_widget_ids: SmallVec<[u64; 16]>,
}

impl AnimationRefresh {
    fn push_layout_widget(&mut self, widget_id: u64) {
        if !self.layout_widget_ids.contains(&widget_id) {
            self.layout_widget_ids.push(widget_id);
        }
    }

    fn push_scene_widget(&mut self, widget_id: u64) {
        if !self.scene_widget_ids.contains(&widget_id) {
            self.scene_widget_ids.push(widget_id);
        }
    }
}

#[derive(Default)]
pub(crate) struct AnimationEngine {
    colors: AnimationStore<Color>,
    floats: AnimationStore<f32>,
    dps: AnimationStore<Dp>,
    points: AnimationStore<Point>,
    insets: AnimationStore<Insets>,
}

impl AnimationEngine {
    pub(crate) fn contains_key(&self, key: AnimationKey) -> bool {
        self.colors.contains(key)
            || self.floats.contains(key)
            || self.dps.contains(key)
            || self.points.contains(key)
            || self.insets.contains(key)
    }

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

    pub(crate) fn resolve_dp(
        &mut self,
        key: AnimationKey,
        target: Dp,
        transition: Option<Transition>,
        now: Instant,
    ) -> Dp {
        self.dps.resolve(key, target, transition, now)
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

    pub(crate) fn resolve_insets(
        &mut self,
        key: AnimationKey,
        target: Insets,
        transition: Option<Transition>,
        now: Instant,
    ) -> Insets {
        self.insets.resolve(key, target, transition, now)
    }

    pub(crate) fn refresh(&mut self, now: Instant) -> AnimationRefresh {
        let stores = [
            self.colors.refresh(now),
            self.floats.refresh(now),
            self.dps.refresh(now),
            self.points.refresh(now),
            self.insets.refresh(now),
        ];
        stores
            .into_iter()
            .fold(AnimationRefresh::default(), |mut acc, next| {
                acc.changed |= next.changed;
                acc.layout_changed |= next.layout_changed;
                for widget_id in next.layout_widget_ids {
                    acc.push_layout_widget(widget_id);
                }
                for widget_id in next.scene_widget_ids {
                    acc.push_scene_widget(widget_id);
                }
                acc
            })
    }

    pub(crate) fn has_active_animations(&self) -> bool {
        self.colors.has_active()
            || self.floats.has_active()
            || self.dps.has_active()
            || self.points.has_active()
            || self.insets.has_active()
    }

    pub(crate) fn active_keys_summary(&self) -> String {
        fn push_active_keys<T>(summary: &mut Vec<String>, kind: &str, store: &AnimationStore<T>) {
            for key in store
                .slots
                .iter()
                .filter_map(|(key, state)| state.animation.as_ref().map(|_| *key))
            {
                summary.push(format!("{kind}:{key:?}"));
            }
        }

        let mut summary = Vec::new();
        push_active_keys(&mut summary, "color", &self.colors);
        push_active_keys(&mut summary, "float", &self.floats);
        push_active_keys(&mut summary, "dp", &self.dps);
        push_active_keys(&mut summary, "point", &self.points);
        push_active_keys(&mut summary, "insets", &self.insets);
        if summary.is_empty() {
            "none".to_string()
        } else {
            summary.join("|")
        }
    }

    pub(crate) fn next_frame_deadline(&self, now: Instant) -> Option<Instant> {
        self.has_active_animations().then_some(now + FRAME_INTERVAL)
    }
}

pub(crate) fn default_theme_transition() -> Transition {
    Transition::ease_in_out(Duration::from_millis(THEME_DURATION_MS))
}
