use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use taffy::prelude::TaffyTree;

use crate::animation::{AnimationKey, Transition, WidgetProperty};
use crate::foundation::binding::{ToastEntry, ToastId, ToastKind, ToastPlacement, ToastQueue};
use crate::ui::layout::{pct, Axis, Justify, Length, Value};
use crate::ui::unit::{dp, sp, Dp};
use crate::ui::widget::button::Button;
use crate::ui::widget::common::{ClipMask, RenderCommand, ScrollRegion};
use crate::ui::widget::common::{LayoutNode, MeasureContext};
use crate::ui::widget::container::{Flex, Stack};
use crate::ui::widget::core::compute_taffy_layout_with_measure;
use crate::ui::widget::icon::{Icon, SvgIconId};
use crate::ui::widget::overlay::{
    collect::emit_overlay, Anchor, Overlay, OverlayContent, OverlayId, OverlayLayer, Placement,
};
use crate::ui::widget::style::{ContainerStyle, IconStyle, TextWidgetStyle, ToastStyle};
use crate::ui::widget::text::Text;
use crate::ui::widget::{
    ComputedScene, CursorStyle, DefaultActivation, Element, HitGeometry, HitInteraction, HitRegion,
    InteractionHandlers, Point, Rect, WidgetId,
};

use super::super::scene::{CollectContext, VisualContext};
use super::super::types::ResolvedElement;
use super::CollectVisualState;
use crate::foundation::binding::DependencyGraph;
use crate::foundation::view_model::Command;

const TOAST_OVERLAY_TAG: u64 = 0x544F4153545F484F; // "TOAST_HO"
const TOAST_STACK_HOVER_TAG: u64 = 0x544F4153545F5354; // "TOAST_ST"
const TOAST_AUTO_COLLAPSE_THRESHOLD: usize = 3;
const TOAST_STACK_VISIBLE_BACK_LAYERS: usize = 2;
const TOAST_STACK_LAYER_INSET_X: Dp = Dp::new(12.0);
const TOAST_STACK_LAYER_OFFSET_Y: Dp = Dp::new(16.0);
const TOAST_STACK_LAYER_OPACITY_STEP: f32 = 0.08;
const TOAST_ENTER_CLIP_MARGIN_X: Dp = Dp::new(160.0);
const TOAST_ENTER_CLIP_MARGIN_Y: Dp = Dp::new(48.0);
const TOAST_ENTER_OFFSET_X: Dp = Dp::new(40.0);
const TOAST_ENTER_OFFSET_Y: Dp = Dp::new(16.0);

thread_local! {
    static REUSE_PREPARED_TOAST_CARDS: Cell<bool> = const { Cell::new(false) };
    static REPLAY_TOAST_BASE_SCENES: Cell<bool> = const { Cell::new(false) };
    #[cfg(feature = "bench-support")]
    static DISABLE_TOAST_BASE_SCENE_REPLAY: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn with_prepared_toast_card_cache<R>(body: impl FnOnce() -> R) -> R {
    REUSE_PREPARED_TOAST_CARDS.with(|reuse| {
        let previous = reuse.replace(true);
        struct Reset<'a> {
            reuse: &'a Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.reuse.set(self.previous);
            }
        }
        let _reset = Reset { reuse, previous };
        body()
    })
}

fn reuse_prepared_toast_cards() -> bool {
    REUSE_PREPARED_TOAST_CARDS.with(Cell::get)
}

pub(crate) fn with_toast_base_scene_replay<R>(body: impl FnOnce() -> R) -> R {
    REPLAY_TOAST_BASE_SCENES.with(|enabled| {
        let previous = enabled.replace(true);
        struct Reset<'a> {
            enabled: &'a Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.enabled.set(self.previous);
            }
        }
        let _reset = Reset { enabled, previous };
        body()
    })
}

fn replay_toast_base_scenes() -> bool {
    REPLAY_TOAST_BASE_SCENES.with(Cell::get) && {
        #[cfg(feature = "bench-support")]
        {
            !DISABLE_TOAST_BASE_SCENE_REPLAY.with(Cell::get)
        }
        #[cfg(not(feature = "bench-support"))]
        {
            true
        }
    }
}

#[cfg(feature = "bench-support")]
pub(crate) fn without_toast_base_scene_replay<R>(body: impl FnOnce() -> R) -> R {
    DISABLE_TOAST_BASE_SCENE_REPLAY.with(|disabled| {
        let previous = disabled.replace(true);
        struct Reset<'a> {
            disabled: &'a Cell<bool>,
            previous: bool,
        }
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.disabled.set(self.previous);
            }
        }
        let _reset = Reset { disabled, previous };
        body()
    })
}

#[cfg(feature = "bench-support")]
pub(crate) mod bench_profile {
    use std::cell::Cell;
    use std::time::{Duration, Instant};

    thread_local! {
        static MEASURE: Cell<Duration> = const { Cell::new(Duration::ZERO) };
        static COLLECT: Cell<Duration> = const { Cell::new(Duration::ZERO) };
        static COMPOSE: Cell<Duration> = const { Cell::new(Duration::ZERO) };
        static MEASURED_CARDS: Cell<usize> = const { Cell::new(0) };
        static COLLECTED_CARDS: Cell<usize> = const { Cell::new(0) };
        static LAYOUT_PASSES: Cell<usize> = const { Cell::new(0) };
        static BASE_SCENE_REPLAY_HITS: Cell<usize> = const { Cell::new(0) };
        static BASE_SCENE_REPLAY_FALLBACKS: Cell<usize> = const { Cell::new(0) };
        static FORCE_DOUBLE_LAYOUT: Cell<bool> = const { Cell::new(false) };
    }

    #[derive(Clone, Copy, Debug, Default)]
    pub(crate) struct Snapshot {
        pub measure: Duration,
        pub collect: Duration,
        pub compose: Duration,
        pub measured_cards: usize,
        pub collected_cards: usize,
        pub layout_passes: usize,
        pub base_scene_replay_hits: usize,
        pub base_scene_replay_fallbacks: usize,
    }

    pub(crate) fn reset() {
        MEASURE.with(|value| value.set(Duration::ZERO));
        COLLECT.with(|value| value.set(Duration::ZERO));
        COMPOSE.with(|value| value.set(Duration::ZERO));
        MEASURED_CARDS.with(|value| value.set(0));
        COLLECTED_CARDS.with(|value| value.set(0));
        LAYOUT_PASSES.with(|value| value.set(0));
        BASE_SCENE_REPLAY_HITS.with(|value| value.set(0));
        BASE_SCENE_REPLAY_FALLBACKS.with(|value| value.set(0));
    }

    pub(super) fn measure<R>(body: impl FnOnce() -> R) -> R {
        let started = Instant::now();
        let result = body();
        MEASURE.with(|value| value.set(value.get() + started.elapsed()));
        MEASURED_CARDS.with(|value| value.set(value.get() + 1));
        result
    }

    pub(super) fn collect<R>(body: impl FnOnce() -> R) -> R {
        let started = Instant::now();
        let result = body();
        COLLECT.with(|value| value.set(value.get() + started.elapsed()));
        COLLECTED_CARDS.with(|value| value.set(value.get() + 1));
        result
    }

    pub(super) fn compose<R>(body: impl FnOnce() -> R) -> R {
        let started = Instant::now();
        let result = body();
        COMPOSE.with(|value| value.set(value.get() + started.elapsed()));
        result
    }

    pub(super) fn record_layout_pass() {
        LAYOUT_PASSES.with(|value| value.set(value.get() + 1));
    }

    pub(super) fn record_base_scene_replay(hit: bool) {
        if hit {
            BASE_SCENE_REPLAY_HITS.with(|value| value.set(value.get() + 1));
        } else {
            BASE_SCENE_REPLAY_FALLBACKS.with(|value| value.set(value.get() + 1));
        }
    }

    pub(crate) fn snapshot() -> Snapshot {
        Snapshot {
            measure: MEASURE.with(Cell::get),
            collect: COLLECT.with(Cell::get),
            compose: COMPOSE.with(Cell::get),
            measured_cards: MEASURED_CARDS.with(Cell::get),
            collected_cards: COLLECTED_CARDS.with(Cell::get),
            layout_passes: LAYOUT_PASSES.with(Cell::get),
            base_scene_replay_hits: BASE_SCENE_REPLAY_HITS.with(Cell::get),
            base_scene_replay_fallbacks: BASE_SCENE_REPLAY_FALLBACKS.with(Cell::get),
        }
    }

