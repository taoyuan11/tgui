use taffy::prelude::{AvailableSpace, TaffyTree};
use taffy::Size as TaffySize;

use crate::foundation::binding::{ToastEntry, ToastKind, ToastPlacement, ToastQueue};
use crate::ui::layout::{Axis, Justify, Length, Value};
use crate::ui::unit::{dp, sp, Dp, Sp};
use crate::ui::widget::button::Button;
use crate::ui::widget::container::{Flex, Stack};
use crate::ui::widget::core::measure_node;
use crate::ui::widget::overlay::{
    collect::emit_overlay, Anchor, Overlay, OverlayContent, OverlayId, OverlayLayer, Placement,
};
use crate::ui::widget::style::{ContainerStyle, TextWidgetStyle, ToastStyle};
use crate::ui::widget::text::Text;
use crate::ui::widget::{ComputedScene, Element, Point, Rect};

use super::super::scene::{CollectContext, VisualContext};
use super::super::types::ResolvedElement;
use super::CollectVisualState;
use crate::foundation::binding::DependencyGraph;
use crate::foundation::view_model::Command;

const TOAST_OVERLAY_TAG: u64 = 0x544F4153545F484F; // "TOAST_HO"
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
            // 动画进行中，设置下一帧唤醒（60fps）
            let next_frame = now + std::time::Duration::from_millis(16);
            let merged = match context.next_toast_wakeup.get() {
                Some(current) => Some(current.min(next_frame)),
                None => Some(next_frame),
            };
            context.next_toast_wakeup.set(merged);
        }

        let mut entries = queue.snapshot();
        if entries.is_empty() {
            return;
        }
        if let Some(limit) = *max_visible {
            if entries.len() > limit {
                entries = entries.split_off(entries.len() - limit);
            }
        }

        let resolved_placement = default_placement(*placement);
        let Some((content_scene, content_size)) = build_toast_scene(
            queue.clone(),
            entries,
            style.clone(),
            resolved_placement,
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
    context: &mut CollectContext<'_, '_>,
) -> Option<(ComputedScene<VM>, (Dp, Dp))> {
    let now = context.now;
    let (result, dependencies): (Option<(ComputedScene<VM>, (Dp, Dp))>, DependencyGraph) =
        crate::foundation::binding::with_dependency_collection(|| {
            super::super::tree::with_widget_stack(|| {
                let width = toast_width(&style, context.viewport);

                // 为每个Toast entry独立构建scene，以便单独应用动画
                let rendered_entries = ordered_entries(entries, placement);
                let mut combined = ComputedScene::default();
                let mut total_height = Dp::ZERO;
                let mut max_width = Dp::ZERO;

                for (i, entry) in rendered_entries.iter().enumerate() {
                    let (opacity, offset_x, offset_y) =
                        calculate_animation_progress(entry, placement, now);

                    // 构建单个Toast card
                    let mut root = Flex::<VM>::new(Axis::Vertical)
                        .width(width)
                        .align(match placement {
                            ToastPlacement::TopCenter | ToastPlacement::BottomCenter => {
                                crate::ui::layout::Align::Center
                            }
                            ToastPlacement::TopEnd | ToastPlacement::BottomEnd => {
                                crate::ui::layout::Align::End
                            }
                            _ => crate::ui::layout::Align::Start,
                        })
                        .justify(Justify::Start);

                    root = root.child(build_toast_card(
                        queue.clone(),
                        entry.clone(),
                        style.clone(),
                    ));

                    let resolved: Element<VM> = root.into();
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
                    taffy
                        .compute_layout_with_measure(
                            layout_root.node,
                            TaffySize {
                                width: AvailableSpace::Definite(context.viewport.width.get()),
                                height: AvailableSpace::Definite(context.viewport.height.get()),
                            },
                            |known_dimensions, _, _, node_context, _| {
                                measure_node(
                                    node_context,
                                    known_dimensions,
                                    context.font_manager,
                                    context.theme,
                                    context.media,
                                    context.units,
                                )
                            },
                        )
                        .ok()?;
                    let layout = taffy.layout(layout_root.node).ok()?;
                    let card_size = (Dp::new(layout.size.width), Dp::new(layout.size.height));
                    let card_bounds = Rect::new(Dp::ZERO, Dp::ZERO, card_size.0, card_size.1);

                    let mut lifecycle_states = std::collections::HashMap::new();
                    let mut chunks = std::collections::HashMap::new();
                    let mut chunk_parts = std::collections::HashMap::new();
                    let mut visual_contexts = std::collections::HashMap::new();
                    let mut local_context = CollectContext {
                        taffy: &taffy,
                        font_manager: context.font_manager,
                        theme: context.theme,
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
                        scroll_offsets: context.scroll_offsets,
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
                    };

                    // 关键！将动画opacity和offset传入VisualContext
                    let origin = Point::new(offset_x, total_height + offset_y);
                    let root_id = resolved.collect_subtree_cache(
                        &layout_root,
                        VisualContext {
                            origin,
                            opacity,
                            clip_rect: Rect::new(
                                Dp::ZERO,
                                Dp::ZERO,
                                context.viewport.width,
                                context.viewport.height,
                            ),
                            clip_mask: None,
                        },
                        &mut local_context,
                        &mut lifecycle_states,
                        &mut chunks,
                        &mut chunk_parts,
                        &mut visual_contexts,
                    );
                    if let Some(card_scene) = chunks.get(&root_id) {
                        combined.extend(card_scene);
                    }

                    total_height = total_height + card_size.1 + style.stack_gap;
                    if card_size.0 > max_width {
                        max_width = card_size.0;
                    }
                }

                let size = (max_width, total_height);
                Some((combined, size))
            })
        });
    let (mut computed, size) = result?;
    computed.dependencies = dependencies.clone();
    Some((computed, size))
}

