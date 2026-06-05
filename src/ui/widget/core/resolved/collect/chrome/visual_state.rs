use super::*;

mod paint;
mod styles;

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
        let frame = self.apply_data_grid_sticky_frame(frame, visual_context, context);
        let scale = if context.reduced_motion {
            self.visual.scale.resolve().clamp(0.01, 16.0)
        } else {
            self.visual.scale.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Scale,
                context.now,
                0.01,
                16.0,
            )
        };
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
        let disabled = self.collect_visual_disabled_state();
        let widget_state = self.collect_widget_state(disabled, context);
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
        let styles = self.resolve_collect_styles(widget_state, context.theme);
        let border_width = self.resolve_collect_border_width(&styles, context).max(0.0);
        let border_radius = self
            .resolve_collect_border_radius(&styles, context)
            .max(0.0);
        let validation_color = self.collect_validation_color(context.theme);
        let border_color = self
            .resolve_collect_border_color(widget_state, opacity, validation_color, &styles, context)
            .with_alpha_factor(opacity);
        let background = self
            .resolve_collect_background(widget_state, opacity, &styles, context)
            .with_alpha_factor(opacity);
        let background_inset = border_width
            .min((frame.width * 0.5).get())
            .min((frame.height * 0.5).get());
        let background_frame = frame.inset(Insets::all(Dp::new(background_inset)));
        let background_radius = (border_radius - background_inset).max(0.0);

        CollectVisualState {
            frame,
            background_frame,
            background_radius: Dp::new(background_radius),
            primitive_clip: Some(visual_context.clip_rect),
            primitive_clip_mask: visual_context.clip_mask,
            disabled,
            widget_state,
            opacity,
            border_width: Dp::new(border_width),
            border_radius: Dp::new(border_radius),
            border_color,
            background,
            styles,
        }
    }

    fn apply_data_grid_sticky_frame(
        &self,
        frame: Rect,
        visual_context: VisualContext,
        context: &CollectContext<'_, '_>,
    ) -> Rect {
        let Some((scroll_container_id, pin, pin_offset, is_header)) = self
            .data_grid_cell
            .as_ref()
            .map(|cell| (cell.scroll_container_id, cell.pin, cell.pin_offset, false))
            .or_else(|| {
                self.data_grid_header.as_ref().map(|header| {
                    (
                        header.scroll_container_id,
                        header.pin,
                        header.pin_offset,
                        true,
                    )
                })
            })
        else {
            return frame;
        };
        let scroll_x = context
            .scroll_offsets
            .get(&scroll_container_id)
            .copied()
            .unwrap_or(Point::ZERO)
            .x;
        match pin {
            crate::ui::widget::DataGridColumnPin::None if is_header => {
                Rect::new(frame.x - scroll_x, frame.y, frame.width, frame.height)
            }
            crate::ui::widget::DataGridColumnPin::Start if !is_header => {
                Rect::new(frame.x + scroll_x, frame.y, frame.width, frame.height)
            }
            crate::ui::widget::DataGridColumnPin::End => Rect::new(
                visual_context.clip_rect.right() - pin_offset - frame.width,
                frame.y,
                frame.width,
                frame.height,
            ),
            _ => frame,
        }
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
        if disabled {
            WidgetState {
                disabled: true,
                ..Default::default()
            }
        } else {
            context.widget_states.get(self.id)
        }
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