    pub(crate) fn with_legacy_double_layout<R>(body: impl FnOnce() -> R) -> R {
        FORCE_DOUBLE_LAYOUT.with(|force| {
            let previous = force.replace(true);
            struct Reset<'a> {
                force: &'a Cell<bool>,
                previous: bool,
            }
            impl Drop for Reset<'_> {
                fn drop(&mut self) {
                    self.force.set(self.previous);
                }
            }
            let _reset = Reset { force, previous };
            body()
        })
    }

    pub(super) fn force_double_layout() -> bool {
        FORCE_DOUBLE_LAYOUT.with(Cell::get)
    }
}

#[cfg(feature = "bench-support")]
#[inline]
fn profile_measure<R>(body: impl FnOnce() -> R) -> R {
    bench_profile::measure(body)
}

#[cfg(not(feature = "bench-support"))]
#[inline(always)]
fn profile_measure<R>(body: impl FnOnce() -> R) -> R {
    body()
}

#[cfg(feature = "bench-support")]
#[inline]
fn profile_collect<R>(body: impl FnOnce() -> R) -> R {
    bench_profile::collect(body)
}

#[cfg(not(feature = "bench-support"))]
#[inline(always)]
fn profile_collect<R>(body: impl FnOnce() -> R) -> R {
    body()
}

#[cfg(feature = "bench-support")]
#[inline]
fn profile_compose<R>(body: impl FnOnce() -> R) -> R {
    bench_profile::compose(body)
}

#[cfg(not(feature = "bench-support"))]
#[inline(always)]
fn profile_compose<R>(body: impl FnOnce() -> R) -> R {
    body()
}

#[cfg(feature = "bench-support")]
#[inline]
fn record_layout_pass() {
    bench_profile::record_layout_pass();
}

#[cfg(not(feature = "bench-support"))]
#[inline(always)]
fn record_layout_pass() {}

#[inline]
fn can_reuse_expanded_layout(stack_width: Dp, expanded_width: Dp) -> bool {
    #[cfg(feature = "bench-support")]
    if bench_profile::force_double_layout() {
        return false;
    }
    stack_width == expanded_width
}

fn prepare_cached_toast_card<VM: 'static>(
    cache: &Arc<Mutex<ToastPreparedCardCache<VM>>>,
    queue: ToastQueue<VM>,
    entry: ToastEntry<VM>,
    style: ToastStyle,
    placement: ToastPlacement,
    card_width: Dp,
    context: &mut CollectContext<'_, '_>,
) -> Option<std::sync::Arc<PreparedToastCard<VM>>> {
    let toast_id = entry.id;
    let width_bits = card_width.get().to_bits();
    let previous_resolved = if let Ok(cache) = cache.lock() {
        if let Some(cached) = cache.cards.get(&toast_id) {
            if reuse_prepared_toast_cards() && cached.width_bits == width_bits {
                return Some(Arc::clone(&cached.prepared));
            }
            Some(Arc::clone(&cached.prepared.resolved))
        } else {
            None
        }
    } else {
        None
    };

    let prepared = Arc::new(prepare_toast_card_layout(
        queue,
        entry,
        style,
        placement,
        card_width,
        previous_resolved.as_deref(),
        context,
    )?);
    if let Ok(mut cache) = cache.lock() {
        cache.cards.insert(
            toast_id,
            CachedPreparedToastCard {
                width_bits,
                prepared: Arc::clone(&prepared),
            },
        );
    }
    Some(prepared)
}

impl<VM: 'static> ResolvedElement<VM> {
    pub(super) fn emit_toast_overlay_if_visible(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        _visual: &CollectVisualState,
    ) {
        let super::super::types::ResolvedWidgetKind::ToastHost {
            queue,
            placement,
            max_visible,
            style,
            prepared_cards,
        } = &self.kind
        else {
            return;
        };

        let now = context.now;
        let enter_transition = context.style_context.motion_slow_transition();
        let exit_transition = context.style_context.motion_normal_transition();
        let exit_duration = exit_transition
            .map(Transition::duration)
            .unwrap_or(Duration::ZERO);
        let _ = queue.flush_expired_after(now, exit_duration);

        let mut entries = queue.snapshot();
        if let Some(deadline) = earliest_upcoming_deadline(&entries, now) {
            let merged = match context.next_toast_wakeup.get() {
                Some(current) => Some(current.min(deadline)),
                None => Some(deadline),
            };
            context.next_toast_wakeup.set(merged);
        }

        // Manual card motion uses the theme transitions' durations and curves.
        // It never requests frames when motion is reduced or duration is zero.
        let has_animating =
            entries_have_animation(&entries, now, enter_transition, exit_transition);
        if has_animating {
            request_next_toast_frame(context, now);
        }

        if entries.is_empty() {
            queue.set_stack_expanded(false);
            if let Ok(mut cache) = prepared_cards.lock() {
                cache.cards.clear();
            }
            return;
        }
        if let Some(limit) = *max_visible {
            if entries.len() > limit {
                entries = entries.split_off(entries.len() - limit);
            }
        }
        if entries.is_empty() {
            queue.set_stack_expanded(false);
            if let Ok(mut cache) = prepared_cards.lock() {
                cache.cards.clear();
            }
            return;
        }
        let auto_collapsible = entries.len() > TOAST_AUTO_COLLAPSE_THRESHOLD;
        if !auto_collapsible {
            queue.set_stack_expanded(false);
        }

        let resolved_placement = default_placement(*placement);
        let stack_hover_widget_id = WidgetId::from_raw(self.id.raw() ^ TOAST_STACK_HOVER_TAG);
        let Some((content_scene, content_size)) = build_toast_scene(
            queue.clone(),
            entries,
            style.clone(),
            prepared_cards,
            resolved_placement,
            auto_collapsible,
            stack_hover_widget_id,
            context,
        ) else {
            return;
        };

        computed
            .dependencies
            .merge_from(&content_scene.dependencies);
        let overlay_id = OverlayId::new(self.id.raw() ^ TOAST_OVERLAY_TAG);
        let overlay = Overlay::<VM>::new(overlay_id, Anchor::Rect(context.viewport))
            .source_widget(self.id)
            .placement(map_overlay_placement(resolved_placement))
            .offset(Dp::ZERO)
            .viewport_padding(style.margin)
            .layer(OverlayLayer::Toast);

        let _ = emit_overlay(
            computed,
            context.viewport,
            overlay,
            content_size,
            OverlayContent::Scene(Box::new(content_scene)),
        );
    }
}

