use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

mod event_handlers;
mod geometry;
mod hit_scene;
mod hit_scene_state;
mod hit_scene_support;
mod scene_primitives;
mod slider;
mod text_edit;
mod widget_kind;

use crate::animation::{AnimationEngine, AnimationKey, WidgetProperty};
use crate::media::TextureFrame;
use taffy::NodeId as TaffyNodeId;

#[cfg(feature = "audio")]
use crate::audio::Audio;
use crate::foundation::binding::{
    track_dependency_scope, DependencyGraph, DependencyOwner, DependencyPhase, TextChangeSet,
    TextController,
};
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::text::font::{FontWeight, TextLayoutInfo};
use crate::ui::layout::{
    Align, Axis, Insets, Justify, Length, Overflow, ScrollbarStyle, Track, Value, Wrap,
};
use crate::ui::theme::{Shadow, Theme, WidgetState};
use crate::ui::unit::{dp, Dp, Sp, UnitContext};
#[cfg(feature = "video")]
use crate::video::VideoSurface;

use super::background::{BackgroundBrush, BackgroundGradientStop, BackgroundImage};
use super::canvas::{
    CanvasBlendMode, CanvasColorFilter, CanvasDragEvent, CanvasItemId, CanvasMouseEvent,
    CanvasScene, CanvasTextHit, CanvasTextHorizontalAlign, CanvasTextOverflow,
    CanvasTextVerticalAlign, CanvasTextWrap, CanvasWheelEvent,
};
use super::image::Image;
pub(crate) use super::slider_shared::{SliderOrientation, SliderValueFormatter};
#[cfg(feature = "video")]
use super::style::VideoSurfaceStyle;
use super::style::{
    ButtonStyle as WidgetButtonStyle, CanvasStyle, CheckboxStyle as WidgetCheckboxStyle,
    ContainerStyle, SelectStyle as WidgetSelectStyle, SliderStyle as WidgetSliderStyle,
    StyleResolver, SwitchStyle as WidgetSwitchStyle,
};
use super::style::{InputStyle as WidgetInputStyle, TextareaStyle as WidgetTextareaStyle};
use super::text::Text;
pub(crate) use event_handlers::*;
use geometry::point_in_triangle;
pub use geometry::{Point, Rect};
pub use hit_scene::FocusScopeOptions;
pub(crate) use hit_scene::*;
pub(crate) use hit_scene_state::*;
pub(crate) use hit_scene_support::*;
pub use scene_primitives::*;
pub(crate) use slider::*;
pub(crate) use text_edit::*;
pub(crate) use widget_kind::*;
pub use widget_kind::{DividerOrientation, TabPlacement};

static NEXT_WIDGET_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CursorStyle {
    Default,
    Pointer,
    Text,
    Crosshair,
    Move,
    NotAllowed,
    Grab,
    Grabbing,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
}

