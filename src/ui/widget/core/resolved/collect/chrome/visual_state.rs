use super::*;

mod paint;
mod styles;

impl<VM> ResolvedElement<VM> {
    pub(in super::super) fn resolve_collect_visual_state(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
    ) -> Box<CollectVisualState> {
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
        let border_color = self
            .resolve_collect_border_color(widget_state, opacity, &styles, context)
            .with_alpha_factor(opacity);
        let background = self
            .resolve_collect_background(widget_state, opacity, &styles, context)
            .with_alpha_factor(opacity);
        let background_inset = border_width
            .min((frame.width * 0.5).get())
            .min((frame.height * 0.5).get());
        let background_frame = frame.inset(Insets::all(Dp::new(background_inset)));
        let background_radius = (border_radius - background_inset).max(0.0);

        Box::new(CollectVisualState {
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
        })
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