fn build_toast_scene<VM: 'static>(
    queue: ToastQueue<VM>,
    entries: Vec<ToastEntry<VM>>,
    style: ToastStyle,
    prepared_cards: &Arc<Mutex<ToastPreparedCardCache<VM>>>,
    placement: ToastPlacement,
    auto_collapsible: bool,
    stack_hover_widget_id: WidgetId,
    context: &mut CollectContext<'_, '_>,
) -> Option<(ComputedScene<VM>, (Dp, Dp))> {
    let now = context.now;
    let (result, dependencies): (Option<(ComputedScene<VM>, (Dp, Dp))>, DependencyGraph) =
        crate::foundation::binding::with_dependency_collection(|| {
            super::super::tree::with_widget_stack(|| {
                let width = toast_width(&style, context.viewport);
                let stack_target = if auto_collapsible && queue.stack_expanded() {
                    1.0
                } else {
                    0.0
                };
                let stack_progress = if auto_collapsible {
                    context.animations.resolve_f32(
                        AnimationKey::Widget {
                            id: stack_hover_widget_id.raw(),
                            property: WidgetProperty::ToastStackExpand,
                        },
                        stack_target,
                        context.style_context.motion_normal_transition(),
                        now,
                    )
                } else {
                    1.0
                };
                if auto_collapsible && (stack_progress - stack_target).abs() > 0.001 {
                    request_next_toast_frame(context, now);
                }

                let rendered_entries = ordered_entries(entries);
                let rendered_len = rendered_entries.len();
                let enter_transition = context.style_context.motion_slow_transition();
                let exit_transition = context.style_context.motion_normal_transition();

                let mut combined = ComputedScene::default();
                let mut cards = Vec::with_capacity(rendered_len);
                let mut expanded_y = Dp::ZERO;
                let mut occupied_height = Dp::ZERO;
                let mut occupied_count = 0.0_f32;

                for (index, entry) in rendered_entries.iter().enumerate() {
                    let expanded_card = profile_measure(|| {
                        prepare_cached_toast_card(
                            prepared_cards,
                            queue.clone(),
                            entry.clone(),
                            style.clone(),
                            placement,
                            width,
                            context,
                        )
                    })?;
                    let expanded_size = expanded_card.size;
                    let motion = calculate_animation_progress(
                        entry,
                        placement,
                        now,
                        enter_transition,
                        exit_transition,
                    );
                    let collapsed = collapsed_stack_frame(index, width);
                    let expanded = ToastStackFrame {
                        x: Dp::ZERO,
                        y: expanded_y,
                        width,
                        opacity: 1.0,
                    };
                    let stack_frame = if auto_collapsible {
                        interpolate_stack_frame(collapsed, expanded, stack_progress)
                    } else {
                        expanded
                    };
                    let origin = Point::new(
                        stack_frame.x + motion.offset_x,
                        stack_frame.y + motion.offset_y,
                    );
                    let content_reveal = if auto_collapsible && index > 0 {
                        stack_progress
                    } else {
                        1.0
                    };
                    let card_opacity = motion.opacity * content_reveal;

                    let reuse_expanded_layout = can_reuse_expanded_layout(stack_frame.width, width);
                    let (card_scene, card_size) = if reuse_expanded_layout {
                        profile_collect(|| {
                            let card_size = expanded_card.size;
                            let scene = collect_prepared_toast_card_scene(
                                &expanded_card,
                                origin,
                                card_opacity,
                                context,
                            );
                            (scene, card_size)
                        })
                    } else {
                        // Stack expansion changes the available width. The expanded flow size is
                        // still measured at the final width, while the visible card must lay out
                        // at the interpolated width. Keep the original second-layout path here;
                        // reusing the full-width tree would alter wrapping and hit geometry.
                        profile_collect(|| {
                            collect_toast_card_scene(
                                queue.clone(),
                                entry.clone(),
                                style.clone(),
                                placement,
                                stack_frame.width,
                                Some(expanded_card.resolved.as_ref()),
                                origin,
                                card_opacity,
                                context,
                            )
                        })?
                    };

                    let all_cards_interactive = !auto_collapsible || stack_progress >= 0.98;
                    let logically_open = entry.deadline.is_none_or(|deadline| deadline > now);
                    let shell_scene = if auto_collapsible
                        && stack_progress < 0.999
                        && (1..=TOAST_STACK_VISIBLE_BACK_LAYERS).contains(&index)
                    {
                        let shell_opacity = stack_frame.opacity * (1.0 - stack_progress);
                        Some(collect_toast_card_shell_scene(
                            style.clone(),
                            stack_frame.width,
                            expanded_size.1,
                            origin,
                            shell_opacity,
                            context,
                        )?)
                    } else {
                        None
                    };
                    cards.push(ToastCardRender {
                        scene: card_scene,
                        shell_scene,
                        scene_visible: card_opacity > 0.001 || index == 0,
                        size: card_size,
                        interactive: logically_open && (all_cards_interactive || index == 0),
                    });

                    // Each card owns a fractional flow slot while entering/exiting. Accumulating
                    // those slots moves only scene origins; card layout remains unchanged. The
                    // same absolute-time sample drives opacity and flow, so a mid-enter dismiss
                    // reverses without either the card or its siblings jumping.
                    expanded_y += (expanded_size.1 + style.stack_gap) * motion.flow_occupancy;
                    occupied_height += expanded_size.1 * motion.flow_occupancy;
                    occupied_count += motion.flow_occupancy;
                }

                if let Ok(mut cache) = prepared_cards.lock() {
                    cache.cards.retain(|toast_id, _| {
                        rendered_entries.iter().any(|entry| entry.id == *toast_id)
                    });
                }

                let collapsed_height = collapsed_stack_height(&cards);
                let expanded_height =
                    occupied_height + style.stack_gap * (occupied_count - 1.0).max(0.0);
                let total_height = if auto_collapsible {
                    interpolate_dp(collapsed_height, expanded_height, stack_progress)
                } else {
                    expanded_height
                };
                let draw_back_to_front = auto_collapsible && stack_progress < 0.999;
                profile_compose(|| {
                    if draw_back_to_front {
                        for index in (0..cards.len()).rev() {
                            extend_toast_card_scene(&mut combined, &cards[index]);
                        }
                    } else {
                        for card in cards.iter() {
                            extend_toast_card_scene(&mut combined, card);
                        }
                    }
                });

                let size = (width, total_height);
                if auto_collapsible {
                    push_toast_stack_hover_region(
                        &mut combined,
                        queue.clone(),
                        stack_hover_widget_id,
                        size,
                    );
                }
                Some((combined, size))
            })
        });
    let (mut computed, size) = result?;
    computed.dependencies = dependencies.clone();
    Some((computed, size))
}

struct ToastCardRender<VM> {
    scene: ComputedScene<VM>,
    shell_scene: Option<ComputedScene<VM>>,
    scene_visible: bool,
    size: (Dp, Dp),
    interactive: bool,
}

#[derive(Clone, Copy)]
struct ToastStackFrame {
    x: Dp,
    y: Dp,
    width: Dp,
    opacity: f32,
}

#[derive(Clone, Copy)]
struct ToastMotionSample {
    opacity: f32,
    offset_x: Dp,
    offset_y: Dp,
    /// Fraction of this card's vertical flow slot occupied by the current lifecycle frame.
    flow_occupancy: f32,
}

pub(crate) struct ToastPreparedCardCache<VM> {
    cards: HashMap<ToastId, CachedPreparedToastCard<VM>>,
}

impl<VM> Default for ToastPreparedCardCache<VM> {
    fn default() -> Self {
        Self {
            cards: HashMap::new(),
        }
    }
}

struct CachedPreparedToastCard<VM> {
    width_bits: u32,
    prepared: Arc<PreparedToastCard<VM>>,
}

struct PreparedToastCard<VM> {
    resolved: Arc<ResolvedElement<VM>>,
    taffy: TaffyTree<MeasureContext>,
    layout_root: LayoutNode,
    size: (Dp, Dp),
    base_scene: OnceLock<Option<ToastBaseScene<VM>>>,
    shadow: crate::theme::Shadow,
    border_width: Dp,
    border_radius: Dp,
    replay_style_supported: bool,
}

struct ToastBaseScene<VM> {
    computed: ComputedScene<VM>,
    outer_clip: Rect,
    shadow_texture_id: Option<u64>,
    shadow_frame: Rect,
    shadow_radius: f32,
}

fn prepare_toast_card_layout<VM: 'static>(
    queue: ToastQueue<VM>,
    entry: ToastEntry<VM>,
    style: ToastStyle,
    placement: ToastPlacement,
    card_width: Dp,
    previous: Option<&ResolvedElement<VM>>,
    context: &mut CollectContext<'_, '_>,
) -> Option<PreparedToastCard<VM>> {
    record_layout_pass();
    let shadow = style.shadow.clone();
    let border_width = style.border_width.resolve();
    let border_radius = style.radius.resolve();
    let action_surface = &style.action_button.surface;
    let replay_style_supported = style.close_button.surface.shadow.is_none()
        && (entry.toast.action.is_none()
            || (action_surface.shadow.is_none()
                && matches!(&action_surface.opacity, Value::Static(value) if (*value - 1.0).abs() <= f32::EPSILON)
                && matches!(&action_surface.offset, Value::Static(value) if *value == Point::ZERO)));
    let root = toast_card_root(queue, entry, style, placement, card_width);
    let mut resolved: Element<VM> = root.into();
    super::prepare_nested_scene_root(&mut resolved, context, context.viewport);
    let resolved = Arc::new(resolved.resolve_with_previous(context.theme, previous));
    let mut taffy = TaffyTree::new();
    let layout_root = resolved
        .build_layout_tree(
            &mut taffy,
            context.animations,
            context.theme,
            context.units,
            None,
            context.viewport,
            false,
            context.now,
        )
        .ok()?;
    compute_taffy_layout_with_measure(
        &mut taffy,
        layout_root.node,
        context.viewport,
        context.font_manager,
        context.theme,
        context.media,
        context.units,
    )
    .ok()?;
    let layout = taffy.layout(layout_root.node).ok()?;
    let size = (Dp::new(layout.size.width), Dp::new(layout.size.height));
    Some(PreparedToastCard {
        resolved,
        taffy,
        layout_root,
        size,
        base_scene: OnceLock::new(),
        shadow,
        border_width,
        border_radius,
        replay_style_supported,
    })
}

