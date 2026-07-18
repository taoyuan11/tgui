use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::foundation::binding::PropertySlot;
use crate::foundation::color::Color;
use crate::ui::layout::Insets;
use crate::ui::unit::{Dp, Sp};
use crate::ui::widget::{Point, WidgetId};
use smallvec::SmallVec;

#[cfg(feature = "bench-support")]
use std::cell::Cell;

use super::controller::sample_timeline;
use super::spec::{AnimationCurve, Keyframe, Keyframes, Transition};

const THEME_DURATION_MS: u64 = 240;
const INACTIVE_SLOT_INDEX: usize = usize::MAX;

#[cfg(feature = "bench-support")]
thread_local! {
    static LEGACY_REFRESH_WIDGET_DEDUP: Cell<bool> = const { Cell::new(false) };
    static REFRESH_WIDGET_DEDUP_COMPARISONS: Cell<u64> = const { Cell::new(0) };
    static REFRESH_WIDGET_ID_BUFFER_SPILLS: Cell<u64> = const { Cell::new(0) };
}

#[cfg(feature = "bench-support")]
pub(crate) fn with_legacy_refresh_widget_dedup<R>(legacy: bool, f: impl FnOnce() -> R) -> R {
    LEGACY_REFRESH_WIDGET_DEDUP.with(|mode| {
        let previous = mode.replace(legacy);
        let result = f();
        mode.set(previous);
        result
    })
}

#[cfg(feature = "bench-support")]
pub(crate) fn reset_refresh_widget_dedup_stats() {
    REFRESH_WIDGET_DEDUP_COMPARISONS.with(|counter| counter.set(0));
    REFRESH_WIDGET_ID_BUFFER_SPILLS.with(|counter| counter.set(0));
}

#[cfg(feature = "bench-support")]
pub(crate) fn refresh_widget_dedup_stats() -> (u64, u64) {
    (
        REFRESH_WIDGET_DEDUP_COMPARISONS.with(Cell::get),
        REFRESH_WIDGET_ID_BUFFER_SPILLS.with(Cell::get),
    )
}

#[cfg(feature = "bench-support")]
fn legacy_refresh_widget_dedup_enabled() -> bool {
    LEGACY_REFRESH_WIDGET_DEDUP.with(Cell::get)
}

#[cfg(feature = "bench-support")]
fn record_refresh_widget_dedup_comparison() {
    REFRESH_WIDGET_DEDUP_COMPARISONS.with(|counter| {
        counter.set(counter.get().saturating_add(1));
    });
}

#[cfg(feature = "bench-support")]
fn record_refresh_widget_id_buffer_spill() {
    REFRESH_WIDGET_ID_BUFFER_SPILLS.with(|counter| {
        counter.set(counter.get().saturating_add(1));
    });
}

