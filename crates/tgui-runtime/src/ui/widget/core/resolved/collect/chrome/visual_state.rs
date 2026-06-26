use super::*;

mod paint;
mod styles;

#[derive(Clone, Copy)]
struct DataGridStickyInfo {
    scroll_container_id: WidgetId,
    pin: crate::ui::widget::DataGridColumnPin,
    pin_offset: Dp,
    start_pin_extent: Dp,
    end_pin_extent: Dp,
    is_header: bool,
}

impl<VM> ResolvedElement<VM> {
    pub(in super::super) fn resolve_collect_visual_state(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> CollectVisualState {
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
        let disabled = self.collect_visual_disabled_state();
        let widget_state = self.collect_widget_state(disabled, context);
        let (runtime_background, runtime_visual) =
            self.resolve_runtime_visual(widget_state, context);
        let offset = track_property_scope(PropertySlot::Offset, || {
            runtime_visual.offset.resolve_widget(
                context.animations,
                self.id,
                WidgetProperty::Offset,
                context.now,
            )
        });
        let frame = Rect::new(
            layout_frame.x + offset.x,
            layout_frame.y + offset.y,
            layout_frame.width,
            layout_frame.height,
        );
        let data_grid_sticky = self.data_grid_sticky_info();
        let frame = data_grid_sticky
            .map(|sticky| self.apply_data_grid_sticky_frame(frame, visual_context, context, sticky))
            .unwrap_or(frame);
        let scale = track_property_scope(PropertySlot::Scale, || {
            if context.reduced_motion {
                runtime_visual.scale.resolve().clamp(0.01, 16.0)
            } else {
                runtime_visual.scale.resolve_widget_clamped(
                    context.animations,
                    self.id,
                    WidgetProperty::Scale,
                    context.now,
                    0.01,
                    16.0,
                )
            }
        });
        let frame = if (scale - 1.0).abs() > f32::EPSILON {
            let width = frame.width * scale;
            let height = frame.height * scale;
            Rect::new(
                frame.x + (frame.width - width) * 0.5,
                frame.y + (frame.height - height) * 0.5,
                width,
                height,
            )
        } else {
            frame
        };
        let opacity = visual_context.opacity
            * track_property_scope(PropertySlot::Opacity, || {
                runtime_visual.opacity.resolve_widget_clamped(
                    context.animations,
                    self.id,
                    WidgetProperty::Opacity,
                    context.now,
                    0.0,
                    1.0,
                )
            })
            * if disabled { 0.55 } else { 1.0 };
        let styles = self.resolve_collect_styles(widget_state, context);
        let border_width = track_property_scope(PropertySlot::BorderWidth, || {
            self.resolve_collect_border_width(&runtime_visual, &styles, context)
        })
        .max(0.0);
        let border_radius = track_property_scope(PropertySlot::BorderRadius, || {
            self.resolve_collect_border_radius(&runtime_visual, &styles, context)
        })
        .max(0.0);
        let validation_color = self.collect_validation_color(context.theme);
        let border_color = track_property_scope(PropertySlot::BorderColor, || {
            self.resolve_collect_border_color(
                &runtime_visual,
                widget_state,
                opacity,
                validation_color,
                &styles,
                context,
            )
        })
        .with_alpha_factor(opacity);
        let background = track_property_scope(PropertySlot::Background, || {
            self.resolve_collect_background(
                runtime_background.as_ref(),
                widget_state,
                opacity,
                &styles,
                context,
            )
        })
        .with_alpha_factor(opacity);
        let background_inset = border_width
            .min((frame.width * 0.5).get())
            .min((frame.height * 0.5).get());
        let background_frame = frame.inset(Insets::all(Dp::new(background_inset)));
        let background_radius = (border_radius - background_inset).max(0.0);
        let reactive_style_background = styles
            .button_style
            .as_ref()
            .map(|style| matches!(style.background_value, Value::Signal(_)))
            .unwrap_or(false);
        let reactive_style_border_color = styles
            .button_style
            .as_ref()
            .map(|style| matches!(style.border_color_value, Value::Signal(_)))
            .unwrap_or(false);
        let reactive_border_color = matches!(runtime_visual.border_color, Some(Value::Signal(_)))
            || reactive_style_border_color;
        let reactive_offset = matches!(runtime_visual.offset, Value::Signal(_));
        let reactive_opacity = matches!(runtime_visual.opacity, Value::Signal(_));

        CollectVisualState {
            frame,
            background_frame,
            background_radius: Dp::new(background_radius),
            runtime_visual,
            offset,
            reactive_offset,
            primitive_clip: Some(
                data_grid_sticky
                    .map(|sticky| {
                        self.apply_data_grid_sticky_clip(visual_context.clip_rect, sticky)
                    })
                    .unwrap_or(visual_context.clip_rect),
            ),
            overflow_clip: visual_context.overflow_clip_rect,
            primitive_clip_mask: visual_context.clip_mask,
            disabled,
            widget_state,
            opacity,
            border_width: Dp::new(border_width),
            border_radius: Dp::new(border_radius),
            border_color,
            background,
            reactive_background: matches!(runtime_background, Some(Value::Signal(_)))
                || reactive_style_background,
            reactive_border_color,
            reactive_opacity,
            styles,
        }
    }

    fn resolve_runtime_visual(
        &self,
        widget_state: WidgetState,
        context: &CollectContext<'_, '_>,
    ) -> (Option<Value<Color>>, VisualStyle) {
        match &self.kind {
            ResolvedWidgetKind::Container { runtime_style, .. }
            | ResolvedWidgetKind::Virtual { runtime_style, .. } => {
                resolved_runtime_container_surface(
                    runtime_style,
                    &context.style_context,
                    context.style_sheet,
                    widget_state,
                )
            }
            ResolvedWidgetKind::Text { runtime_style, .. } => resolved_runtime_text_surface(
                runtime_style,
                &context.style_context,
                context.style_sheet,
                widget_state,
            ),
            ResolvedWidgetKind::Image { runtime_style, .. } => resolved_runtime_image_surface(
                runtime_style,
                &context.style_context,
                context.style_sheet,
                widget_state,
            ),
            ResolvedWidgetKind::Canvas { runtime_style, .. } => resolved_runtime_canvas_surface(
                runtime_style,
                &context.style_context,
                context.style_sheet,
                widget_state,
            ),
            #[cfg(feature = "video")]
            ResolvedWidgetKind::VideoSurface { runtime_style, .. } => {
                resolved_runtime_video_surface(
                    runtime_style,
                    &context.style_context,
                    context.style_sheet,
                    widget_state,
                )
            }
            _ => (self.background.clone(), self.visual.clone()),
        }
    }

    fn data_grid_sticky_info(&self) -> Option<DataGridStickyInfo> {
        self.data_grid_cell
            .as_ref()
            .map(|cell| DataGridStickyInfo {
                scroll_container_id: cell.scroll_container_id,
                pin: cell.pin,
                pin_offset: cell.pin_offset,
                start_pin_extent: cell.start_pin_extent,
                end_pin_extent: cell.end_pin_extent,
                is_header: false,
            })
            .or_else(|| {
                self.data_grid_header
                    .as_ref()
                    .map(|header| DataGridStickyInfo {
                        scroll_container_id: header.scroll_container_id,
                        pin: header.pin,
                        pin_offset: header.pin_offset,
                        start_pin_extent: header.start_pin_extent,
                        end_pin_extent: header.end_pin_extent,
                        is_header: true,
                    })
            })
    }

    fn apply_data_grid_sticky_frame(
        &self,
        frame: Rect,
        visual_context: VisualContext,
        context: &CollectContext<'_, '_>,
        sticky: DataGridStickyInfo,
    ) -> Rect {
        let scroll_x = context
            .scroll_offsets
            .get(&sticky.scroll_container_id)
            .copied()
            .unwrap_or(Point::ZERO)
            .x;
        match sticky.pin {
            crate::ui::widget::DataGridColumnPin::None if sticky.is_header => {
                Rect::new(frame.x - scroll_x, frame.y, frame.width, frame.height)
            }
            crate::ui::widget::DataGridColumnPin::Start if !sticky.is_header => {
                Rect::new(frame.x + scroll_x, frame.y, frame.width, frame.height)
            }
            crate::ui::widget::DataGridColumnPin::End => {
                let natural_frame = if sticky.is_header {
                    Rect::new(frame.x - scroll_x, frame.y, frame.width, frame.height)
                } else {
                    frame
                };
                let sticky_x = visual_context.clip_rect.right() - sticky.pin_offset - frame.width;
                if natural_frame.x > sticky_x {
                    Rect::new(sticky_x, frame.y, frame.width, frame.height)
                } else {
                    natural_frame
                }
            }
            _ => frame,
        }
    }

    fn apply_data_grid_sticky_clip(&self, clip_rect: Rect, sticky: DataGridStickyInfo) -> Rect {
        if sticky.pin != crate::ui::widget::DataGridColumnPin::None {
            return clip_rect;
        }

        let left = clip_rect.x + sticky.start_pin_extent;
        let right = (clip_rect.right() - sticky.end_pin_extent).max(left);
        let unpinned_clip = Rect::new(left, clip_rect.y, right - left, clip_rect.height);
        clip_rect.intersect(unpinned_clip).unwrap_or(unpinned_clip)
    }

    fn collect_visual_disabled_state(&self) -> bool {
        match &self.kind {
            ResolvedWidgetKind::Button { disabled, .. }
            | ResolvedWidgetKind::Checkbox { disabled, .. }
            | ResolvedWidgetKind::Radio { disabled, .. }
            | ResolvedWidgetKind::Switch { disabled, .. }
            | ResolvedWidgetKind::Select { disabled, .. }
            | ResolvedWidgetKind::Slider { disabled, .. }
            | ResolvedWidgetKind::TextEditor { disabled, .. } => disabled.resolve(),
            _ => false,
        }
    }

    fn collect_validation_color(&self, theme: &Theme) -> Option<Color> {
        let state = match &self.kind {
            ResolvedWidgetKind::Checkbox { validation, .. }
            | ResolvedWidgetKind::Radio { validation, .. }
            | ResolvedWidgetKind::Switch { validation, .. }
            | ResolvedWidgetKind::Select { validation, .. }
            | ResolvedWidgetKind::Slider { validation, .. }
            | ResolvedWidgetKind::TextEditor { validation, .. } => validation.resolve(),
            _ => return None,
        };
        validation_state_color(&state, theme)
    }

    fn collect_widget_state(
        &self,
        disabled: bool,
        context: &CollectContext<'_, '_>,
    ) -> WidgetState {
        let mut state = if disabled {
            WidgetState {
                disabled: true,
                ..Default::default()
            }
        } else {
            context.widget_states.get(self.id)
        };

        match &self.kind {
            ResolvedWidgetKind::Checkbox {
                checked,
                validation,
                ..
            }
            | ResolvedWidgetKind::Radio {
                checked,
                validation,
                ..
            }
            | ResolvedWidgetKind::Switch {
                checked,
                validation,
                ..
            } => {
                let checked = checked.resolve();
                state.selected = checked;
                state.checked = checked;
                state.invalid = validation.resolve().invalid;
            }
            ResolvedWidgetKind::Select {
                open, validation, ..
            } => {
                state.open = open
                    .as_ref()
                    .map(Value::resolve)
                    .or_else(|| context.select_open_states.get(&self.id).copied())
                    .unwrap_or(false);
                state.invalid = validation.resolve().invalid;
            }
            ResolvedWidgetKind::Slider { validation, .. }
            | ResolvedWidgetKind::TextEditor { validation, .. } => {
                state.invalid = validation.resolve().invalid;
            }
            _ => {}
        }

        if disabled {
            state.disabled = true;
        }
        state
    }
}

fn validation_state_color(
    state: &crate::foundation::form::ValidationVisualState,
    theme: &Theme,
) -> Option<Color> {
    if state.invalid {
        Some(theme.colors.error)
    } else if state.pending {
        Some(theme.colors.primary)
    } else {
        None
    }
}

fn apply_validation_focus_ring(
    focus_ring: &mut Option<crate::theme::FocusRingStyle>,
    color: Color,
) {
    if let Some(ring) = focus_ring.as_mut() {
        ring.color = color;
    }
}