fn collect_toast_card_scene<VM: 'static>(
    queue: ToastQueue<VM>,
    entry: ToastEntry<VM>,
    style: ToastStyle,
    placement: ToastPlacement,
    card_width: Dp,
    previous: Option<&ResolvedElement<VM>>,
    origin: Point,
    opacity: f32,
    context: &mut CollectContext<'_, '_>,
) -> Option<(ComputedScene<VM>, (Dp, Dp))> {
    let prepared = prepare_toast_card_layout(
        queue, entry, style, placement, card_width, previous, context,
    )?;
    let card_size = prepared.size;
    let scene = collect_prepared_toast_card_scene(&prepared, origin, opacity, context);
    Some((scene, card_size))
}

fn collect_prepared_toast_card_scene<VM: 'static>(
    prepared: &PreparedToastCard<VM>,
    origin: Point,
    opacity: f32,
    context: &mut CollectContext<'_, '_>,
) -> ComputedScene<VM> {
    if replay_toast_base_scenes() {
        let base = prepared
            .base_scene
            .get_or_init(|| prepare_toast_base_scene(prepared, context));
        if let Some(base) = base.as_ref() {
            if let Some(replayed) =
                replay_toast_base_scene(prepared, base, origin, opacity, context)
            {
                #[cfg(feature = "bench-support")]
                bench_profile::record_base_scene_replay(true);
                return replayed;
            }
        }
        #[cfg(feature = "bench-support")]
        bench_profile::record_base_scene_replay(false);
    }

    collect_prepared_toast_card_scene_uncached(prepared, origin, opacity, context)
}

fn collect_prepared_toast_card_scene_uncached<VM: 'static>(
    prepared: &PreparedToastCard<VM>,
    origin: Point,
    opacity: f32,
    context: &mut CollectContext<'_, '_>,
) -> ComputedScene<VM> {
    let mut lifecycle_states = std::collections::HashMap::new();
    let mut chunks = std::collections::HashMap::new();
    let mut chunk_parts = std::collections::HashMap::new();
    let mut visual_contexts = std::collections::HashMap::new();
    let mut accessibility_geometry = Vec::new();
    let mut local_context = CollectContext {
        taffy: &prepared.taffy,
        font_manager: context.font_manager,
        theme: context.theme,
        style_context: context.style_context,
        style_sheet: context.style_sheet,
        media: context.media,
        focused_input: context.focused_input,
        focused_text_state: context.focused_text_state,
        focused_text_value: context.focused_text_value,
        focused_text_layout: context.focused_text_layout,
        text_layout_overrides: context.text_layout_overrides,
        active_slider_value: context.active_slider_value,
        caret_visible: context.caret_visible,
        selected_text: context.selected_text,
        selected_text_state: context.selected_text_state,
        hovered_scrollbar: context.hovered_scrollbar,
        active_scrollbar: context.active_scrollbar,
        widget_states: context.widget_states,
        select_open_states: context.select_open_states,
        menu_open_states: context.menu_open_states,
        menubar_active_states: context.menubar_active_states,
        context_menu_anchor_states: context.context_menu_anchor_states,
        scroll_offsets: context.scroll_offsets,
        virtual_states: context.virtual_states,
        viewport: context.viewport,
        units: context.units,
        animations: context.animations,
        reduced_motion: context.reduced_motion,
        now: context.now,
        frame_clock: context.frame_clock,
        focus: Default::default(),
        tooltip_hover_started_at: context.tooltip_hover_started_at,
        next_tooltip_wakeup: context.next_tooltip_wakeup,
        next_toast_wakeup: context.next_toast_wakeup,
        active_tooltip: context.active_tooltip,
        active_hover_popover: context.active_hover_popover,
        gpu_scroll_enabled: false,
        gpu_scroll_container: None,
        transform_stack: context.transform_stack.clone(),
        portal_accessibility_geometry: Some(&mut accessibility_geometry),
        portal_accessibility_path: smallvec::SmallVec::new(),
    };

    let root_id = prepared.resolved.collect_subtree_cache(
        &prepared.layout_root,
        VisualContext {
            origin,
            opacity,
            clip_rect: toast_scene_clip_rect(context.viewport),
            overflow_clip_rect: None,
            clip_mask: None,
        },
        &mut local_context,
        &mut lifecycle_states,
        &mut chunks,
        &mut chunk_parts,
        &mut visual_contexts,
    );
    drop(local_context);
    let mut computed = chunks.get(&root_id).cloned().unwrap_or_default();
    if let Some(fragment) = super::portal::collect_accessibility_fragment(
        Arc::clone(&prepared.resolved),
        &prepared.layout_root,
        &accessibility_geometry,
        &computed.hit_regions,
        &computed.scroll_regions,
    ) {
        computed.accessibility_fragments.push(fragment);
    }
    computed
}

fn prepare_toast_base_scene<VM: 'static>(
    prepared: &PreparedToastCard<VM>,
    context: &mut CollectContext<'_, '_>,
) -> Option<ToastBaseScene<VM>> {
    if !prepared.replay_style_supported {
        return None;
    }
    let outer_clip = toast_scene_clip_rect(context.viewport);
    let computed = collect_prepared_toast_card_scene_uncached(prepared, Point::ZERO, 1.0, context);
    let border_width = prepared
        .border_width
        .get()
        .min((prepared.size.0 * 0.5).get())
        .min((prepared.size.1 * 0.5).get())
        .max(0.0);
    let shadow_frame = Rect::new(Dp::ZERO, Dp::ZERO, prepared.size.0, prepared.size.1)
        .inset(crate::ui::layout::Insets::all(Dp::new(border_width)));
    let shadow_radius = (prepared.border_radius.get() - border_width).max(0.0);
    let expected_shadow = super::super::super::rounded_rect_shadow_texture(
        shadow_frame,
        shadow_radius,
        super::super::super::RoundedRectShadowSpec {
            shadow: prepared.shadow.clone(),
            opacity: 1.0,
            clip_rect: None,
            clip_mask: None,
        },
        context.media,
        context.units,
    );
    let shadow_texture_id = expected_shadow.as_ref().map(|texture| texture.texture.id());
    if !toast_base_scene_is_replayable(&computed, outer_clip, shadow_texture_id) {
        return None;
    }
    Some(ToastBaseScene {
        computed,
        outer_clip,
        shadow_texture_id,
        shadow_frame,
        shadow_radius,
    })
}

