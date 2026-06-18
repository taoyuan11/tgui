use taffy::prelude::TaffyTree;

use crate::animation::{AnimationKey, Transition, WidgetProperty};
use crate::foundation::binding::{ToastEntry, ToastKind, ToastPlacement, ToastQueue};
use crate::ui::layout::{pct, Axis, Justify, Length, Value};
use crate::ui::unit::{dp, sp, Dp};
use crate::ui::widget::button::Button;
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
const TOAST_STACK_TRANSITION_MS: u64 = 180;
const TOAST_STACK_VISIBLE_BACK_LAYERS: usize = 2;
const TOAST_STACK_LAYER_INSET_X: Dp = Dp::new(12.0);
const TOAST_STACK_LAYER_OFFSET_Y: Dp = Dp::new(16.0);
const TOAST_STACK_LAYER_OPACITY_STEP: f32 = 0.08;
const TOAST_ENTER_CLIP_MARGIN_X: Dp = Dp::new(160.0);
const TOAST_ENTER_CLIP_MARGIN_Y: Dp = Dp::new(48.0);
const ENTER_DURATION_MS: u64 = 400;
const EXIT_DURATION_MS: u64 = 300;

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
        } = &self.kind
        else {
            return;
        };

        let now = context.now;
        let _ = queue.flush_expired(now);
        if let Some(deadline) = queue.earliest_deadline() {
            let merged = match context.next_toast_wakeup.get() {
                Some(current) => Some(current.min(deadline)),
                None => Some(deadline),
            };
            context.next_toast_wakeup.set(merged);
        }

        // 检查是否有正在动画中的Toast
        let has_animating = entries_have_animation(queue, now);
        if has_animating {
            request_next_toast_frame(context, now);
        }

        let mut entries = queue.snapshot();
        if entries.is_empty() {
            queue.set_stack_expanded(false);
            return;
        }
        if let Some(limit) = *max_visible {
            if entries.len() > limit {
                entries = entries.split_off(entries.len() - limit);
            }
        }
        if entries.is_empty() {
            queue.set_stack_expanded(false);
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
                        Some(Transition::ease_in_out(std::time::Duration::from_millis(
                            TOAST_STACK_TRANSITION_MS,
                        ))),
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
                let mut expanded_sizes = Vec::with_capacity(rendered_len);
                for entry in rendered_entries.iter() {
                    expanded_sizes.push(measure_toast_card_size(
                        queue.clone(),
                        entry.clone(),
                        style.clone(),
                        placement,
                        width,
                        context,
                    )?);
                }

                let mut combined = ComputedScene::default();
                let mut cards = Vec::with_capacity(rendered_len);
                let mut expanded_y = Dp::ZERO;
                let mut expanded_height = Dp::ZERO;

                for (index, entry) in rendered_entries.iter().enumerate() {
                    let expanded_size = expanded_sizes[index];
                    let (opacity, offset_x, offset_y) =
                        calculate_animation_progress(entry, placement, now);
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
                    let origin = Point::new(stack_frame.x + offset_x, stack_frame.y + offset_y);
                    let content_reveal = if auto_collapsible && index > 0 {
                        stack_progress
                    } else {
                        1.0
                    };
                    let card_opacity = opacity * content_reveal;

                    let (card_scene, card_size) = collect_toast_card_scene(
                        queue.clone(),
                        entry.clone(),
                        style.clone(),
                        placement,
                        stack_frame.width,
                        origin,
                        card_opacity,
                        context,
                    )?;

                    let all_cards_interactive = !auto_collapsible || stack_progress >= 0.98;
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
                        interactive: all_cards_interactive || index == 0,
                    });

                    expanded_height = expanded_y + expanded_size.1;
                    if index + 1 < rendered_len {
                        expanded_y += expanded_size.1 + style.stack_gap;
                    } else {
                        expanded_y += expanded_size.1;
                    }
                }

                let collapsed_height = collapsed_stack_height(&cards);
                let total_height = if auto_collapsible {
                    interpolate_dp(collapsed_height, expanded_height, stack_progress)
                } else {
                    expanded_height
                };
                let draw_back_to_front = auto_collapsible && stack_progress < 0.999;
                if draw_back_to_front {
                    for index in (0..cards.len()).rev() {
                        extend_toast_card_scene(&mut combined, &cards[index]);
                    }
                } else {
                    for card in cards.iter() {
                        extend_toast_card_scene(&mut combined, card);
                    }
                }

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

