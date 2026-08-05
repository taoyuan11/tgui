use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPath {
    pub(crate) style: CanvasItemStyle,
    pub(crate) path: PathBuilder,
    pub(crate) fill_rule: CanvasFillRule,
    pub(crate) fill: Option<Value<CanvasBrush>>,
    pub(crate) stroke: Option<CanvasStroke>,
    pub(crate) shadow: Option<Value<CanvasShadow>>,
}

impl CanvasPath {
    pub fn new(id: impl Into<CanvasItemId>, path: PathBuilder) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            path,
            fill_rule: CanvasFillRule::NonZero,
            fill: None,
            stroke: None,
            shadow: None,
        }
    }

    pub fn fill_rule(mut self, fill_rule: CanvasFillRule) -> Self {
        self.fill_rule = fill_rule;
        self
    }

    pub fn id(&self) -> CanvasItemId {
        self.style.id
    }

    pub fn name(&self) -> Option<&str> {
        self.style.name.as_deref()
    }

    pub fn path(&self) -> &PathBuilder {
        &self.path
    }

    pub fn fill_brush(&self) -> Option<&Value<CanvasBrush>> {
        self.fill.as_ref()
    }

    pub fn stroke_style(&self) -> Option<&CanvasStroke> {
        self.stroke.as_ref()
    }

    pub fn shadow_style(&self) -> Option<&Value<CanvasShadow>> {
        self.shadow.as_ref()
    }

    pub fn transform(mut self, transform: CanvasTransform2D) -> Self {
        self.style.transform = transform;
        self
    }

    pub fn name_item(mut self, name: impl Into<String>) -> Self {
        self.style.name = Some(name.into());
        self
    }

    pub fn fill(mut self, brush: impl Into<Value<CanvasBrush>>) -> Self {
        self.fill = Some(brush.into());
        self
    }

    pub fn stroke(mut self, stroke: CanvasStroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    pub fn shadow(mut self, shadow: impl Into<Value<CanvasShadow>>) -> Self {
        self.shadow = Some(shadow.into());
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.style.opacity = normalized_unit_interval(opacity);
        self
    }

    pub fn blend_mode(mut self, blend_mode: CanvasBlendMode) -> Self {
        self.style.blend_mode = blend_mode;
        self
    }

    pub fn effects(mut self, effects: impl Into<Vec<CanvasEffect>>) -> Self {
        self.style.effects = effects.into();
        self
    }

    pub fn isolation(mut self, isolation: bool) -> Self {
        self.style.isolation = isolation;
        self
    }

    pub fn cursor(mut self, cursor: CursorStyle) -> Self {
        self.style.cursor = Some(cursor);
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.style.visible = visible;
        self
    }

    pub fn hit_test(mut self, hit_test: bool) -> Self {
        self.style.hit_test = hit_test;
        self
    }
}