fn toast_base_scene_is_replayable<VM>(
    computed: &ComputedScene<VM>,
    _outer_clip: Rect,
    shadow_texture_id: Option<u64>,
) -> bool {
    let scene = &computed.scene;
    if !scene.backdrop_blurs.is_empty()
        || !scene.brushes.is_empty()
        || !scene.canvas_composites.is_empty()
        || !scene.meshes.is_empty()
        || !scene.overlay_shapes.is_empty()
        || !scene.overlay_textures.is_empty()
        || !scene.overlay_meshes.is_empty()
        || !scene.overlay_texts.is_empty()
        || !scene.overlay_text_decorations.is_empty()
        || !scene.overlay_commands.is_empty()
        || !scene.overlay_command_sources.is_empty()
        || !scene.dirty_draw_ranges().is_empty()
        || scene.cache_liveness_dirty()
        || scene
            .command_gpu_scroll_containers()
            .iter()
            .any(Option::is_some)
        || scene
            .command_transform_chains()
            .iter()
            .any(|chain| !chain.is_empty())
    {
        return false;
    }
    #[cfg(feature = "video")]
    if !scene.video_textures.is_empty() {
        return false;
    }
    if !computed.overlay_hit_regions.is_empty()
        || !computed.overlay_close_handlers.is_empty()
        || computed.portal_overlay_counts.shapes != 0
        || computed.portal_overlay_counts.textures != 0
        || computed.portal_overlay_counts.meshes != 0
        || computed.portal_overlay_counts.texts != 0
        || computed.portal_overlay_counts.text_decorations != 0
        || computed.portal_overlay_counts.commands != 0
        || computed.portal_overlay_counts.hits != 0
        || computed.portal_overlay_counts.close_handlers != 0
        || computed.portal_overlay_counts.focus_scopes != 0
        || computed.portal_overlay_counts.accessibility_fragments != 0
        || !computed.focus_scopes.is_empty()
        || !computed.carousel_auto_play.is_empty()
        || !computed.overlay_anchors.is_empty()
        || !computed.portal_entries.is_empty()
        || !computed.external_portal_requests.is_empty()
        || computed.overlay_layers.iter().any(|layer| {
            !layer.commands.is_empty()
                || !layer.command_sources.is_empty()
                || !layer.backdrop_blurs.is_empty()
                || !layer.shapes.is_empty()
                || !layer.textures.is_empty()
                || !layer.meshes.is_empty()
                || !layer.texts.is_empty()
                || !layer.text_decorations.is_empty()
                || !layer.hits.is_empty()
                || !layer.close_handlers.is_empty()
                || !layer.focus_scopes.is_empty()
                || !layer.accessibility_fragments.is_empty()
        })
        || !computed.overlay_layer_graph.layers.is_empty()
        || !computed.overlay_layer_graph.anchor_slots.is_empty()
        || computed.ime_cursor_area.is_some()
        || !computed.virtual_state_updates.is_empty()
        || !computed.transform_records.is_empty()
    {
        return false;
    }

    let mut matched_shadow = 0_usize;
    for command in &scene.commands {
        match command {
            RenderCommand::Shape(primitive) => {
                if primitive.color.a == 0 {
                    return false;
                }
            }
            RenderCommand::Text(primitive) => {
                if primitive.rich_spans.is_some() || primitive.quad.is_some() {
                    return false;
                }
            }
            RenderCommand::Texture(primitive) => {
                if shadow_texture_id == Some(primitive.texture.id()) {
                    matched_shadow += 1;
                    continue;
                }
                if primitive.media_key.is_some()
                    || primitive.media_layout.is_some()
                    || primitive.mask_tint.is_some()
                    || primitive.quad.is_some()
                    || primitive.uv_rect.is_some()
                    || primitive.clip_mask.is_none()
                    || primitive.corner_radius != 0.0
                    || primitive.opacity != 1.0
                {
                    return false;
                }
                let Some(mask) = primitive.clip_mask else {
                    return false;
                };
                if primitive.frame.x < mask.rect.x
                    || primitive.frame.y < mask.rect.y
                    || primitive.frame.right() > mask.rect.right()
                    || primitive.frame.bottom() > mask.rect.bottom()
                {
                    return false;
                }
            }
            RenderCommand::TextDecoration(primitive) => {
                if primitive.segments.is_empty() {
                    return false;
                }
            }
            RenderCommand::BackdropBlur(_)
            | RenderCommand::Brush(_)
            | RenderCommand::CanvasComposite(_)
            | RenderCommand::Mesh(_) => return false,
            #[cfg(feature = "video")]
            RenderCommand::VideoTexture(_) => return false,
        }
    }
    if matched_shadow != usize::from(shadow_texture_id.is_some()) {
        return false;
    }
    computed.hit_regions.iter().all(|hit| {
        matches!(hit.geometry, HitGeometry::Rect)
            && hit.transform_chain.is_empty()
            && hit.gpu_scroll_container.is_none()
    })
}

fn replay_toast_base_scene<VM: 'static>(
    prepared: &PreparedToastCard<VM>,
    base: &ToastBaseScene<VM>,
    origin: Point,
    opacity: f32,
    context: &mut CollectContext<'_, '_>,
) -> Option<ComputedScene<VM>> {
    let opacity = opacity.clamp(0.0, 1.0);
    let mut computed = ComputedScene::default();
    for command in base.computed.scene.commands.iter().cloned() {
        let command = match command {
            RenderCommand::Shape(mut primitive) => {
                primitive.rect = translate_toast_rect(primitive.rect, origin);
                primitive.color = primitive.color.with_alpha_factor(opacity);
                primitive.clip_rect =
                    translate_toast_clip(primitive.clip_rect, base.outer_clip, origin);
                primitive.clip_mask = translate_toast_mask(primitive.clip_mask, origin);
                if primitive.color.a == 0 {
                    continue;
                }
                RenderCommand::Shape(primitive)
            }
            RenderCommand::Text(mut primitive) => {
                primitive.frame = translate_toast_rect(primitive.frame, origin);
                primitive.color = primitive.color.with_alpha_factor(opacity);
                primitive.clip_rect =
                    translate_toast_clip(primitive.clip_rect, base.outer_clip, origin);
                primitive.clip_mask = translate_toast_mask(primitive.clip_mask, origin);
                RenderCommand::Text(primitive)
            }
            RenderCommand::TextDecoration(mut primitive) => {
                primitive.segments = Arc::from(
                    primitive
                        .segments
                        .iter()
                        .copied()
                        .map(|rect| translate_toast_rect(rect, origin))
                        .collect::<Vec<_>>(),
                );
                primitive.color = primitive.color.with_alpha_factor(opacity);
                primitive.clip_rect =
                    translate_toast_clip(primitive.clip_rect, base.outer_clip, origin);
                primitive.clip_mask = translate_toast_mask(primitive.clip_mask, origin);
                RenderCommand::TextDecoration(primitive)
            }
            RenderCommand::Texture(mut primitive) => {
                if base.shadow_texture_id == Some(primitive.texture.id()) {
                    let shadow = super::super::super::rounded_rect_shadow_texture(
                        translate_toast_rect(base.shadow_frame, origin),
                        base.shadow_radius,
                        super::super::super::RoundedRectShadowSpec {
                            shadow: prepared.shadow.clone(),
                            opacity,
                            clip_rect: translate_toast_clip(
                                primitive.clip_rect,
                                base.outer_clip,
                                origin,
                            ),
                            clip_mask: translate_toast_mask(primitive.clip_mask, origin),
                        },
                        context.media,
                        context.units,
                    );
                    let Some(shadow) = shadow else {
                        continue;
                    };
                    RenderCommand::Texture(shadow)
                } else {
                    if opacity <= 0.0 {
                        continue;
                    }
                    primitive.frame = translate_toast_rect(primitive.frame, origin);
                    primitive.opacity = opacity;
                    primitive.clip_rect =
                        translate_toast_clip(primitive.clip_rect, base.outer_clip, origin);
                    primitive.clip_mask = translate_toast_mask(primitive.clip_mask, origin);
                    RenderCommand::Texture(primitive)
                }
            }
            RenderCommand::BackdropBlur(_)
            | RenderCommand::Brush(_)
            | RenderCommand::CanvasComposite(_)
            | RenderCommand::Mesh(_) => return None,
            #[cfg(feature = "video")]
            RenderCommand::VideoTexture(_) => return None,
        };
        computed.scene.push_render_command(command);
    }

    for mut hit in base.computed.hit_regions.iter().cloned() {
        hit.rect = translate_toast_rect(hit.rect, origin);
        hit.clip_rect = translate_toast_clip(hit.clip_rect, base.outer_clip, origin);
        hit.interaction = hit.interaction.translated(origin);
        computed.hit_regions.push(hit);
    }
    computed.scroll_regions.extend(
        base.computed
            .scroll_regions
            .iter()
            .copied()
            .map(|region| translate_toast_scroll_region(region, origin)),
    );
    computed.accessibility_fragments.extend(
        base.computed
            .accessibility_fragments
            .iter()
            .cloned()
            .map(|fragment| {
                translate_toast_accessibility_fragment(fragment, base.outer_clip, origin)
            }),
    );
    computed.dependencies = base.computed.dependencies.clone();
    Some(computed)
}

