use super::*;
use crate::ui::widget::common::TreeNodeState;
use crate::ui::widget::DefaultActivation;
use crate::ui::widget::TreeCheckState;
use std::sync::Arc;
use std::time::Duration;

const TREE_DISCLOSURE_ICON: &str = "keyboard_arrow_right";
const TREE_CHECKED_ICON: &str = "check_box";
const TREE_UNCHECKED_ICON: &str = "check_box_outline_blank";
const TREE_INDETERMINATE_ICON: &str = "indeterminate_check_box";
const TREE_DISCLOSURE_TRANSITION_MS: u64 = 160;
const TREE_CHECKBOX_TRANSITION_MS: u64 = 140;

impl<VM> ResolvedElement<VM> {
    pub(in super::super) fn push_surface_primitives_and_base_hit_regions(
        &self,
        computed: &mut ComputedScene<VM>,
        context: &mut CollectContext<'_, '_>,
        visual: &CollectVisualState,
    ) {
        let background_blur = self
            .visual
            .background_blur
            .resolve_widget_to_logical(
                context.animations,
                self.id,
                WidgetProperty::BackgroundBlur,
                context.now,
                context.units,
            )
            .max(0.0);
        let shadow = self.visual.shadow.as_ref().map(Value::resolve);
        let background_brush = self
            .visual
            .background_brush
            .as_ref()
            .map(|brush| brush.resolve_widget());
        let background_image = self
            .visual
            .background_image
            .as_ref()
            .map(|image| image.resolve_widget());

        if let Some(shadow) = shadow.as_ref() {
            if let Some(texture) = rounded_rect_shadow_texture(
                visual.background_frame,
                visual.background_radius.get(),
                RoundedRectShadowSpec {
                    shadow: shadow.clone(),
                    opacity: visual.opacity,
                    clip_rect: visual.primitive_clip,
                    clip_mask: visual.primitive_clip_mask,
                },
                context.media,
                context.units,
            ) {
                computed.scene.push_texture(texture);
            }
        }

        if background_blur > 0.0
            && visual.background_frame.width > Dp::ZERO
            && visual.background_frame.height > Dp::ZERO
        {
            computed.scene.push_backdrop_blur(BackdropBlurPrimitive {
                rect: visual.background_frame,
                corner_radius: visual.background_radius.get(),
                blur_radius: background_blur,
                clip_rect: visual.primitive_clip,
                clip_mask: visual.primitive_clip_mask,
            });
        }

        let preserve_solid_background = matches!(self.kind, ResolvedWidgetKind::Switch { .. });

        if visual.background_frame.width > Dp::ZERO && visual.background_frame.height > Dp::ZERO {
            let should_draw_base_background = visual.background.a > 0
                && (background_image.is_some()
                    || background_brush.is_none()
                    || preserve_solid_background);
            if should_draw_base_background {
                computed.scene.push_shape(RenderPrimitive {
                    rect: visual.background_frame,
                    color: visual.background,
                    corner_radius: visual.background_radius.get(),
                    stroke_width: 0.0,
                    clip_rect: visual.primitive_clip,
                    clip_mask: visual.primitive_clip_mask,
                });
            }

            if let Some(image) = background_image.as_ref() {
                push_background_media_texture(
                    &image.source,
                    image.fit,
                    visual.background_frame,
                    visual.background_radius.get(),
                    visual.primitive_clip,
                    visual.primitive_clip_mask,
                    context,
                    computed,
                );
            }

            if let Some(brush) = background_brush.clone() {
                computed.scene.push_brush(BrushPrimitive {
                    rect: visual.background_frame,
                    brush,
                    corner_radius: visual.background_radius.get(),
                    clip_rect: visual.primitive_clip,
                    clip_mask: visual.primitive_clip_mask,
                });
            }
        }

        push_border_primitives(
            &mut computed.scene,
            visual.frame,
            visual.border_width.get(),
            visual.border_color,
            visual.border_radius.get(),
            visual.primitive_clip,
            visual.primitive_clip_mask,
        );

        let focus_ring = match &self.kind {
            ResolvedWidgetKind::Button { .. } => visual
                .styles
                .button_style
                .as_ref()
                .and_then(|style| style.focus_ring.clone()),
            ResolvedWidgetKind::Select { .. } => visual
                .styles
                .select_style
                .as_ref()
                .and_then(|style| style.focus_ring.clone()),
            ResolvedWidgetKind::Slider { .. } => visual
                .styles
                .slider_style
                .as_ref()
                .and_then(|style| style.focus_ring.clone()),
            ResolvedWidgetKind::TextEditor { .. } => None,
            ResolvedWidgetKind::Switch { .. } => {
                visual.styles.switch_style.as_ref().and_then(|style| {
                    resolve_focus_ring(
                        context.theme,
                        style.focus_ring.as_ref(),
                        visual.widget_state,
                    )
                })
            }
            _ if (self.list_item.is_some() || self.tree_node.is_some())
                && visual.widget_state.focused =>
            {
                Some(context.theme.focus_ring.clone())
            }
            _ => None,
        };
        push_focus_ring_primitives(
            &mut computed.scene,
            visual.frame,
            visual.border_radius.get(),
            focus_ring.as_ref(),
            visual.opacity,
        );

        if let Some(tree_node) = self.tree_node.as_ref() {
            push_tree_node_chrome_primitives(
                tree_node,
                self.id,
                visual,
                context,
                &mut computed.scene,
            );
        }

        let paints_surface = visual.opacity > 0.0
            && (background_blur > 0.0
                || shadow.is_some()
                || background_image.is_some()
                || background_brush.is_some()
                || visual.background.a > 0
                || (visual.border_width > Dp::ZERO && visual.border_color.a > 0));

        if visual.disabled {
            computed.hit_regions.push(HitRegion {
                rect: visual.frame,
                clip_rect: visual.primitive_clip,
                geometry: HitGeometry::Rect,
                scope_path: context.focus_scope_path(),
                focus: None,
                interaction: HitInteraction::Disabled { id: self.id },
            });
        } else if !matches!(&self.kind, ResolvedWidgetKind::Text { text } if text.user_select)
            && !matches!(&self.kind, ResolvedWidgetKind::Select { .. })
        {
            let fallback_focusable = matches!(
                self.kind,
                ResolvedWidgetKind::Button { .. }
                    | ResolvedWidgetKind::Checkbox { .. }
                    | ResolvedWidgetKind::Radio { .. }
                    | ResolvedWidgetKind::Switch { .. }
                    | ResolvedWidgetKind::Select { .. }
                    | ResolvedWidgetKind::Slider { .. }
                    | ResolvedWidgetKind::TextEditor { .. }
            ) || matches!(
                &self.kind,
                ResolvedWidgetKind::Container { layout, .. } if layout.scroll_view.is_some()
            ) || self.list_item.is_some()
                || self.tree_node.is_some()
                || self.data_grid_cell.is_some()
                || self.data_grid_header.is_some()
                || self.data_grid_resize_handle.is_some();
            let focus = context.build_focus_meta(
                self.id,
                &self.focus,
                &self.interactions,
                fallback_focusable,
            );
            if self.interactions.has_any()
                || focus.is_some()
                || self.list_item.is_some()
                || self.tree_node.is_some()
                || self.data_grid_cell.is_some()
                || self.data_grid_header.is_some()
                || self.data_grid_resize_handle.is_some()
            {
                let interaction = if let Some(trigger) = self.tab_trigger.as_ref() {
                    HitInteraction::TabTrigger {
                        id: self.id,
                        group_id: trigger.group_id,
                        index: trigger.index,
                        placement: trigger.placement,
                        key: trigger.key.clone(),
                        label: trigger.label.clone(),
                        interactions: self.interactions.clone(),
                        on_change: trigger.on_change.clone(),
                        reorderable: trigger.reorderable.resolve(),
                        on_reorder: trigger.on_reorder.clone(),
                    }
                } else if let Some(list_item) = self.list_item.as_ref() {
                    HitInteraction::ListItem {
                        id: self.id,
                        state: list_item.clone(),
                        interactions: self.interactions.clone(),
                    }
                } else if let Some(tree_node) = self.tree_node.as_ref() {
                    HitInteraction::TreeNode {
                        id: self.id,
                        state: tree_node.clone(),
                        interactions: self.interactions.clone(),
                    }
                } else if let Some(cell) = self.data_grid_cell.as_ref() {
                    HitInteraction::DataGridCell {
                        id: self.id,
                        state: cell.clone(),
                        interactions: self.interactions.clone(),
                    }
                } else if let Some(header) = self.data_grid_header.as_ref() {
                    HitInteraction::DataGridHeader {
                        id: self.id,
                        state: header.clone(),
                        interactions: self.interactions.clone(),
                    }
                } else if let Some(handle) = self.data_grid_resize_handle.as_ref() {
                    HitInteraction::DataGridResizeHandle {
                        id: self.id,
                        state: handle.clone(),
                        interactions: self.interactions.clone(),
                    }
                } else {
                    let keyboard_click_activation =
                        self.focus.focusable.unwrap_or(fallback_focusable)
                            && self.interactions.on_click.is_some();
                    HitInteraction::Widget {
                        id: self.id,
                        interactions: self.interactions.clone(),
                        focusable: fallback_focusable,
                        default_activation: match self.kind {
                            ResolvedWidgetKind::Button { .. } => DefaultActivation::EnterAndSpace,
                            _ if keyboard_click_activation => DefaultActivation::EnterAndSpace,
                            _ => DefaultActivation::None,
                        },
                    }
                };
                computed.hit_regions.push(HitRegion {
                    rect: visual.frame,
                    clip_rect: visual.primitive_clip,
                    geometry: HitGeometry::Rect,
                    scope_path: context.focus_scope_path(),
                    focus,
                    interaction,
                });
                if let Some(tree_node) = self.tree_node.as_ref() {
                    push_tree_node_control_hit_regions(
                        tree_node,
                        self.id,
                        &self.interactions,
                        visual,
                        context,
                        &mut computed.hit_regions,
                    );
                }
            } else if matches!(self.kind, ResolvedWidgetKind::Container { .. }) && paints_surface {
                computed.hit_regions.push(HitRegion {
                    rect: visual.frame,
                    clip_rect: visual.primitive_clip,
                    geometry: HitGeometry::Rect,
                    scope_path: context.focus_scope_path(),
                    focus: None,
                    interaction: HitInteraction::Occluder { id: self.id },
                });
            }
        }
    }
}

