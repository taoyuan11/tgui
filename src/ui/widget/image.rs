use crate::foundation::binding::Binding;
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::media::{ContentFit, MediaBytes, MediaSource};
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};
use crate::ui::unit::Dp;

use super::common::{
    CursorStyle, InteractionHandlers, MediaEventHandlers, Point, VisualStyle, WidgetId, WidgetKind,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;

#[derive(Clone)]
pub struct Image {
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) source: Value<MediaSource>,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) fit: ContentFit,
    pub(crate) cursor_style: Option<Value<CursorStyle>>,
}

macro_rules! impl_image_layout_api {
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
            self.layout.padding = insets.into();
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

pub trait IntoImagePathSource {
    fn into_image_path_source(self) -> Value<MediaSource>;
}

pub trait IntoImageUrlSource {
    fn into_image_url_source(self) -> Value<MediaSource>;
}

impl IntoImagePathSource for std::path::PathBuf {
    fn into_image_path_source(self) -> Value<MediaSource> {
        MediaSource::Path(self).into()
    }
}

impl IntoImagePathSource for &std::path::Path {
    fn into_image_path_source(self) -> Value<MediaSource> {
        MediaSource::Path(self.to_path_buf()).into()
    }
}

impl IntoImagePathSource for String {
    fn into_image_path_source(self) -> Value<MediaSource> {
        MediaSource::Path(self.into()).into()
    }
}

impl IntoImagePathSource for &str {
    fn into_image_path_source(self) -> Value<MediaSource> {
        MediaSource::Path(self.into()).into()
    }
}

impl IntoImagePathSource for Binding<std::path::PathBuf> {
    fn into_image_path_source(self) -> Value<MediaSource> {
        self.map(MediaSource::Path).into()
    }
}

impl IntoImagePathSource for Binding<String> {
    fn into_image_path_source(self) -> Value<MediaSource> {
        self.map(|path| MediaSource::Path(path.into())).into()
    }
}

impl IntoImagePathSource for Value<std::path::PathBuf> {
    fn into_image_path_source(self) -> Value<MediaSource> {
        match self {
            Value::Static(path) => MediaSource::Path(path).into(),
            Value::Bound(binding) => binding.map(MediaSource::Path).into(),
        }
    }
}

impl IntoImagePathSource for Value<String> {
    fn into_image_path_source(self) -> Value<MediaSource> {
        match self {
            Value::Static(path) => MediaSource::Path(path.into()).into(),
            Value::Bound(binding) => binding.map(|path| MediaSource::Path(path.into())).into(),
        }
    }
}

impl IntoImageUrlSource for String {
    fn into_image_url_source(self) -> Value<MediaSource> {
        MediaSource::Url(self).into()
    }
}

impl IntoImageUrlSource for &str {
    fn into_image_url_source(self) -> Value<MediaSource> {
        MediaSource::Url(self.into()).into()
    }
}

impl IntoImageUrlSource for Binding<String> {
    fn into_image_url_source(self) -> Value<MediaSource> {
        self.map(MediaSource::Url).into()
    }
}

impl IntoImageUrlSource for Value<String> {
    fn into_image_url_source(self) -> Value<MediaSource> {
        match self {
            Value::Static(url) => MediaSource::Url(url).into(),
            Value::Bound(binding) => binding.map(MediaSource::Url).into(),
        }
    }
}

impl Image {
    pub fn new(source: impl Into<Value<MediaSource>>) -> Self {
        Self {
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            source: source.into(),
            background: None,
            fit: ContentFit::Contain,
            cursor_style: None,
        }
    }

    pub fn from_path(path: impl IntoImagePathSource) -> Self {
        Self::new(path.into_image_path_source())
    }

    pub fn from_url(url: impl IntoImageUrlSource) -> Self {
        Self::new(url.into_image_url_source())
    }

    pub fn from_bytes(bytes: impl Into<MediaBytes>) -> Self {
        Self::new(MediaSource::Bytes(bytes.into()))
    }

    impl_image_layout_api!();

    pub fn fit(mut self, fit: ContentFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn background(mut self, color: impl Into<Value<Color>>) -> Self {
        self.background = Some(color.into());
        self
    }

    pub fn border(mut self, width: impl Into<Value<Dp>>, color: impl Into<Value<Color>>) -> Self {
        self.visual.border_width = width.into();
        self.visual.border_color = color.into();
        self
    }

    pub fn border_color(mut self, color: impl Into<Value<Color>>) -> Self {
        self.visual.border_color = color.into();
        self
    }

    pub fn border_radius(mut self, radius: impl Into<Value<Dp>>) -> Self {
        self.visual.border_radius = radius.into();
        self
    }

    pub fn border_width(mut self, width: impl Into<Value<Dp>>) -> Self {
        self.visual.border_width = width.into();
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

    pub fn on_loading<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_loading: Some(command),
            ..Default::default()
        })
    }

    pub fn on_success<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_success: Some(command),
            ..Default::default()
        })
    }

    pub fn on_error<VM>(self, command: ValueCommand<VM, String>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_error: Some(command),
            ..Default::default()
        })
    }

    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.cursor_style = Some(cursor.into());
        self
    }

    fn into_element_with_interactions<VM>(
        self,
        mut interactions: InteractionHandlers<VM>,
    ) -> Element<VM> {
        interactions.cursor_style = self.cursor_style.clone();
        Element {
            id: WidgetId::next(),
            layout: self.layout.clone(),
            visual: self.visual.clone(),
            interactions,
            media_events: MediaEventHandlers::default(),
            background: self.background.clone(),
            kind: WidgetKind::Image { image: self },
        }
    }

    fn into_element_with_media_events<VM>(
        self,
        media_events: MediaEventHandlers<VM>,
    ) -> Element<VM> {
        Element {
            id: WidgetId::next(),
            layout: self.layout.clone(),
            visual: self.visual.clone(),
            interactions: InteractionHandlers {
                cursor_style: self.cursor_style.clone(),
                ..Default::default()
            },
            media_events,
            background: self.background.clone(),
            kind: WidgetKind::Image { image: self },
        }
    }
}

