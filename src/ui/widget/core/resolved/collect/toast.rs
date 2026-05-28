use taffy::prelude::{AvailableSpace, TaffyTree};
use taffy::Size as TaffySize;

use crate::foundation::binding::{ToastEntry, ToastKind, ToastPlacement, ToastQueue};
use crate::ui::layout::{Axis, Justify, Length, Value};
use crate::ui::unit::{dp, Dp};
use crate::ui::widget::button::Button;
use crate::ui::widget::container::{Flex, Stack};
use crate::ui::widget::core::measure_node;
use crate::ui::widget::overlay::{
    collect::emit_overlay, Anchor, Overlay, OverlayContent, OverlayId, OverlayLayer, Placement,
};
use crate::ui::widget::style::{ContainerStyle, TextWidgetStyle, ToastStyle};
use crate::ui::widget::text::Text;
use crate::ui::widget::{ComputedScene, Element, Rect};

use super::super::scene::{CollectContext, VisualContext};
use super::super::types::ResolvedElement;
use super::CollectVisualState;
use crate::foundation::binding::DependencyGraph;
use crate::foundation::view_model::Command;

const TOAST_OVERLAY_TAG: u64 = 0x544F4153545F484F; // "TOAST_HO"

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
        let Some((content_scene, content_size)) =
            build_toast_scene(queue.clone(), entries, style.clone(), resolved_placement, context)
        else {
            return;
        };

        computed.dependencies.merge_from(&content_scene.dependencies);
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
    let (result, dependencies): (Option<(ComputedScene<VM>, (Dp, Dp))>, DependencyGraph) =
        crate::foundation::binding::with_dependency_collection(|| {
            let width = toast_width(&style, context.viewport);
            let mut root = Flex::<VM>::new(Axis::Vertical)
                .gap(style.stack_gap)
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

            let rendered_entries = ordered_entries(entries, placement);
            for entry in rendered_entries {
                root = root.child(build_toast_card(queue.clone(), entry, style.clone()));
            }

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
            let size = (Dp::new(layout.size.width), Dp::new(layout.size.height));
            let local_bounds = Rect::new(Dp::ZERO, Dp::ZERO, size.0, size.1);

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
            let mut computed = resolved.collect_subtree_cache(
                &layout_root,
                VisualContext {
                    origin: crate::ui::widget::Point::ZERO,
                    opacity: 1.0,
                    clip_rect: local_bounds,
                    clip_mask: None,
                },
                &mut local_context,
                &mut lifecycle_states,
                &mut chunks,
                &mut chunk_parts,
                &mut visual_contexts,
            );
            computed.finalize_portals(context.viewport);
            Some((computed, size))
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
    let (background, foreground) = colors_for_kind(&style, entry.toast.kind);
    let show_hover_pause = cfg!(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    ));

    let title_style = style.title_text_style.clone();
    let icon_style = title_style.clone();
    let body_style = style.body_text_style.clone();
    let action_style = style.action_button.clone();
    let close_style = style.close_button.clone();

    let title = entry.toast.title.clone();
    let message = entry.toast.message.clone();
    let action = entry.toast.action.clone();
    let show_close = entry.toast.show_close_button;
    let id = entry.id;

    let title_element: Element<VM> = if let Some(title) = title {
        let title_text_style = title_style.clone();
        Text::new(title)
            .style(move |mode| {
                let mut text_style = TextWidgetStyle::default_for(mode);
                text_style.color = foreground.into();
                text_style.typography = title_text_style.clone();
                text_style
            })
            .into()
    } else {
        Stack::<VM>::new().height(Dp::ZERO).into()
    };

    let action_element: Element<VM> = if let Some(action) = action {
        let mut button = Button::new(action.label).ghost().style(move |_| {
            let mut button_style = action_style.clone();
            button_style.foreground = crate::ui::theme::Stateful {
                normal: foreground.into(),
                hovered: foreground.into(),
                pressed: foreground.into(),
                disabled: foreground.with_alpha_factor(0.6).into(),
            };
            button_style
        });
        button = button.on_click(action.on_click);
        button.into()
    } else {
        Stack::<VM>::new().height(Dp::ZERO).into()
    };

    let close_element: Element<VM> = if show_close {
        let dismiss_queue = queue.clone();
        Button::new("×")
            .ghost()
            .style(move |_| {
                let mut button_style = close_style.clone();
                button_style.foreground = crate::ui::theme::Stateful {
                    normal: foreground.into(),
                    hovered: foreground.into(),
                    pressed: foreground.into(),
                    disabled: foreground.with_alpha_factor(0.6).into(),
                };
                button_style
            })
            .on_click(Command::new(move |_vm| {
                dismiss_queue.dismiss(id);
            }))
            .into()
    } else {
        Stack::<VM>::new().height(Dp::ZERO).into()
    };

    let mut card = Stack::<VM>::new()
        .width(pct_or_fixed(style.max_width))
        .style(move |mode| {
            let mut container = ContainerStyle::default_for(mode);
            container.surface.background = Some(background.into());
            container.surface.border_color = Some(style.border.clone());
            container.surface.border_width = Some(style.border_width.clone());
            container.surface.border_radius = Some(style.radius.clone());
            container.surface.shadow = Some(Value::Static(style.shadow.clone()));
            container
        })
        .child(
            Flex::<VM>::new(Axis::Horizontal)
                .padding(style.padding)
                .gap(style.gap)
                .align(crate::ui::layout::Align::Start)
                .child(Text::new(kind_glyph(entry.toast.kind)).style(move |mode| {
                    let mut text_style = TextWidgetStyle::default_for(mode);
                    text_style.color = foreground.into();
                    text_style.typography = icon_style.clone();
                    text_style
                }))
                .child(
                    Flex::<VM>::new(Axis::Vertical)
                        .grow(1.0)
                        .gap(dp(6.0))
                        .child(title_element)
                        .child(Text::new(message).style(move |mode| {
                            let mut text_style = TextWidgetStyle::default_for(mode);
                            text_style.color = foreground.into();
                            text_style.typography = body_style.clone();
                            text_style
                        }))
                        .child(
                            Flex::<VM>::new(Axis::Horizontal)
                                .gap(dp(8.0))
                                .child(action_element)
                                .child(close_element),
                        ),
                ),
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
        ToastPlacement::Adaptive | ToastPlacement::BottomEnd => Placement::bottom().align(crate::ui::widget::OverlayAlignment::End),
        ToastPlacement::BottomCenter => Placement::bottom(),
        ToastPlacement::BottomStart => Placement::bottom().align(crate::ui::widget::OverlayAlignment::Start),
        ToastPlacement::TopStart => Placement::top().align(crate::ui::widget::OverlayAlignment::Start),
        ToastPlacement::TopCenter => Placement::top(),
        ToastPlacement::TopEnd => Placement::top().align(crate::ui::widget::OverlayAlignment::End),
    }
}

fn ordered_entries<VM>(entries: Vec<ToastEntry<VM>>, placement: ToastPlacement) -> Vec<ToastEntry<VM>> {
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

fn colors_for_kind(style: &ToastStyle, kind: ToastKind) -> (crate::foundation::color::Color, crate::foundation::color::Color) {
    match kind {
        ToastKind::Success => (style.success_background.resolve(), style.success_foreground.resolve()),
        ToastKind::Error => (style.error_background.resolve(), style.error_foreground.resolve()),
        ToastKind::Warning => (style.warning_background.resolve(), style.warning_foreground.resolve()),
        ToastKind::Info => (style.info_background.resolve(), style.info_foreground.resolve()),
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
