use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::text::font::FontWeight;
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};
use crate::ui::unit::{Dp, Sp};

use super::background::{BackgroundBrush, BackgroundImage};
use super::common::{
    CursorStyle, InteractionHandlers, MediaEventHandlers, Point, VisualStyle, WidgetId, WidgetKind,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;

#[derive(Clone)]
pub struct Text {
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) content: Value<String>,
    pub(crate) font_family: Option<String>,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) color: Option<Value<Color>>,
    pub(crate) font_size: Option<Sp>,
    pub(crate) font_weight: Option<FontWeight>,
    pub(crate) letter_spacing: Option<Sp>,
    pub(crate) cursor_style: Option<Value<CursorStyle>>,
    pub(crate) user_select: bool,
}

macro_rules! impl_text_layout_api {
    () => {
        pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
            set_layout_lengths(&mut self.layout, width, height);
            self
        }

        pub fn width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.width, width);
            self
        }

        pub fn height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.height, height);
            self
        }

        pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.min_width, width);
            self
        }

        pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.min_height, height);
            self
        }

        pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.max_width, width);
            self
        }

        pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.max_height, height);
            self
        }

        pub fn aspect_ratio(mut self, aspect_ratio: impl Into<Value<f32>>) -> Self {
            self.layout.aspect_ratio = Some(aspect_ratio.into());
            self
        }

        pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.layout.margin = insets.into();
            self
        }

        pub fn padding(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.layout.padding = Some(insets.into());
            self
        }

        pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
            self.layout.grow = grow.into();
            self
        }

        pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
            self.layout.shrink = shrink.into();
            self
        }

        pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
            self.layout.basis = Some(basis.into_length_value());
            self
        }

        pub fn align_self(mut self, align: Align) -> Self {
            self.layout.align_self = Some(align);
            self
        }

        pub fn justify_self(mut self, align: Align) -> Self {
            self.layout.justify_self = Some(align);
            self
        }

        pub fn column(mut self, start: usize) -> Self {
            self.layout.column_start = Some(start.max(1));
            self
        }

        pub fn row(mut self, start: usize) -> Self {
            self.layout.row_start = Some(start.max(1));
            self
        }

        pub fn column_span(mut self, span: usize) -> Self {
            self.layout.column_span = span.max(1);
            self
        }

        pub fn row_span(mut self, span: usize) -> Self {
            self.layout.row_span = span.max(1);
            self
        }

        pub fn position_absolute(mut self) -> Self {
            self.layout.position_type = crate::ui::layout::PositionType::Absolute;
            self
        }

        pub fn left(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.left, value);
            self
        }

        pub fn top(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.top, value);
            self
        }

        pub fn right(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.right, value);
            self
        }

        pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.bottom, value);
            self
        }

        pub fn inset(mut self, value: impl IntoLengthValue + Copy) -> Self {
            set_layout_inset(&mut self.layout.left, value);
            set_layout_inset(&mut self.layout.top, value);
            set_layout_inset(&mut self.layout.right, value);
            set_layout_inset(&mut self.layout.bottom, value);
            self
        }
    };
}

impl Text {
    pub fn new(content: impl Into<Value<String>>) -> Self {
        Self {
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            content: content.into(),
            font_family: None,
            background: None,
            color: None,
            font_size: None,
            font_weight: None,
            letter_spacing: None,
            cursor_style: None,
            user_select: false,
        }
    }

    impl_text_layout_api!();

    pub fn font(mut self, font_family: impl Into<String>) -> Self {
        self.font_family = Some(font_family.into());
        self
    }

    pub fn background(mut self, color: impl Into<Value<Color>>) -> Self {
        self.background = Some(color.into());
        self
    }

    pub fn background_brush(mut self, brush: impl Into<Value<BackgroundBrush>>) -> Self {
        self.visual.background_brush = Some(brush.into());
        self
    }

    pub fn background_image(mut self, image: impl Into<Value<BackgroundImage>>) -> Self {
        self.visual.background_image = Some(image.into());
        self
    }

    pub fn background_blur(mut self, blur: impl Into<Value<Dp>>) -> Self {
        self.visual.background_blur = blur.into();
        self
    }

    pub fn border(mut self, width: impl Into<Value<Dp>>, color: impl Into<Value<Color>>) -> Self {
        self.visual.border_width = Some(width.into());
        self.visual.border_color = Some(color.into());
        self
    }

    pub fn border_color(mut self, color: impl Into<Value<Color>>) -> Self {
        self.visual.border_color = Some(color.into());
        self
    }

    pub fn border_radius(mut self, radius: impl Into<Value<Dp>>) -> Self {
        self.visual.border_radius = Some(radius.into());
        self
    }

    pub fn border_width(mut self, width: impl Into<Value<Dp>>) -> Self {
        self.visual.border_width = Some(width.into());
        self
    }

    pub fn color(mut self, color: impl Into<Value<Color>>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn font_size(mut self, size: Sp) -> Self {
        self.font_size = Some(size.max(Sp::new(1.0)));
        self
    }

    pub fn font_weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = Some(weight);
        self
    }

    pub fn character_spacing(mut self, spacing: Sp) -> Self {
        self.letter_spacing = Some(spacing);
        self
    }

    pub fn opacity(mut self, opacity: impl Into<Value<f32>>) -> Self {
        self.visual.opacity = opacity.into();
        self
    }

    pub fn offset(mut self, offset: impl Into<Value<Point>>) -> Self {
        self.visual.offset = offset.into();
        self
    }

    pub fn on_click<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_click: Some(command),
            ..Default::default()
        })
    }

    pub fn on_double_click<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_double_click: Some(command),
            ..Default::default()
        })
    }

    pub fn on_mouse_enter<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_enter: Some(command),
            ..Default::default()
        })
    }

    pub fn on_mouse_leave<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_leave: Some(command),
            ..Default::default()
        })
    }

    pub fn on_mouse_move<VM>(self, command: ValueCommand<VM, Point>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_move: Some(command),
            ..Default::default()
        })
    }

    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.cursor_style = Some(cursor.into());
        self
    }

    pub fn user_select(mut self, user_select: bool) -> Self {
        self.user_select = user_select;
        self
    }

    fn resolved_cursor_style(&self) -> Option<Value<CursorStyle>> {
        self.cursor_style
            .clone()
            .or_else(|| self.user_select.then_some(Value::Static(CursorStyle::Text)))
    }

    fn into_element_with_interactions<VM>(
        self,
        mut interactions: InteractionHandlers<VM>,
    ) -> Element<VM> {
        let background = self.background.clone();
        let layout = self.layout.clone();
        let visual = self.visual.clone();
        interactions.cursor_style = self.resolved_cursor_style();
        Element {
            id: WidgetId::next(),
            layout,
            visual,
            interactions,
            media_events: MediaEventHandlers::default(),
            background,
            kind: WidgetKind::Text { text: self },
        }
    }
}

impl<VM> From<Text> for Element<VM> {
    fn from(value: Text) -> Self {
        let background = value.background.clone();
        let layout = value.layout.clone();
        let visual = value.visual.clone();
        Element {
            id: WidgetId::next(),
            layout,
            visual,
            interactions: InteractionHandlers {
                cursor_style: value.resolved_cursor_style(),
                ..InteractionHandlers::default()
            },
            media_events: MediaEventHandlers::default(),
            background,
            kind: WidgetKind::Text { text: value },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Text;
    use crate::Sp;

    #[test]
    fn character_spacing_updates_letter_spacing() {
        let text = Text::new("hello").character_spacing(Sp::new(2.5));
        assert_eq!(text.letter_spacing, Some(Sp::new(2.5)));
    }
}
