use std::borrow::Cow;

use super::resolved_freeze::lifecycle_snapshot;
use super::*;
use crate::ui::widget::canvas::CanvasHitGeometry;
use crate::ui::widget::common;

impl<VM> ResolvedElement<VM> {
    pub(in super::super) fn collect_subtree_cache(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
        chunks: &mut HashMap<WidgetId, ComputedScene<VM>>,
        chunk_parts: &mut HashMap<WidgetId, SceneChunkParts<VM>>,
        visual_contexts: &mut HashMap<WidgetId, VisualContextSnapshot>,
    ) -> ComputedScene<VM> {
        let owner = self.id.dependency_owner(DependencyPhase::Scene);
        track_dependency_scope(owner, || {
            self.collect_subtree_cache_tracked(
                layout_node,
                visual_context,
                context,
                lifecycle_states,
                chunks,
                chunk_parts,
                visual_contexts,
            )
        })
    }

    fn collect_subtree_cache_tracked(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
        chunks: &mut HashMap<WidgetId, ComputedScene<VM>>,
        chunk_parts: &mut HashMap<WidgetId, SceneChunkParts<VM>>,
        visual_contexts: &mut HashMap<WidgetId, VisualContextSnapshot>,
    ) -> ComputedScene<VM> {
        self.collect_runtime_lifecycle_state(lifecycle_states);
        let mut computed = ComputedScene::default();
        let layout = context
            .taffy
            .layout(layout_node.node)
            .expect("layout node should exist");
        let layout_frame = Rect::new(
            visual_context.origin.x + layout.location.x,
            visual_context.origin.y + layout.location.y,
            layout.size.width,
            layout.size.height,
        );
        let offset = self.visual.offset.resolve_widget(
            context.animations,
            self.id,
            WidgetProperty::Offset,
            context.now,
        );
        let frame = Rect::new(
            layout_frame.x + offset.x,
            layout_frame.y + offset.y,
            layout_frame.width,
            layout_frame.height,
        );
        let disabled = match &self.kind {
            ResolvedWidgetKind::Button { disabled, .. }
            | ResolvedWidgetKind::Checkbox { disabled, .. }
            | ResolvedWidgetKind::Radio { disabled, .. }
            | ResolvedWidgetKind::Switch { disabled, .. }
            | ResolvedWidgetKind::Select { disabled, .. }
            | ResolvedWidgetKind::Slider { disabled, .. }
            | ResolvedWidgetKind::TextEditor { disabled, .. } => disabled.resolve(),
            _ => false,
        };
        let widget_state = if disabled {
            WidgetState {
                disabled: true,
                ..Default::default()
            }
        } else {
            context.widget_states.get(self.id)
        };
        let opacity = visual_context.opacity
            * self.visual.opacity.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Opacity,
                context.now,
                0.0,
                1.0,
            )
            * if disabled { 0.55 } else { 1.0 };
        let button_style = match &self.kind {
            ResolvedWidgetKind::Button { style, .. } => {
                Some(resolve_button_style(style, widget_state, context.theme))
            }
            _ => None,
        };
        let select_style = match &self.kind {
            ResolvedWidgetKind::Select { style, .. } => {
                Some(resolve_select_style(style, widget_state, context.theme))
            }
            _ => None,
        };
        let slider_style = match &self.kind {
            ResolvedWidgetKind::Slider { style, .. } => {
                Some(resolve_slider_style(style, widget_state, context.theme))
            }
            _ => None,
        };
        let input_style = match &self.kind {
            ResolvedWidgetKind::TextEditor { style, .. } => {
                Some(resolve_input_style(style, widget_state))
            }
            _ => None,
        };
        let checkbox_style =
            match &self.kind {
                ResolvedWidgetKind::Checkbox { checked, style, .. } => Some(
                    resolve_checkbox_style(style, widget_state, checked.resolve(), context.theme),
                ),
                _ => None,
            };
        let radio_style = match &self.kind {
            ResolvedWidgetKind::Radio { checked, style, .. } => Some(resolve_radio_style(
                style,
                widget_state,
                checked.resolve(),
                context.theme,
            )),
            _ => None,
        };


