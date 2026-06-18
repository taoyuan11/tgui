use super::*;

mod drawing;
mod state;

use self::state::{CanvasRecorderFrame, CanvasRecorderState};

pub struct CanvasRecorder {
    frames: Vec<CanvasRecorderFrame>,
    current_path: PathBuilder,
    next_auto_id: u64,
    pending_item_id: Option<CanvasItemId>,
    pending_item_name: Option<String>,
}

impl Default for CanvasRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl CanvasRecorder {
    pub fn new() -> Self {
        Self::with_auto_ids(1_u64)
    }

    pub fn with_auto_ids(start: impl Into<CanvasItemId>) -> Self {
        let start = start.into().get();
        Self {
            frames: vec![CanvasRecorderFrame::new(CanvasRecorderState::default())],
            current_path: PathBuilder::new(),
            next_auto_id: start,
            pending_item_id: None,
            pending_item_name: None,
        }
    }

    pub fn build(builder: impl FnOnce(&mut Self)) -> CanvasScene {
        let mut recorder = Self::new();
        builder(&mut recorder);
        recorder.finish()
    }

    pub fn finish(mut self) -> CanvasScene {
        while self.frames.len() > 1 {
            self.restore();
        }
        let frame = self.frames.pop().expect("recorder root frame should exist");
        CanvasScene::from_items(self.finalize_frame(frame))
    }

    pub fn save(&mut self) -> &mut Self {
        let state = self.current_state().clone();
        self.frames.push(CanvasRecorderFrame::new(state));
        self
    }

    pub fn restore(&mut self) -> &mut Self {
        if self.frames.len() <= 1 {
            return self;
        }

        let frame = self
            .frames
            .pop()
            .expect("nested recorder frame should exist");
        let items = self.finalize_frame(frame);
        for item in items {
            self.push_item(item);
        }
        self
    }

    pub fn next_item_id(&mut self, id: impl Into<CanvasItemId>) -> &mut Self {
        self.pending_item_id = Some(id.into());
        self
    }