/// 槽位回收软上限:槽位总数超过此值才会尝试回收陈旧的已稳定槽位。常规应用
/// 远低于此值,因此不触发任何回收、零行为变化。
pub(crate) const SLOT_GC_SOFT_CAP: usize = 8192;
/// 已稳定槽位在超过软上限后,`last_touch` 早于此时长即视为陈旧(对应已销毁
/// widget)可回收。取值足够宽松,确保仅短暂未重收集的存活 widget 不被误回收。
pub(crate) const SLOT_GC_TTL: Duration = Duration::from_secs(10);
/// 大型槽位表的最短 GC 间隔。GC 是 O(retained slots) 的维护操作，不能在每次事件
/// refresh 中重复执行；一秒节流把过期回收延迟限制在最多一秒，同时避免输入热路径抖动。
pub(crate) const SLOT_GC_SWEEP_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum WidgetProperty {
    Background,
    BackgroundAlt,
    BackgroundBlur,
    BorderColor,
    BorderRadius,
    BorderWidth,
    TextColor,
    TextureMaskTint,
    Opacity,
    Offset,
    Scale,
    TooltipVisibility,
    PopoverVisibility,
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

    /// Whether an in-place scene animation can change geometry exposed through AccessKit.
    ///
    /// Layout properties already force a layout/scene rebuild (and therefore a new retained-scene
    /// serial). Most scene properties are paint-only and must not make accessibility rebuild the
    /// complete tree every animation frame. Offset/scale are the exceptions: they can move the
    /// hit-region bounds used by the accessibility tree while the retained scene serial stays
    /// stable. The currently dormant collapse/carousel channels remain conservative because their
    /// eventual implementations may move or window child content.
    pub(crate) const fn affects_accessibility_geometry(self) -> bool {
        matches!(
            self,
            Self::Offset | Self::Scale | Self::CollapseProgress | Self::CarouselSlideProgress
        )
    }

    /// Retained scene-property slot corresponding to this transition channel. Properties without
    /// a one-to-one retained representation deliberately return `None` and keep the existing
    /// subtree patch/full-recollect fallback.
    pub(crate) const fn retained_property_slot(self) -> Option<PropertySlot> {
        match self {
            Self::Background => Some(PropertySlot::Background),
            Self::BackgroundBlur => Some(PropertySlot::BackgroundBlur),
            Self::BorderColor => Some(PropertySlot::BorderColor),
            Self::BorderRadius => Some(PropertySlot::BorderRadius),
            Self::BorderWidth => Some(PropertySlot::BorderWidth),
            Self::TextColor => Some(PropertySlot::TextColor),
            Self::TextureMaskTint => Some(PropertySlot::TextureMaskTint),
            Self::Opacity => Some(PropertySlot::Opacity),
            Self::Offset => Some(PropertySlot::Offset),
            Self::Scale => Some(PropertySlot::Scale),
            Self::Width => Some(PropertySlot::Width),
            Self::Height => Some(PropertySlot::Height),
            Self::Margin => Some(PropertySlot::Margin),
            Self::Padding => Some(PropertySlot::Padding),
            Self::Grow => Some(PropertySlot::Grow),
            _ => None,
        }
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
        let curve = self.curve_mode();
        if self.frames_are_sorted_by_offset() {
            return sample_sorted_keyframes(self.frames(), curve, time);
        }

        let frames = self.sorted_frames();
        sample_sorted_keyframe_refs(&frames, curve, time)
    }
}

fn sample_sorted_keyframes<T: Animatable>(
    frames: &[Keyframe<T>],
    curve: AnimationCurve,
    time: Duration,
) -> Option<T> {
    let first = frames.first()?;
    let last = frames.last()?;
    if frames.len() == 1 || time <= first.offset() {
        return Some(first.value().clone());
    }
    if time >= last.offset() {
        return Some(last.value().clone());
    }

    let to_index = frames.partition_point(|frame| frame.offset() < time);
    Some(interpolate_keyframes(
        &frames[to_index - 1],
        &frames[to_index],
        curve,
        time,
    ))
}

fn sample_sorted_keyframe_refs<T: Animatable>(
    frames: &[&Keyframe<T>],
    curve: AnimationCurve,
    time: Duration,
) -> Option<T> {
    let first = *frames.first()?;
    let last = *frames.last()?;
    if frames.len() == 1 || time <= first.offset() {
        return Some(first.value().clone());
    }
    if time >= last.offset() {
        return Some(last.value().clone());
    }

    let to_index = frames.partition_point(|frame| frame.offset() < time);
    Some(interpolate_keyframes(
        frames[to_index - 1],
        frames[to_index],
        curve,
        time,
    ))
}

fn interpolate_keyframes<T: Animatable>(
    from: &Keyframe<T>,
    to: &Keyframe<T>,
    curve: AnimationCurve,
    time: Duration,
) -> T {
    let span = to.offset().saturating_sub(from.offset());
    if span.is_zero() {
        return to.value().clone();
    }
    let elapsed = time.saturating_sub(from.offset());
    let progress = (elapsed.as_secs_f32() / span.as_secs_f32()).clamp(0.0, 1.0);
    let eased = curve.sample(progress);
    T::interpolate(from.value(), to.value(), eased)
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
    /// Index in `AnimationStore::active_keys`, or `INACTIVE_SLOT_INDEX` while settled. Keeping the
    /// index beside the slot makes activation/deactivation O(1), including a reduced-motion switch
    /// that immediately settles a large batch of running transitions.
    active_index: usize,
}

impl<T: Animatable> SlotState<T> {
    fn settled(value: T, now: Instant) -> Self {
        Self {
            displayed: value.clone(),
            target: value,
            animation: None,
            last_touch: now,
            active_index: INACTIVE_SLOT_INDEX,
        }
    }