fn translate_toast_accessibility_fragment<VM>(
    mut fragment: crate::ui::widget::AccessibilityFragment<VM>,
    outer_clip: Rect,
    delta: Point,
) -> crate::ui::widget::AccessibilityFragment<VM> {
    fragment.clip_rect = translate_toast_clip(fragment.clip_rect, outer_clip, delta);
    for node in &mut fragment.nodes {
        node.bounds = translate_toast_rect(node.bounds, delta);
        node.clip_rect = translate_toast_clip(node.clip_rect, outer_clip, delta);
        for hit in &mut node.hits {
            hit.rect = translate_toast_rect(hit.rect, delta);
            hit.clip_rect = translate_toast_clip(hit.clip_rect, outer_clip, delta);
            hit.interaction = hit.interaction.clone().translated(delta);
        }
        for region in &mut node.scroll_regions {
            *region = translate_toast_scroll_region(*region, delta);
        }
    }
    fragment
}

fn translate_toast_rect(mut rect: Rect, delta: Point) -> Rect {
    rect.x += delta.x;
    rect.y += delta.y;
    rect
}

fn translate_toast_clip(clip: Option<Rect>, outer_clip: Rect, delta: Point) -> Option<Rect> {
    clip.map(|rect| {
        if rect == outer_clip {
            rect
        } else {
            translate_toast_derived_rect(rect, delta)
        }
    })
}

fn translate_toast_derived_rect(rect: Rect, delta: Point) -> Rect {
    let x = rect.x + delta.x;
    let y = rect.y + delta.y;
    let right = x + rect.width;
    let bottom = y + rect.height;
    Rect::new(x, y, right - x, bottom - y)
}

fn translate_toast_mask(mask: Option<ClipMask>, delta: Point) -> Option<ClipMask> {
    mask.map(|mask| ClipMask {
        rect: translate_toast_rect(mask.rect, delta),
        corner_radius: mask.corner_radius,
    })
}

fn translate_toast_scroll_region(region: ScrollRegion, delta: Point) -> ScrollRegion {
    ScrollRegion {
        id: region.id,
        content_viewport: translate_toast_rect(region.content_viewport, delta),
        visible_frame: translate_toast_derived_rect(region.visible_frame, delta),
        content_bounds: translate_toast_rect(region.content_bounds, delta),
        gpu_base_scroll_offset: region.gpu_base_scroll_offset,
        scroll_offset: region.scroll_offset,
        overflow_x: region.overflow_x,
        overflow_y: region.overflow_y,
        horizontal_track: region
            .horizontal_track
            .map(|rect| translate_toast_derived_rect(rect, delta)),
        horizontal_thumb: region
            .horizontal_thumb
            .map(|rect| translate_toast_derived_rect(rect, delta)),
        vertical_track: region
            .vertical_track
            .map(|rect| translate_toast_derived_rect(rect, delta)),
        vertical_thumb: region
            .vertical_thumb
            .map(|rect| translate_toast_derived_rect(rect, delta)),
    }
}

fn toast_card_root<VM: 'static>(
    queue: ToastQueue<VM>,
    entry: ToastEntry<VM>,
    style: ToastStyle,
    placement: ToastPlacement,
    card_width: Dp,
) -> Flex<VM> {
    Flex::<VM>::new(Axis::Vertical)
        .width(card_width)
        .align(match placement {
            ToastPlacement::TopCenter | ToastPlacement::BottomCenter => {
                crate::ui::layout::Align::Center
            }
            ToastPlacement::TopEnd | ToastPlacement::BottomEnd => crate::ui::layout::Align::End,
            _ => crate::ui::layout::Align::Start,
        })
        .justify(Justify::Start)
        .child(build_toast_card(queue, entry, style, card_width))
}

fn collect_toast_card_shell_scene<VM: 'static>(
    style: ToastStyle,
    width: Dp,
    height: Dp,
    origin: Point,
    opacity: f32,
    context: &mut CollectContext<'_, '_>,
) -> Option<ComputedScene<VM>> {
    let background = style.background.resolve();
    let shell = Stack::<VM>::new()
        .size(width, height)
        .style_full(move |context| {
            let mut container = ContainerStyle::default_for_theme(context.theme);
            container.surface.background = Some(Value::Static(background));
            container.surface.border_color = Some(style.border.clone());
            container.surface.border_width = Some(style.border_width.clone());
            container.surface.border_radius = Some(style.radius.clone());
            container.surface.shadow = Some(Value::Static(style.shadow.clone()));
            container
        });

    let mut resolved: Element<VM> = shell.into();
    super::prepare_nested_scene_root(&mut resolved, context, context.viewport);
    let resolved = resolved.resolve(context.theme);
    let mut taffy = TaffyTree::new();
    let layout_root = resolved
        .build_layout_tree(
            &mut taffy,
            context.animations,
            context.theme,
            context.units,
            None,
            context.viewport,
            false,
            context.now,
        )
        .ok()?;
    compute_taffy_layout_with_measure(
        &mut taffy,
        layout_root.node,
        context.viewport,
        context.font_manager,
        context.theme,
        context.media,
        context.units,
    )
    .ok()?;

    let mut lifecycle_states = std::collections::HashMap::new();
    let mut chunks = std::collections::HashMap::new();
    let mut chunk_parts = std::collections::HashMap::new();
    let mut visual_contexts = std::collections::HashMap::new();
    let mut local_context = CollectContext {
        taffy: &taffy,
        font_manager: context.font_manager,
        theme: context.theme,
        style_context: context.style_context,
        style_sheet: context.style_sheet,
        media: context.media,
        focused_input: context.focused_input,
        focused_text_state: context.focused_text_state,
        focused_text_value: context.focused_text_value,
        focused_text_layout: context.focused_text_layout,
        text_layout_overrides: context.text_layout_overrides,
        active_slider_value: context.active_slider_value,
        caret_visible: context.caret_visible,
        selected_text: context.selected_text,
        selected_text_state: context.selected_text_state,
        hovered_scrollbar: context.hovered_scrollbar,
        active_scrollbar: context.active_scrollbar,
        widget_states: context.widget_states,
        select_open_states: context.select_open_states,
        menu_open_states: context.menu_open_states,
        menubar_active_states: context.menubar_active_states,
        context_menu_anchor_states: context.context_menu_anchor_states,
        scroll_offsets: context.scroll_offsets,
        virtual_states: context.virtual_states,
        viewport: context.viewport,
        units: context.units,
        animations: context.animations,
        reduced_motion: context.reduced_motion,
        now: context.now,
        frame_clock: context.frame_clock,
        focus: Default::default(),
        tooltip_hover_started_at: context.tooltip_hover_started_at,
        next_tooltip_wakeup: context.next_tooltip_wakeup,
        next_toast_wakeup: context.next_toast_wakeup,
        active_tooltip: context.active_tooltip,
        active_hover_popover: context.active_hover_popover,
        gpu_scroll_enabled: false,
        gpu_scroll_container: None,
        transform_stack: context.transform_stack.clone(),
        portal_accessibility_geometry: None,
        portal_accessibility_path: smallvec::SmallVec::new(),
    };
    let root_id = resolved.collect_subtree_cache(
        &layout_root,
        VisualContext {
            origin,
            opacity,
            clip_rect: toast_scene_clip_rect(context.viewport),
            overflow_clip_rect: None,
            clip_mask: None,
        },
        &mut local_context,
        &mut lifecycle_states,
        &mut chunks,
        &mut chunk_parts,
        &mut visual_contexts,
    );
    Some(chunks.get(&root_id).cloned().unwrap_or_default())
}

fn extend_toast_card_scene<VM>(combined: &mut ComputedScene<VM>, card: &ToastCardRender<VM>) {
    if let Some(shell_scene) = card.shell_scene.as_ref() {
        combined.extend(shell_scene);
    }
    if !card.scene_visible {
        combined.dependencies.merge_from(&card.scene.dependencies);
        return;
    }
    if card.interactive {
        combined.extend(&card.scene);
        return;
    }

    let mut visual_only = card.scene.clone();
    visual_only.hit_regions.clear();
    visual_only.overlay_hit_regions.clear();
    visual_only.overlay_close_handlers.clear();
    visual_only.focus_scopes.clear();
    visual_only.accessibility_fragments.clear();
    combined.extend(&visual_only);
}