/// 文件拖放到组件上时派发的事件。
#[derive(Clone, Debug, PartialEq)]
pub struct FileDropEvent {
    /// 拖放位置，使用窗口逻辑坐标。
    pub position: Point,
    /// 本次拖放携带的本地文件路径。
    pub paths: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WidgetId(u64);

impl WidgetId {
    pub(crate) fn next() -> Self {
        Self(NEXT_WIDGET_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub(crate) fn raw(self) -> u64 {
        self.0
    }

    pub(crate) fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub(crate) fn dependency_owner(self, phase: DependencyPhase) -> DependencyOwner {
        DependencyOwner {
            widget_id: self.0,
            phase,
            property: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WidgetKey(String);

impl From<String> for WidgetKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for WidgetKey {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<&String> for WidgetKey {
    fn from(value: &String) -> Self {
        Self(value.clone())
    }
}

macro_rules! impl_widget_key_from_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for WidgetKey {
                fn from(value: $ty) -> Self {
                    Self(value.to_string())
                }
            }
        )*
    };
}

impl_widget_key_from_int!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TextInputContentGeometry {
    pub content_frame: Rect,
    pub content_width: Dp,
    pub content_height: Dp,
    pub scroll_offset: Point,
}

pub(crate) fn text_input_content_viewport(
    frame: Rect,
    padding: Insets,
    multiline: bool,
    show_scrollbar: bool,
    theme: &Theme,
    units: UnitContext,
) -> Rect {
    let inner = frame.inset(padding);
    if !multiline || !show_scrollbar {
        return inner;
    }

    let defaults = ContainerStyle::default_for_theme(theme).scrollbar;
    let thickness = Dp::new(units.resolve_dp(defaults.thickness.unwrap_or(dp(5.0)).max(dp(2.0))));

    Rect::new(
        inner.x,
        inner.y,
        (inner.width - thickness.min(inner.width)).max(0.0),
        inner.height,
    )
}

pub(crate) fn text_input_content_geometry(
    layout: &TextLayoutInfo,
    line_height: f32,
    content_viewport: Rect,
    multiline: bool,
    auto_wrap: bool,
    scroll: Point,
    trailing_padding: f32,
) -> TextInputContentGeometry {
    let content_width = if multiline && auto_wrap {
        content_viewport.width.max(0.0)
    } else {
        Dp::new(
            layout
                .width
                .max(content_viewport.width.get() + trailing_padding),
        )
    };
    let content_height = if multiline {
        Dp::new(layout.height.max(line_height))
    } else {
        content_viewport
            .height
            .min(layout.height.max(line_height))
            .max(Dp::new(line_height))
    };
    let scroll_offset = Point::new(
        if multiline && auto_wrap {
            Dp::ZERO
        } else {
            scroll.x.clamp(
                0.0,
                (layout.width + trailing_padding - content_viewport.width.get()).max(0.0),
            )
        },
        if multiline {
            scroll.y.clamp(
                0.0,
                (layout.height.max(line_height) - content_viewport.height.get()).max(0.0),
            )
        } else {
            Dp::ZERO
        },
    );
    let content_frame = Rect::new(
        content_viewport.x
            - if multiline && auto_wrap {
                Dp::ZERO
            } else {
                scroll_offset.x
            },
        if multiline {
            content_viewport.y - scroll_offset.y
        } else {
            content_viewport.y + ((content_viewport.height - content_height).max(0.0) * 0.5)
        },
        content_width,
        content_height,
    );

    TextInputContentGeometry {
        content_frame,
        content_width,
        content_height,
        scroll_offset,
    }
}

pub(crate) fn text_input_layout_width(
    content_viewport: Rect,
    multiline: bool,
    auto_wrap: bool,
    trailing_padding: f32,
) -> f32 {
    if multiline && auto_wrap {
        (content_viewport.width.get() - trailing_padding).max(0.0)
    } else {
        content_viewport.width.get().max(0.0)
    }
}

#[derive(Clone, PartialEq)]
pub struct VisualStyle {
    pub style_id: Option<String>,
    pub classes: Vec<String>,
    pub border_color: Option<Value<Color>>,
    pub border_radius: Option<Value<Dp>>,
    pub border_width: Option<Value<Dp>>,
    pub background_brush: Option<Value<BackgroundBrush>>,
    pub background_image: Option<Value<BackgroundImage>>,
    pub background_blur: Value<Dp>,
    pub shadow: Option<Value<Shadow>>,
    pub opacity: Value<f32>,
    pub offset: Value<Point>,
    pub scale: Value<f32>,
}

impl Default for VisualStyle {
    fn default() -> Self {
        Self {
            style_id: None,
            classes: Vec::new(),
            border_color: None,
            border_radius: None,
            border_width: None,
            background_brush: None,
            background_image: None,
            background_blur: Value::Static(Dp::ZERO),
            shadow: None,
            opacity: Value::Static(1.0),
            offset: Value::Static(Point::ZERO),
            scale: Value::Static(1.0),
        }
    }
}

impl Value<Color> {
    pub(crate) fn resolve_widget(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
    ) -> Color {
        animations.resolve_color(
            AnimationKey::Widget {
                id: widget_id.raw(),
                property,
            },
            self.resolve(),
            self.transition(),
            now,
        )
    }
}

impl Value<f32> {
    pub(crate) fn resolve_widget(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
    ) -> f32 {
        animations.resolve_f32(
            AnimationKey::Widget {
                id: widget_id.raw(),
                property,
            },
            self.resolve(),
            self.transition(),
            now,
        )
    }

    pub(crate) fn resolve_widget_clamped(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
        min: f32,
        max: f32,
    ) -> f32 {
        self.resolve_widget(animations, widget_id, property, now)
            .clamp(min, max)
    }
}

impl Value<Dp> {
    pub(crate) fn resolve_widget(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
    ) -> Dp {
        animations.resolve_dp(
            AnimationKey::Widget {
                id: widget_id.raw(),
                property,
            },
            self.resolve(),
            self.transition(),
            now,
        )
    }

    pub(crate) fn resolve_widget_to_logical(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
        units: UnitContext,
    ) -> f32 {
        units.resolve_dp(self.resolve_widget(animations, widget_id, property, now))
    }
}

impl Value<Length> {
    pub(crate) fn resolve_widget(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
    ) -> Length {
        match self {
            Value::Static(value) => *value,
            Value::Signal(signal) => {
                let target = signal.get();
                match target {
                    Length::Px(target_dp) => Length::Px(animations.resolve_dp(
                        AnimationKey::Widget {
                            id: widget_id.raw(),
                            property,
                        },
                        target_dp,
                        signal.transition(),
                        now,
                    )),
                    Length::Auto | Length::Percent(_) => target,
                }
            }
        }
    }
}

impl Value<Point> {
    pub(crate) fn resolve_widget(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
    ) -> Point {
        animations.resolve_point(
            AnimationKey::Widget {
                id: widget_id.raw(),
                property,
            },
            self.resolve(),
            self.transition(),
            now,
        )
    }
}

impl Value<Insets> {
    pub(crate) fn resolve_widget(
        &self,
        animations: &mut AnimationEngine,
        widget_id: WidgetId,
        property: WidgetProperty,
        now: Instant,
    ) -> Insets {
        animations.resolve_insets(
            AnimationKey::Widget {
                id: widget_id.raw(),
                property,
            },
            self.resolve(),
            self.transition(),
            now,
        )
    }
}

impl Value<BackgroundBrush> {
    pub(crate) fn resolve_widget(&self) -> BackgroundBrush {
        match self {
            Value::Static(value) => value.clone(),
            Value::Signal(signal) => signal.get(),
        }
    }
}

impl Value<BackgroundImage> {
    pub(crate) fn resolve_widget(&self) -> BackgroundImage {
        match self {
            Value::Static(value) => value.clone(),
            Value::Signal(signal) => signal.get(),
        }
    }
}
