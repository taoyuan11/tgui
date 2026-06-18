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

/// 槽位回收软上限:槽位总数超过此值才会尝试回收陈旧的已稳定槽位。常规应用
/// 远低于此值,因此不触发任何回收、零行为变化。
pub(crate) const SLOT_GC_SOFT_CAP: usize = 8192;
/// 已稳定槽位在超过软上限后,`last_touch` 早于此时长即视为陈旧(对应已销毁
/// widget)可回收。取值足够宽松,确保仅短暂未重收集的存活 widget 不被误回收。
pub(crate) const SLOT_GC_TTL: Duration = Duration::from_secs(10);

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
    Scale,
    TooltipVisibility,
    ModalVisibility,
    DrawerVisibility,
    SwitchThumbColor,
    SwitchThumbOffset,
    SelectMenuOpen,
    SelectArrowColor,
    CheckboxBackground,
    CheckboxBorder,
    CheckboxCheckmarkColor,
    CheckboxCheckmarkOpacity,
    RadioBackground,
    RadioBorder,
    RadioIndicatorColor,
    RadioIndicatorOpacity,
    SliderTrackColor,
    SliderActiveTrackColor,
    SliderThumbColor,
    SliderTickColor,
    Width,
    Height,
    Margin,
    Padding,
    Gap,
    Grow,
    ProgressIndeterminatePhase,
    #[allow(dead_code)]
    SkeletonShimmerPhase,
    SpinnerPhase,
    ToastStackExpand,
    TreeDisclosureRotation,
    TreeCheckboxState,
    #[allow(dead_code)]
    CollapseProgress,
    #[allow(dead_code)]
    CarouselSlideProgress,
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
    /// 最近一次被 `resolve`(即 collect 阶段触达该 widget 属性)的时间。用于在槽位
    /// 总数超过软上限时回收长期未触达的「已稳定」槽位(对应已销毁的 widget),
    /// 避免长会话中动态创建/销毁的 widget 让槽位表无界增长。
    last_touch: Instant,
}

impl<T: Animatable> SlotState<T> {
    fn settled(value: T, now: Instant) -> Self {
        Self {
            displayed: value.clone(),
            target: value,
            animation: None,
            last_touch: now,
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
    active_count: usize,
}

impl<T> Default for AnimationStore<T> {
    fn default() -> Self {
        Self {
            slots: HashMap::new(),
            active_count: 0,
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
            let was_active = self
                .slots
                .get(&key)
                .map(|state| state.animation.is_some())
                .unwrap_or(false);
            self.slots
                .insert(key, SlotState::settled(target.clone(), now));
            if was_active {
                self.active_count = self.active_count.saturating_sub(1);
            }
            return target;
        };

        let (value, was_active, is_active) = {
            let state = self
                .slots
                .entry(key)
                .or_insert_with(|| SlotState::settled(target.clone(), now));
            let was_active = state.animation.is_some();
            // widget 本帧被解析,刷新触达时间,使其免于槽位回收。
            state.last_touch = now;

            // 仅在目标变化时,才需要先把动画推进到「当前显示值」作为新动画起点;
            // 目标未变(占绝大多数 resolve 调用)时跳过这次额外采样 + 克隆,末尾统一采样。
            if state.target != target {
                let current = state.sample(now);
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

            let value = state.sample(now);
            let is_active = state.animation.is_some();
            (value, was_active, is_active)
        };
        match (was_active, is_active) {
            (false, true) => self.active_count += 1,
            (true, false) => self.active_count = self.active_count.saturating_sub(1),
            _ => {}
        }

        value
    }

    fn refresh(&mut self, now: Instant) -> AnimationRefresh {
        let mut refresh = AnimationRefresh::default();
        let mut completed_count = 0;
        for (key, state) in self.slots.iter_mut() {
            // 「已稳定」(无活动动画)的槽位采样必然返回 `displayed` 不变,不可能产生变化 ——
            // 直接跳过,使每帧 refresh 的成本正比于「活动动画数」而非「槽位总数」,
            // 同时省掉每个已稳定槽位每帧两次 `displayed.clone()`。
            if state.animation.is_none() {
                continue;
            }
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
            if state.animation.is_none() {
                completed_count += 1;
            }
        }
        if completed_count > 0 {
            self.active_count = self.active_count.saturating_sub(completed_count);
        }
        if self.slots.len() > SLOT_GC_SOFT_CAP {
            self.gc_stale_settled_slots(now);
        }
        refresh
    }

    /// 当槽位总数超过软上限时,回收长期未被 `resolve` 触达的已稳定槽位 ——
    /// 这些几乎必然对应已从树中销毁的 widget(全局自增 id 不复用)。仍存活的
    /// widget 每次 collect 都会刷新 `last_touch`,因此永不会被误回收;活动动画
    /// 槽位也始终保留。常规应用槽位数远低于上限,完全不触发,无任何行为变化。
    fn gc_stale_settled_slots(&mut self, now: Instant) {
        self.slots.retain(|key, state| {
            matches!(key, AnimationKey::Window(_))
                || state.animation.is_some()
                || now.saturating_duration_since(state.last_touch) < SLOT_GC_TTL
        });
    }

    fn has_active(&self) -> bool {
        self.active_count > 0
    }

    fn settled_at(&self, key: AnimationKey, target: &T) -> bool {
        self.slots
            .get(&key)
            .map(|state| {
                state.animation.is_none() && state.target == *target && state.displayed == *target
            })
            .unwrap_or(false)
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

    pub(crate) fn color_settled_at(&self, key: AnimationKey, target: Color) -> bool {
        self.colors.settled_at(key, &target)
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

    #[cfg(test)]
    pub(crate) fn debug_total_slots(&self) -> usize {
        self.colors.slots.len()
            + self.floats.slots.len()
            + self.dps.slots.len()
            + self.points.slots.len()
            + self.insets.slots.len()
    }
}

pub(crate) fn default_theme_transition() -> Transition {
    Transition::ease_in_out(Duration::from_millis(THEME_DURATION_MS))
}