fn push_tree_node_control_hit_regions<VM>(
    tree_node: &TreeNodeState<VM>,
    widget_id: WidgetId,
    interactions: &InteractionHandlers<VM>,
    visual: &CollectVisualState,
    context: &CollectContext<'_, '_>,
    hit_regions: &mut smallvec::SmallVec<[HitRegion<VM>; 1]>,
) {
    if tree_node.disabled.resolve() {
        return;
    }
    let content_frame = visual.frame.inset(tree_node.item_padding);
    if content_frame.is_empty() {
        return;
    }

    let disclosure_x = content_frame.x + tree_node.indent_width * tree_node.depth as f32;
    let disclosure_slot = Rect::new(
        disclosure_x,
        content_frame.y,
        tree_node.disclosure_width,
        content_frame.height,
    );
    let scope_path = context.focus_scope_path();
    if tree_node.has_children && !disclosure_slot.is_empty() {
        hit_regions.push(HitRegion {
            rect: disclosure_slot,
            clip_rect: visual.primitive_clip,
            geometry: HitGeometry::Rect,
            scope_path: scope_path.clone(),
            focus: None,
            interaction: HitInteraction::TreeDisclosure {
                id: widget_id,
                state: tree_node.clone(),
                interactions: interactions.clone(),
            },
        });
    }

    if tree_node.checkable.resolve() {
        let checkbox_slot = Rect::new(
            disclosure_slot.right(),
            content_frame.y,
            tree_node.checkbox_width,
            content_frame.height,
        );
        if !checkbox_slot.is_empty() {
            hit_regions.push(HitRegion {
                rect: checkbox_slot,
                clip_rect: visual.primitive_clip,
                geometry: HitGeometry::Rect,
                scope_path,
                focus: None,
                interaction: HitInteraction::TreeCheckbox {
                    id: widget_id,
                    state: tree_node.clone(),
                    interactions: interactions.clone(),
                },
            });
        }
    }
}

