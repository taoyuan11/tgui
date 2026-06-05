use super::*;
use crate::ui::widget::DefaultActivation;

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
            ResolvedWidgetKind::Switch { style, .. } => resolve_focus_ring(
                context.theme,
                style.focus_ring.as_ref(),
                visual.widget_state,
            ),
            _ => None,
        };
        push_focus_ring_primitives(
            &mut computed.scene,
            visual.frame,
            visual.border_radius.get(),
            focus_ring.as_ref(),
            visual.opacity,
        );

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
            );
            let focus = context.build_focus_meta(
                self.id,
                &self.focus,
                &self.interactions,
                fallback_focusable,
            );
            if self.interactions.has_any() || focus.is_some() {
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
                } else {
                    HitInteraction::Widget {
                        id: self.id,
                        interactions: self.interactions.clone(),
                        focusable: fallback_focusable,
                        default_activation: match self.kind {
                            ResolvedWidgetKind::Button { .. } => DefaultActivation::EnterAndSpace,
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
