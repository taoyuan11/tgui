use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

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
use crate::ui::unit::{dp, Dp, UnitContext};
#[cfg(feature = "video")]
use crate::video::VideoSurface;

use super::background::{BackgroundBrush, BackgroundGradientStop, BackgroundImage};
use super::canvas::{
    CanvasBlendMode, CanvasColorFilter, CanvasDragEvent, CanvasItemId, CanvasMouseEvent,
    CanvasScene, CanvasTextHit, CanvasTextHorizontalAlign, CanvasTextOverflow,
    CanvasTextVerticalAlign, CanvasTextWrap, CanvasWheelEvent,
};
use super::image::Image;
pub(crate) use super::slider_shared::SliderValueFormatter;
#[cfg(feature = "video")]
use super::style::VideoSurfaceStyle;
use super::style::{
    infer_theme_mode, ButtonStyle as WidgetButtonStyle, CanvasStyle,
    CheckboxStyle as WidgetCheckboxStyle, ContainerStyle, SelectStyle as WidgetSelectStyle,
    SliderStyle as WidgetSliderStyle, StyleResolver, SwitchStyle as WidgetSwitchStyle,
};
use super::style::{InputStyle as WidgetInputStyle, TextareaStyle as WidgetTextareaStyle};
use super::text::Text;

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
pub struct Point {
    pub x: Dp,
    pub y: Dp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: Dp,
    pub y: Dp,
    pub width: Dp,
    pub height: Dp,
}

impl Rect {
    pub fn new(
        x: impl Into<Dp>,
        y: impl Into<Dp>,
        width: impl Into<Dp>,
        height: impl Into<Dp>,
    ) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
            width: width.into(),
            height: height.into(),
        }
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    pub(crate) fn inset(self, insets: Insets) -> Self {
        let width = (self.width - insets.left - insets.right).max(Dp::ZERO);
        let height = (self.height - insets.top - insets.bottom).max(Dp::ZERO);
        Self {
            x: self.x + insets.left,
            y: self.y + insets.top,
            width,
            height,
        }
    }

    pub(crate) fn right(self) -> Dp {
        self.x + self.width
    }

    pub(crate) fn bottom(self) -> Dp {
        self.y + self.height
    }

    pub(crate) fn is_empty(self) -> bool {
        self.width <= Dp::ZERO || self.height <= Dp::ZERO
    }

    pub(crate) fn intersect(self, other: Self) -> Option<Self> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        let width = right - x;
        let height = bottom - y;
        (width > Dp::ZERO && height > Dp::ZERO).then_some(Self::new(x, y, width, height))
    }

    pub(crate) fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self::new(x, y, right - x, bottom - y)
    }
}

pub(crate) fn slider_effective_step(min: f32, max: f32, step: f32) -> Option<f32> {
    if !step.is_finite() || step <= 0.0 {
        return None;
    }
    let range = (max - min).abs();
    if !range.is_finite() || range <= f32::EPSILON {
        return None;
    }
    Some(step.min(range))
}

pub(crate) fn slider_clamp_value(value: f32, min: f32, max: f32) -> f32 {
    if !value.is_finite() {
        min
    } else {
        value.clamp(min, max)
    }
}

pub(crate) fn slider_quantize_value(value: f32, min: f32, max: f32, step: f32) -> f32 {
    let clamped = slider_clamp_value(value, min, max);
    let Some(step) = slider_effective_step(min, max, step) else {
        return clamped;
    };
    let steps = ((clamped - min) / step).round();
    slider_clamp_value(min + (steps * step), min, max)
}

pub(crate) fn slider_resolve_value(value: f32, min: f32, max: f32, step: f32) -> f32 {
    slider_quantize_value(value, min, max, step)
}

pub(crate) fn slider_normalized_value(value: f32, min: f32, max: f32, step: f32) -> f32 {
    let range = max - min;
    if range.abs() <= f32::EPSILON {
        return 0.0;
    }
    ((slider_resolve_value(value, min, max, step) - min) / range).clamp(0.0, 1.0)
}

pub(crate) fn slider_value_from_normalized(normalized: f32, min: f32, max: f32, step: f32) -> f32 {
    let range = max - min;
    if range.abs() <= f32::EPSILON {
        return min;
    }
    slider_quantize_value(min + normalized.clamp(0.0, 1.0) * range, min, max, step)
}

pub(crate) fn slider_tick_count(min: f32, max: f32, step: f32, explicit: Option<usize>) -> usize {
    if let Some(explicit) = explicit {
        return explicit.max(2).min(101);
    }
    let Some(step) = slider_effective_step(min, max, step) else {
        return 2;
    };
    let count = (((max - min).abs() / step).round() as usize).saturating_add(1);
    count.max(2).min(101)
}

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

    let defaults = ContainerStyle::default_for(infer_theme_mode(theme)).scrollbar;
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
    pub border_color: Option<Value<Color>>,
    pub border_radius: Option<Value<Dp>>,
    pub border_width: Option<Value<Dp>>,
    pub background_brush: Option<Value<BackgroundBrush>>,
    pub background_image: Option<Value<BackgroundImage>>,
    pub background_blur: Value<Dp>,
    pub shadow: Option<Value<Shadow>>,
    pub opacity: Value<f32>,
    pub offset: Value<Point>,
}

impl Default for VisualStyle {
    fn default() -> Self {
        Self {
            border_color: None,
            border_radius: None,
            border_width: None,
            background_brush: None,
            background_image: None,
            background_blur: Value::Static(Dp::ZERO),
            shadow: None,
            opacity: Value::Static(1.0),
            offset: Value::Static(Point::ZERO),
        }
    }
}

pub(crate) struct InteractionHandlers<VM> {
    pub on_click: Option<Command<VM>>,
    pub on_double_click: Option<Command<VM>>,
    pub on_focus: Option<Command<VM>>,
    pub on_blur: Option<Command<VM>>,
    pub on_mouse_enter: Option<Command<VM>>,
    pub on_mouse_leave: Option<Command<VM>>,
    pub on_mouse_move: Option<ValueCommand<VM, Point>>,
    pub cursor_style: Option<Value<CursorStyle>>,
}

