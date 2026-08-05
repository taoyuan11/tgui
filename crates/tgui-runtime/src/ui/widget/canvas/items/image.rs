use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasImage {
    pub(crate) style: CanvasItemStyle,
    pub(crate) frame: Rect,
    pub(crate) source: MediaSource,
    pub(crate) fit: ContentFit,
    pub(crate) corner_radius: Dp,
    pub(crate) source_rect: Option<Rect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasImageOptions {
    pub fit: ContentFit,
    pub corner_radius: Dp,
    pub source_rect: Option<Rect>,
}

impl Default for CanvasImageOptions {
    fn default() -> Self {
        Self {
            fit: ContentFit::Contain,
            corner_radius: Dp::ZERO,
            source_rect: None,
        }
    }
}

impl CanvasImageOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fit(mut self, fit: ContentFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn corner_radius(mut self, corner_radius: impl Into<Dp>) -> Self {
        self.corner_radius = corner_radius.into();
        self
    }

    pub fn source_rect(mut self, source_rect: Rect) -> Self {
        self.source_rect = Some(source_rect);
        self
    }
}

impl CanvasImage {
    pub fn new(id: impl Into<CanvasItemId>, frame: Rect, source: impl Into<MediaSource>) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            frame,
            source: source.into(),
            fit: ContentFit::Contain,
            corner_radius: Dp::ZERO,
            source_rect: None,
        }
    }

    pub fn options(mut self, options: CanvasImageOptions) -> Self {
        self.fit = options.fit;
        self.corner_radius = options.corner_radius;
        self.source_rect = options.source_rect;
        self
    }

    pub fn id(&self) -> CanvasItemId {
        self.style.id
    }

    pub fn name(&self) -> Option<&str> {
        self.style.name.as_deref()
    }

    pub fn frame(&self) -> Rect {
        self.frame
    }

    pub fn source(&self) -> &MediaSource {
        &self.source
    }

    pub fn fit_mode(&self) -> ContentFit {
        self.fit
    }

    pub fn name_item(mut self, name: impl Into<String>) -> Self {
        self.style.name = Some(name.into());
        self
    }

    pub fn transform(mut self, transform: CanvasTransform2D) -> Self {
        self.style.transform = transform;
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
