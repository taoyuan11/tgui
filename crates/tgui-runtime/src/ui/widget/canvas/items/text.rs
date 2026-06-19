use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasTextStyle {
    pub font_family: Option<String>,
    pub color: Color,
    pub font_size: Sp,
    pub font_weight: FontWeight,
    pub line_height: Option<Sp>,
    pub letter_spacing: Sp,
}

impl Default for CanvasTextStyle {
    fn default() -> Self {
        Self {
            font_family: None,
            color: Color::BLACK,
            font_size: Sp::new(14.0),
            font_weight: FontWeight::NORMAL,
            line_height: None,
            letter_spacing: Sp::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanvasTextWrap {
    #[default]
    Word,
    Glyph,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanvasTextHorizontalAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanvasTextVerticalAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CanvasTextOverflow {
    #[default]
    Clip,
    Ellipsis,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasParagraphStyle {
    pub wrap: CanvasTextWrap,
    pub horizontal_align: CanvasTextHorizontalAlign,
    pub vertical_align: CanvasTextVerticalAlign,
    pub overflow: CanvasTextOverflow,
}

impl Default for CanvasParagraphStyle {
    fn default() -> Self {
        Self {
            wrap: CanvasTextWrap::Word,
            horizontal_align: CanvasTextHorizontalAlign::Start,
            vertical_align: CanvasTextVerticalAlign::Start,
            overflow: CanvasTextOverflow::Clip,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CanvasTextContent {
    Plain(String),
    Rich(Vec<CanvasTextSpan>),
}

impl CanvasTextContent {
    pub(crate) fn plain_text(&self) -> String {
        match self {
            Self::Plain(content) => content.clone(),
            Self::Rich(spans) => spans.iter().map(|span| span.content.as_str()).collect(),
        }
    }

    pub(crate) fn plain_text_char_count(&self) -> usize {
        match self {
            Self::Plain(content) => content.chars().count(),
            Self::Rich(spans) => spans.iter().map(|span| span.content.chars().count()).sum(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasText {
    pub(crate) style: CanvasItemStyle,
    pub(crate) frame: Rect,
    pub(crate) content: CanvasTextContent,
    pub(crate) text_style: CanvasTextStyle,
    pub(crate) paragraph_style: CanvasParagraphStyle,
}

impl CanvasText {
    pub fn new(id: impl Into<CanvasItemId>, frame: Rect, content: impl Into<String>) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            frame,
            content: CanvasTextContent::Plain(content.into()),
            text_style: CanvasTextStyle::default(),
            paragraph_style: CanvasParagraphStyle::default(),
        }
    }

    pub fn rich(
        id: impl Into<CanvasItemId>,
        frame: Rect,
        spans: impl Into<Vec<CanvasTextSpan>>,
    ) -> Self {
        Self {
            style: CanvasItemStyle::new(id),
            frame,
            content: CanvasTextContent::Rich(spans.into()),
            text_style: CanvasTextStyle::default(),
            paragraph_style: CanvasParagraphStyle::default(),
        }
    }

    pub fn text_style(mut self, text_style: CanvasTextStyle) -> Self {
        self.text_style = text_style;
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

    pub fn plain_text(&self) -> String {
        self.content.plain_text()
    }

    pub(crate) fn plain_text_char_count(&self) -> usize {
        self.content.plain_text_char_count()
    }

    pub fn name_item(mut self, name: impl Into<String>) -> Self {
        self.style.name = Some(name.into());
        self
    }

    pub fn paragraph_style(mut self, paragraph_style: CanvasParagraphStyle) -> Self {
        self.paragraph_style = paragraph_style;
        self
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
}