pub(crate) struct CanvasItemInteractionHandlers<VM> {
    pub on_click: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_double_click: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_mouse_down: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_mouse_up: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_mouse_enter: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_mouse_leave: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_mouse_move: Option<ValueCommand<VM, CanvasMouseEvent>>,
    pub on_wheel: Option<ValueCommand<VM, CanvasWheelEvent>>,
    pub on_drag_start: Option<ValueCommand<VM, CanvasDragEvent>>,
    pub on_drag: Option<ValueCommand<VM, CanvasDragEvent>>,
    pub on_drag_end: Option<ValueCommand<VM, CanvasDragEvent>>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum MediaEventPhase {
    Loading,
    Success,
    Error(String),
}

pub(crate) struct MediaEventHandlers<VM> {
    pub on_loading: Option<Command<VM>>,
    pub on_success: Option<Command<VM>>,
    pub on_error: Option<ValueCommand<VM, String>>,
}

pub(crate) struct LifecycleEventHandlers<VM> {
    pub on_mount: Option<Command<VM>>,
    pub on_unmount: Option<Command<VM>>,
    pub on_update: Option<Command<VM>>,
}

impl<VM> Clone for MediaEventHandlers<VM> {
    fn clone(&self) -> Self {
        Self {
            on_loading: self.on_loading.clone(),
            on_success: self.on_success.clone(),
            on_error: self.on_error.clone(),
        }
    }
}

impl<VM> Default for MediaEventHandlers<VM> {
    fn default() -> Self {
        Self {
            on_loading: None,
            on_success: None,
            on_error: None,
        }
    }
}

impl<VM> MediaEventHandlers<VM> {
    pub(crate) fn has_any(&self) -> bool {
        self.on_loading.is_some() || self.on_success.is_some() || self.on_error.is_some()
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> MediaEventHandlers<RootVm>
    where
        VM: 'static,
    {
        MediaEventHandlers {
            on_loading: self
                .on_loading
                .map(|command| command.scope(selector.clone())),
            on_success: self
                .on_success
                .map(|command| command.scope(selector.clone())),
            on_error: self.on_error.map(|command| command.scope(selector)),
        }
    }
}

impl<VM> Clone for LifecycleEventHandlers<VM> {
    fn clone(&self) -> Self {
        Self {
            on_mount: self.on_mount.clone(),
            on_unmount: self.on_unmount.clone(),
            on_update: self.on_update.clone(),
        }
    }
}

impl<VM> Default for LifecycleEventHandlers<VM> {
    fn default() -> Self {
        Self {
            on_mount: None,
            on_unmount: None,
            on_update: None,
        }
    }
}

impl<VM> LifecycleEventHandlers<VM> {
    pub(crate) fn has_any(&self) -> bool {
        self.on_mount.is_some() || self.on_unmount.is_some() || self.on_update.is_some()
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> LifecycleEventHandlers<RootVm>
    where
        VM: 'static,
    {
        LifecycleEventHandlers {
            on_mount: self.on_mount.map(|command| command.scope(selector.clone())),
            on_unmount: self
                .on_unmount
                .map(|command| command.scope(selector.clone())),
            on_update: self.on_update.map(|command| command.scope(selector)),
        }
    }
}

#[derive(Clone)]
pub(crate) struct MediaEventState<VM> {
    pub widget_id: WidgetId,
    pub media_phase: Option<MediaEventPhase>,
    pub handlers: MediaEventHandlers<VM>,
}

pub(crate) struct LifecycleEventState<VM> {
    pub widget_id: WidgetId,
    pub snapshot: super::core::LifecycleSnapshot,
    pub handlers: LifecycleEventHandlers<VM>,
}

impl<VM> Clone for LifecycleEventState<VM> {
    fn clone(&self) -> Self {
        Self {
            widget_id: self.widget_id,
            snapshot: self.snapshot.clone(),
            handlers: self.handlers.clone(),
        }
    }
}

impl<VM> Clone for InteractionHandlers<VM> {
    fn clone(&self) -> Self {
        Self {
            on_click: self.on_click.clone(),
            on_double_click: self.on_double_click.clone(),
            on_focus: self.on_focus.clone(),
            on_blur: self.on_blur.clone(),
            on_mouse_enter: self.on_mouse_enter.clone(),
            on_mouse_leave: self.on_mouse_leave.clone(),
            on_mouse_move: self.on_mouse_move.clone(),
            cursor_style: self.cursor_style.clone(),
        }
    }
}

impl<VM> Clone for CanvasItemInteractionHandlers<VM> {
    fn clone(&self) -> Self {
        Self {
            on_click: self.on_click.clone(),
            on_double_click: self.on_double_click.clone(),
            on_mouse_down: self.on_mouse_down.clone(),
            on_mouse_up: self.on_mouse_up.clone(),
            on_mouse_enter: self.on_mouse_enter.clone(),
            on_mouse_leave: self.on_mouse_leave.clone(),
            on_mouse_move: self.on_mouse_move.clone(),
            on_wheel: self.on_wheel.clone(),
            on_drag_start: self.on_drag_start.clone(),
            on_drag: self.on_drag.clone(),
            on_drag_end: self.on_drag_end.clone(),
        }
    }
}

impl<VM> Default for InteractionHandlers<VM> {
    fn default() -> Self {
        Self {
            on_click: None,
            on_double_click: None,
            on_focus: None,
            on_blur: None,
            on_mouse_enter: None,
            on_mouse_leave: None,
            on_mouse_move: None,
            cursor_style: None,
        }
    }
}

impl<VM> Default for CanvasItemInteractionHandlers<VM> {
    fn default() -> Self {
        Self {
            on_click: None,
            on_double_click: None,
            on_mouse_down: None,
            on_mouse_up: None,
            on_mouse_enter: None,
            on_mouse_leave: None,
            on_mouse_move: None,
            on_wheel: None,
            on_drag_start: None,
            on_drag: None,
            on_drag_end: None,
        }
    }
}

impl<VM> InteractionHandlers<VM> {
    pub(crate) fn has_any(&self) -> bool {
        self.on_click.is_some()
            || self.on_double_click.is_some()
            || self.on_focus.is_some()
            || self.on_blur.is_some()
            || self.on_mouse_enter.is_some()
            || self.on_mouse_leave.is_some()
            || self.on_mouse_move.is_some()
            || self.cursor_style.is_some()
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> InteractionHandlers<RootVm>
    where
        VM: 'static,
    {
        InteractionHandlers {
            on_click: self.on_click.map(|command| command.scope(selector.clone())),
            on_double_click: self
                .on_double_click
                .map(|command| command.scope(selector.clone())),
            on_focus: self.on_focus.map(|command| command.scope(selector.clone())),
            on_blur: self.on_blur.map(|command| command.scope(selector.clone())),
            on_mouse_enter: self
                .on_mouse_enter
                .map(|command| command.scope(selector.clone())),
            on_mouse_leave: self
                .on_mouse_leave
                .map(|command| command.scope(selector.clone())),
            on_mouse_move: self.on_mouse_move.map(|command| command.scope(selector)),
            cursor_style: self.cursor_style,
        }
    }
}

impl<VM: 'static> CanvasItemInteractionHandlers<VM> {
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> CanvasItemInteractionHandlers<RootVm> {
        CanvasItemInteractionHandlers {
            on_click: self.on_click.map(|command| command.scope(selector.clone())),
            on_double_click: self
                .on_double_click
                .map(|command| command.scope(selector.clone())),
            on_mouse_down: self
                .on_mouse_down
                .map(|command| command.scope(selector.clone())),
            on_mouse_up: self
                .on_mouse_up
                .map(|command| command.scope(selector.clone())),
            on_mouse_enter: self
                .on_mouse_enter
                .map(|command| command.scope(selector.clone())),
            on_mouse_leave: self
                .on_mouse_leave
                .map(|command| command.scope(selector.clone())),
            on_mouse_move: self
                .on_mouse_move
                .map(|command| command.scope(selector.clone())),
            on_wheel: self.on_wheel.map(|command| command.scope(selector.clone())),
            on_drag_start: self
                .on_drag_start
                .map(|command| command.scope(selector.clone())),
            on_drag: self.on_drag.map(|command| command.scope(selector.clone())),
            on_drag_end: self.on_drag_end.map(|command| command.scope(selector)),
        }
    }
}

impl<VM> CanvasItemInteractionHandlers<VM> {
    pub(crate) fn has_any(&self) -> bool {
        self.on_click.is_some()
            || self.on_double_click.is_some()
            || self.on_mouse_down.is_some()
            || self.on_mouse_up.is_some()
            || self.on_mouse_enter.is_some()
            || self.on_mouse_leave.is_some()
            || self.on_mouse_move.is_some()
            || self.on_wheel.is_some()
            || self.on_drag_start.is_some()
            || self.on_drag.is_some()
            || self.on_drag_end.is_some()
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

#[derive(Clone, Copy)]
pub struct RenderPrimitive {
    pub rect: Rect,
    pub color: Color,
    pub corner_radius: f32,
    pub stroke_width: f32,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushPrimitive {
    pub rect: Rect,
    pub brush: BackgroundBrush,
    pub corner_radius: f32,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackdropBlurPrimitive {
    pub rect: Rect,
    pub corner_radius: f32,
    pub blur_radius: f32,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone)]
pub struct CanvasTextSpanPrimitive {
    pub content: String,
    pub font_family: Option<String>,
    pub color: Color,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub line_height: Option<f32>,
    pub letter_spacing: f32,
}

#[derive(Clone)]
pub struct TextPrimitive {
    pub content: String,
    pub rich_spans: Option<Arc<[CanvasTextSpanPrimitive]>>,
    pub frame: Rect,
    pub quad: Option<[Point; 4]>,
    pub color: Color,
    pub force_color: bool,
    pub font_family: Option<String>,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub wrap: CanvasTextWrap,
    pub overflow: CanvasTextOverflow,
    pub horizontal_align: CanvasTextHorizontalAlign,
    pub vertical_align: CanvasTextVerticalAlign,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone)]
pub struct TexturePrimitive {
    pub texture: Arc<TextureFrame>,
    pub frame: Rect,
    pub quad: Option<[Point; 4]>,
    pub uv_rect: Option<Rect>,
    pub corner_radius: f32,
    pub opacity: f32,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone)]
pub struct CanvasCompositePrimitive {
    pub bounds: Rect,
    pub opacity: f32,
    pub blend_mode: CanvasBlendMode,
    pub blur_radius: f32,
    pub color_filter: Option<CanvasColorFilter>,
    pub inner_shadow_color: Option<Color>,
    pub inner_shadow_offset: Point,
    pub inner_shadow_blur_radius: f32,
    pub clip_rect: Option<Rect>,
    pub clip_mask: Option<ClipMask>,
    pub content_commands: Arc<[RenderCommand]>,
    pub mask_commands: Option<Arc<[RenderCommand]>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipMask {
    pub rect: Rect,
    pub corner_radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshVertex {
    pub position: [f32; 2],
    pub local_position: [f32; 2],
    pub brush_meta: [f32; 4],
    pub gradient_data0: [f32; 4],
    pub gradient_data1: [f32; 4],
    pub stop_offsets0: [f32; 4],
    pub stop_offsets1: [f32; 4],
    pub stop_colors: [[f32; 4]; 8],
}

#[derive(Clone)]
pub struct MeshPrimitive {
    pub vertices: Arc<[MeshVertex]>,
    pub(crate) triangles: Arc<[[Point; 3]]>,
    pub clip_rect: Option<Rect>,
    #[allow(dead_code)]
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone)]
pub(crate) enum RenderCommand {
    BackdropBlur(BackdropBlurPrimitive),
    Brush(BrushPrimitive),
    CanvasComposite(CanvasCompositePrimitive),
    Shape(RenderPrimitive),
    Texture(TexturePrimitive),
    Text(TextPrimitive),
    Mesh(MeshPrimitive),
}

#[derive(Clone, Default)]
pub struct ScenePrimitives {
    pub backdrop_blurs: Vec<BackdropBlurPrimitive>,
    pub brushes: Vec<BrushPrimitive>,
    pub canvas_composites: Vec<CanvasCompositePrimitive>,
    pub shapes: Vec<RenderPrimitive>,
    pub meshes: Vec<MeshPrimitive>,
    pub textures: Vec<TexturePrimitive>,
    pub texts: Vec<TextPrimitive>,
    pub overlay_shapes: Vec<RenderPrimitive>,
    pub overlay_textures: Vec<TexturePrimitive>,
    #[allow(dead_code)]
    pub overlay_meshes: Vec<MeshPrimitive>,
    #[allow(dead_code)]
    pub overlay_texts: Vec<TextPrimitive>,
    pub(crate) commands: Vec<RenderCommand>,
    pub(crate) overlay_commands: Vec<RenderCommand>,
}

impl ScenePrimitives {
    pub(crate) fn push_render_command(&mut self, command: RenderCommand) {
        match command {
            RenderCommand::BackdropBlur(primitive) => self.push_backdrop_blur(primitive),
            RenderCommand::Brush(primitive) => self.push_brush(primitive),
            RenderCommand::CanvasComposite(primitive) => self.push_canvas_composite(primitive),
            RenderCommand::Shape(primitive) => self.push_shape(primitive),
            RenderCommand::Texture(primitive) => self.push_texture(primitive),
            RenderCommand::Text(primitive) => self.push_text(primitive),
            RenderCommand::Mesh(primitive) => self.push_mesh(primitive),
        }
    }

