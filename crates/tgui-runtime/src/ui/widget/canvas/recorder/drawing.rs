use super::*;

impl CanvasRecorder {
    pub fn begin_path(&mut self) -> &mut Self {
        self.current_path = PathBuilder::new().fill_rule(self.current_state().fill_rule);
        self
    }

    pub fn close_path(&mut self) -> &mut Self {
        self.current_path.push_close();
        self
    }

    pub fn move_to(&mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> &mut Self {
        self.current_path.push_move_to(x, y);
        self
    }

    pub fn line_to(&mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> &mut Self {
        self.current_path.push_line_to(x, y);
        self
    }

    pub fn quad_to(
        &mut self,
        ctrl_x: impl Into<Dp>,
        ctrl_y: impl Into<Dp>,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
    ) -> &mut Self {
        self.current_path.push_quad_to(ctrl_x, ctrl_y, x, y);
        self
    }

    pub fn cubic_to(
        &mut self,
        ctrl1_x: impl Into<Dp>,
        ctrl1_y: impl Into<Dp>,
        ctrl2_x: impl Into<Dp>,
        ctrl2_y: impl Into<Dp>,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
    ) -> &mut Self {
        self.current_path
            .push_cubic_to(ctrl1_x, ctrl1_y, ctrl2_x, ctrl2_y, x, y);
        self
    }

    pub fn arc(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius: impl Into<Dp>,
        start_angle: f32,
        sweep_angle: f32,
    ) -> &mut Self {
        self.update_current_path(|path| {
            path.arc(center_x, center_y, radius, start_angle, sweep_angle)
        });
        self
    }

    pub fn arc_to(
        &mut self,
        ctrl_x: impl Into<Dp>,
        ctrl_y: impl Into<Dp>,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.update_current_path(|path| path.arc_to(ctrl_x, ctrl_y, x, y, radius));
        self
    }

    pub fn rect(
        &mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
    ) -> &mut Self {
        self.update_current_path(|path| path.rect(x, y, width, height));
        self
    }

    pub fn rounded_rect(
        &mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.update_current_path(|path| path.rounded_rect(x, y, width, height, radius));
        self
    }

    pub fn circle(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.update_current_path(|path| path.circle(center_x, center_y, radius));
        self
    }

    pub fn ellipse(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius_x: impl Into<Dp>,
        radius_y: impl Into<Dp>,
    ) -> &mut Self {
        self.update_current_path(|path| path.ellipse(center_x, center_y, radius_x, radius_y));
        self
    }

    pub fn svg_path(&mut self, data: &str) -> Result<&mut Self, CanvasSvgPathError> {
        self.current_path = self.current_path.clone().svg_path(data)?;
        Ok(self)
    }

    pub fn draw_path(&mut self, path: impl Into<PathBuilder>) -> &mut Self {
        self.draw_path_internal(path.into())
    }

    pub fn fill(&mut self) -> &mut Self {
        let mut item = CanvasPath::new(self.take_item_id(), self.transformed_current_path())
            .fill_rule(self.current_state().fill_rule);
        if let Some(fill) = self.current_state().fill.clone() {
            item = item.fill(fill);
        }
        self.push_item(self.apply_path_state(item, true));
        self
    }

    pub fn stroke(&mut self) -> &mut Self {
        let mut item = CanvasPath::new(self.take_item_id(), self.transformed_current_path())
            .fill_rule(self.current_state().fill_rule);
        if let Some(stroke) = self.current_state().stroke.clone() {
            item = item.stroke(stroke);
        }
        self.push_item(self.apply_path_state(item, false));
        self
    }

    pub fn fill_and_stroke(&mut self) -> &mut Self {
        let mut item = CanvasPath::new(self.take_item_id(), self.transformed_current_path())
            .fill_rule(self.current_state().fill_rule);
        if let Some(fill) = self.current_state().fill.clone() {
            item = item.fill(fill);
        }
        if let Some(stroke) = self.current_state().stroke.clone() {
            item = item.stroke(stroke);
        }
        self.push_item(self.apply_path_state(item, true));
        self
    }

    pub fn fill_rect(
        &mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_fill_shape(PathBuilder::new().rect(x, y, width, height))
    }

    pub fn stroke_rect(
        &mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_stroke_shape(PathBuilder::new().rect(x, y, width, height))
    }

    pub fn fill_round_rect(
        &mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_fill_shape(PathBuilder::new().rounded_rect(x, y, width, height, radius))
    }

    pub fn stroke_round_rect(
        &mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_stroke_shape(PathBuilder::new().rounded_rect(x, y, width, height, radius))
    }

    pub fn fill_circle(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_fill_shape(PathBuilder::new().circle(center_x, center_y, radius))
    }

    pub fn stroke_circle(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_stroke_shape(PathBuilder::new().circle(center_x, center_y, radius))
    }

    pub fn fill_ellipse(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius_x: impl Into<Dp>,
        radius_y: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_fill_shape(PathBuilder::new().ellipse(center_x, center_y, radius_x, radius_y))
    }

    pub fn stroke_ellipse(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius_x: impl Into<Dp>,
        radius_y: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_stroke_shape(PathBuilder::new().ellipse(center_x, center_y, radius_x, radius_y))
    }

    pub fn draw_line(
        &mut self,
        start_x: impl Into<Dp>,
        start_y: impl Into<Dp>,
        end_x: impl Into<Dp>,
        end_y: impl Into<Dp>,
    ) -> &mut Self {
        self.draw_stroke_shape(
            PathBuilder::new()
                .move_to(start_x, start_y)
                .line_to(end_x, end_y),
        )
    }

    pub fn draw_svg_path(&mut self, data: &str) -> Result<&mut Self, CanvasSvgPathError> {
        let path = PathBuilder::new()
            .fill_rule(self.current_state().fill_rule)
            .svg_path(data)?;
        Ok(self.draw_path_internal(path))
    }

    pub fn draw_text(&mut self, frame: Rect, content: impl Into<String>) -> &mut Self {
        let pending_name = self.take_item_name();
        let item_id = self.take_item_id();
        let (
            text_style,
            paragraph_style,
            transform,
            opacity,
            blend_mode,
            effects,
            isolation,
            visible,
            hit_test,
            cursor,
        ) = {
            let state = self.current_state();
            (
                state.text_style.clone(),
                state.paragraph_style.clone(),
                state.transform,
                state.opacity,
                state.blend_mode,
                state.effects.clone(),
                state.isolation,
                state.visible,
                state.hit_test,
                state.cursor,
            )
        };
        let mut text = CanvasText::new(item_id, frame, content)
            .text_style(text_style)
            .paragraph_style(paragraph_style)
            .transform(transform)
            .opacity(opacity)
            .blend_mode(blend_mode)
            .effects(effects)
            .isolation(isolation)
            .visible(visible)
            .hit_test(hit_test);
        if let Some(name) = pending_name {
            text = text.name_item(name);
        }
        if let Some(cursor) = cursor {
            text = text.cursor(cursor);
        }
        self.push_item(CanvasItem::Text(text));
        self
    }

    pub fn draw_rich_text(
        &mut self,
        frame: Rect,
        spans: impl Into<Vec<CanvasTextSpan>>,
    ) -> &mut Self {
        let pending_name = self.take_item_name();
        let item_id = self.take_item_id();
        let (
            text_style,
            paragraph_style,
            transform,
            opacity,
            blend_mode,
            effects,
            isolation,
            visible,
            hit_test,
            cursor,
        ) = {
            let state = self.current_state();
            (
                state.text_style.clone(),
                state.paragraph_style.clone(),
                state.transform,
                state.opacity,
                state.blend_mode,
                state.effects.clone(),
                state.isolation,
                state.visible,
                state.hit_test,
                state.cursor,
            )
        };
        let mut text = CanvasText::rich(item_id, frame, spans)
            .text_style(text_style)
            .paragraph_style(paragraph_style)
            .transform(transform)
            .opacity(opacity)
            .blend_mode(blend_mode)
            .effects(effects)
            .isolation(isolation)
            .visible(visible)
            .hit_test(hit_test);
        if let Some(name) = pending_name {
            text = text.name_item(name);
        }
        if let Some(cursor) = cursor {
            text = text.cursor(cursor);
        }
        self.push_item(CanvasItem::Text(text));
        self
    }

    pub fn draw_image(&mut self, frame: Rect, source: impl Into<MediaSource>) -> &mut Self {
        self.draw_image_with_options(frame, source, CanvasImageOptions::default())
    }

    pub fn draw_image_with_options(
        &mut self,
        frame: Rect,
        source: impl Into<MediaSource>,
        options: CanvasImageOptions,
    ) -> &mut Self {
        let pending_name = self.take_item_name();
        let item_id = self.take_item_id();
        let (transform, opacity, blend_mode, effects, isolation, visible, hit_test, cursor) = {
            let state = self.current_state();
            (
                state.transform,
                state.opacity,
                state.blend_mode,
                state.effects.clone(),
                state.isolation,
                state.visible,
                state.hit_test,
                state.cursor,
            )
        };
        let mut image = CanvasImage::new(item_id, frame, source)
            .options(options)
            .transform(transform)
            .opacity(opacity)
            .blend_mode(blend_mode)
            .effects(effects)
            .isolation(isolation)
            .visible(visible)
            .hit_test(hit_test);
        if let Some(name) = pending_name {
            image = image.name_item(name);
        }
        if let Some(cursor) = cursor {
            image = image.cursor(cursor);
        }
        self.push_item(CanvasItem::Image(image));
        self
    }

    fn draw_path_internal(&mut self, path: PathBuilder) -> &mut Self {
        let pending_name = self.take_item_name();
        let fill_rule = path.fill_rule;
        let item_id = self.take_item_id();
        let (transform, fill, stroke) = {
            let state = self.current_state();
            (state.transform, state.fill.clone(), state.stroke.clone())
        };
        let path = transform_path_builder_owned(path, transform);
        let mut item = CanvasPath::new(item_id, path).fill_rule(fill_rule);
        if let Some(name) = pending_name {
            item = item.name_item(name);
        }
        if let Some(fill) = fill {
            item = item.fill(fill);
        }
        if let Some(stroke) = stroke {
            item = item.stroke(stroke);
        }
        self.push_item(self.apply_path_state(item, true));
        self
    }

    fn draw_fill_shape(&mut self, path: PathBuilder) -> &mut Self {
        let pending_name = self.take_item_name();
        let item_id = self.take_item_id();
        let (transform, fill_rule, fill) = {
            let state = self.current_state();
            (state.transform, state.fill_rule, state.fill.clone())
        };
        let path = transform_path_builder_owned(path, transform);
        let mut item = CanvasPath::new(item_id, path).fill_rule(fill_rule);
        if let Some(name) = pending_name {
            item = item.name_item(name);
        }
        if let Some(fill) = fill {
            item = item.fill(fill);
        }
        self.push_item(self.apply_path_state(item, true));
        self
    }

    fn draw_stroke_shape(&mut self, path: PathBuilder) -> &mut Self {
        let pending_name = self.take_item_name();
        let item_id = self.take_item_id();
        let (transform, fill_rule, stroke) = {
            let state = self.current_state();
            (state.transform, state.fill_rule, state.stroke.clone())
        };
        let path = transform_path_builder_owned(path, transform);
        let mut item = CanvasPath::new(item_id, path).fill_rule(fill_rule);
        if let Some(name) = pending_name {
            item = item.name_item(name);
        }
        if let Some(stroke) = stroke {
            item = item.stroke(stroke);
        }
        self.push_item(self.apply_path_state(item, false));
        self
    }

    pub(super) fn transformed_current_path(&self) -> PathBuilder {
        transform_path_builder(&self.current_path, self.current_state().transform)
    }

    fn apply_path_state(&self, mut item: CanvasPath, include_shadow: bool) -> CanvasItem {
        let state = self.current_state();
        if include_shadow {
            if let Some(shadow) = state.shadow.clone() {
                item = item.shadow(shadow);
            }
        }
        item = item
            .opacity(state.opacity)
            .blend_mode(state.blend_mode)
            .effects(state.effects.clone())
            .isolation(state.isolation)
            .visible(state.visible)
            .hit_test(state.hit_test);
        if let Some(cursor) = state.cursor {
            item = item.cursor(cursor);
        }
        CanvasItem::Path(item)
    }
}