fn push_tree_node_chrome_primitives<VM>(
    tree_node: &TreeNodeState<VM>,
    widget_id: WidgetId,
    visual: &CollectVisualState,
    context: &mut CollectContext<'_, '_>,
    scene: &mut ScenePrimitives,
) {
    let content_frame = visual.frame.inset(tree_node.item_padding);
    if content_frame.is_empty() || visual.opacity <= 0.0 {
        return;
    }

    let disabled = tree_node.disabled.resolve();
    push_tree_indent_guides(tree_node, content_frame, visual, scene);

    let disclosure_x = content_frame.x + tree_node.indent_width * tree_node.depth as f32;
    let disclosure_slot = Rect::new(
        disclosure_x,
        content_frame.y,
        tree_node.disclosure_width,
        content_frame.height,
    );
    if tree_node.has_children {
        let icon_color = if disabled {
            tree_node.checkbox_disabled_color.resolve()
        } else {
            tree_node.disclosure_icon_color.resolve()
        };
        if visual.widget_state.hovered && !disabled {
            push_tree_icon_hover_background(
                disclosure_slot,
                tree_node.disclosure_hover_background.resolve(),
                visual,
                scene,
            );
        }
        let target_rotation = if tree_node.expanded {
            std::f32::consts::FRAC_PI_2
        } else {
            0.0
        };
        let rotation = resolve_tree_icon_f32(
            context,
            widget_id,
            WidgetProperty::TreeDisclosureRotation,
            target_rotation,
            TREE_DISCLOSURE_TRANSITION_MS,
        );
        push_tree_icon_text(
            TREE_DISCLOSURE_ICON,
            disclosure_slot,
            context.units.resolve_sp(tree_node.disclosure_icon_size),
            icon_color,
            visual.opacity,
            rotation,
            1.0,
            visual,
            context,
            scene,
        );
    }

    if tree_node.checkable.resolve() {
        let checkbox_x = disclosure_slot.right();
        let checkbox_slot = Rect::new(
            checkbox_x,
            content_frame.y,
            tree_node.checkbox_width,
            content_frame.height,
        );
        if visual.widget_state.hovered && !disabled {
            push_tree_icon_hover_background(
                checkbox_slot,
                tree_node.disclosure_hover_background.resolve(),
                visual,
                scene,
            );
        }
        let (glyph, target_state, color) = match tree_node.check_state {
            TreeCheckState::Checked => (
                TREE_CHECKED_ICON,
                1.0,
                tree_node.checkbox_checked_color.resolve(),
            ),
            TreeCheckState::Indeterminate => (
                TREE_INDETERMINATE_ICON,
                0.5,
                tree_node.checkbox_indeterminate_color.resolve(),
            ),
            TreeCheckState::Unchecked => (
                TREE_UNCHECKED_ICON,
                0.0,
                tree_node.checkbox_unchecked_color.resolve(),
            ),
        };
        let checkbox_state = resolve_tree_icon_f32(
            context,
            widget_id,
            WidgetProperty::TreeCheckboxState,
            target_state,
            TREE_CHECKBOX_TRANSITION_MS,
        )
        .clamp(0.0, 1.0);
        let scale = match tree_node.check_state {
            TreeCheckState::Checked => 0.94 + checkbox_state * 0.06,
            TreeCheckState::Indeterminate => 0.97 + (0.5 - (checkbox_state - 0.5).abs()) * 0.06,
            TreeCheckState::Unchecked => 0.96 + (1.0 - checkbox_state) * 0.04,
        };
        let color = if disabled {
            tree_node.checkbox_disabled_color.resolve()
        } else {
            color
        };
        push_tree_icon_text(
            glyph,
            checkbox_slot,
            context.units.resolve_sp(tree_node.checkbox_icon_size),
            color,
            if disabled {
                visual.opacity * 0.72
            } else {
                visual.opacity
            },
            0.0,
            scale,
            visual,
            context,
            scene,
        );
    }
}

