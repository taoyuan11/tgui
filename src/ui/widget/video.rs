use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::media::ContentFit;
use crate::ui::layout::{Insets, LayoutStyle, Value};
use crate::ui::unit::Dp;
use crate::video::VideoController;

use super::common::{
    CursorStyle, InteractionHandlers, MediaEventHandlers, Point, VisualStyle, WidgetId, WidgetKind,
};
use super::core::Element;

#[derive(Clone)]
pub struct VideoSurface {
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) controller: VideoController,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) fit: ContentFit,
    pub(crate) aspect_ratio: Option<f32>,
    pub(crate) cursor_style: Option<Value<CursorStyle>>,
}

impl VideoSurface {
    pub fn new(controller: VideoController) -> Self {
        Self {
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            controller,
            background: None,
            fit: ContentFit::Contain,
            aspect_ratio: None,
            cursor_style: None,
        }
    }

    pub fn size(mut self, width: impl Into<Value<Dp>>, height: impl Into<Value<Dp>>) -> Self {
        self.layout.width = Some(width.into());
        self.layout.height = Some(height.into());
        self.layout.fill_width = false;
        self.layout.fill_height = false;
        self
    }

    pub fn width(mut self, width: impl Into<Value<Dp>>) -> Self {
        self.layout.width = Some(width.into());
        self.layout.fill_width = false;
        self
    }

    pub fn height(mut self, height: impl Into<Value<Dp>>) -> Self {
        self.layout.height = Some(height.into());
        self.layout.fill_height = false;
        self
    }

    pub fn fill_width(mut self) -> Self {
        self.layout.fill_width = true;
        self.layout.width = None;
        self
    }

    pub fn fill_height(mut self) -> Self {
        self.layout.fill_height = true;
        self.layout.height = None;
        self
    }

    pub fn fill_size(mut self) -> Self {
        self.layout.fill_width = true;
        self.layout.fill_height = true;
        self.layout.width = None;
        self.layout.height = None;
        self
    }

    pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
        self.layout.margin = insets.into();
        self
    }

    pub fn fit(mut self, fit: ContentFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn aspect_ratio(mut self, aspect_ratio: f32) -> Self {
        self.aspect_ratio =
            (aspect_ratio.is_finite() && aspect_ratio > 0.0).then_some(aspect_ratio);
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

    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.cursor_style = Some(cursor.into());
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
            kind: WidgetKind::VideoSurface { video: self },
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
            kind: WidgetKind::VideoSurface { video: self },
        }
    }
}

impl<VM> From<VideoSurface> for Element<VM> {
    fn from(value: VideoSurface) -> Self {
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
            kind: WidgetKind::VideoSurface { video: value },
        }
    }
}