    fn advance(&mut self, now: Instant) {
        let Some(animation) = self.animation.as_ref() else {
            return;
        };

        let Some(sample) = sample_timeline(
            animation.transition.duration(),
            animation.transition.playback_mode(),
            now.saturating_duration_since(animation.started_at),
        ) else {
            self.displayed = animation.from.clone();
            return;
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
    }

    fn sample(&mut self, now: Instant) -> T {
        self.advance(now);
        self.displayed.clone()
    }
}

struct AnimationStore<T> {
    slots: HashMap<AnimationKey, SlotState<T>>,
    /// Dense list of running slots. A stable scene can retain thousands of settled slots, while
    /// only a handful animate; refreshing this list avoids walking every retained HashMap bucket.
    // Boxed lazily so adding the dense index does not grow the inline `AnimationStore` (and, in
    // turn, every runtime handler) compared with the former `active_count: usize` field.
    active_keys: Option<Box<Vec<AnimationKey>>>,
}

impl<T> Default for AnimationStore<T> {
    fn default() -> Self {
        Self {
            slots: HashMap::new(),
            active_keys: None,
        }
    }
}

impl<T: Animatable> AnimationStore<T> {
    fn activate(&mut self, key: AnimationKey) {
        let active_index = self
            .active_keys
            .as_ref()
            .map_or(0, |active_keys| active_keys.len());
        let state = self
            .slots
            .get_mut(&key)
            .expect("animation slot must exist before activation");
        debug_assert!(state.animation.is_some());
        debug_assert_eq!(state.active_index, INACTIVE_SLOT_INDEX);
        state.active_index = active_index;
        self.active_keys
            .get_or_insert_with(|| Box::new(Vec::new()))
            .push(key);
    }

    fn deactivate_at(&mut self, index: usize) {
        let (removed_key, moved_key) = {
            let active_keys = self
                .active_keys
                .as_mut()
                .expect("active animation index must be allocated");
            let removed_key = active_keys.swap_remove(index);
            (removed_key, active_keys.get(index).copied())
        };
        self.slots
            .get_mut(&removed_key)
            .expect("active animation slot must exist")
            .active_index = INACTIVE_SLOT_INDEX;

        if let Some(moved_key) = moved_key {
            self.slots
                .get_mut(&moved_key)
                .expect("moved active animation slot must exist")
                .active_index = index;
        }
    }

    fn deactivate(&mut self, key: AnimationKey) {
        let index = self
            .slots
            .get(&key)
            .expect("animation slot must exist before deactivation")
            .active_index;
        debug_assert_ne!(index, INACTIVE_SLOT_INDEX);
        self.deactivate_at(index);
    }

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
            if let Some(state) = self.slots.get_mut(&key) {
                let was_active = state.animation.take().is_some();
                state.last_touch = now;
                if was_active || state.target != target || state.displayed != target {
                    state.displayed = target.clone();
                    state.target = target.clone();
                }
                if was_active {
                    self.deactivate(key);
                }
            } else {
                self.slots
                    .insert(key, SlotState::settled(target.clone(), now));
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
            (false, true) => self.activate(key),
            (true, false) => self.deactivate(key),
            _ => {}
        }

        value
    }