fn push_tree_indent_guides<VM>(
    tree_node: &TreeNodeState<VM>,
    content_frame: Rect,
    visual: &CollectVisualState,
    scene: &mut ScenePrimitives,
) {
    if tree_node.depth == 0 {
        return;
    }
    let color = tree_node.indent_line_color.resolve();
    if color.a == 0 {
        return;
    }
    for depth in 0..tree_node.depth {
        let x =
            content_frame.x + tree_node.indent_width * depth as f32 + tree_node.indent_width * 0.5;
        scene.push_shape(RenderPrimitive {
            rect: Rect::new(x, content_frame.y, dp(1.0), content_frame.height),
            color: color.with_alpha_factor(visual.opacity),
            corner_radius: 1.0,
            stroke_width: 0.0,
            clip_rect: visual.primitive_clip,
            clip_mask: visual.primitive_clip_mask,
        });
    }
}

fn push_tree_icon_hover_background(
    slot: Rect,
    color: Color,
    visual: &CollectVisualState,
    scene: &mut ScenePrimitives,
) {
    if color.a == 0 {
        return;
    }
    let side = slot.width.min(slot.height).min(dp(24.0)).max(dp(1.0));
    let frame = Rect::new(
        slot.x + ((slot.width - side).max(Dp::ZERO) * 0.5),
        slot.y + ((slot.height - side).max(Dp::ZERO) * 0.5),
        side,
        side,
    );
    scene.push_shape(RenderPrimitive {
        rect: frame,
        color: color.with_alpha_factor(visual.opacity),
        corner_radius: side.get() * 0.5,
        stroke_width: 0.0,
        clip_rect: visual.primitive_clip,
        clip_mask: visual.primitive_clip_mask,
    });
}