impl<VM> From<Image> for Element<VM> {
    fn from(value: Image) -> Self {
        Element {
            id: WidgetId::next(),
            layout: value.layout.clone(),
            visual: value.visual.clone(),
            interactions: InteractionHandlers {
                cursor_style: value.cursor_style.clone(),
                ..Default::default()
            },
            media_events: MediaEventHandlers::default(),
            background: value.background.clone(),
            kind: WidgetKind::Image { image: value },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::foundation::binding::Binding;

    use super::Image;
    use crate::media::MediaSource;
    use crate::ui::layout::Value;

    #[test]
    fn from_path_accepts_binding_pathbuf() {
        let image = Image::from_path(Binding::new(|| PathBuf::from("static/logo.svg")));

        match image.source {
            Value::Bound(binding) => {
                assert_eq!(
                    binding.get(),
                    MediaSource::Path(PathBuf::from("static/logo.svg"))
                );
            }
            Value::Static(_) => panic!("expected bound source"),
        }
    }

    #[test]
    fn from_path_accepts_value_string() {
        let image = Image::from_path(Value::Bound(Binding::new(|| "static/logo.svg".to_string())));

        match image.source {
            Value::Bound(binding) => {
                assert_eq!(
                    binding.get(),
                    MediaSource::Path(PathBuf::from("static/logo.svg"))
                );
            }
            Value::Static(_) => panic!("expected bound source"),
        }
    }

    #[test]
    fn from_url_accepts_binding_string() {
        let image = Image::from_url(Binding::new(|| "https://example.com/logo.svg".to_string()));

        match image.source {
            Value::Bound(binding) => {
                assert_eq!(
                    binding.get(),
                    MediaSource::Url("https://example.com/logo.svg".to_string())
                );
            }
            Value::Static(_) => panic!("expected bound source"),
        }
    }
}