        let border_width = match &self.kind {
            ResolvedWidgetKind::Button { .. } => {
                let button_style = button_style
                    .as_ref()
                    .expect("button style should be resolved for button widgets");
                self.visual
                    .border_width
                    .as_ref()
                    .map(|width| {
                        width.resolve_widget_to_logical(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderWidth,
                            context.now,
                            context.units,
                        )
                    })
                    .unwrap_or_else(|| context.units.resolve_dp(button_style.border_width))
            }
            ResolvedWidgetKind::Select { .. } => self
                .visual
                .border_width
                .as_ref()
                .map(|width| {
                    width.resolve_widget_to_logical(
                        context.animations,
                        self.id,
                        WidgetProperty::BorderWidth,
                        context.now,
                        context.units,
                    )
                })
                .unwrap_or_else(|| {
                    context.units.resolve_dp(
                        select_style
                            .as_ref()
                            .expect("select style should be resolved for select widgets")
                            .border_width,
                    )
                }),
            ResolvedWidgetKind::TextEditor { .. } => self
                .visual
                .border_width
                .as_ref()
                .map(|width| {
                    width.resolve_widget_to_logical(
                        context.animations,
                        self.id,
                        WidgetProperty::BorderWidth,
                        context.now,
                        context.units,
                    )
                })
                .unwrap_or_else(|| {
                    context.units.resolve_dp(
                        input_style
                            .as_ref()
                            .expect("input style should be resolved for input widgets")
                            .border_width,
                    )
                }),
            ResolvedWidgetKind::Checkbox { .. } => {
                let checkbox_style = checkbox_style
                    .as_ref()
                    .expect("checkbox style should be resolved for checkbox widgets");
                self.visual
                    .border_width
                    .as_ref()
                    .map(|width| {
                        width.resolve_widget_to_logical(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderWidth,
                            context.now,
                            context.units,
                        )
                    })
                    .unwrap_or_else(|| context.units.resolve_dp(checkbox_style.border_width))
            }
            ResolvedWidgetKind::Radio { .. } => {
                let radio_style = radio_style
                    .as_ref()
                    .expect("radio style should be resolved for radio widgets");
                self.visual
                    .border_width
                    .as_ref()
                    .map(|width| {
                        width.resolve_widget_to_logical(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderWidth,
                            context.now,
                            context.units,
                        )
                    })
                    .unwrap_or_else(|| context.units.resolve_dp(radio_style.border_width))
            }
            ResolvedWidgetKind::Switch { style, .. } => {
                let switch_style = style;
                self.visual
                    .border_width
                    .as_ref()
                    .map(|width| {
                        width.resolve_widget_to_logical(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderWidth,
                            context.now,
                            context.units,
                        )
                    })
                    .unwrap_or_else(|| {
                        context
                            .units
                            .resolve_dp(switch_style.border_width.resolve())
                    })
            }
            ResolvedWidgetKind::Slider { .. } => self
                .visual
                .border_width
                .as_ref()
                .map(|width| {
                    width.resolve_widget_to_logical(
                        context.animations,
                        self.id,
                        WidgetProperty::BorderWidth,
                        context.now,
                        context.units,
                    )
                })
                .unwrap_or_else(|| {
                    context.units.resolve_dp(
                        slider_style
                            .as_ref()
                            .expect("slider style should be resolved for slider widgets")
                            .border_width,
                    )
                }),
            _ => self
                .visual
                .border_width
                .as_ref()
                .map(|width| {
                    width.resolve_widget_to_logical(
                        context.animations,
                        self.id,
                        WidgetProperty::BorderWidth,
                        context.now,
                        context.units,
                    )
                })
                .unwrap_or(0.0),
        }
        .max(0.0);
        let border_radius = self
            .visual
            .border_radius
            .as_ref()
            .map(|radius| {
                radius.resolve_widget_to_logical(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderRadius,
                    context.now,
                    context.units,
                )
            })
            .unwrap_or_else(|| match &self.kind {
                ResolvedWidgetKind::Button { .. } => context.units.resolve_dp(
                    button_style
                        .as_ref()
                        .expect("button style should be resolved for button widgets")
                        .radius,
                ),
                ResolvedWidgetKind::Select { .. } => context.units.resolve_dp(
                    select_style
                        .as_ref()
                        .expect("select style should be resolved for select widgets")
                        .radius,
                ),
                ResolvedWidgetKind::TextEditor { .. } => context.units.resolve_dp(
                    input_style
                        .as_ref()
                        .expect("input style should be resolved for input widgets")
                        .radius,
                ),
                ResolvedWidgetKind::Checkbox { .. } => context.units.resolve_dp(
                    checkbox_style
                        .as_ref()
                        .expect("checkbox style should be resolved for checkbox widgets")
                        .radius,
                ),
                ResolvedWidgetKind::Radio { .. } => context.units.resolve_dp(
                    radio_style
                        .as_ref()
                        .expect("radio style should be resolved for radio widgets")
                        .radius,
                ),
                ResolvedWidgetKind::Switch { style, .. } => {
                    context.units.resolve_dp(style.radius.resolve())
                }
                ResolvedWidgetKind::Slider { .. } => context.units.resolve_dp(
                    slider_style
                        .as_ref()
                        .expect("slider style should be resolved for slider widgets")
                        .radius,
                ),
                _ => 0.0,
            })
            .max(0.0);
        let border_color = match &self.kind {
            ResolvedWidgetKind::Button { .. } => {
                let button_style = button_style
                    .as_ref()
                    .expect("button style should be resolved for button widgets");
                self.visual
                    .border_color
                    .as_ref()
                    .map(|color| {
                        color.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderColor,
                            context.now,
                        )
                    })
                    .unwrap_or_else(|| {
                        context.animations.resolve_color(
                            crate::animation::AnimationKey::Widget {
                                id: self.id.raw(),
                                property: WidgetProperty::BorderColor,
                            },
                            button_style.border_color,
                            Some(Transition::default()),
                            context.now,
                        )
                    })
            }
            ResolvedWidgetKind::Select { .. } => {
                let select_style = select_style
                    .as_ref()
                    .expect("select style should be resolved for select widgets");
                self.visual
                    .border_color
                    .as_ref()
                    .map(|color| {
                        color.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderColor,
                            context.now,
                        )
                    })
                    .unwrap_or_else(|| {
                        context.animations.resolve_color(
                            crate::animation::AnimationKey::Widget {
                                id: self.id.raw(),
                                property: WidgetProperty::BorderColor,
                            },
                            select_style.border,
                            Some(Transition::default()),
                            context.now,
                        )
                    })
            }
            ResolvedWidgetKind::TextEditor { .. } => {
                let input_style = input_style
                    .as_ref()
                    .expect("input style should be resolved for input widgets");
                self.visual
                    .border_color
                    .as_ref()
                    .map(|color| {
                        color.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderColor,
                            context.now,
                        )
                    })
                    .unwrap_or_else(|| {
                        context.animations.resolve_color(
                            crate::animation::AnimationKey::Widget {
                                id: self.id.raw(),
                                property: WidgetProperty::BorderColor,
                            },
                            input_style.border,
                            Some(Transition::default()),
                            context.now,
                        )
                    })
            }
            ResolvedWidgetKind::Switch { style, checked, .. } => {
                let visual_state = base_interaction_state(widget_state);
                let switch_style = if checked.resolve() {
                    resolve_stateful_widget_color(&style.border_checked, visual_state)
                } else {
                    resolve_stateful_widget_color(&style.border, visual_state)
                };
                self.visual
                    .border_color
                    .as_ref()
                    .map(|color| {
                        color.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderColor,
                            context.now,
                        )
                    })
                    .unwrap_or(switch_style)
            }
            ResolvedWidgetKind::Slider { .. } => self
                .visual
                .border_color
                .as_ref()
                .map(|color| {
                    color.resolve_widget(
                        context.animations,
                        self.id,
                        WidgetProperty::BorderColor,
                        context.now,
                    )
                })
                .unwrap_or(Color::TRANSPARENT),
            _ => self
                .visual
                .border_color
                .as_ref()
                .map(|color| {
                    color.resolve_widget(
                        context.animations,
                        self.id,
                        WidgetProperty::BorderColor,
                        context.now,
                    )
                })
                .unwrap_or(Color::TRANSPARENT),
        }
        .with_alpha_factor(opacity);
        let background = match &self.kind {
            ResolvedWidgetKind::Button { .. } => self
                .background
                .as_ref()
                .map(|background| {
                    background.resolve_widget(
                        context.animations,
                        self.id,
                        WidgetProperty::Background,
                        context.now,
                    )
                })
                .unwrap_or_else(|| {
                    let button_style = button_style
                        .as_ref()
                        .expect("button style should be resolved for button widgets");
                    context.animations.resolve_color(
                        crate::animation::AnimationKey::Widget {
                            id: self.id.raw(),
                            property: WidgetProperty::Background,
                        },
                        button_style.background,
                        Some(Transition::default()),
                        context.now,
                    )
                }),
            ResolvedWidgetKind::Select { .. } => self
                .background
                .as_ref()
                .map(|background| {
                    background.resolve_widget(
                        context.animations,
                        self.id,
                        WidgetProperty::Background,
                        context.now,
                    )
                })
                .unwrap_or(
                    select_style
                        .as_ref()
                        .expect("select style should be resolved for select widgets")
                        .background,
                ),
            ResolvedWidgetKind::TextEditor { .. } => self
                .background
                .as_ref()
                .map(|background| {
                    background.resolve_widget(
                        context.animations,
                        self.id,
                        WidgetProperty::Background,
                        context.now,
                    )
                })
                .unwrap_or(
                    input_style
                        .as_ref()
                        .expect("input style should be resolved for input widgets")
                        .background,
                ),
            ResolvedWidgetKind::Switch {
                checked,
                active_background,
                inactive_background,
                style,
                ..
            } => context.animations.resolve_color(
                crate::animation::AnimationKey::Widget {
                    id: self.id.raw(),
                    property: WidgetProperty::BackgroundAlt,
                },
                {
                    let visual_state = base_interaction_state(widget_state);
                    if checked.resolve() {
                        active_background.as_ref().map(Value::resolve).unwrap_or(
                            resolve_stateful_widget_color(&style.track_checked, visual_state),
                        )
                    } else {
                        inactive_background
                            .as_ref()
                            .map(Value::resolve)
                            .unwrap_or(resolve_stateful_widget_color(&style.track, visual_state))
                    }
                },
                Some(default_switch_transition()),
                context.now,
            ),
            ResolvedWidgetKind::Slider { .. } => self
                .background
                .as_ref()
                .map(|background| {
                    background.resolve_widget(
                        context.animations,
                        self.id,
                        WidgetProperty::Background,
                        context.now,
                    )
                })
                .unwrap_or(Color::TRANSPARENT),
            _ => self
                .background
                .as_ref()
                .map(|background| {
                    background.resolve_widget(
                        context.animations,
                        self.id,
                        WidgetProperty::Background,
                        context.now,
                    )
                })
                .unwrap_or(Color::TRANSPARENT),
        }
        .with_alpha_factor(opacity);

        let background_inset = border_width
            .min((frame.width * 0.5).get())
            .min((frame.height * 0.5).get());
        let background_frame = frame.inset(Insets::all(Dp::new(background_inset)));
        let background_radius = (border_radius - background_inset).max(0.0);
        let primitive_clip = Some(visual_context.clip_rect);
        let primitive_clip_mask = visual_context.clip_mask;
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


        if let Some(shadow) = shadow {
            if let Some(texture) = rounded_rect_shadow_texture(
                background_frame,
                background_radius,
                RoundedRectShadowSpec {
                    shadow,
                    opacity,
                    clip_rect: primitive_clip,
                    clip_mask: primitive_clip_mask,
                },
                context.media,
                context.units,
            ) {
                computed.scene.push_texture(texture);
            }
        }

        if background_blur > 0.0
            && background_frame.width > Dp::ZERO
            && background_frame.height > Dp::ZERO
        {
            computed.scene.push_backdrop_blur(BackdropBlurPrimitive {
                rect: background_frame,
                corner_radius: background_radius,
                blur_radius: background_blur,
                clip_rect: primitive_clip,
                clip_mask: primitive_clip_mask,
            });
        }

        let preserve_solid_background = matches!(self.kind, ResolvedWidgetKind::Switch { .. });

        if background_frame.width > Dp::ZERO && background_frame.height > Dp::ZERO {
            let should_draw_base_background = background.a > 0
                && (background_image.is_some()
                    || background_brush.is_none()
                    || preserve_solid_background);
            if should_draw_base_background {
                computed.scene.push_shape(RenderPrimitive {
                    rect: background_frame,
                    color: background,
                    corner_radius: background_radius,
                    stroke_width: 0.0,
                    clip_rect: primitive_clip,
                    clip_mask: primitive_clip_mask,
                });
            }

            if let Some(image) = background_image.as_ref() {
                push_background_media_texture(
                    &image.source,
                    image.fit,
                    background_frame,
                    background_radius,
                    primitive_clip,
                    primitive_clip_mask,
                    context,
                    &mut computed,
                );
            }

            if let Some(brush) = background_brush.clone() {
                computed.scene.push_brush(BrushPrimitive {
                    rect: background_frame,
                    brush,
                    corner_radius: background_radius,
                    clip_rect: primitive_clip,
                    clip_mask: primitive_clip_mask,
                });
            }
        }

        push_border_primitives(
            &mut computed.scene,
            frame,
            border_width,
            border_color,
            border_radius,
            primitive_clip,
            primitive_clip_mask,
        );
        let focus_ring = match &self.kind {
            ResolvedWidgetKind::Button { .. } => button_style
                .as_ref()
                .and_then(|style| style.focus_ring.clone()),
            ResolvedWidgetKind::Select { .. } => select_style
                .as_ref()
                .and_then(|style| style.focus_ring.clone()),
            ResolvedWidgetKind::Slider { .. } => slider_style
                .as_ref()
                .and_then(|style| style.focus_ring.clone()),
            ResolvedWidgetKind::TextEditor { .. } => None,
            ResolvedWidgetKind::Switch { style, .. } => {
                resolve_focus_ring(context.theme, style.focus_ring.as_ref(), widget_state)
            }
            _ => None,
        };
        push_focus_ring_primitives(
            &mut computed.scene,
            frame,
            border_radius,
            focus_ring.as_ref(),
            opacity,
        );

        if disabled {
            computed.hit_regions.push(HitRegion {
                rect: frame,
                clip_rect: primitive_clip,
                geometry: HitGeometry::Rect,
                interaction: HitInteraction::Disabled { id: self.id },
            });
        } else if self.interactions.has_any()
            && !matches!(&self.kind, ResolvedWidgetKind::Text { text } if text.user_select)
            && !matches!(&self.kind, ResolvedWidgetKind::Select { .. })
        {
            computed.hit_regions.push(HitRegion {
                rect: frame,
                clip_rect: primitive_clip,
                geometry: HitGeometry::Rect,
                interaction: HitInteraction::Widget {
                    id: self.id,
                    interactions: self.interactions.clone(),
                    focusable: matches!(
                        self.kind,
                        ResolvedWidgetKind::Button { .. }
                            | ResolvedWidgetKind::Checkbox { .. }
                            | ResolvedWidgetKind::Radio { .. }
                            | ResolvedWidgetKind::Switch { .. }
                            | ResolvedWidgetKind::Select { .. }
                            | ResolvedWidgetKind::Slider { .. }
                            | ResolvedWidgetKind::TextEditor { .. }
                    ),
                },
            });
        }


        match &self.kind {
            ResolvedWidgetKind::Container { layout, children } => {
                let content_bounds =
                    compute_container_content_bounds(self, children, layout_node, frame, context);
                let max_scroll = Point {
                    x: (content_bounds.right() - background_frame.right()).max(0.0),
                    y: (content_bounds.bottom() - background_frame.bottom()).max(0.0),
                };
                let requested_scroll = context
                    .scroll_offsets
                    .get(&self.id)
                    .copied()
                    .unwrap_or(Point::ZERO);
                let scroll_offset = Point {
                    x: if layout.overflow_x == Overflow::Scroll {
                        requested_scroll.x.clamp(0.0, max_scroll.x)
                    } else {
                        Dp::ZERO
                    },
                    y: if layout.overflow_y == Overflow::Scroll {
                        requested_scroll.y.clamp(0.0, max_scroll.y)
                    } else {
                        Dp::ZERO
                    },
                };
                let child_clip_rect = apply_overflow_clip(
                    visual_context.clip_rect,
                    background_frame,
                    layout.overflow_x,
                    layout.overflow_y,
                );
                let child_clip_mask = apply_overflow_clip_mask(
                    visual_context.clip_mask,
                    background_frame,
                    background_radius,
                    layout.overflow_x,
                    layout.overflow_y,
                );
                let scrollbar_geometry = compute_scrollbar_geometry(
                    background_frame,
                    content_bounds,
                    scroll_offset,
                    layout,
                    context.theme,
                    context.units,
                );
                let visible_frame = frame
                    .intersect(visual_context.clip_rect)
                    .unwrap_or(Rect::new(frame.x, frame.y, 0.0, 0.0));
                computed.scroll_regions.push(ScrollRegion {
                    id: self.id,
                    content_viewport: background_frame,
                    visible_frame,
                    content_bounds,
                    scroll_offset,
                    overflow_x: layout.overflow_x,
                    overflow_y: layout.overflow_y,
                    horizontal_track: scrollbar_geometry.horizontal_track,
                    horizontal_thumb: scrollbar_geometry.horizontal_thumb,
                    vertical_track: scrollbar_geometry.vertical_track,
                    vertical_thumb: scrollbar_geometry.vertical_thumb,
                });
                let before_children = computed.clone();
                for (child, child_layout) in children.iter().zip(layout_node.children.iter()) {
                    let child_chunk = child.collect_subtree_cache(
                        child_layout,
                        VisualContext {
                            origin: Point {
                                x: frame.x - scroll_offset.x,
                                y: frame.y - scroll_offset.y,
                            },
                            opacity,
                            clip_rect: child_clip_rect,
                            clip_mask: child_clip_mask,
                        },
                        context,
                        lifecycle_states,
                        chunks,
                        chunk_parts,
                        visual_contexts,
                    );
                    computed.extend(&child_chunk);
                }
                let mut after_children = ComputedScene::default();
                push_scrollbar_primitives(
                    &mut after_children.scene,
                    context.theme,
                    child_clip_rect,
                    opacity,
                    layout,
                    scrollbar_geometry,
                    self.id,
                    context.hovered_scrollbar,
                    context.active_scrollbar,
                );
                chunk_parts.insert(
                    self.id,
                    SceneChunkParts {
                        before_children,
                        after_children: after_children.clone(),
                    },
                );
                computed.extend(&after_children);
            }
            ResolvedWidgetKind::Text { text } => {
                let padding = text
                    .layout
                    .padding
                    .as_ref()
                    .map(|padding| {
                        padding.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Padding,
                            context.now,
                        )
                    })
                    .unwrap_or(Insets::ZERO);
                push_text_primitives(
                    text,
                    frame,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    false,
                    false,
                    padding,
                    None,
                    (text.user_select && context.selected_text == Some(self.id))
                        .then_some(context.selected_text_state)
                        .flatten(),
                    context.theme.colors.on_surface,
                    opacity,
                    self.id,
                    primitive_clip,
                    primitive_clip_mask,
                );
                if text.user_select && !disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        clip_rect: primitive_clip,
                        geometry: HitGeometry::Rect,
                        interaction: HitInteraction::SelectableText {
                            id: self.id,
                            frame,
                            padding,
                            interactions: self.interactions.clone(),
                            text_style: text.clone(),
                            text: text.content.resolve(),
                        },
                    });
                }
            }
            #[cfg(feature = "audio")]
            ResolvedWidgetKind::Audio { .. } => {}
            ResolvedWidgetKind::Image { image } => {
                let source = image.source.resolve();
                let loading_background = image
                    .background
                    .as_ref()
                    .map(|background| {
                        background.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Background,
                            context.now,
                        )
                    })
                    .unwrap_or(Color::rgba(255, 255, 255, 0));
                push_media_texture_or_placeholder(
                    self.id,
                    &source,
                    image.fit,
                    frame,
                    background_frame,
                    background_radius,
                    primitive_clip,
                    primitive_clip_mask,
                    opacity,
                    loading_background,
                    context,
                    &mut computed,
                    "image",
                );
            }
            ResolvedWidgetKind::Canvas {
                scene,
                item_interactions,
            } => {
                let scene = scene.resolve();
                let padding = self
                    .layout
                    .padding
                    .as_ref()
                    .map(|padding| {
                        padding.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Padding,
                            context.now,
                        )
                    })
                    .unwrap_or(Insets::ZERO);
                let canvas_frame = background_frame.inset(padding);
                let canvas_clip = primitive_clip.and_then(|clip| clip.intersect(canvas_frame));
                let canvas_visible = primitive_clip.is_none() || canvas_clip.is_some();
                let canvas_clip_mask = if background_radius > 0.0
                    && canvas_frame.width > Dp::ZERO
                    && canvas_frame.height > Dp::ZERO
                {
                    Some(ClipMask {
                        rect: canvas_frame,
                        corner_radius: background_radius,
                    })
                } else {
                    primitive_clip_mask
                };
                let canvas_origin = Point::new(canvas_frame.x, canvas_frame.y);

                if canvas_frame.width > Dp::ZERO
                    && canvas_frame.height > Dp::ZERO
                    && !visual_context.clip_rect.is_empty()
                    && canvas_visible
                {
                    for rendered in tessellate_canvas_scene_items(
                        &scene,
                        canvas_origin,
                        opacity,
                        canvas_clip,
                        canvas_clip_mask,
                        context.font_manager,
                        context.media,
                        context.units,
                    ) {
                        let meshes = rendered.output.meshes;
                        for command in rendered.output.commands {
                            computed.scene.push_render_command(command);
                        }
                        for texture in rendered.output.textures {
                            computed.scene.push_texture(texture);
                        }
                        for text in rendered.output.texts {
                            computed.scene.push_text(text);
                        }
                        for mesh in &meshes {
                            computed.scene.push_mesh(mesh.clone());
                        }

                        if item_interactions.has_any() {
                            if let Some(bounds) = rendered.hit_bounds {
                                let geometry = match rendered.hit_geometry.clone() {
                                    Some(CanvasHitGeometry::Quad(quad)) => {
                                        HitGeometry::Quad(quad)
                                    }
                                    Some(CanvasHitGeometry::Triangles(triangles)) => {
                                        HitGeometry::Triangles(triangles)
                                    }
                                    None => HitGeometry::Rect,
                                };
                                computed.hit_regions.push(HitRegion {
                                    rect: Rect::new(
                                        canvas_frame.x + bounds.min_x,
                                        canvas_frame.y + bounds.min_y,
                                        bounds.width(),
                                        bounds.height(),
                                    ),
                                    clip_rect: canvas_clip,
                                    geometry,
                                    interaction: HitInteraction::CanvasItem {
                                        id: self.id,
                                        item_id: rendered.item_id,
                                        item_interactions: item_interactions.clone(),
                                        cursor_style: rendered.cursor,
                                        canvas_origin,
                                        item_origin: Point::new(
                                            canvas_origin.x + rendered.local_origin.x,
                                            canvas_origin.y + rendered.local_origin.y,
                                        ),
                                        inverse_transform: rendered.inverse_transform.matrix,
                                        text_hits: rendered
                                            .text_hits
                                            .iter()
                                            .cloned()
                                            .map(|entry| {
                                                common::CanvasTextHitRegion {
                                                    hit: entry.hit,
                                                    quad: entry.quad,
                                                }
                                            })
                                            .collect::<Vec<_>>()
                                            .into(),
                                    },
                                });
                            }
                        }
                    }
                }
            }
            #[cfg(feature = "video")]
            ResolvedWidgetKind::VideoSurface { video, .. } => {
                let loading_background = video
                    .background
                    .as_ref()
                    .map(|background| {
                        background.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Background,
                            context.now,
                        )
                    })
                    .unwrap_or(Color::rgba(255, 255, 255, 0));
                push_video_texture_or_placeholder(
                    self.id,
                    video,
                    frame,
                    background_frame,
                    background_radius,
                    primitive_clip,
                    None,
                    opacity,
                    loading_background,
                    context,
                    &mut computed,
                );
            }

            ResolvedWidgetKind::Button { label, style, .. } => {
                let button_style = style.clone();
                let padding = Insets::symmetric(button_style.padding_x, button_style.padding_y);
                let button_foreground = context.animations.resolve_color(
                    crate::animation::AnimationKey::Widget {
                        id: self.id.raw(),
                        property: WidgetProperty::TextColor,
                    },
                    resolve_stateful_widget_color(&button_style.foreground, widget_state),
                    Some(Transition::default()),
                    context.now,
                );
                let label_text = text_with_typography(label.clone(), &button_style.text_style);
                push_text_primitives(
                    &label_text,
                    frame,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    false,
                    true,
                    padding,
                    None,
                    None,
                    button_foreground,
                    opacity,
                    self.id,
                    primitive_clip,
                    primitive_clip_mask,
                );
            }
            ResolvedWidgetKind::Checkbox {
                checked,
                label,
                on_change,
                ..
            } => {
                let checkbox_style = checkbox_style
                    .as_ref()
                    .expect("checkbox style should be resolved for checkbox widgets");
                push_checkbox_primitives(
                    frame,
                    checked.resolve(),
                    label.as_ref(),
                    checkbox_style,
                    opacity,
                    self.id,
                    primitive_clip,
                    primitive_clip_mask,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                );
                if !disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        clip_rect: primitive_clip,
                        geometry: HitGeometry::Rect,
                        interaction: HitInteraction::Checkbox {
                            id: self.id,
                            interactions: self.interactions.clone(),
                            on_change: on_change.clone(),
                            current: checked.resolve(),
                        },
                    });
                }
            }
            ResolvedWidgetKind::Radio {
                checked,
                label,
                on_change,
                ..
            } => {
                let radio_style = radio_style
                    .as_ref()
                    .expect("radio style should be resolved for radio widgets");
                push_radio_primitives(
                    frame,
                    checked.resolve(),
                    label.as_ref(),
                    radio_style,
                    opacity,
                    self.id,
                    primitive_clip,
                    primitive_clip_mask,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                );
                if !disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        clip_rect: primitive_clip,
                        geometry: HitGeometry::Rect,
                        interaction: HitInteraction::Radio {
                            id: self.id,
                            interactions: self.interactions.clone(),
                            on_change: on_change.clone(),
                            current: checked.resolve(),
                        },
                    });
                }
            }
            ResolvedWidgetKind::Switch {
                checked,
                on_change,
                active_thumb_color,
                inactive_thumb_color,
                style,
                ..
            } => {
                let switch_style = style;
                let padding = self
                    .layout
                    .padding
                    .as_ref()
                    .map(|padding| {
                        padding.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Padding,
                            context.now,
                        )
                    })
                    .unwrap_or(switch_style.padding);
                push_switch_primitives(
                    background_frame,
                    background_radius,
                    padding,
                    checked.resolve(),
                    active_thumb_color.as_ref().map(Value::resolve).unwrap_or(
                        resolve_stateful_widget_color(&switch_style.thumb_checked, widget_state),
                    ),
                    inactive_thumb_color.as_ref().map(Value::resolve).unwrap_or(
                        resolve_stateful_widget_color(&switch_style.thumb, widget_state),
                    ),
                    opacity,
                    self.id,
                    primitive_clip,
                    primitive_clip_mask,
                    context.animations,
                    &mut computed.scene,
                    context.now,
                );
                if !disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        clip_rect: primitive_clip,
                        geometry: HitGeometry::Rect,
                        interaction: HitInteraction::Switch {
                            id: self.id,
                            interactions: self.interactions.clone(),
                            on_change: on_change.clone(),
                            current: checked.resolve(),
                        },
                    });
                }
            }
            ResolvedWidgetKind::Slider {
                value,
                min,
                max,
                step,
                show_ticks,
                show_value_label,
                tick_count,
                value_formatter,
                on_change,
                ..
            } => {
                let slider_style = slider_style
                    .as_ref()
                    .expect("slider style should be resolved for slider widgets");
                let resolved_value =
                    common::slider_resolve_value(value.resolve(), *min, *max, *step);
                let display_value = context
                    .active_slider_value
                    .filter(|(widget_id, _)| *widget_id == self.id)
                    .map(|(_, raw_value)| {
                        common::slider_resolve_value(raw_value, *min, *max, *step)
                    })
                    .unwrap_or(resolved_value);
                let value_label = if *show_value_label {
                    Some(
                        value_formatter
                            .as_ref()
                            .map(|formatter| formatter.format(display_value))
                            .unwrap_or_else(|| format!("{display_value:.2}")),
                    )
                } else {
                    None
                };
                let geometry = push_slider_primitives(
                    frame,
                    display_value,
                    *min,
                    *max,
                    *step,
                    *show_ticks,
                    *show_value_label,
                    common::slider_tick_count(*min, *max, *step, *tick_count),
                    value_label.as_deref(),
                    slider_style,
                    opacity,
                    self.id,
                    primitive_clip,
                    primitive_clip_mask,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    context.media,
                );
                if !disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        clip_rect: primitive_clip,
                        geometry: HitGeometry::Rect,
                        interaction: HitInteraction::Slider {
                            id: self.id,
                            interactions: self.interactions.clone(),
                            on_change: on_change.clone(),
                            value: display_value,
                            min: *min,
                            max: *max,
                            step: *step,
                            track_rect: geometry.track_rect,
                            thumb_rect: geometry.thumb_rect,
                        },
                    });
                }
            }
            ResolvedWidgetKind::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                style: _,
                ..
            } => {
                let active = open
                    .as_ref()
                    .map(Value::resolve)
                    .or_else(|| context.select_open_states.get(&self.id).copied())
                    .unwrap_or(false);
                let menu_progress = context.animations.resolve_f32(
                    crate::animation::AnimationKey::Widget {
                        id: self.id.raw(),
                        property: WidgetProperty::SelectMenuOpen,
                    },
                    if active { 1.0 } else { 0.0 },
                    Some(default_select_menu_transition()),
                    context.now,
                );
                let select_style = select_style
                    .as_ref()
                    .expect("select style should be resolved for select widgets");
                let padding = Insets::symmetric(select_style.padding_x, select_style.padding_y);
                push_select_primitives(
                    frame,
                    selected_label.resolve(),
                    placeholder,
                    select_style,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    padding,
                    opacity,
                    self.id,
                    primitive_clip,
                    primitive_clip_mask,
                );
                if menu_progress > f32::EPSILON && !disabled {
                    push_select_menu_primitives(
                        self.id,
                        frame,
                        context.viewport,
                        options,
                        on_open_change.as_ref(),
                        select_style,
                        context,
                        &mut computed,
                        opacity,
                        menu_progress,
                    );
                }
                if !disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        clip_rect: primitive_clip,
                        geometry: HitGeometry::Rect,
                        interaction: HitInteraction::SelectTrigger {
                            id: self.id,
                            interactions: self.interactions.clone(),
                            on_open_change: on_open_change.clone(),
                            is_open: active,
                        },
                    });
                }
            }
            ResolvedWidgetKind::TextEditor {
                controller,
                placeholder,
                on_change,
                on_change_set,
                multiline,
                show_scrollbar,
                auto_wrap,
                ..
            } => {
                let input_style = input_style
                    .as_ref()
                    .expect("input style should be resolved for input widgets");
                let show_scrollbar = show_scrollbar.resolve();
                let auto_wrap = auto_wrap.resolve();
                let padding = Insets::symmetric(input_style.padding_x, input_style.padding_y);
                let inner = frame.inset(padding);
                let content_viewport = text_input_content_viewport(
                    frame,
                    padding,
                    *multiline,
                    show_scrollbar,
                    context.theme,
                    context.units,
                );
                let focused = context.focused_input == Some(self.id);
                let controller_revision = (!focused).then(|| controller.revision());
                let cached_layout = (!focused)
                    .then(|| {
                        context
                            .text_layout_overrides
                            .and_then(|overrides| overrides.get(&self.id))
                            .filter(|override_layout| {
                                Some(override_layout.revision) == controller_revision
                            })
                    })
                    .flatten();
                let resolved_value = if focused {
                    context
                        .focused_text_value
                        .map(Cow::Borrowed)
                        .unwrap_or_else(|| Cow::Owned(controller.text()))
                } else if let Some(override_layout) = cached_layout {
                    Cow::Borrowed(override_layout.text)
                } else {
                    Cow::Owned(controller.text())
                };
                let display_value = if resolved_value.is_empty() {
                    Cow::Owned(placeholder.resolve())
                } else {
                    Cow::Borrowed(resolved_value.as_ref())
                };
                let mut text = text_with_typography("", &input_style.text_style);
                text.color = Some(Value::Static(input_style.text));
                let precomputed_layout = if resolved_value.is_empty() {
                    None
                } else if focused {
                    context.focused_text_layout
                } else {
                    cached_layout.map(|override_layout| override_layout.layout)
                };
                let scroll_offset = context
                    .scroll_offsets
                    .get(&self.id)
                    .copied()
                    .unwrap_or(Point::ZERO);
                let text_render = push_text_input_primitives(
                    display_value.as_ref(),
                    &text,
                    frame,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    context.caret_visible && context.focused_input == Some(self.id),
                    *multiline,
                    auto_wrap,
                    show_scrollbar,
                    padding,
                    scroll_offset,
                    context
                        .focused_text_state
                        .filter(|_| context.focused_input == Some(self.id)),
                    input_style.placeholder,
                    input_style.selection,
                    input_style.caret,
                    opacity,
                    self.id,
                    precomputed_layout,
                    primitive_clip,
                    primitive_clip_mask,
                );
                if *multiline {
                    let content_bounds = Rect::new(
                        content_viewport.x,
                        content_viewport.y,
                        text_render.content_width,
                        text_render.content_height.max(content_viewport.height),
                    );
                    let overflow_x = if auto_wrap {
                        Overflow::Hidden
                    } else {
                        Overflow::Scroll
                    };
                    let overflow_y = Overflow::Scroll;
                    let mut scrollbar_layout = ContainerLayout::flow();
                    scrollbar_layout.overflow_x = overflow_x;
                    scrollbar_layout.overflow_y = overflow_y;
                    let max_scroll = Point {
                        x: (content_bounds.right() - content_viewport.right()).max(0.0),
                        y: (content_bounds.bottom() - content_viewport.bottom()).max(0.0),
                    };
                    let clamped_scroll = Point {
                        x: if overflow_x == Overflow::Scroll {
                            scroll_offset.x.clamp(0.0, max_scroll.x)
                        } else {
                            Dp::ZERO
                        },
                        y: scroll_offset.y.clamp(0.0, max_scroll.y),
                    };
                    let scrollbar_geometry = compute_scrollbar_geometry(
                        inner,
                        content_bounds,
                        clamped_scroll,
                        &scrollbar_layout,
                        context.theme,
                        context.units,
                    );
                    let visible_frame = frame
                        .intersect(visual_context.clip_rect)
                        .unwrap_or(Rect::new(frame.x, frame.y, 0.0, 0.0));
                    computed.scroll_regions.push(ScrollRegion {
                        id: self.id,
                        content_viewport,
                        visible_frame,
                        content_bounds,
                        scroll_offset: clamped_scroll,
                        overflow_x,
                        overflow_y,
                        horizontal_track: scrollbar_geometry.horizontal_track,
                        horizontal_thumb: scrollbar_geometry.horizontal_thumb,
                        vertical_track: scrollbar_geometry.vertical_track,
                        vertical_thumb: scrollbar_geometry.vertical_thumb,
                    });
                    if show_scrollbar {
                        push_scrollbar_primitives(
                            &mut computed.scene,
                            context.theme,
                            inner,
                            opacity,
                            &scrollbar_layout,
                            scrollbar_geometry,
                            self.id,
                            context.hovered_scrollbar,
                            context.active_scrollbar,
                        );
                    }
                }
                if context.focused_input == Some(self.id) {
                    computed.ime_cursor_area = text_render.ime_cursor_area;
                }
                if !disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        clip_rect: primitive_clip,
                        geometry: HitGeometry::Rect,
                        interaction: HitInteraction::TextInput {
                            id: self.id,
                            interactions: self.interactions.clone(),
                            controller: controller.clone(),
                            on_change: on_change.clone(),
                            on_change_set: on_change_set.clone(),
                            multiline: *multiline,
                            auto_wrap,
                            show_scrollbar,
                            frame,
                            padding,
                            text_style: text,
                        },
                    });
                }
            }
        }

        chunk_parts
            .entry(self.id)
            .or_insert_with(|| SceneChunkParts {
                before_children: computed.clone(),
                after_children: ComputedScene::default(),
            });
        visual_contexts.insert(self.id, visual_context.into());
        chunks.insert(self.id, computed.clone());
        computed

    }

    fn collect_runtime_lifecycle_state(
        &self,
        lifecycle_states: &mut HashMap<WidgetId, LifecycleEventState<VM>>,
    ) {
        if self.lifecycle_events.has_any() || self.requires_runtime_lifecycle() {
            lifecycle_states.insert(
                self.id,
                LifecycleEventState {
                    widget_id: self.id,
                    snapshot: lifecycle_snapshot(self),
                    handlers: self.lifecycle_events.clone(),
                },
            );
        }
    }
}