    fn refresh(&mut self, now: Instant, refresh: &mut AnimationRefresh) {
        if !self.has_active() {
            return;
        }

        let mut active_index = 0;
        while self
            .active_keys
            .as_ref()
            .is_some_and(|active_keys| active_index < active_keys.len())
        {
            let key = self
                .active_keys
                .as_ref()
                .expect("active animation index must be allocated")[active_index];
            #[cfg(test)]
            {
                refresh.visited_slots += 1;
            }
            let (changed, completed) = {
                let state = self
                    .slots
                    .get_mut(&key)
                    .expect("active animation key must reference a retained slot");
                debug_assert_eq!(state.active_index, active_index);
                debug_assert!(state.animation.is_some());
                let before = state.displayed.clone();
                state.advance(now);
                (state.displayed != before, state.animation.is_none())
            };

            if changed {
                refresh.changed = true;
                if key.affects_layout() {
                    refresh.layout_changed = true;
                    if let AnimationKey::Widget { id, property } = key {
                        refresh.push_layout_widget(id, property);
                    }
                } else if let AnimationKey::Widget { id, property } = key {
                    if property.affects_scene() {
                        refresh.push_scene_widget(id, property);
                    }
                    if property.affects_accessibility_geometry() {
                        refresh.accessibility_geometry_changed = true;
                    }
                }
            }

            if completed {
                // `swap_remove` moves another active key into this index, so process the same index
                // again instead of incrementing it.
                self.deactivate_at(active_index);
            } else {
                active_index += 1;
            }
        }
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
        self.active_keys
            .as_ref()
            .is_some_and(|active_keys| !active_keys.is_empty())
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
    pub(crate) accessibility_geometry_changed: bool,
    pub(crate) layout_widget_ids: SmallVec<[u64; 16]>,
    pub(crate) layout_property_targets: SmallVec<[(WidgetId, PropertySlot); 16]>,
    pub(crate) has_unscoped_layout_changes: bool,
    pub(crate) scene_widget_ids: SmallVec<[u64; 16]>,
    pub(crate) scene_property_targets: SmallVec<[(WidgetId, PropertySlot); 16]>,
    pub(crate) has_unscoped_scene_changes: bool,
    #[cfg(test)]
    pub(crate) visited_slots: usize,
}

impl AnimationRefresh {
    fn push_layout_widget(&mut self, widget_id: u64, property: WidgetProperty) {
        #[cfg(feature = "bench-support")]
        if legacy_refresh_widget_dedup_enabled() {
            let duplicate = self.layout_widget_ids.iter().any(|existing| {
                record_refresh_widget_dedup_comparison();
                *existing == widget_id
            });
            if !duplicate {
                let was_spilled = self.layout_widget_ids.spilled();
                self.layout_widget_ids.push(widget_id);
                if !was_spilled && self.layout_widget_ids.spilled() {
                    record_refresh_widget_id_buffer_spill();
                }
            }
            if let Some(property) = property.retained_property_slot() {
                self.layout_property_targets
                    .push((WidgetId::from_raw(widget_id), property));
            } else {
                self.has_unscoped_layout_changes = true;
            }
            return;
        }
        #[cfg(feature = "bench-support")]
        let was_spilled = self.layout_widget_ids.spilled();
        self.layout_widget_ids.push(widget_id);
        #[cfg(feature = "bench-support")]
        if !was_spilled && self.layout_widget_ids.spilled() {
            record_refresh_widget_id_buffer_spill();
        }
        if let Some(property) = property.retained_property_slot() {
            self.layout_property_targets
                .push((WidgetId::from_raw(widget_id), property));
        } else {
            self.has_unscoped_layout_changes = true;
        }
    }

    fn push_scene_widget(&mut self, widget_id: u64, property: WidgetProperty) {
        #[cfg(feature = "bench-support")]
        if legacy_refresh_widget_dedup_enabled() {
            let duplicate = self.scene_widget_ids.iter().any(|existing| {
                record_refresh_widget_dedup_comparison();
                *existing == widget_id
            });
            if !duplicate {
                let was_spilled = self.scene_widget_ids.spilled();
                self.scene_widget_ids.push(widget_id);
                if !was_spilled && self.scene_widget_ids.spilled() {
                    record_refresh_widget_id_buffer_spill();
                }
            }
            if let Some(property) = property.retained_property_slot() {
                self.scene_property_targets
                    .push((WidgetId::from_raw(widget_id), property));
            } else {
                self.has_unscoped_scene_changes = true;
            }
            return;
        }
        #[cfg(feature = "bench-support")]
        let was_spilled = self.scene_widget_ids.spilled();
        self.scene_widget_ids.push(widget_id);
        #[cfg(feature = "bench-support")]
        if !was_spilled && self.scene_widget_ids.spilled() {
            record_refresh_widget_id_buffer_spill();
        }
        if let Some(property) = property.retained_property_slot() {
            self.scene_property_targets
                .push((WidgetId::from_raw(widget_id), property));
        } else {
            self.has_unscoped_scene_changes = true;
        }
    }