fn build_toast_card<VM: 'static>(
    queue: ToastQueue<VM>,
    entry: ToastEntry<VM>,
    style: ToastStyle,
) -> Element<VM> {
    let (icon_bg, icon_fg) = icon_colors_for_kind(&style, entry.toast.kind);
    let show_hover_pause = cfg!(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
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
        .style(move |mode| {
            let mut container = ContainerStyle::default_for(mode);
            container.surface.background = Some(Value::Static(icon_bg));
            container.surface.border_radius = Some(Value::Static(dp(9.0)));
            container
        })
        .child(
            Stack::<VM>::new()
                .size(dp(18.0), dp(18.0))
                .child(
                    Text::new(kind_icon_glyph(kind))
                        .style(move |mode| {
                            let mut text_style = TextWidgetStyle::default_for(mode);
                            text_style.color = Value::Static(icon_fg);
                            text_style.typography.size = sp(12.0);
                            text_style.typography.font_family = Some("tgui-icons".to_string());
                            text_style
                        })
                        .width(dp(18.0))
                        .height(dp(18.0)),
                ),
        );

    let title_style_for_label = title_text_style.clone();
    let kind_label = Text::new(kind_label(kind)).style(move |mode| {
        let mut text_style = TextWidgetStyle::default_for(mode);
        text_style.color = Value::Static(foreground);
        text_style.typography = title_style_for_label.clone();
        text_style
    });

    let close_element: Element<VM> = if show_close {
        let dismiss_queue = queue.clone();
        let close_fg = foreground.with_alpha_factor(0.6);
        let close_hover = foreground.with_alpha_factor(0.8);
        let close_pressed = foreground;
        Button::new("\u{e5cd}") // close icon
            .ghost()
            .style(move |_| {
                let mut button_style = close_style.clone();
                button_style.text_style.font_family = Some("tgui-icons".to_string());
                button_style.text_style.size = sp(14.0);
                button_style.foreground = crate::ui::theme::Stateful {
                    normal: close_fg.into(),
                    hovered: close_hover.into(),
                    pressed: close_pressed.into(),
                    disabled: close_fg.with_alpha_factor(0.5).into(),
                };
                button_style
            })
            .on_click(Command::new(move |_vm| {
                dismiss_queue.dismiss(id);
            }))
            .into()
    } else {
        Stack::<VM>::new().width(Dp::ZERO).into()
    };

    let top_row = Flex::<VM>::new(Axis::Horizontal)
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
            .child(Text::new(title_text).style(move |mode| {
                let mut text_style = TextWidgetStyle::default_for(mode);
                text_style.color = Value::Static(foreground);
                text_style.typography = title_style_for_content.clone();
                text_style
            }))
            .child(Text::new(message).style(move |mode| {
                let mut text_style = TextWidgetStyle::default_for(mode);
                text_style.color = Value::Static(foreground);
                text_style.typography = body_style_for_content.clone();
                text_style
            }))
    } else {
        Flex::<VM>::new(Axis::Vertical)
            .child(Text::new(message).style(move |mode| {
                let mut text_style = TextWidgetStyle::default_for(mode);
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
                    .style(move |_| action_style.clone())
                    .on_click(action.on_click),
            )
            .into()
    } else {
        Stack::<VM>::new().height(Dp::ZERO).into()
    };

    let mut card = Stack::<VM>::new()
        .width(pct_or_fixed(style.max_width))
        .style(move |mode| {
            let mut container = ContainerStyle::default_for(mode);
            container.surface.background = Some(Value::Static(background));
            container.surface.border_color = Some(style.border.clone());
            container.surface.border_width = Some(style.border_width.clone());
            container.surface.border_radius = Some(style.radius.clone());
            container.surface.shadow = Some(Value::Static(style.shadow.clone()));
            container
        })
        .child(
            Flex::<VM>::new(Axis::Vertical)
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

    card.max_width(style.max_width).into()
}

fn default_placement(placement: ToastPlacement) -> ToastPlacement {
    match placement {
        ToastPlacement::Adaptive => {
            #[cfg(any(target_os = "android", target_env = "ohos"))]
            {
                ToastPlacement::BottomCenter
            }
            #[cfg(not(any(target_os = "android", target_env = "ohos")))]
            {
                ToastPlacement::BottomEnd
            }
        }
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

fn ordered_entries<VM>(
    entries: Vec<ToastEntry<VM>>,
    placement: ToastPlacement,
) -> Vec<ToastEntry<VM>> {
    match placement {
        ToastPlacement::BottomStart
        | ToastPlacement::BottomCenter
        | ToastPlacement::BottomEnd
        | ToastPlacement::Adaptive => entries.into_iter().rev().collect(),
        _ => entries,
    }
}

fn toast_width(style: &ToastStyle, viewport: Rect) -> Dp {
    let mobile_like = cfg!(any(target_os = "android", target_env = "ohos"));
    if mobile_like {
        (viewport.width - style.margin * 2.0).max(style.min_width)
    } else {
        style.max_width
    }
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

fn kind_icon_glyph(kind: ToastKind) -> &'static str {
    match kind {
        ToastKind::Success => "\u{e86c}", // check_circle
        ToastKind::Error => "\u{e000}",   // error
        ToastKind::Warning => "\u{e002}",  // warning
        ToastKind::Info => "\u{e88e}",    // info
    }
}

fn kind_glyph(kind: ToastKind) -> &'static str {
    match kind {
        ToastKind::Success => "✓",
        ToastKind::Error => "!",
        ToastKind::Warning => "⚠",
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

    // 退场动画：检查是否已过deadline
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

    // 正常显示
    (1.0, Dp::ZERO, Dp::ZERO)
}