    pub(crate) fn push_backdrop_blur(&mut self, primitive: BackdropBlurPrimitive) {
        self.backdrop_blurs.push(primitive);
        self.commands.push(RenderCommand::BackdropBlur(primitive));
    }

    pub(crate) fn push_brush(&mut self, primitive: BrushPrimitive) {
        self.brushes.push(primitive.clone());
        self.commands.push(RenderCommand::Brush(primitive));
    }

    pub(crate) fn push_canvas_composite(&mut self, primitive: CanvasCompositePrimitive) {
        self.canvas_composites.push(primitive.clone());
        self.commands
            .push(RenderCommand::CanvasComposite(primitive));
    }

    pub(crate) fn push_shape(&mut self, primitive: RenderPrimitive) {
        self.shapes.push(primitive);
        self.commands.push(RenderCommand::Shape(primitive));
    }

    pub(crate) fn push_mesh(&mut self, primitive: MeshPrimitive) {
        self.meshes.push(primitive.clone());
        self.commands.push(RenderCommand::Mesh(primitive));
    }

    pub(crate) fn push_texture(&mut self, primitive: TexturePrimitive) {
        self.textures.push(primitive.clone());
        self.commands.push(RenderCommand::Texture(primitive));
    }

    pub(crate) fn push_text(&mut self, primitive: TextPrimitive) {
        self.texts.push(primitive.clone());
        self.commands.push(RenderCommand::Text(primitive));
    }

