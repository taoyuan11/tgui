use super::*;

#[derive(Clone)]
pub(super) struct CanvasRecorderState {
    pub(super) transform: CanvasTransform2D,
    pub(super) fill: Option<Value<CanvasBrush>>,
    pub(super) fill_rule: CanvasFillRule,
    pub(super) stroke: Option<CanvasStroke>,
    pub(super) shadow: Option<Value<CanvasShadow>>,
    pub(super) opacity: f32,
    pub(super) blend_mode: CanvasBlendMode,
    pub(super) effects: Vec<CanvasEffect>,
    pub(super) isolation: bool,
    pub(super) cursor: Option<CursorStyle>,
    pub(super) visible: bool,
    pub(super) hit_test: bool,
    pub(super) text_style: CanvasTextStyle,
    pub(super) paragraph_style: CanvasParagraphStyle,
}

impl Default for CanvasRecorderState {
    fn default() -> Self {
        Self {
            transform: CanvasTransform2D::IDENTITY,
            fill: Some(Value::Static(CanvasBrush::Solid(Color::BLACK))),
            fill_rule: CanvasFillRule::NonZero,
            stroke: Some(CanvasStroke::new(Dp::new(1.0), Color::BLACK)),
            shadow: None,
            opacity: 1.0,
            blend_mode: CanvasBlendMode::Normal,
            effects: Vec::new(),
            isolation: false,
            cursor: None,
            visible: true,
            hit_test: true,
            text_style: CanvasTextStyle::default(),
            paragraph_style: CanvasParagraphStyle::default(),
        }
    }
}

#[derive(Clone)]
pub(super) struct CanvasRecorderFrame {
    pub(super) state: CanvasRecorderState,
    pub(super) items: Vec<CanvasItem>,
    pub(super) group_path: Option<PathBuilder>,
    pub(super) group_mode: Option<CanvasGroupMode>,
    pub(super) grouped_items: Vec<CanvasItem>,
}

impl CanvasRecorderFrame {
    pub(super) fn new(state: CanvasRecorderState) -> Self {
        Self {
            state,
            items: Vec::new(),
            group_path: None,
            group_mode: None,
            grouped_items: Vec::new(),
        }
    }
}
