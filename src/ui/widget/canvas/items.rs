use super::*;

mod image;
mod text;

pub use self::image::{CanvasImage, CanvasImageOptions};
pub(crate) use self::text::CanvasTextContent;
pub use self::text::{
    CanvasParagraphStyle, CanvasText, CanvasTextHorizontalAlign, CanvasTextOverflow,
    CanvasTextStyle, CanvasTextVerticalAlign, CanvasTextWrap,
};

#[derive(Clone, Debug, PartialEq)]
pub enum CanvasGroupShape {
    Path {
        path: PathBuilder,
        fill_rule: CanvasFillRule,
    },
}

impl CanvasGroupShape {
    pub fn path(path: PathBuilder) -> Self {
        Self::Path {
            fill_rule: path.fill_rule,
            path,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CanvasGroupMode {
    Clip,
    Mask,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasGroup {
    pub(crate) style: CanvasItemStyle,
    pub(crate) mode: CanvasGroupMode,
    pub(crate) shape: CanvasGroupShape,
    pub(crate) items: Vec<CanvasItem>,
}

impl CanvasGroup {
    pub fn new(
        id: impl Into<CanvasItemId>,
        mode: CanvasGroupMode,
        shape: CanvasGroupShape,
        items: impl Into<Vec<CanvasItem>>,
    ) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            mode,
            shape,
            items: items.into(),
        }
    }

    pub fn id(&self) -> CanvasItemId {
        self.style.id
    }

    pub fn name(&self) -> Option<&str> {
        self.style.name.as_deref()
    }

    pub fn mode(&self) -> &CanvasGroupMode {
        &self.mode
    }

    pub fn shape(&self) -> &CanvasGroupShape {
        &self.shape
    }

    pub fn items(&self) -> &[CanvasItem] {
        &self.items
    }

    pub fn items_mut(&mut self) -> &mut Vec<CanvasItem> {
        &mut self.items
    }

    pub fn transform(mut self, transform: CanvasTransform2D) -> Self {
        self.style.transform = transform;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.style.opacity = opacity.clamp(0.0, 1.0);
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

    pub fn name_item(mut self, name: impl Into<String>) -> Self {
        self.style.name = Some(name.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CanvasItem {
    Path(CanvasPath),
    Text(CanvasText),
    Image(CanvasImage),
    Group(CanvasGroup),
}

impl CanvasItem {
    pub fn id(&self) -> CanvasItemId {
        match self {
            Self::Path(path) => path.style.id,
            Self::Text(text) => text.style.id,
            Self::Image(image) => image.style.id,
            Self::Group(group) => group.style.id,
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.style().name.as_deref()
    }

    pub fn kind(&self) -> CanvasItemKind {
        match self {
            Self::Path(_) => CanvasItemKind::Path,
            Self::Text(_) => CanvasItemKind::Text,
            Self::Image(_) => CanvasItemKind::Image,
            Self::Group(_) => CanvasItemKind::Group,
        }
    }

    pub fn children(&self) -> &[CanvasItem] {
        match self {
            Self::Group(group) => group.items(),
            _ => &[],
        }
    }

    pub fn children_mut(&mut self) -> Option<&mut Vec<CanvasItem>> {
        match self {
            Self::Group(group) => Some(group.items_mut()),
            _ => None,
        }
    }

    pub fn bounds_rect(&self) -> Option<Rect> {
        self.layout_bounds().map(rect_from_bounds)
    }

    pub fn hit_bounds_rect(&self) -> Option<Rect> {
        self.hit_bounds().map(rect_from_bounds)
    }

    pub(crate) fn tessellate(
        &self,
        origin: Point,
        opacity: f32,
        clip_context: CanvasClipContext,
        media: &MediaManager,
        units: UnitContext,
    ) -> CanvasRenderOutput {
        if !self.style().visible {
            return CanvasRenderOutput::default();
        }

        if item_requires_composite(self) {
            return tessellate_composite_item(self, origin, opacity, clip_context, media, units);
        }

        let mut output = match self {
            Self::Path(path) => tessellate_path(path, origin, opacity, clip_context, media, units),
            Self::Text(text) => tessellate_text(text, origin, opacity, clip_context),
            Self::Image(image) => {
                tessellate_image(image, origin, opacity, clip_context, media, units)
            }
            Self::Group(group) => {
                let nested_clip = CanvasClipContext {
                    clip_rect: compose_clip_rect(
                        clip_context.clip_rect,
                        match group.mode {
                            CanvasGroupMode::Clip => group_shape_clip_rect(&group.shape, origin),
                            CanvasGroupMode::Mask => None,
                        },
                    ),
                    clip_mask: clip_context.clip_mask,
                };
                tessellate_items(
                    &group.items,
                    origin,
                    opacity * group.style.opacity,
                    nested_clip,
                    media,
                    units,
                )
            }
        };

        apply_transform_to_output(&mut output, self.style().transform, origin);
        output
    }

    pub(crate) fn style(&self) -> &CanvasItemStyle {
        match self {
            Self::Path(path) => &path.style,
            Self::Text(text) => &text.style,
            Self::Image(image) => &image.style,
            Self::Group(group) => &group.style,
        }
    }

    pub(crate) fn layout_bounds(&self) -> Option<RectBounds> {
        let bounds = match self {
            Self::Path(path) => {
                let mut rect = path_base_bounds(path)?;
                if let Some(shadow) = path.shadow.as_ref().map(Value::resolve) {
                    rect = rect.expand_for_shadow(shadow);
                }
                Some(rect)
            }
            Self::Text(text) => Some(RectBounds::from_rect(text.frame)),
            Self::Image(image) => Some(RectBounds::from_rect(image.frame)),
            Self::Group(group) => {
                group_shape_bounds(&group.shape).or_else(|| canvas_bounds(&group.items))
            }
        }?;
        Some(transform_bounds(bounds, self.style().transform))
    }

    pub(crate) fn hit_bounds(&self) -> Option<RectBounds> {
        if !self.style().hit_test || !self.style().visible {
            return None;
        }
        let bounds = match self {
            Self::Path(path) => path_base_bounds(path),
            Self::Text(text) => Some(RectBounds::from_rect(text.frame)),
            Self::Image(image) => Some(RectBounds::from_rect(image.frame)),
            Self::Group(group) => {
                group_shape_bounds(&group.shape).or_else(|| canvas_bounds(&group.items))
            }
        }?;
        Some(transform_bounds(bounds, self.style().transform))
    }
}

impl From<CanvasPath> for CanvasItem {
    fn from(value: CanvasPath) -> Self {
        Self::Path(value)
    }
}

impl From<CanvasText> for CanvasItem {
    fn from(value: CanvasText) -> Self {
        Self::Text(value)
    }
}

impl From<CanvasImage> for CanvasItem {
    fn from(value: CanvasImage) -> Self {
        Self::Image(value)
    }
}

impl From<CanvasGroup> for CanvasItem {
    fn from(value: CanvasGroup) -> Self {
        Self::Group(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CanvasItemKind {
    Path,
    Text,
    Image,
    Group,
}

pub(crate) fn item_requires_composite(item: &CanvasItem) -> bool {
    match item {
        CanvasItem::Path(path) => {
            path.style.blend_mode != CanvasBlendMode::Normal
                || path.style.isolation
                || !path.style.effects.is_empty()
        }
        CanvasItem::Text(text) => {
            text.style.blend_mode != CanvasBlendMode::Normal
                || text.style.isolation
                || !text.style.effects.is_empty()
        }
        CanvasItem::Image(image) => {
            image.style.blend_mode != CanvasBlendMode::Normal
                || image.style.isolation
                || !image.style.effects.is_empty()
        }
        CanvasItem::Group(_) => true,
    }
}