fn collapsed_stack_frame(index: usize, width: Dp) -> ToastStackFrame {
    let layer = index.min(TOAST_STACK_VISIBLE_BACK_LAYERS);
    let layer_factor = layer as f32;
    let hidden = index > TOAST_STACK_VISIBLE_BACK_LAYERS;
    let inset = TOAST_STACK_LAYER_INSET_X * layer_factor;
    ToastStackFrame {
        x: inset,
        y: TOAST_STACK_LAYER_OFFSET_Y * layer_factor,
        width: (width - inset * 2.0).max(Dp::ZERO),
        opacity: if hidden {
            0.0
        } else {
            (1.0 - TOAST_STACK_LAYER_OPACITY_STEP * layer_factor).clamp(0.0, 1.0)
        },
    }
}

fn collapsed_stack_height<VM>(cards: &[ToastCardRender<VM>]) -> Dp {
    let Some(front) = cards.first() else {
        return Dp::ZERO;
    };
    let visible_back_layers = cards
        .len()
        .saturating_sub(1)
        .min(TOAST_STACK_VISIBLE_BACK_LAYERS);
    front.size.1 + TOAST_STACK_LAYER_OFFSET_Y * visible_back_layers as f32
}

fn interpolate_stack_frame(
    from: ToastStackFrame,
    to: ToastStackFrame,
    progress: f32,
) -> ToastStackFrame {
    ToastStackFrame {
        x: interpolate_dp(from.x, to.x, progress),
        y: interpolate_dp(from.y, to.y, progress),
        width: interpolate_dp(from.width, to.width, progress),
        opacity: interpolate_f32(from.opacity, to.opacity, progress),
    }
}

fn interpolate_dp(from: Dp, to: Dp, progress: f32) -> Dp {
    from + (to - from) * progress
}

fn interpolate_f32(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

fn toast_scene_clip_rect(viewport: Rect) -> Rect {
    Rect::new(
        -TOAST_ENTER_CLIP_MARGIN_X,
        -TOAST_ENTER_CLIP_MARGIN_Y,
        viewport.width + TOAST_ENTER_CLIP_MARGIN_X * 2.0,
        viewport.height + TOAST_ENTER_CLIP_MARGIN_Y * 2.0,
    )
}

fn request_next_toast_frame(context: &mut CollectContext<'_, '_>, now: std::time::Instant) {
    let next_frame = context.frame_clock.next_deadline_after(now);
    let merged = match context.next_toast_wakeup.get() {
        Some(current) => Some(current.min(next_frame)),
        None => Some(next_frame),
    };
    context.next_toast_wakeup.set(merged);
}

fn push_toast_stack_hover_region<VM: 'static>(
    computed: &mut ComputedScene<VM>,
    queue: ToastQueue<VM>,
    stack_hover_widget_id: WidgetId,
    size: (Dp, Dp),
) {
    if size.0 <= Dp::ZERO || size.1 <= Dp::ZERO {
        return;
    }

    let expand_queue = queue.clone();
    let collapse_queue = queue;
    let interactions = InteractionHandlers {
        cursor_style: Some(Value::Static(CursorStyle::Default)),
        on_mouse_enter: Some(Command::new(move |_vm| {
            expand_queue.set_stack_expanded(true);
        })),
        on_mouse_leave: Some(Command::new(move |_vm| {
            collapse_queue.set_stack_expanded(false);
        })),
        ..Default::default()
    };

    computed.hit_regions.insert(
        0,
        HitRegion {
            rect: Rect::new(Dp::ZERO, Dp::ZERO, size.0, size.1),
            clip_rect: None,
            geometry: HitGeometry::Rect,
            transform_chain: Default::default(),
            scope_path: Vec::new(),
            focus: None,
            interaction: HitInteraction::Widget {
                id: stack_hover_widget_id,
                interactions,
                focusable: false,
                default_activation: DefaultActivation::None,
            },
            gpu_scroll_container: None,
        },
    );
}

fn build_toast_card<VM: 'static>(
    queue: ToastQueue<VM>,
    entry: ToastEntry<VM>,
    style: ToastStyle,
    card_width: Dp,
) -> Element<VM> {
    let (icon_bg, icon_fg) = icon_colors_for_kind(&style, entry.toast.kind);
    let show_hover_pause = cfg!(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    ));

    let title_text_style = style.title_text_style.clone();
    let body_text_style = style.body_text_style.clone();
    let action_style = style.action_button.clone();
    let close_style = style.close_button.clone();
    let card_padding = style.padding;
    let card_gap = style.gap;
    let row_gap = style.gap * 0.75;
    let content_gap = style.gap * 0.375;
    let icon_size = style.icon_size;
    let icon_radius = icon_size * 0.5;
    let icon_glyph_size = sp((icon_size.get() * (2.0 / 3.0)).max(10.0));
    let close_size = close_style.min_height;
    let close_icon_size = (icon_size * 0.75).max(dp(12.0));
    let foreground = style.foreground.resolve();
    let background = style.background.resolve();

    let title = entry.toast.title.clone();
    let message = entry.toast.message.clone();
    let action = entry.toast.action.clone();
    let show_close = entry.toast.show_close_button;
    let id = entry.id;
    let kind = entry.toast.kind;

    // 顶部行：图标圆圈 + 类型文字 + spacer + 关闭按钮
    let icon_circle = Stack::<VM>::new()
        .size(icon_size, icon_size)
        .center()
        .style_full(move |context| {
            let mut container = ContainerStyle::default_for_theme(context.theme);
            container.surface.background = Some(Value::Static(icon_bg));
            container.surface.border_radius = Some(Value::Static(icon_radius));
            container
        })
        .child(Text::new(kind_glyph(kind)).style_full(move |context| {
            let mut text_style = TextWidgetStyle::default_for_theme(context.theme);
            text_style.color = Value::Static(icon_fg);
            text_style.typography.size = icon_glyph_size;
            text_style.typography.line_height = Some(icon_glyph_size);
            text_style
        }));

    let title_style_for_label = title_text_style.clone();
    let kind_label = Text::new(kind_label(kind)).style_full(move |context| {
        let mut text_style = TextWidgetStyle::default_for_theme(context.theme);
        text_style.color = Value::Static(foreground);
        text_style.typography = title_style_for_label.clone();
        text_style
    });

    let close_element: Element<VM> = if show_close {
        let dismiss_queue = queue.clone();
        let close_fg = foreground.with_alpha_factor(0.6);
        let mut close_button: Element<VM> = Button::new("")
            .size(close_size, close_size)
            .style_full(move |_| close_style.clone())
            .on_click(Command::new(move |_vm| {
                dismiss_queue.dismiss(id);
            }))
            .into();
        close_button.visual.accessibility_label =
            Some(Value::Static("Dismiss notification".to_string()));
        Stack::new()
            .size(close_size, close_size)
            .center()
            .child(close_button)
            .child(
                Icon::internal(SvgIconId::Close)
                    .size(close_icon_size)
                    .style(move |icon_style: &mut IconStyle, _context| {
                        icon_style.color = Value::Static(close_fg);
                        icon_style.size = close_icon_size;
                    }),
            )
            .into()
    } else {
        Stack::<VM>::new().width(Dp::ZERO).into()
    };

    let top_row = Flex::<VM>::new(Axis::Horizontal)
        .width(pct(100.0))
        .gap(row_gap)
        .align(crate::ui::layout::Align::Center)
        .child(icon_circle)
        .child(kind_label)
        .child(Stack::<VM>::new().grow(1.0)) // spacer
        .child(close_element);

    // 中间内容区
    let title_style_for_content = title_text_style.clone();
    let body_style_for_content = body_text_style.clone();
    let body_style_for_content_else = body_text_style.clone();
    let content_area = if let Some(title_text) = title {
        Flex::<VM>::new(Axis::Vertical)
            .gap(content_gap)
            .child(Text::new(title_text).style_full(move |context| {
                let mut text_style = TextWidgetStyle::default_for_theme(context.theme);
                text_style.color = Value::Static(foreground);
                text_style.typography = title_style_for_content.clone();
                text_style
            }))
            .child(Text::new(message).style_full(move |context| {
                let mut text_style = TextWidgetStyle::default_for_theme(context.theme);
                text_style.color = Value::Static(foreground);
                text_style.typography = body_style_for_content.clone();
                text_style
            }))
    } else {
        Flex::<VM>::new(Axis::Vertical).child(Text::new(message).style_full(move |context| {
            let mut text_style = TextWidgetStyle::default_for_theme(context.theme);
            text_style.color = Value::Static(foreground);
            text_style.typography = body_style_for_content_else.clone();
            text_style
        }))
    };

    // 底部按钮区（如果有 action）
    let bottom_buttons: Element<VM> = if let Some(action) = action {
        Flex::<VM>::new(Axis::Horizontal)
            .gap(row_gap)
            .child(
                Button::new(action.label)
                    .ghost()
                    .style_full(move |_| action_style.clone())
                    .on_click(action.on_click),
            )
            .into()
    } else {
        Stack::<VM>::new().height(Dp::ZERO).into()
    };

    let mut card = Stack::<VM>::new()
        .width(pct_or_fixed(card_width))
        .style_full(move |context| {
            let mut container = ContainerStyle::default_for_theme(context.theme);
            container.surface.background = Some(Value::Static(background));
            container.surface.border_color = Some(style.border.clone());
            container.surface.border_width = Some(style.border_width.clone());
            container.surface.border_radius = Some(style.radius.clone());
            container.surface.shadow = Some(Value::Static(style.shadow.clone()));
            container
        })
        .child(
            Flex::<VM>::new(Axis::Vertical)
                .width(pct(100.0))
                .padding(card_padding)
                .gap(card_gap)
                .child(top_row)
                .child(content_area)
                .child(bottom_buttons),
        );

    if show_hover_pause {
        let pause_queue = queue.clone();
        let resume_queue = queue.clone();
        card = card
            .on_mouse_enter(Command::new(move |_vm| {
                pause_queue.pause(id);
            }))
            .on_mouse_leave(Command::new(move |_vm| {
                resume_queue.resume(id);
            }));
    }

    card.max_width(card_width).into()
}

