use super::*;

impl CanvasRecorder {
    pub fn begin_path(&mut self) -> &mut Self {
        self.current_path = PathBuilder::new().fill_rule(self.current_state().fill_rule);
        self
    }

    pub fn close_path(&mut self) -> &mut Self {
        self.current_path = self.current_path.clone().close();
        self
    }

    pub fn move_to(&mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> &mut Self {
        self.current_path = self.current_path.clone().move_to(x, y);
        self
    }

    pub fn line_to(&mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> &mut Self {
        self.current_path = self.current_path.clone().line_to(x, y);
        self
    }

    pub fn quad_to(
        &mut self,
        ctrl_x: impl Into<Dp>,
        ctrl_y: impl Into<Dp>,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
    ) -> &mut Self {
        self.current_path = self.current_path.clone().quad_to(ctrl_x, ctrl_y, x, y);
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
        self.current_path = self
            .current_path
            .clone()
            .cubic_to(ctrl1_x, ctrl1_y, ctrl2_x, ctrl2_y, x, y);
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
        self.current_path =
            self.current_path
                .clone()
                .arc(center_x, center_y, radius, start_angle, sweep_angle);
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
        self.current_path = self
            .current_path
            .clone()
            .arc_to(ctrl_x, ctrl_y, x, y, radius);
        self
    }

    pub fn rect(
        &mut self,
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
    ) -> &mut Self {
        self.current_path = self.current_path.clone().rect(x, y, width, height);
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
        self.current_path = self
            .current_path
            .clone()
            .rounded_rect(x, y, width, height, radius);
        self
    }

    pub fn circle(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius: impl Into<Dp>,
    ) -> &mut Self {
        self.current_path = self.current_path.clone().circle(center_x, center_y, radius);
        self
    }

    pub fn ellipse(
        &mut self,
        center_x: impl Into<Dp>,
        center_y: impl Into<Dp>,
        radius_x: impl Into<Dp>,
        radius_y: impl Into<Dp>,
    ) -> &mut Self {
        self.current_path = self
            .current_path
            .clone()
            .ellipse(center_x, center_y, radius_x, radius_y);
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
        let mut text = CanvasText::new(self.take_item_id(), frame, content)
            .text_style(self.current_state().text_style.clone())
            .paragraph_style(self.current_state().paragraph_style.clone())
            .transform(self.current_state().transform)
            .opacity(self.current_state().opacity)
            .blend_mode(self.current_state().blend_mode)
            .effects(self.current_state().effects.clone())
            .isolation(self.current_state().isolation)
            .visible(self.current_state().visible)
            .hit_test(self.current_state().hit_test);
        if let Some(name) = pending_name {
            text = text.name_item(name);
        }
        if let Some(cursor) = self.current_state().cursor {
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
        let mut text = CanvasText::rich(self.take_item_id(), frame, spans)
            .text_style(self.current_state().text_style.clone())
            .paragraph_style(self.current_state().paragraph_style.clone())
            .transform(self.current_state().transform)
            .opacity(self.current_state().opacity)
            .blend_mode(self.current_state().blend_mode)
            .effects(self.current_state().effects.clone())
            .isolation(self.current_state().isolation)
            .visible(self.current_state().visible)
            .hit_test(self.current_state().hit_test);
        if let Some(name) = pending_name {
            text = text.name_item(name);
        }
        if let Some(cursor) = self.current_state().cursor {
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
        let mut image = CanvasImage::new(self.take_item_id(), frame, source)
            .options(options)
            .transform(self.current_state().transform)
            .opacity(self.current_state().opacity)
            .blend_mode(self.current_state().blend_mode)
            .effects(self.current_state().effects.clone())
            .isolation(self.current_state().isolation)
            .visible(self.current_state().visible)
            .hit_test(self.current_state().hit_test);
        if let Some(name) = pending_name {
            image = image.name_item(name);
        }
        if let Some(cursor) = self.current_state().cursor {
            image = image.cursor(cursor);
        }
        self.push_item(CanvasItem::Image(image));
        self
    }

    fn draw_path_internal(&mut self, path: PathBuilder) -> &mut Self {
        let pending_name = self.take_item_name();
        let mut item = CanvasPath::new(
            self.take_item_id(),
            transform_path_builder(&path, self.current_state().transform),
        )
        .fill_rule(path.fill_rule);
        if let Some(name) = pending_name {
            item = item.name_item(name);
        }
        if let Some(fill) = self.current_state().fill.clone() {
            item = item.fill(fill);
        }
        if let Some(stroke) = self.current_state().stroke.clone() {
            item = item.stroke(stroke);
        }
        self.push_item(self.apply_path_state(item, true));
        self
    }

    fn draw_fill_shape(&mut self, path: PathBuilder) -> &mut Self {
        let pending_name = self.take_item_name();
        let mut item = CanvasPath::new(
            self.take_item_id(),
            transform_path_builder(&path, self.current_state().transform),
        )
        .fill_rule(self.current_state().fill_rule);
        if let Some(name) = pending_name {
            item = item.name_item(name);
        }
        if let Some(fill) = self.current_state().fill.clone() {
            item = item.fill(fill);
        }
        self.push_item(self.apply_path_state(item, true));
        self
    }

    fn draw_stroke_shape(&mut self, path: PathBuilder) -> &mut Self {
        let pending_name = self.take_item_name();
        let mut item = CanvasPath::new(
            self.take_item_id(),
            transform_path_builder(&path, self.current_state().transform),
        )
        .fill_rule(self.current_state().fill_rule);
        if let Some(name) = pending_name {
            item = item.name_item(name);
        }
        if let Some(stroke) = self.current_state().stroke.clone() {
            item = item.stroke(stroke);
        }
        self.push_item(self.apply_path_state(item, false));
        self
    }

    pub(super) fn transformed_current_path(&self) -> PathBuilder {
        transform_path_builder(&self.current_path, self.current_state().transform)
    }

    fn apply_path_state(&self, mut item: CanvasPath, include_shadow: bool) -> CanvasItem {
        if include_shadow {
            if let Some(shadow) = self.current_state().shadow.clone() {
                item = item.shadow(shadow);
            }
        }
        item = item
            .opacity(self.current_state().opacity)
            .blend_mode(self.current_state().blend_mode)
            .effects(self.current_state().effects.clone())
            .isolation(self.current_state().isolation)
            .visible(self.current_state().visible)
            .hit_test(self.current_state().hit_test);
        if let Some(cursor) = self.current_state().cursor {
            item = item.cursor(cursor);
        }
        CanvasItem::Path(item)
    }
}