    fn normalize_widget_ids(&mut self) {
        // One widget can have several animated properties, possibly stored in different typed
        // animation stores. Deduplicating on every insertion made a frame with N independently
        // animated widgets O(N²), even though the common case has no duplicates at all. Runtime
        // consumers treat these as sets and order patch roots independently, so collect densely
        // and canonicalize once after all stores have refreshed.
        #[cfg(feature = "bench-support")]
        if legacy_refresh_widget_dedup_enabled() {
            self.layout_property_targets
                .sort_unstable_by_key(|(widget_id, property)| (widget_id.raw(), *property as u8));
            self.layout_property_targets.dedup();
            self.scene_property_targets
                .sort_unstable_by_key(|(widget_id, property)| (widget_id.raw(), *property as u8));
            self.scene_property_targets.dedup();
            return;
        }
        #[cfg(feature = "bench-support")]
        self.layout_widget_ids.sort_unstable_by(|left, right| {
            record_refresh_widget_dedup_comparison();
            left.cmp(right)
        });
        #[cfg(not(feature = "bench-support"))]
        self.layout_widget_ids.sort_unstable();
        self.layout_widget_ids.dedup();
        self.layout_property_targets
            .sort_unstable_by_key(|(widget_id, property)| (widget_id.raw(), *property as u8));
        self.layout_property_targets.dedup();
        #[cfg(feature = "bench-support")]
        self.scene_widget_ids.sort_unstable_by(|left, right| {
            record_refresh_widget_dedup_comparison();
            left.cmp(right)
        });
        #[cfg(not(feature = "bench-support"))]
        self.scene_widget_ids.sort_unstable();
        self.scene_widget_ids.dedup();
        self.scene_property_targets
            .sort_unstable_by_key(|(widget_id, property)| (widget_id.raw(), *property as u8));
        self.scene_property_targets.dedup();
    }
}

#[derive(Default)]
pub(crate) struct AnimationEngine {
    colors: AnimationStore<Color>,
    floats: AnimationStore<f32>,
    dps: AnimationStore<Dp>,
    points: AnimationStore<Point>,
    insets: AnimationStore<Insets>,
    next_slot_gc_sweep_at: Option<Instant>,
    #[cfg(test)]
    slot_gc_sweep_count: usize,
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
        if !self.has_active_animations() {
            self.gc_stale_settled_slots_if_due(now);
            return AnimationRefresh::default();
        }

        let mut refresh = AnimationRefresh::default();
        self.colors.refresh(now, &mut refresh);
        self.floats.refresh(now, &mut refresh);
        self.dps.refresh(now, &mut refresh);
        self.points.refresh(now, &mut refresh);
        self.insets.refresh(now, &mut refresh);
        self.gc_stale_settled_slots_if_due(now);
        refresh.normalize_widget_ids();
        refresh
    }

    fn gc_stale_settled_slots_if_due(&mut self, now: Instant) {
        let has_over_cap_store = self.colors.slots.len() > SLOT_GC_SOFT_CAP
            || self.floats.slots.len() > SLOT_GC_SOFT_CAP
            || self.dps.slots.len() > SLOT_GC_SOFT_CAP
            || self.points.slots.len() > SLOT_GC_SOFT_CAP
            || self.insets.slots.len() > SLOT_GC_SOFT_CAP;
        if !has_over_cap_store {
            self.next_slot_gc_sweep_at = None;
            return;
        }
        if self
            .next_slot_gc_sweep_at
            .is_some_and(|deadline| now < deadline)
        {
            return;
        }

        if self.colors.slots.len() > SLOT_GC_SOFT_CAP {
            self.colors.gc_stale_settled_slots(now);
        }
        if self.floats.slots.len() > SLOT_GC_SOFT_CAP {
            self.floats.gc_stale_settled_slots(now);
        }
        if self.dps.slots.len() > SLOT_GC_SOFT_CAP {
            self.dps.gc_stale_settled_slots(now);
        }
        if self.points.slots.len() > SLOT_GC_SOFT_CAP {
            self.points.gc_stale_settled_slots(now);
        }
        if self.insets.slots.len() > SLOT_GC_SOFT_CAP {
            self.insets.gc_stale_settled_slots(now);
        }
        #[cfg(test)]
        {
            self.slot_gc_sweep_count = self.slot_gc_sweep_count.saturating_add(1);
        }

        let still_over_cap = self.colors.slots.len() > SLOT_GC_SOFT_CAP
            || self.floats.slots.len() > SLOT_GC_SOFT_CAP
            || self.dps.slots.len() > SLOT_GC_SOFT_CAP
            || self.points.slots.len() > SLOT_GC_SOFT_CAP
            || self.insets.slots.len() > SLOT_GC_SOFT_CAP;
        self.next_slot_gc_sweep_at =
            still_over_cap.then(|| now.checked_add(SLOT_GC_SWEEP_INTERVAL).unwrap_or(now));
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

    #[cfg(test)]
    pub(crate) fn debug_total_slots(&self) -> usize {
        self.colors.slots.len()
            + self.floats.slots.len()
            + self.dps.slots.len()
            + self.points.slots.len()
            + self.insets.slots.len()
    }

    #[cfg(test)]
    pub(crate) fn debug_slot_gc_sweep_count(&self) -> usize {
        self.slot_gc_sweep_count
    }
}

pub(crate) fn default_theme_transition() -> Transition {
    Transition::ease_in_out(Duration::from_millis(THEME_DURATION_MS))
}