fn collect_toast_card_scene<VM: 'static>(
    queue: ToastQueue<VM>,
    entry: ToastEntry<VM>,
    style: ToastStyle,
    placement: ToastPlacement,
    card_width: Dp,
    origin: Point,
    opacity: f32,
    context: &mut CollectContext<'_, '_>,
) -> Option<(ComputedScene<VM>, (Dp, Dp))> {
    let root = toast_card_root(queue, entry, style, placement, card_width);
    let mut resolved: Element<VM> = root.into();
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
    let layout = taffy.layout(layout_root.node).ok()?;
    let card_size = (Dp::new(layout.size.width), Dp::new(layout.size.height));

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
        focus: Default::default(),
        tooltip_hover_started_at: context.tooltip_hover_started_at,
        next_tooltip_wakeup: context.next_tooltip_wakeup,
        next_toast_wakeup: context.next_toast_wakeup,
        active_tooltip: context.active_tooltip,
        active_hover_popover: context.active_hover_popover,
        gpu_scroll_enabled: false,
        gpu_scroll_container: None,
        transform_stack: context.transform_stack.clone(),
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
    let scene = chunks.get(&root_id).cloned().unwrap_or_default();
    Some((scene, card_size))
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

fn measure_toast_card_size<VM: 'static>(
    queue: ToastQueue<VM>,
    entry: ToastEntry<VM>,
    style: ToastStyle,
    placement: ToastPlacement,
    card_width: Dp,
    context: &mut CollectContext<'_, '_>,
) -> Option<(Dp, Dp)> {
    let root = toast_card_root(queue, entry, style, placement, card_width);
    let mut resolved: Element<VM> = root.into();
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
    let layout = taffy.layout(layout_root.node).ok()?;
    Some((Dp::new(layout.size.width), Dp::new(layout.size.height)))
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
        focus: Default::default(),
        tooltip_hover_started_at: context.tooltip_hover_started_at,
        next_tooltip_wakeup: context.next_tooltip_wakeup,
        next_toast_wakeup: context.next_toast_wakeup,
        active_tooltip: context.active_tooltip,
        active_hover_popover: context.active_hover_popover,
        gpu_scroll_enabled: false,
        gpu_scroll_container: None,
        transform_stack: context.transform_stack.clone(),
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
    let next_frame = now + std::time::Duration::from_millis(16);
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
        .size(dp(18.0), dp(18.0))
        .center()
        .style_full(move |context| {
            let mut container = ContainerStyle::default_for_theme(context.theme);
            container.surface.background = Some(Value::Static(icon_bg));
            container.surface.border_radius = Some(Value::Static(dp(9.0)));
            container
        })
        .child(Text::new(kind_glyph(kind)).style_full(move |context| {
            let mut text_style = TextWidgetStyle::default_for_theme(context.theme);
            text_style.color = Value::Static(icon_fg);
            text_style.typography.size = sp(12.0);
            text_style.typography.line_height = Some(sp(12.0));
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
        Stack::new()
            .size(dp(32.0), dp(32.0))
            .center()
            .style_full(move |context| {
                let button_style = close_style.clone();
                let mut container = ContainerStyle::default_for_theme(context.theme);
                container.surface.background = Some(button_style.background.normal);
                container.surface.border_color = Some(button_style.border.normal);
                container.surface.border_width = Some(button_style.border_width);
                container.surface.border_radius = Some(button_style.radius);
                container.surface.shadow = button_style.surface.shadow;
                container
            })
            .child(
                Icon::internal(SvgIconId::Close)
                    .size(dp(14.0), dp(14.0))
                    .style(move |icon_style: &mut IconStyle, _context| {
                        icon_style.color = Value::Static(close_fg);
                        icon_style.size = dp(14.0);
                    }),
            )
            .on_click(Command::new(move |_vm| {
                dismiss_queue.dismiss(id);
            }))
            .into()
    } else {
        Stack::<VM>::new().width(Dp::ZERO).into()
    };

    let top_row = Flex::<VM>::new(Axis::Horizontal)
        .width(pct(100.0))
        .gap(dp(6.0))
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
            .gap(dp(3.0))
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
            .gap(dp(6.0))
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
                .padding(style.padding)
                .gap(dp(8.0))
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
    let _ = viewport;
    style.max_width
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

/// 检查队列中是否有正在动画中的Toast
fn entries_have_animation<VM>(queue: &ToastQueue<VM>, now: std::time::Instant) -> bool {
    queue.snapshot().iter().any(|entry| {
        let elapsed = now.saturating_duration_since(entry.created_at);
        let entering = elapsed.as_millis() < ENTER_DURATION_MS as u128;
        let exiting = entry.deadline.map_or(false, |d| now >= d && !entry.paused);
        entering || exiting
    })
}

/// 根据生命周期状态和位置计算动画进度
fn calculate_animation_progress<VM>(
    entry: &ToastEntry<VM>,
    placement: ToastPlacement,
    now: std::time::Instant,
) -> (f32, Dp, Dp) {
    let elapsed = now.saturating_duration_since(entry.created_at);

    // 退场动画优先于入场动画，确保刚创建后手动关闭也能立即退出。
    if let Some(deadline) = entry.deadline {
        if now >= deadline && !entry.paused {
            let exit_elapsed = now.saturating_duration_since(deadline);
            let progress = (exit_elapsed.as_millis() as f32 / EXIT_DURATION_MS as f32).min(1.0);
            let eased = progress.powi(3); // ease-in

            let opacity = 1.0 - eased;
            let (offset_x, offset_y) = match placement {
                ToastPlacement::TopStart | ToastPlacement::BottomStart => {
                    (dp(eased * -150.0), Dp::ZERO)
                }
                ToastPlacement::TopEnd | ToastPlacement::BottomEnd | ToastPlacement::Adaptive => {
                    (dp(eased * 150.0), Dp::ZERO)
                }
                ToastPlacement::TopCenter => (Dp::ZERO, dp(eased * -40.0)),
                ToastPlacement::BottomCenter => (Dp::ZERO, dp(eased * 40.0)),
            };
            return (opacity, offset_x, offset_y);
        }
    }

    // 入场动画
    if elapsed.as_millis() < ENTER_DURATION_MS as u128 {
        let progress = (elapsed.as_millis() as f32 / ENTER_DURATION_MS as f32).min(1.0);
        let eased = 1.0 - (1.0 - progress).powi(3); // ease-out

        let opacity = eased;
        let (offset_x, offset_y) = match placement {
            ToastPlacement::TopStart | ToastPlacement::BottomStart => {
                (dp((1.0 - eased) * -150.0), Dp::ZERO)
            }
            ToastPlacement::TopEnd | ToastPlacement::BottomEnd | ToastPlacement::Adaptive => {
                (dp((1.0 - eased) * 150.0), Dp::ZERO)
            }
            ToastPlacement::TopCenter => (Dp::ZERO, dp((1.0 - eased) * -40.0)),
            ToastPlacement::BottomCenter => (Dp::ZERO, dp((1.0 - eased) * 40.0)),
        };
        return (opacity, offset_x, offset_y);
    }

    // 正常显示
    (1.0, Dp::ZERO, Dp::ZERO)
}