#[allow(clippy::too_many_arguments)]
fn push_tree_icon_text(
    content: &'static str,
    slot: Rect,
    requested_font_size: f32,
    color: Color,
    opacity: f32,
    rotation: f32,
    scale: f32,
    visual: &CollectVisualState,
    context: &CollectContext<'_, '_>,
    scene: &mut ScenePrimitives,
) {
    if color.a == 0 || opacity <= 0.0 || slot.is_empty() {
        return;
    }
    let font_size = requested_font_size
        .min(slot.width.get())
        .min(slot.height.get())
        .max(1.0);
    let frame_size = Dp::new(font_size);
    let frame = Rect::new(
        slot.x + ((slot.width - frame_size).max(Dp::ZERO) * 0.5),
        slot.y + ((slot.height - frame_size).max(Dp::ZERO) * 0.5),
        frame_size,
        frame_size,
    );
    let request = TextFontRequest {
        preferred_font: Some(ICON_FONT_FAMILY),
        weight: crate::text::font::FontWeight::Regular,
    };
    let resolved = context.font_manager.resolve_text(content, request);
    let transformed = rotation.abs() > f32::EPSILON || (scale - 1.0).abs() > f32::EPSILON;
    scene.push_text(TextPrimitive {
        content: Arc::from(content.to_string()),
        rich_spans: None,
        frame,
        quad: transformed.then(|| tree_icon_quad(frame, rotation, scale)),
        color: color.with_alpha_factor(opacity),
        force_color: true,
        font_family: Some(Arc::from(resolved.primary_font)),
        font_size,
        font_weight: crate::text::font::FontWeight::Regular,
        line_height: font_size,
        letter_spacing: 0.0,
        wrap: crate::ui::widget::CanvasTextWrap::None,
        overflow: crate::ui::widget::CanvasTextOverflow::Clip,
        horizontal_align: crate::ui::widget::CanvasTextHorizontalAlign::Center,
        vertical_align: crate::ui::widget::CanvasTextVerticalAlign::Center,
        clip_rect: visual.primitive_clip,
        clip_mask: visual.primitive_clip_mask,
    });
}

fn tree_icon_quad(frame: Rect, rotation: f32, scale: f32) -> [Point; 4] {
    let center_x = frame.x.get() + frame.width.get() * 0.5;
    let center_y = frame.y.get() + frame.height.get() * 0.5;
    let (sin, cos) = rotation.sin_cos();
    let scale = scale.max(0.0);
    let transform = |x: f32, y: f32| {
        let dx = (x - center_x) * scale;
        let dy = (y - center_y) * scale;
        Point::new(
            center_x + dx * cos - dy * sin,
            center_y + dx * sin + dy * cos,
        )
    };
    [
        transform(frame.x.get(), frame.y.get()),
        transform(frame.right().get(), frame.y.get()),
        transform(frame.right().get(), frame.bottom().get()),
        transform(frame.x.get(), frame.bottom().get()),
    ]
}

fn resolve_tree_icon_f32(
    context: &mut CollectContext<'_, '_>,
    widget_id: WidgetId,
    property: WidgetProperty,
    target: f32,
    duration_ms: u64,
) -> f32 {
    let key = crate::animation::AnimationKey::Widget {
        id: widget_id.raw(),
        property,
    };
    if context.reduced_motion {
        context
            .animations
            .resolve_f32(key, target, None, context.now)
    } else {
        context.animations.resolve_f32(
            key,
            target,
            Some(Transition::ease_out(Duration::from_millis(duration_ms))),
            context.now,
        )
    }
}