fn default_placement(placement: ToastPlacement) -> ToastPlacement {
    match placement {
        ToastPlacement::Adaptive => ToastPlacement::BottomEnd,
        other => other,
    }
}

fn map_overlay_placement(placement: ToastPlacement) -> Placement {
    match placement {
        ToastPlacement::Adaptive | ToastPlacement::BottomEnd => {
            Placement::bottom().align(crate::ui::widget::OverlayAlignment::End)
        }
        ToastPlacement::BottomCenter => Placement::bottom(),
        ToastPlacement::BottomStart => {
            Placement::bottom().align(crate::ui::widget::OverlayAlignment::Start)
        }
        ToastPlacement::TopStart => {
            Placement::top().align(crate::ui::widget::OverlayAlignment::Start)
        }
        ToastPlacement::TopCenter => Placement::top(),
        ToastPlacement::TopEnd => Placement::top().align(crate::ui::widget::OverlayAlignment::End),
    }
}

fn ordered_entries<VM>(entries: Vec<ToastEntry<VM>>) -> Vec<ToastEntry<VM>> {
    entries.into_iter().rev().collect()
}

fn toast_width(style: &ToastStyle, viewport: Rect) -> Dp {
    let available = (viewport.width - style.margin * 2.0).max(Dp::ZERO);
    style.max_width.min(available)
}

fn pct_or_fixed(width: Dp) -> Value<Length> {
    Value::Static(Length::Px(width))
}

fn icon_colors_for_kind(
    style: &ToastStyle,
    kind: ToastKind,
) -> (
    crate::foundation::color::Color,
    crate::foundation::color::Color,
) {
    match kind {
        ToastKind::Success => (
            style.success_icon_background.resolve(),
            style.success_icon_foreground.resolve(),
        ),
        ToastKind::Error => (
            style.error_icon_background.resolve(),
            style.error_icon_foreground.resolve(),
        ),
        ToastKind::Warning => (
            style.warning_icon_background.resolve(),
            style.warning_icon_foreground.resolve(),
        ),
        ToastKind::Info => (
            style.info_icon_background.resolve(),
            style.info_icon_foreground.resolve(),
        ),
    }
}

fn kind_label(kind: ToastKind) -> &'static str {
    match kind {
        ToastKind::Success => "Success",
        ToastKind::Error => "Error",
        ToastKind::Warning => "Warning",
        ToastKind::Info => "Info",
    }
}

fn kind_glyph(kind: ToastKind) -> &'static str {
    match kind {
        ToastKind::Success => "✓",
        ToastKind::Error => "×",
        ToastKind::Warning => "!",
        ToastKind::Info => "i",
    }
}

fn earliest_upcoming_deadline<VM>(
    entries: &[ToastEntry<VM>],
    now: std::time::Instant,
) -> Option<std::time::Instant> {
    entries
        .iter()
        .filter(|entry| !entry.paused)
        .filter_map(|entry| entry.deadline)
        .filter(|deadline| *deadline > now)
        .min()
}

/// 检查队列中是否有正在动画中的Toast。
fn entries_have_animation<VM>(
    entries: &[ToastEntry<VM>],
    now: std::time::Instant,
    enter: Option<Transition>,
    exit: Option<Transition>,
) -> bool {
    entries.iter().any(|entry| {
        let elapsed = now.saturating_duration_since(entry.created_at);
        let entering = enter.is_some_and(|transition| elapsed < transition.duration());
        let exiting = exit.is_some_and(|transition| {
            entry.deadline.is_some_and(|deadline| {
                now >= deadline
                    && !entry.paused
                    && now.saturating_duration_since(deadline) < transition.duration()
            })
        });
        entering || exiting
    })
}

/// 根据生命周期状态和位置计算动画进度
fn calculate_animation_progress<VM>(
    entry: &ToastEntry<VM>,
    placement: ToastPlacement,
    now: std::time::Instant,
    enter: Option<Transition>,
    exit: Option<Transition>,
) -> ToastMotionSample {
    let enter_sample = |at: std::time::Instant| {
        let progress = enter.map_or(1.0, |transition| {
            let elapsed = at.saturating_duration_since(entry.created_at);
            transition
                .curve_mode()
                .sample(transition_progress(elapsed, transition))
        });
        let (offset_x, offset_y) = toast_motion_offset(placement, 1.0 - progress);
        ToastMotionSample {
            opacity: progress,
            offset_x,
            offset_y,
            flow_occupancy: progress,
        }
    };

    // Exit starts from the exact visual/flow state sampled at the deadline. In particular, a
    // dismiss during enter reverses from the partially visible card instead of snapping to the
    // fully-entered state before leaving.
    if let Some(deadline) = entry.deadline {
        if now >= deadline && !entry.paused {
            let Some(exit) = exit else {
                return ToastMotionSample {
                    opacity: 0.0,
                    offset_x: Dp::ZERO,
                    offset_y: Dp::ZERO,
                    flow_occupancy: 0.0,
                };
            };
            let start = enter_sample(deadline);
            let exit_elapsed = now.saturating_duration_since(deadline);
            let progress = transition_progress(exit_elapsed, exit);
            let eased = exit.curve_mode().sample(progress);
            let (target_x, target_y) = toast_motion_offset(placement, 1.0);
            return ToastMotionSample {
                opacity: start.opacity * (1.0 - eased),
                offset_x: interpolate_dp(start.offset_x, target_x, eased),
                offset_y: interpolate_dp(start.offset_y, target_y, eased),
                flow_occupancy: start.flow_occupancy * (1.0 - eased),
            };
        }
    }

    enter_sample(now)
}

fn toast_motion_offset(placement: ToastPlacement, amount: f32) -> (Dp, Dp) {
    match placement {
        ToastPlacement::TopStart | ToastPlacement::BottomStart => {
            (-TOAST_ENTER_OFFSET_X * amount, Dp::ZERO)
        }
        ToastPlacement::TopEnd | ToastPlacement::BottomEnd | ToastPlacement::Adaptive => {
            (TOAST_ENTER_OFFSET_X * amount, Dp::ZERO)
        }
        ToastPlacement::TopCenter => (Dp::ZERO, -TOAST_ENTER_OFFSET_Y * amount),
        ToastPlacement::BottomCenter => (Dp::ZERO, TOAST_ENTER_OFFSET_Y * amount),
    }
}

#[inline]
fn transition_progress(elapsed: Duration, transition: Transition) -> f32 {
    let duration = transition.duration();
    if duration.is_zero() {
        1.0
    } else {
        (elapsed.as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0)
    }
}