    pub fn next_item_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.pending_item_name = Some(name.into());
        self
    }

    pub fn clip(&mut self) -> &mut Self {
        self.begin_group(CanvasGroupMode::Clip)
    }

    pub fn mask(&mut self) -> &mut Self {
        self.begin_group(CanvasGroupMode::Mask)
    }

    pub fn translate(&mut self, x: impl Into<Dp>, y: impl Into<Dp>) -> &mut Self {
        self.current_state_mut().transform = self
            .current_state()
            .transform
            .then(CanvasTransform2D::translate(x, y));
        self
    }

    pub fn scale(&mut self, x: f32, y: f32) -> &mut Self {
        self.current_state_mut().transform = self
            .current_state()
            .transform
            .then(CanvasTransform2D::scale(x, y));
        self
    }

    pub fn rotate(&mut self, radians: f32) -> &mut Self {
        self.current_state_mut().transform = self
            .current_state()
            .transform
            .then(CanvasTransform2D::rotate(radians));
        self
    }

    pub fn transform(&mut self, transform: CanvasTransform2D) -> &mut Self {
        self.current_state_mut().transform = self.current_state().transform.then(transform);
        self
    }

    pub fn set_fill(&mut self, fill: impl Into<Value<CanvasBrush>>) -> &mut Self {
        self.current_state_mut().fill = Some(fill.into());
        self
    }

    pub fn set_fill_rule(&mut self, fill_rule: CanvasFillRule) -> &mut Self {
        self.current_state_mut().fill_rule = fill_rule;
        self.current_path = self.current_path.clone().fill_rule(fill_rule);
        self
    }

    pub fn clear_fill(&mut self) -> &mut Self {
        self.current_state_mut().fill = None;
        self
    }

    pub fn set_stroke(&mut self, stroke: CanvasStroke) -> &mut Self {
        self.current_state_mut().stroke = Some(stroke);
        self
    }

    pub fn clear_stroke(&mut self) -> &mut Self {
        self.current_state_mut().stroke = None;
        self
    }

    pub fn set_shadow(&mut self, shadow: impl Into<Value<CanvasShadow>>) -> &mut Self {
        self.current_state_mut().shadow = Some(shadow.into());
        self
    }

    pub fn clear_shadow(&mut self) -> &mut Self {
        self.current_state_mut().shadow = None;
        self
    }

    pub fn set_opacity(&mut self, opacity: f32) -> &mut Self {
        self.current_state_mut().opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn set_blend_mode(&mut self, blend_mode: CanvasBlendMode) -> &mut Self {
        self.current_state_mut().blend_mode = blend_mode;
        self
    }

    pub fn push_effect(&mut self, effect: CanvasEffect) -> &mut Self {
        self.current_state_mut().effects.push(effect);
        self
    }

    pub fn set_effects(&mut self, effects: impl Into<Vec<CanvasEffect>>) -> &mut Self {
        self.current_state_mut().effects = effects.into();
        self
    }

    pub fn clear_effects(&mut self) -> &mut Self {
        self.current_state_mut().effects.clear();
        self
    }

    pub fn set_isolation(&mut self, isolation: bool) -> &mut Self {
        self.current_state_mut().isolation = isolation;
        self
    }

    pub fn set_cursor(&mut self, cursor: CursorStyle) -> &mut Self {
        self.current_state_mut().cursor = Some(cursor);
        self
    }

    pub fn clear_cursor(&mut self) -> &mut Self {
        self.current_state_mut().cursor = None;
        self
    }

    pub fn set_hit_test(&mut self, hit_test: bool) -> &mut Self {
        self.current_state_mut().hit_test = hit_test;
        self
    }

    pub fn set_visible(&mut self, visible: bool) -> &mut Self {
        self.current_state_mut().visible = visible;
        self
    }

    pub fn set_text_style(&mut self, text_style: CanvasTextStyle) -> &mut Self {
        self.current_state_mut().text_style = text_style;
        self
    }

    pub fn set_paragraph_style(&mut self, paragraph_style: CanvasParagraphStyle) -> &mut Self {
        self.current_state_mut().paragraph_style = paragraph_style;
        self
    }

    fn begin_group(&mut self, mode: CanvasGroupMode) -> &mut Self {
        let path = self.transformed_current_path();
        if path.commands_internal().is_empty() {
            return self;
        }

        let should_flush = {
            let frame = self.current_frame();
            frame.group_path.is_some()
                && !frame.grouped_items.is_empty()
                && frame.group_mode != Some(mode.clone())
        };
        if should_flush {
            let group_item = self.take_pending_group();
            self.current_frame().items.push(group_item);
        }

        let current_mode = self.current_frame().group_mode.clone();
        let current_path = self.current_frame().group_path.clone();
        let new_group_path = match (current_mode, current_path) {
            (Some(CanvasGroupMode::Clip), Some(existing)) if mode == CanvasGroupMode::Clip => {
                existing.intersect(&path).unwrap_or(path.clone())
            }
            (Some(existing_mode), Some(existing)) if existing_mode == mode => {
                existing.intersect(&path).unwrap_or(path.clone())
            }
            _ => path,
        };
        let frame = self.current_frame();
        frame.group_mode = Some(mode);
        frame.group_path = Some(new_group_path);
        self
    }

    fn current_frame(&mut self) -> &mut CanvasRecorderFrame {
        self.frames
            .last_mut()
            .expect("recorder should always have an active frame")
    }

    fn current_state(&self) -> &CanvasRecorderState {
        &self
            .frames
            .last()
            .expect("recorder should always have an active frame")
            .state
    }

    fn current_state_mut(&mut self) -> &mut CanvasRecorderState {
        &mut self.current_frame().state
    }

    fn take_item_id(&mut self) -> CanvasItemId {
        self.pending_item_id
            .take()
            .unwrap_or_else(|| self.take_generated_id())
    }

    fn take_item_name(&mut self) -> Option<String> {
        self.pending_item_name.take()
    }

    fn take_generated_id(&mut self) -> CanvasItemId {
        let id = CanvasItemId::new(self.next_auto_id);
        self.next_auto_id = self.next_auto_id.saturating_add(1);
        id
    }

    fn push_item(&mut self, item: CanvasItem) {
        let frame = self.current_frame();
        if frame.group_path.is_some() {
            frame.grouped_items.push(item);
        } else {
            frame.items.push(item);
        }
    }

    fn finalize_frame(&mut self, mut frame: CanvasRecorderFrame) -> Vec<CanvasItem> {
        if frame.group_path.is_some() && !frame.grouped_items.is_empty() {
            let group_id = self.take_generated_id();
            let group_path = frame.group_path.take().expect("group path should exist");
            let fill_rule = group_path.fill_rule;
            let mode = frame
                .group_mode
                .take()
                .expect("group mode should exist when group path exists");
            frame.items.push(CanvasItem::Group(CanvasGroup::new(
                group_id,
                mode,
                CanvasGroupShape::Path {
                    path: group_path,
                    fill_rule,
                },
                frame.grouped_items,
            )));
        }
        frame.items
    }

    fn take_pending_group(&mut self) -> CanvasItem {
        let frame = self.current_frame();
        let group_path = frame
            .group_path
            .clone()
            .expect("group path should exist before finalizing a group");
        let grouped_items = std::mem::take(&mut frame.grouped_items);
        let mode = frame
            .group_mode
            .clone()
            .expect("group mode should exist before finalizing a group");
        let fill_rule = group_path.fill_rule;
        CanvasItem::Group(CanvasGroup::new(
            self.take_generated_id(),
            mode,
            CanvasGroupShape::Path {
                path: group_path,
                fill_rule,
            },
            grouped_items,
        ))
    }
}

#[doc(hidden)]
pub trait IntoCanvasContent {
    fn into_canvas_scene(self) -> Value<CanvasScene>;
}

impl IntoCanvasContent for CanvasScene {
    fn into_canvas_scene(self) -> Value<CanvasScene> {
        Value::Static(self)
    }
}

impl IntoCanvasContent for Value<CanvasScene> {
    fn into_canvas_scene(self) -> Value<CanvasScene> {
        self
    }
}

impl IntoCanvasContent for Signal<CanvasScene> {
    fn into_canvas_scene(self) -> Value<CanvasScene> {
        Value::Signal(self)
    }
}