    pub(crate) fn push_overlay_shape(&mut self, primitive: RenderPrimitive) {
        self.overlay_shapes.push(primitive);
        self.overlay_commands.push(RenderCommand::Shape(primitive));
    }

    #[allow(dead_code)]
    pub(crate) fn push_overlay_texture(&mut self, primitive: TexturePrimitive) {
        self.overlay_textures.push(primitive.clone());
        self.overlay_commands
            .push(RenderCommand::Texture(primitive));
    }

    #[allow(dead_code)]
    pub(crate) fn push_overlay_mesh(&mut self, primitive: MeshPrimitive) {
        self.overlay_meshes.push(primitive.clone());
        self.overlay_commands.push(RenderCommand::Mesh(primitive));
    }

    #[allow(dead_code)]
    pub(crate) fn push_overlay_text(&mut self, primitive: TextPrimitive) {
        self.overlay_texts.push(primitive.clone());
        self.overlay_commands.push(RenderCommand::Text(primitive));
    }

    pub(crate) fn extend(&mut self, other: &ScenePrimitives) {
        self.backdrop_blurs
            .extend(other.backdrop_blurs.iter().copied());
        self.brushes.extend(other.brushes.iter().cloned());
        self.canvas_composites
            .extend(other.canvas_composites.iter().cloned());
        self.shapes.extend(other.shapes.iter().copied());
        self.meshes.extend(other.meshes.iter().cloned());
        self.textures.extend(other.textures.iter().cloned());
        self.texts.extend(other.texts.iter().cloned());
        self.overlay_shapes
            .extend(other.overlay_shapes.iter().copied());
        self.overlay_textures
            .extend(other.overlay_textures.iter().cloned());
        self.overlay_meshes
            .extend(other.overlay_meshes.iter().cloned());
        self.overlay_texts
            .extend(other.overlay_texts.iter().cloned());
        self.commands.extend(other.commands.iter().cloned());
        self.overlay_commands
            .extend(other.overlay_commands.iter().cloned());
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BrushPrimitiveData {
    pub brush_meta: [f32; 4],
    pub gradient_data0: [f32; 4],
    pub gradient_data1: [f32; 4],
    pub stop_offsets0: [f32; 4],
    pub stop_offsets1: [f32; 4],
    pub stop_colors: [[f32; 4]; 7],
}

impl BrushPrimitiveData {
    pub(crate) fn from_background_brush(brush: &BackgroundBrush, opacity: f32) -> Option<Self> {
        match brush {
            BackgroundBrush::Solid(color) => Some(Self {
                brush_meta: [0.0, 2.0, 0.0, 0.0],
                gradient_data0: [0.0; 4],
                gradient_data1: [0.0; 4],
                stop_offsets0: [0.0, 1.0, 0.0, 0.0],
                stop_offsets1: [0.0; 4],
                stop_colors: solid_stop_colors(color.with_alpha_factor(opacity)),
            }),
            BackgroundBrush::LinearGradient(gradient) => {
                let stops = normalized_background_stops(&gradient.stops, opacity)?;
                Some(Self::gradient(
                    1.0,
                    stops.len() as f32,
                    [
                        gradient.start.x.get(),
                        gradient.start.y.get(),
                        gradient.end.x.get(),
                        gradient.end.y.get(),
                    ],
                    [0.0; 4],
                    stops,
                ))
            }
            BackgroundBrush::RadialGradient(gradient) => {
                let stops = normalized_background_stops(&gradient.stops, opacity)?;
                Some(Self::gradient(
                    2.0,
                    stops.len() as f32,
                    [0.0; 4],
                    [
                        gradient.center.x.get(),
                        gradient.center.y.get(),
                        gradient.radius.get().max(0.0001),
                        0.0,
                    ],
                    stops,
                ))
            }
        }
    }

    fn gradient(
        kind: f32,
        stop_count: f32,
        gradient_data0: [f32; 4],
        gradient_data1: [f32; 4],
        stops: Vec<BackgroundGradientStopData>,
    ) -> Self {
        let mut stop_offsets0 = [0.0; 4];
        let mut stop_offsets1 = [0.0; 4];
        let mut stop_colors = [[0.0; 4]; 7];

        for (index, stop) in stops.iter().enumerate() {
            if index < 4 {
                stop_offsets0[index] = stop.offset;
            } else {
                stop_offsets1[index - 4] = stop.offset;
            }
            stop_colors[index] = stop.color;
        }

        Self {
            brush_meta: [kind, stop_count, 0.0, 0.0],
            gradient_data0,
            gradient_data1,
            stop_offsets0,
            stop_offsets1,
            stop_colors,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BackgroundGradientStopData {
    offset: f32,
    color: [f32; 4],
}

fn normalized_background_stops(
    stops: &[BackgroundGradientStop],
    opacity: f32,
) -> Option<Vec<BackgroundGradientStopData>> {
    if stops.is_empty() || stops.len() > 7 {
        return None;
    }

    Some(
        stops
            .iter()
            .map(|stop| {
                let color = stop.color.with_alpha_factor(opacity);
                BackgroundGradientStopData {
                    offset: stop.offset,
                    color: color.to_linear_rgba_f32(),
                }
            })
            .collect(),
    )
}

fn solid_stop_colors(color: Color) -> [[f32; 4]; 7] {
    let rgba = color.to_linear_rgba_f32();
    let mut colors = [[0.0; 4]; 7];
    colors[0] = rgba;
    colors[1] = rgba;
    colors
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ContainerKind {
    Flow,
    Stack,
    Grid {
        columns: Vec<Track>,
        rows: Vec<Track>,
    },
    Flex {
        direction: Axis,
        wrap: Wrap,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ContainerLayout {
    pub kind: ContainerKind,
    pub padding: Option<Value<Insets>>,
    pub gap: Value<crate::ui::layout::Length>,
    pub justify: Justify,
    pub align: Align,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub scrollbar_style: ScrollbarStyle,
}

impl ContainerLayout {
    pub(crate) fn flow() -> Self {
        Self {
            kind: ContainerKind::Flow,
            padding: None,
            gap: Value::Static(crate::ui::layout::Length::Px(Dp::ZERO)),
            justify: Justify::Start,
            align: Align::Start,
            overflow_x: Overflow::Hidden,
            overflow_y: Overflow::Hidden,
            scrollbar_style: ScrollbarStyle::default(),
        }
    }
}

pub(crate) enum ChildSource<VM> {
    Static(Vec<super::core::Element<VM>>),
    Dynamic(Arc<dyn Fn() -> Vec<super::core::Element<VM>> + Send + Sync>),
}

impl<VM> ChildSource<VM> {
    pub(crate) fn resolve(&self, owner: Option<WidgetId>) -> Vec<super::core::Element<VM>> {
        match self {
            Self::Static(children) => children.clone(),
            Self::Dynamic(resolver) => {
                if let Some(owner) = owner {
                    track_dependency_scope(
                        owner.dependency_owner(DependencyPhase::Structure),
                        || resolve_dynamic_children(resolver),
                    )
                } else {
                    resolve_dynamic_children(resolver)
                }
            }
        }
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> ChildSource<RootVm>
    where
        VM: 'static,
    {
        match self {
            Self::Static(children) => ChildSource::Static(
                children
                    .into_iter()
                    .map(|child| child.scope_with_selector(selector.clone()))
                    .collect(),
            ),
            Self::Dynamic(resolver) => ChildSource::Dynamic(Arc::new(move || {
                resolver()
                    .into_iter()
                    .map(|child| child.scope_with_selector(selector.clone()))
                    .collect()
            })),
        }
    }
}

fn resolve_dynamic_children<VM>(
    resolver: &Arc<dyn Fn() -> Vec<super::core::Element<VM>> + Send + Sync>,
) -> Vec<super::core::Element<VM>> {
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    {
        const CHILD_RESOLVER_STACK_SIZE: usize = 8 * 1024 * 1024;
        const CHILD_RESOLVER_STACK_RED_ZONE: usize = CHILD_RESOLVER_STACK_SIZE;
        stacker::maybe_grow(
            CHILD_RESOLVER_STACK_RED_ZONE,
            CHILD_RESOLVER_STACK_SIZE,
            || resolver(),
        )
    }

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        all(target_os = "linux", not(target_env = "ohos"))
    )))]
    {
        resolver()
    }
}

impl<VM> Clone for ChildSource<VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Static(children) => Self::Static(children.clone()),
            Self::Dynamic(resolver) => Self::Dynamic(resolver.clone()),
        }
    }
}

pub(crate) enum WidgetKind<VM> {
    Container {
        layout: ContainerLayout,
        children: Vec<ChildSource<VM>>,
        style: Option<StyleResolver<ContainerStyle>>,
    },
    Text {
        text: Text,
    },
    #[cfg(feature = "audio")]
    Audio {
        audio: Audio,
    },
    Image {
        image: Image,
    },
    Canvas {
        scene: Value<CanvasScene>,
        item_interactions: CanvasItemInteractionHandlers<VM>,
        style: Option<StyleResolver<CanvasStyle>>,
    },
    #[cfg(feature = "video")]
    VideoSurface {
        video: VideoSurface,
        style: Option<StyleResolver<VideoSurfaceStyle>>,
    },
    Button {
        label: Value<String>,
        disabled: Value<bool>,
        variant: ButtonVariantKind,
        style: Option<StyleResolver<WidgetButtonStyle>>,
    },
    Checkbox {
        checked: Value<bool>,
        label: Option<Value<String>>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: Option<StyleResolver<WidgetCheckboxStyle>>,
    },
    Radio {
        checked: Value<bool>,
        label: Option<Value<String>>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: Option<StyleResolver<crate::ui::widget::RadioStyle>>,
    },
    Switch {
        checked: Value<bool>,
        on_change: Option<ValueCommand<VM, bool>>,
        active_background: Option<Value<Color>>,
        inactive_background: Option<Value<Color>>,
        active_thumb_color: Option<Value<Color>>,
        inactive_thumb_color: Option<Value<Color>>,
        disabled: Value<bool>,
        style: Option<StyleResolver<WidgetSwitchStyle>>,
    },
    Select {
        selected_label: Value<Option<String>>,
        placeholder: Value<String>,
        options: Vec<SelectOptionState<VM>>,
        open: Option<Value<bool>>,
        on_open_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: Option<StyleResolver<WidgetSelectStyle>>,
    },
    Slider {
        value: Value<f32>,
        min: f32,
        max: f32,
        step: f32,
        show_ticks: bool,
        show_value_label: bool,
        tick_count: Option<usize>,
        value_formatter: Option<SliderValueFormatter>,
        on_change: Option<ValueCommand<VM, f32>>,
        disabled: Value<bool>,
        style: Option<StyleResolver<WidgetSliderStyle>>,
    },
    TextEditor {
        controller: TextController,
        placeholder: Value<String>,
        on_change: Option<Command<VM>>,
        on_change_set: Option<ValueCommand<VM, TextChangeSet>>,
        disabled: Value<bool>,
        input_style: Option<StyleResolver<WidgetInputStyle>>,
        textarea_style: Option<StyleResolver<WidgetTextareaStyle>>,
        multiline: bool,
        show_scrollbar: Value<bool>,
        auto_wrap: Value<bool>,
    },
}

pub(crate) struct SelectOptionState<VM> {
    pub label: Value<String>,
    pub selected: Value<bool>,
    pub disabled: Value<bool>,
    pub on_select: Option<Command<VM>>,
}

impl<VM> Clone for SelectOptionState<VM> {
    fn clone(&self) -> Self {
        Self {
            label: self.label.clone(),
            selected: self.selected.clone(),
            disabled: self.disabled.clone(),
            on_select: self.on_select.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonVariantKind {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

impl<VM> Clone for WidgetKind<VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Container {
                layout,
                children,
                style,
            } => Self::Container {
                layout: layout.clone(),
                children: children.clone(),
                style: style.clone(),
            },
            Self::Text { text } => Self::Text { text: text.clone() },
            #[cfg(feature = "audio")]
            Self::Audio { audio } => Self::Audio {
                audio: audio.clone(),
            },
            Self::Image { image } => Self::Image {
                image: image.clone(),
            },
            Self::Canvas {
                scene,
                item_interactions,
                style,
            } => Self::Canvas {
                scene: scene.clone(),
                item_interactions: item_interactions.clone(),
                style: style.clone(),
            },
            #[cfg(feature = "video")]
            Self::VideoSurface { video, style } => Self::VideoSurface {
                video: video.clone(),
                style: style.clone(),
            },
            Self::Button {
                label,
                disabled,
                variant,
                style,
            } => Self::Button {
                label: label.clone(),
                disabled: disabled.clone(),
                variant: *variant,
                style: style.clone(),
            },
            Self::Checkbox {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => Self::Checkbox {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Radio {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => Self::Radio {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Switch {
                checked,
                on_change,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                style,
            } => Self::Switch {
                checked: checked.clone(),
                on_change: on_change.clone(),
                active_background: active_background.clone(),
                inactive_background: inactive_background.clone(),
                active_thumb_color: active_thumb_color.clone(),
                inactive_thumb_color: inactive_thumb_color.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                disabled,
                style,
            } => Self::Select {
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                options: options.clone(),
                open: open.clone(),
                on_open_change: on_open_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::Slider {
                value,
                min,
                max,
                step,
                show_ticks,
                show_value_label,
                tick_count,
                value_formatter,
                on_change,
                disabled,
                style,
            } => Self::Slider {
                value: value.clone(),
                min: *min,
                max: *max,
                step: *step,
                show_ticks: *show_ticks,
                show_value_label: *show_value_label,
                tick_count: *tick_count,
                value_formatter: value_formatter.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: style.clone(),
            },
            Self::TextEditor {
                controller,
                placeholder,
                on_change,
                on_change_set,
                disabled,
                input_style,
                textarea_style,
                multiline,
                show_scrollbar,
                auto_wrap,
            } => Self::TextEditor {
                controller: controller.clone(),
                placeholder: placeholder.clone(),
                on_change: on_change.clone(),
                on_change_set: on_change_set.clone(),
                disabled: disabled.clone(),
                input_style: input_style.clone(),
                textarea_style: textarea_style.clone(),
                multiline: *multiline,
                show_scrollbar: show_scrollbar.clone(),
                auto_wrap: auto_wrap.clone(),
            },
        }
    }
}

#[derive(Clone)]
pub(crate) enum MeasureContext {
    None,
    Text {
        id: WidgetId,
        text: Text,
    },
    #[cfg(feature = "audio")]
    Audio {
        id: WidgetId,
    },
    Image {
        id: WidgetId,
        image: Image,
    },
    Canvas {
        id: WidgetId,
        scene: Value<CanvasScene>,
    },
    #[cfg(feature = "video")]
    VideoSurface {
        id: WidgetId,
        video: VideoSurface,
    },
    Button {
        id: WidgetId,
        label: Value<String>,
        style: crate::ui::widget::ButtonStyle,
    },
    Checkbox {
        id: WidgetId,
        label: Option<Value<String>>,
        style: crate::ui::widget::CheckboxStyle,
    },
    Radio {
        id: WidgetId,
        label: Option<Value<String>>,
        style: crate::ui::widget::RadioStyle,
    },
    Switch {
        id: WidgetId,
        style: crate::ui::widget::SwitchStyle,
    },
    Select {
        id: WidgetId,
        selected_label: Value<Option<String>>,
        placeholder: Value<String>,
        style: crate::ui::widget::SelectStyle,
    },
    Slider {
        id: WidgetId,
        style: crate::ui::widget::SliderStyle,
    },
    TextEditor {
        id: WidgetId,
        controller: TextController,
        placeholder: Value<String>,
        style: crate::ui::widget::InputStyle,
        multiline: bool,
    },
}

#[derive(Clone)]
pub(crate) struct LayoutNode {
    pub node: TaffyNodeId,
    pub children: Vec<LayoutNode>,
}

pub(crate) enum HitInteraction<VM> {
    Disabled {
        id: WidgetId,
    },
    Widget {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        focusable: bool,
    },
    SelectableText {
        id: WidgetId,
        frame: Rect,
        padding: Insets,
        interactions: InteractionHandlers<VM>,
        text_style: Text,
        text: String,
    },
    Switch {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        on_change: Option<ValueCommand<VM, bool>>,
        current: bool,
    },
    Checkbox {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        on_change: Option<ValueCommand<VM, bool>>,
        current: bool,
    },
    Radio {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        on_change: Option<ValueCommand<VM, bool>>,
        current: bool,
    },
    SelectTrigger {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        on_open_change: Option<ValueCommand<VM, bool>>,
        is_open: bool,
    },
    Slider {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        on_change: Option<ValueCommand<VM, f32>>,
        value: f32,
        min: f32,
        max: f32,
        step: f32,
        track_rect: Rect,
        thumb_rect: Rect,
    },
    TextInput {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        controller: TextController,
        on_change: Option<Command<VM>>,
        on_change_set: Option<ValueCommand<VM, TextChangeSet>>,
        multiline: bool,
        auto_wrap: bool,
        show_scrollbar: bool,
        frame: Rect,
        padding: Insets,
        text_style: Text,
    },
    SelectOption {
        id: WidgetId,
        option_index: usize,
        interactions: InteractionHandlers<VM>,
        on_select: Option<Command<VM>>,
        on_open_change: Option<ValueCommand<VM, bool>>,
    },
    CanvasItem {
        id: WidgetId,
        item_id: CanvasItemId,
        item_interactions: CanvasItemInteractionHandlers<VM>,
        cursor_style: Option<CursorStyle>,
        canvas_origin: Point,
        item_origin: Point,
        inverse_transform: [f32; 6],
        text_hits: Arc<[CanvasTextHitRegion]>,
    },
}

impl<VM> Clone for HitInteraction<VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Disabled { id } => Self::Disabled { id: *id },
            Self::Widget {
                id,
                interactions,
                focusable,
            } => Self::Widget {
                id: *id,
                interactions: interactions.clone(),
                focusable: *focusable,
            },
            Self::SelectableText {
                id,
                frame,
                padding,
                interactions,
                text_style,
                text,
            } => Self::SelectableText {
                id: *id,
                frame: *frame,
                padding: *padding,
                interactions: interactions.clone(),
                text_style: text_style.clone(),
                text: text.clone(),
            },
            Self::Switch {
                id,
                interactions,
                on_change,
                current,
            } => Self::Switch {
                id: *id,
                interactions: interactions.clone(),
                on_change: on_change.clone(),
                current: *current,
            },
            Self::Checkbox {
                id,
                interactions,
                on_change,
                current,
            } => Self::Checkbox {
                id: *id,
                interactions: interactions.clone(),
                on_change: on_change.clone(),
                current: *current,
            },
            Self::Radio {
                id,
                interactions,
                on_change,
                current,
            } => Self::Radio {
                id: *id,
                interactions: interactions.clone(),
                on_change: on_change.clone(),
                current: *current,
            },
            Self::SelectTrigger {
                id,
                interactions,
                on_open_change,
                is_open,
            } => Self::SelectTrigger {
                id: *id,
                interactions: interactions.clone(),
                on_open_change: on_open_change.clone(),
                is_open: *is_open,
            },
            Self::Slider {
                id,
                interactions,
                on_change,
                value,
                min,
                max,
                step,
                track_rect,
                thumb_rect,
            } => Self::Slider {
                id: *id,
                interactions: interactions.clone(),
                on_change: on_change.clone(),
                value: *value,
                min: *min,
                max: *max,
                step: *step,
                track_rect: *track_rect,
                thumb_rect: *thumb_rect,
            },
            Self::TextInput {
                id,
                interactions,
                controller,
                on_change,
                on_change_set,
                multiline,
                auto_wrap,
                show_scrollbar,
                frame,
                padding,
                text_style,
            } => Self::TextInput {
                id: *id,
                interactions: interactions.clone(),
                controller: controller.clone(),
                on_change: on_change.clone(),
                on_change_set: on_change_set.clone(),
                multiline: *multiline,
                auto_wrap: *auto_wrap,
                show_scrollbar: *show_scrollbar,
                frame: *frame,
                padding: *padding,
                text_style: text_style.clone(),
            },
            Self::SelectOption {
                id,
                option_index,
                interactions,
                on_select,
                on_open_change,
            } => Self::SelectOption {
                id: *id,
                option_index: *option_index,
                interactions: interactions.clone(),
                on_select: on_select.clone(),
                on_open_change: on_open_change.clone(),
            },
            Self::CanvasItem {
                id,
                item_id,
                item_interactions,
                cursor_style,
                canvas_origin,
                item_origin,
                inverse_transform,
                text_hits,
            } => Self::CanvasItem {
                id: *id,
                item_id: *item_id,
                item_interactions: item_interactions.clone(),
                cursor_style: *cursor_style,
                canvas_origin: *canvas_origin,
                item_origin: *item_origin,
                inverse_transform: *inverse_transform,
                text_hits: Arc::clone(text_hits),
            },
        }
    }
}

#[derive(Clone)]
pub struct CanvasTextHitRegion {
    pub hit: CanvasTextHit,
    pub quad: [Point; 4],
}

impl<VM> HitInteraction<VM> {
    pub(crate) fn target_id(&self) -> HitTargetId {
        match self {
            Self::Disabled { id }
            | Self::Widget { id, .. }
            | Self::SelectableText { id, .. }
            | Self::Switch { id, .. }
            | Self::Checkbox { id, .. }
            | Self::Radio { id, .. }
            | Self::SelectTrigger { id, .. }
            | Self::Slider { id, .. }
            | Self::TextInput { id, .. } => HitTargetId::Widget(*id),
            Self::SelectOption {
                id, option_index, ..
            } => HitTargetId::SelectOption {
                widget_id: *id,
                option_index: *option_index,
            },
            Self::CanvasItem { id, item_id, .. } => HitTargetId::CanvasItem {
                widget_id: *id,
                item_id: *item_id,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum HitTargetId {
    Widget(WidgetId),
    SelectOption {
        widget_id: WidgetId,
        option_index: usize,
    },
    CanvasItem {
        widget_id: WidgetId,
        item_id: CanvasItemId,
    },
}

#[derive(Clone)]
pub(crate) enum HitGeometry {
    Rect,
    Quad([Point; 4]),
    Triangles(Arc<[[Point; 3]]>),
}

impl HitGeometry {
    pub(crate) fn contains(&self, point: Point) -> bool {
        match self {
            Self::Rect => true,
            Self::Quad(quad) => {
                point_in_triangle(point, quad[0], quad[1], quad[2])
                    || point_in_triangle(point, quad[0], quad[2], quad[3])
            }
            Self::Triangles(triangles) => triangles
                .iter()
                .any(|triangle| point_in_triangle(point, triangle[0], triangle[1], triangle[2])),
        }
    }
}

pub(crate) struct HitRegion<VM> {
    pub rect: Rect,
    pub clip_rect: Option<Rect>,
    pub geometry: HitGeometry,
    pub interaction: HitInteraction<VM>,
}

impl<VM> Clone for HitRegion<VM> {
    fn clone(&self) -> Self {
        Self {
            rect: self.rect,
            clip_rect: self.clip_rect,
            geometry: self.geometry.clone(),
            interaction: self.interaction.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrollRegion {
    pub id: WidgetId,
    pub content_viewport: Rect,
    pub visible_frame: Rect,
    pub content_bounds: Rect,
    pub scroll_offset: Point,
    pub overflow_x: Overflow,
    pub overflow_y: Overflow,
    pub horizontal_track: Option<Rect>,
    pub horizontal_thumb: Option<Rect>,
    pub vertical_track: Option<Rect>,
    pub vertical_thumb: Option<Rect>,
}

impl ScrollRegion {
    pub(crate) fn max_offset(self) -> Point {
        Point {
            x: (self.content_bounds.right() - self.content_viewport.right()).max(0.0),
            y: (self.content_bounds.bottom() - self.content_viewport.bottom()).max(0.0),
        }
    }

    pub(crate) fn can_scroll_x(self) -> bool {
        self.overflow_x == Overflow::Scroll && self.max_offset().x > Dp::ZERO
    }

    pub(crate) fn can_scroll_y(self) -> bool {
        self.overflow_y == Overflow::Scroll && self.max_offset().y > Dp::ZERO
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ScrollbarAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ScrollbarHandle {
    pub id: WidgetId,
    pub axis: ScrollbarAxis,
}

pub(crate) struct ComputedScene<VM> {
    pub scene: ScenePrimitives,
    pub hit_regions: Vec<HitRegion<VM>>,
    pub overlay_hit_regions: Vec<HitRegion<VM>>,
    pub scroll_regions: Vec<ScrollRegion>,
    pub ime_cursor_area: Option<Rect>,
    pub(crate) dependencies: DependencyGraph,
}

impl<VM> Clone for ComputedScene<VM> {
    fn clone(&self) -> Self {
        Self {
            scene: self.scene.clone(),
            hit_regions: self.hit_regions.clone(),
            overlay_hit_regions: self.overlay_hit_regions.clone(),
            scroll_regions: self.scroll_regions.clone(),
            ime_cursor_area: self.ime_cursor_area,
            dependencies: self.dependencies.clone(),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct WidgetStateMap {
    states: HashMap<WidgetId, WidgetState>,
    select_option_states: HashMap<(WidgetId, usize), WidgetState>,
}

impl WidgetStateMap {
    pub(crate) fn set(&mut self, id: WidgetId, state: WidgetState) {
        self.states.insert(id, state);
    }

    pub(crate) fn get(&self, id: WidgetId) -> WidgetState {
        self.states.get(&id).copied().unwrap_or_default()
    }

    pub(crate) fn set_select_option(
        &mut self,
        widget_id: WidgetId,
        option_index: usize,
        state: WidgetState,
    ) {
        self.select_option_states
            .insert((widget_id, option_index), state);
    }

    pub(crate) fn get_select_option(
        &self,
        widget_id: WidgetId,
        option_index: usize,
    ) -> WidgetState {
        self.select_option_states
            .get(&(widget_id, option_index))
            .copied()
            .unwrap_or_default()
    }
}

impl<VM> Default for ComputedScene<VM> {
    fn default() -> Self {
        Self {
            scene: ScenePrimitives::default(),
            hit_regions: Vec::new(),
            overlay_hit_regions: Vec::new(),
            scroll_regions: Vec::new(),
            ime_cursor_area: None,
            dependencies: DependencyGraph::default(),
        }
    }
}

impl<VM> ComputedScene<VM> {
    pub(crate) fn extend(&mut self, other: &ComputedScene<VM>) {
        self.scene.extend(&other.scene);
        self.hit_regions.extend(other.hit_regions.iter().cloned());
        self.overlay_hit_regions
            .extend(other.overlay_hit_regions.iter().cloned());
        self.scroll_regions
            .extend(other.scroll_regions.iter().copied());
        if self.ime_cursor_area.is_none() {
            self.ime_cursor_area = other.ime_cursor_area;
        }
        self.dependencies.merge_from(&other.dependencies);
    }

    #[cfg(test)]
    pub(crate) fn rendered(&self) -> RenderedWidgetScene {
        RenderedWidgetScene {
            primitives: self.scene.clone(),
            scroll_regions: self.scroll_regions.clone(),
            ime_cursor_area: self.ime_cursor_area,
        }
    }
}

impl Point {
    pub const ZERO: Self = Self {
        x: Dp::ZERO,
        y: Dp::ZERO,
    };

    pub fn new(x: impl Into<Dp>, y: impl Into<Dp>) -> Self {
        Self {
            x: x.into(),
            y: y.into(),
        }
    }
}

fn point_in_triangle(point: Point, a: Point, b: Point, c: Point) -> bool {
    let point = (point.x.get(), point.y.get());
    let a = (a.x.get(), a.y.get());
    let b = (b.x.get(), b.y.get());
    let c = (c.x.get(), c.y.get());

    let sign = |p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| {
        (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
    };

    let d1 = sign(point, a, b);
    let d2 = sign(point, b, c);
    let d3 = sign(point, c, a);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TextEditState {
    pub cursor: usize,
    pub anchor: usize,
    pub composition: Option<CompositionState>,
    pub scroll_x: Dp,
    pub scroll_y: Dp,
    pub preferred_column_x: Option<f32>,
}

impl TextEditState {
    pub(crate) fn caret_at(text: &str) -> Self {
        let end = text.len();
        Self {
            cursor: end,
            anchor: end,
            composition: None,
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }
    }

    pub(crate) fn selection_range(&self) -> Option<(usize, usize)> {
        (self.cursor != self.anchor)
            .then_some((self.cursor.min(self.anchor), self.cursor.max(self.anchor)))
    }

    pub(crate) fn clamped_to(mut self, text: &str) -> Self {
        self.cursor = clamp_to_char_boundary(text, self.cursor);
        self.anchor = clamp_to_char_boundary(text, self.anchor);
        if let Some(composition) = &mut self.composition {
            composition.replace_range.0 = clamp_to_char_boundary(text, composition.replace_range.0);
            composition.replace_range.1 = clamp_to_char_boundary(text, composition.replace_range.1);
            if composition.replace_range.0 > composition.replace_range.1 {
                composition.replace_range =
                    (composition.replace_range.1, composition.replace_range.1);
            }
            if let Some((start, end)) = composition.cursor {
                let start = clamp_to_char_boundary(&composition.text, start);
                let end = clamp_to_char_boundary(&composition.text, end);
                composition.cursor = Some(if start <= end {
                    (start, end)
                } else {
                    (end, end)
                });
            }
        }
        self.scroll_x = self.scroll_x.max(Dp::ZERO);
        self.scroll_y = self.scroll_y.max(Dp::ZERO);
        if let Some(preferred_column_x) = self.preferred_column_x {
            self.preferred_column_x = preferred_column_x
                .is_finite()
                .then_some(preferred_column_x.max(0.0));
        }
        self
    }
}

fn clamp_to_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompositionState {
    pub replace_range: (usize, usize),
    pub text: String,
    pub cursor: Option<(usize, usize)>,
}

#[cfg(test)]
mod tests {
    use super::{CompositionState, TextEditState};
    use crate::ui::unit::Dp;

    #[test]
    fn input_edit_state_clamps_to_utf8_char_boundaries() {
        let text = "输入框示例输入框示例输入框示例";

        let state = TextEditState {
            cursor: 25,
            anchor: 29,
            composition: Some(CompositionState {
                replace_range: (25, 29),
                text: "提示".to_string(),
                cursor: Some((1, 4)),
            }),
            scroll_x: Dp::ZERO,
            scroll_y: Dp::ZERO,
            preferred_column_x: None,
        }
        .clamped_to(text);

        assert_eq!(state.cursor, 24);
        assert_eq!(state.anchor, 27);
        assert_eq!(
            state
                .composition
                .as_ref()
                .map(|composition| composition.replace_range),
            Some((24, 27))
        );
        assert_eq!(
            state
                .composition
                .as_ref()
                .and_then(|composition| composition.cursor),
            Some((0, 3))
        );
    }
}

#[cfg(test)]
#[derive(Clone, Default)]
pub(crate) struct RenderedWidgetScene {
    pub primitives: ScenePrimitives,
    pub scroll_regions: Vec<ScrollRegion>,
    #[allow(dead_code)]
    pub ime_cursor_area: Option<Rect>,
}
