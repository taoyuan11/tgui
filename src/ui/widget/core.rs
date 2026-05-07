use std::collections::HashMap;
use std::sync::Arc;

use taffy::prelude::{
    length, line, span, AlignContent as TaffyAlignContent, AlignItems as TaffyAlignItems,
    AvailableSpace, Dimension, Display, FlexDirection, FlexWrap, FromFr, FromLength, FromPercent,
    GridTemplateComponent, JustifyContent as TaffyJustifyContent, LengthPercentage,
    LengthPercentageAuto, Position as TaffyPosition, Style as TaffyStyle, TaffyAuto, TaffyTree,
    TaffyZero, TrackSizingFunction,
};
use taffy::Size as TaffySize;

use crate::animation::{AnimationEngine, Transition, WidgetProperty};
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::media::{
    media_placeholder_color, media_placeholder_label, resolve_media_rect, ContentFit,
    IntrinsicSize, MediaManager, RasterRequest,
};
use crate::text::font::{FontManager, TextFontRequest, ICON_FONT_FAMILY};
use crate::ui::layout::{
    Align, Axis, Insets, Justify, LayoutStyle, Length, Overflow, PositionType, Track, Value, Wrap,
};
use crate::ui::theme::{Theme, WidgetState};
use crate::ui::unit::{dp, sp, Dp, Sp, UnitContext};
#[cfg(feature = "video")]
use crate::video::VideoSurface as PublicVideoSurface;

use super::canvas::{canvas_bounds, CanvasClipContext, CanvasItem};
#[cfg(test)]
use super::common::RenderedWidgetScene;
use super::common::{
    BackdropBlurPrimitive, BrushPrimitive, ButtonVariantKind, ClipMask, ComputedScene,
    ContainerKind, ContainerLayout, CursorStyle, HitGeometry, HitInteraction, HitRegion,
    InteractionHandlers, LayoutNode, MeasureContext, MediaEventHandlers, MediaEventPhase,
    MediaEventState, Point, Rect, RenderPrimitive, ScenePrimitives, ScrollRegion, ScrollbarAxis,
    ScrollbarHandle, SelectOptionState, TextEditState, TextPrimitive, TexturePrimitive,
    VisualStyle, WidgetId, WidgetKind, WidgetStateMap,
};
#[cfg(feature = "video")]
use super::style::VideoSurfaceStyle as WidgetVideoSurfaceStyle;
use super::style::{
    infer_theme_mode, ButtonStyle as WidgetButtonStyle, CheckboxStyle as WidgetCheckboxStyle,
    FocusRingOverride, RadioStyle as WidgetRadioStyle, SelectStyle as WidgetSelectStyle,
    SwitchStyle as WidgetSwitchStyle, TextWidgetStyle,
};
use super::text::Text;

/// Caret width in logical pixels.
/// 光标的像素宽度
const CARET_WIDTH: f32 = 2.0;

/// Caret end gap in logical pixels.
/// 光标末尾间隔
const CARET_END_GAP: f32 = 1.0;

/// Default intrinsic width for selects when no explicit width is set.
const SELECT_DEFAULT_WIDTH: f32 = 160.0;

const CHECKBOX_CHECKMARK_ICON: &str = "\u{e687}";
const SELECT_ARROW_ICON: &str = "\u{e686}";

#[derive(Clone)]
struct ResolvedButtonStyle {
    background: Color,
    border_color: Color,
    focus_ring: Option<crate::theme::FocusRingStyle>,
    border_width: Dp,
    radius: Dp,
    padding_x: Dp,
    padding_y: Dp,
    min_height: Dp,
}

#[derive(Clone)]
struct ResolvedCheckboxStyle {
    background: Color,
    border: Color,
    focus_ring: Option<crate::theme::FocusRingStyle>,
    checkmark: Color,
    label: Color,
    border_width: Dp,
    radius: Dp,
    size: Dp,
    label_gap: Dp,
    text_style: crate::ui::theme::TextStyle,
}

#[derive(Clone)]
struct ResolvedRadioStyle {
    background: Color,
    border: Color,
    focus_ring: Option<crate::theme::FocusRingStyle>,
    indicator: Color,
    label: Color,
    border_width: Dp,
    radius: Dp,
    size: Dp,
    label_gap: Dp,
    text_style: crate::ui::theme::TextStyle,
}

#[derive(Clone)]
struct ResolvedSelectStyle {
    background: Color,
    text: Color,
    placeholder: Color,
    border: Color,
    focus_ring: Option<crate::theme::FocusRingStyle>,
    arrow: Color,
    menu_background: Color,
    selected_option_background: Color,
    border_width: Dp,
    radius: Dp,
    padding_x: Dp,
    padding_y: Dp,
    min_height: Dp,
    option_height: Dp,
    menu_gap: Dp,
    text_style: crate::ui::theme::TextStyle,
}

pub struct Element<VM> {
    pub(crate) id: WidgetId,
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) interactions: InteractionHandlers<VM>,
    pub(crate) media_events: MediaEventHandlers<VM>,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) kind: WidgetKind<VM>,
}

impl<VM> Clone for Element<VM> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            layout: self.layout.clone(),
            visual: self.visual.clone(),
            interactions: self.interactions.clone(),
            media_events: self.media_events.clone(),
            background: self.background.clone(),
            kind: self.kind.clone(),
        }
    }
}

#[derive(Clone)]
struct ResolvedElement<VM> {
    id: WidgetId,
    layout: LayoutStyle,
    visual: VisualStyle,
    interactions: InteractionHandlers<VM>,
    media_events: MediaEventHandlers<VM>,
    background: Option<Value<Color>>,
    kind: ResolvedWidgetKind<VM>,
}

#[derive(Clone)]
enum ResolvedWidgetKind<VM> {
    Container {
        layout: ContainerLayout,
        children: Vec<ResolvedElement<VM>>,
    },
    Text {
        text: Text,
    },
    Image {
        image: super::image::Image,
    },
    Canvas {
        items: Vec<CanvasItem>,
        item_interactions: super::common::CanvasItemInteractionHandlers<VM>,
    },
    #[cfg(feature = "video")]
    VideoSurface {
        video: PublicVideoSurface,
        style: WidgetVideoSurfaceStyle,
    },
    Button {
        label: Value<String>,
        disabled: Value<bool>,
        style: WidgetButtonStyle,
    },
    Checkbox {
        checked: Value<bool>,
        label: Option<Value<String>>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: WidgetCheckboxStyle,
    },
    Radio {
        checked: Value<bool>,
        label: Option<Value<String>>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: WidgetRadioStyle,
    },
    Switch {
        checked: Value<bool>,
        on_change: Option<ValueCommand<VM, bool>>,
        active_background: Option<Value<Color>>,
        inactive_background: Option<Value<Color>>,
        active_thumb_color: Option<Value<Color>>,
        inactive_thumb_color: Option<Value<Color>>,
        disabled: Value<bool>,
        style: WidgetSwitchStyle,
    },
    Select {
        selected_label: Value<Option<String>>,
        placeholder: Value<String>,
        options: Vec<SelectOptionState<VM>>,
        open: Option<Value<bool>>,
        on_open_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        style: WidgetSelectStyle,
    },
}

struct CollectContext<'a, 'b> {
    taffy: &'a TaffyTree<MeasureContext>,
    font_manager: &'a FontManager,
    theme: &'a Theme,
    media: &'a MediaManager,
    selected_text: Option<WidgetId>,
    selected_text_state: Option<&'a TextEditState>,
    hovered_scrollbar: Option<ScrollbarHandle>,
    active_scrollbar: Option<ScrollbarHandle>,
    widget_states: &'a WidgetStateMap,
    select_open_states: &'a HashMap<WidgetId, bool>,
    scroll_offsets: &'a HashMap<WidgetId, Point>,
    viewport: Rect,
    units: UnitContext,
    animations: &'b mut AnimationEngine,
    now: std::time::Instant,
}

#[derive(Clone, Copy)]
struct VisualContext {
    origin: Point,
    opacity: f32,
    clip_rect: Rect,
    clip_mask: Option<ClipMask>,
}

#[derive(Clone)]
pub(crate) struct ResolvedSceneLayout<VM> {
    resolved_root: ResolvedElement<VM>,
    layout_root: LayoutNode,
    taffy: TaffyTree<MeasureContext>,
    units: UnitContext,
}

impl<VM> Element<VM> {
    /// Adapts an element tree built for a child view model so it can be mounted
    /// inside a root view model tree.
    ///
    /// Commands stored anywhere inside the scoped subtree are executed against
    /// the child view model returned by `selector`.
    pub fn scope<RootVm: 'static>(
        self,
        selector: impl for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync + 'static,
    ) -> Element<RootVm>
    where
        VM: 'static,
    {
        self.scope_with_selector(Arc::new(selector))
    }

    pub(crate) fn scope_with_selector<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> Element<RootVm>
    where
        VM: 'static,
    {
        let kind = match self.kind {
            WidgetKind::Container {
                layout,
                children,
                style,
            } => WidgetKind::Container {
                layout,
                children: children
                    .into_iter()
                    .map(|child| child.scope(selector.clone()))
                    .collect(),
                style,
            },
            WidgetKind::Text { text } => WidgetKind::Text { text },
            WidgetKind::Image { image } => WidgetKind::Image { image },
            WidgetKind::Canvas {
                items,
                item_interactions,
                style,
            } => WidgetKind::Canvas {
                items,
                item_interactions: item_interactions.scope(selector.clone()),
                style,
            },
            #[cfg(feature = "video")]
            WidgetKind::VideoSurface { video, style } => WidgetKind::VideoSurface { video, style },
            WidgetKind::Button {
                label,
                disabled,
                variant,
                style,
            } => WidgetKind::Button {
                label,
                disabled,
                variant,
                style,
            },
            WidgetKind::Checkbox {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => WidgetKind::Checkbox {
                checked,
                label,
                on_change: on_change.map(|command| command.scope(selector.clone())),
                disabled,
                style,
            },
            WidgetKind::Radio {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => WidgetKind::Radio {
                checked,
                label,
                on_change: on_change.map(|command| command.scope(selector.clone())),
                disabled,
                style,
            },
            WidgetKind::Switch {
                checked,
                on_change,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                style,
            } => WidgetKind::Switch {
                checked,
                on_change: on_change.map(|command| command.scope(selector.clone())),
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                style,
            },
            WidgetKind::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                disabled,
                style,
            } => WidgetKind::Select {
                selected_label,
                placeholder,
                options: options
                    .into_iter()
                    .map(|option| SelectOptionState {
                        label: option.label,
                        selected: option.selected,
                        disabled: option.disabled,
                        on_select: option
                            .on_select
                            .map(|command| command.scope(selector.clone())),
                    })
                    .collect(),
                open,
                on_open_change: on_open_change.map(|command| command.scope(selector.clone())),
                disabled,
                style,
            },
        };

        Element {
            id: self.id,
            layout: self.layout,
            visual: self.visual,
            interactions: self.interactions.scope(selector.clone()),
            media_events: self.media_events.scope(selector),
            background: self.background,
            kind,
        }
    }

    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.interactions.on_click = Some(command);
        self
    }

    pub fn on_double_click(mut self, command: Command<VM>) -> Self {
        self.interactions.on_double_click = Some(command);
        self
    }

    pub fn on_mouse_enter(mut self, command: Command<VM>) -> Self {
        self.interactions.on_mouse_enter = Some(command);
        self
    }

    pub fn on_mouse_leave(mut self, command: Command<VM>) -> Self {
        self.interactions.on_mouse_leave = Some(command);
        self
    }

    pub fn on_mouse_move(mut self, command: ValueCommand<VM, Point>) -> Self {
        self.interactions.on_mouse_move = Some(command);
        self
    }

    pub fn on_loading(mut self, command: Command<VM>) -> Self {
        self.media_events.on_loading = Some(command);
        self
    }

    pub fn on_success(mut self, command: Command<VM>) -> Self {
        self.media_events.on_success = Some(command);
        self
    }

    pub fn on_error(mut self, command: ValueCommand<VM, String>) -> Self {
        self.media_events.on_error = Some(command);
        self
    }

    fn resolve(&self, theme: &Theme) -> ResolvedElement<VM> {
        let layout = self.layout.clone();
        let mut visual = self.visual.clone();
        let mut background = self.background.clone();
        let kind = match &self.kind {
            WidgetKind::Container {
                layout: container_layout,
                children,
                style,
            } => {
                let resolved_style = resolved_container_style(style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                let mut layout = container_layout.clone();
                layout.scrollbar_style = resolved_style.scrollbar;
                ResolvedWidgetKind::Container {
                    layout,
                    children: children
                        .iter()
                        .flat_map(|child| child.resolve())
                        .map(|child| child.resolve(theme))
                        .collect(),
                }
            }
            WidgetKind::Text { text } => {
                let mut text = text.clone();
                let resolved_style = resolved_text_widget_style(text.style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                apply_text_widget_style(&mut text, &resolved_style);
                ResolvedWidgetKind::Text { text }
            }
            WidgetKind::Image { image } => {
                let mut image = image.clone();
                let resolved_style = resolved_image_style(image.style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                image.background = resolved_style.surface.background.clone();
                image.fit = resolved_style.fit;
                ResolvedWidgetKind::Image { image }
            }
            WidgetKind::Canvas {
                items,
                item_interactions,
                style,
            } => {
                let resolved_style = resolved_canvas_style(style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                ResolvedWidgetKind::Canvas {
                    items: items.resolve(),
                    item_interactions: item_interactions.clone(),
                }
            }
            #[cfg(feature = "video")]
            WidgetKind::VideoSurface { video, style } => {
                let mut video = video.clone();
                let resolved_style = resolved_video_surface_style(style.as_ref(), theme);
                apply_surface_style(&mut background, &mut visual, &resolved_style.surface);
                video.background = resolved_style.surface.background.clone();
                video.fit = resolved_style.fit;
                ResolvedWidgetKind::VideoSurface {
                    video,
                    style: resolved_style,
                }
            }
            WidgetKind::Button {
                label,
                disabled,
                variant,
                style,
            } => ResolvedWidgetKind::Button {
                label: label.clone(),
                disabled: disabled.clone(),
                style: resolved_button_style(style.as_ref(), theme, *variant),
            },
            WidgetKind::Checkbox {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => ResolvedWidgetKind::Checkbox {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: resolved_checkbox_style(style.as_ref(), theme),
            },
            WidgetKind::Radio {
                checked,
                label,
                on_change,
                disabled,
                style,
            } => ResolvedWidgetKind::Radio {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                style: resolved_radio_style(style.as_ref(), theme),
            },
            WidgetKind::Switch {
                checked,
                on_change,
                active_background,
                inactive_background,
                active_thumb_color,
                inactive_thumb_color,
                disabled,
                style,
            } => ResolvedWidgetKind::Switch {
                checked: checked.clone(),
                on_change: on_change.clone(),
                active_background: active_background.clone(),
                inactive_background: inactive_background.clone(),
                active_thumb_color: active_thumb_color.clone(),
                inactive_thumb_color: inactive_thumb_color.clone(),
                disabled: disabled.clone(),
                style: resolved_switch_style(style.as_ref(), theme),
            },
            WidgetKind::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                disabled,
                style,
            } => ResolvedWidgetKind::Select {
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                options: options.clone(),
                open: open.clone(),
                on_open_change: on_open_change.clone(),
                disabled: disabled.clone(),
                style: resolved_select_style(style.as_ref(), theme),
            },
        };

        ResolvedElement {
            id: self.id,
            layout,
            visual,
            interactions: self.interactions.clone(),
            media_events: self.media_events.clone(),
            background,
            kind,
        }
    }
}

impl<VM> ResolvedElement<VM> {
    fn measure_context(&self) -> MeasureContext {
        match &self.kind {
            ResolvedWidgetKind::Container { .. } => MeasureContext::None,
            ResolvedWidgetKind::Text { text } => MeasureContext::Text(text.clone()),
            ResolvedWidgetKind::Image { image } => MeasureContext::Image(image.clone()),
            ResolvedWidgetKind::Canvas { items, .. } => MeasureContext::Canvas(items.clone()),
            #[cfg(feature = "video")]
            ResolvedWidgetKind::VideoSurface { video, .. } => {
                MeasureContext::VideoSurface(video.clone())
            }
            ResolvedWidgetKind::Button { label, style, .. } => MeasureContext::Button {
                label: label.clone(),
                style: style.clone(),
            },
            ResolvedWidgetKind::Checkbox { label, style, .. } => MeasureContext::Checkbox {
                label: label.clone(),
                style: style.clone(),
            },
            ResolvedWidgetKind::Radio { label, style, .. } => MeasureContext::Radio {
                label: label.clone(),
                style: style.clone(),
            },
            ResolvedWidgetKind::Switch { style, .. } => MeasureContext::Switch {
                style: style.clone(),
            },
            ResolvedWidgetKind::Select {
                selected_label,
                placeholder,
                style,
                ..
            } => MeasureContext::Select {
                selected_label: selected_label.resolve(),
                placeholder: placeholder.clone(),
                style: style.clone(),
            },
        }
    }

    fn build_layout_tree(
        &self,
        taffy: &mut TaffyTree<MeasureContext>,
        animations: &mut AnimationEngine,
        theme: &Theme,
        units: UnitContext,
        parent_kind: Option<ContainerKind>,
        viewport: Rect,
        is_root: bool,
        now: std::time::Instant,
    ) -> Result<LayoutNode, taffy::TaffyError> {
        let mut child_layouts = Vec::new();
        if let ResolvedWidgetKind::Container { layout, children } = &self.kind {
            child_layouts.reserve(children.len());
            for child in children {
                child_layouts.push(child.build_layout_tree(
                    taffy,
                    animations,
                    theme,
                    units,
                    Some(layout.kind.clone()),
                    viewport,
                    false,
                    now,
                )?);
            }
        }

        let style = self.taffy_style(
            parent_kind,
            viewport,
            is_root,
            animations,
            theme,
            units,
            now,
        );
        let node = if child_layouts.is_empty() {
            taffy.new_leaf_with_context(style, self.measure_context())?
        } else {
            let child_ids = child_layouts
                .iter()
                .map(|child| child.node)
                .collect::<Vec<_>>();
            taffy.new_with_children(style, &child_ids)?
        };

        Ok(LayoutNode {
            node,
            children: child_layouts,
        })
    }

    fn taffy_style(
        &self,
        parent_kind: Option<ContainerKind>,
        viewport: Rect,
        is_root: bool,
        animations: &mut AnimationEngine,
        theme: &Theme,
        units: UnitContext,
        now: std::time::Instant,
    ) -> TaffyStyle {
        let default_min_width = match &self.kind {
            ResolvedWidgetKind::Select { .. } if self.layout.min_width.is_none() => {
                Dimension::from_length(0.0)
            }
            _ => Dimension::AUTO,
        };
        let width = if is_root {
            Some(Dimension::from_length(viewport.width))
        } else {
            self.layout.width.as_ref().map(|value| {
                resolve_dimension(
                    value,
                    animations,
                    self.id,
                    WidgetProperty::Width,
                    now,
                    units,
                )
            })
        };
        let height = if is_root {
            Some(Dimension::from_length(viewport.height))
        } else {
            self.layout.height.as_ref().map(|value| {
                resolve_dimension(
                    value,
                    animations,
                    self.id,
                    WidgetProperty::Height,
                    now,
                    units,
                )
            })
        };
        let mut style = TaffyStyle {
            size: TaffySize {
                width: width.unwrap_or(Dimension::AUTO),
                height: height.unwrap_or(Dimension::AUTO),
            },
            min_size: TaffySize {
                width: self
                    .layout
                    .min_width
                    .as_ref()
                    .map(|value| {
                        resolve_dimension(
                            value,
                            animations,
                            self.id,
                            WidgetProperty::Width,
                            now,
                            units,
                        )
                    })
                    .unwrap_or(default_min_width),
                height: self
                    .layout
                    .min_height
                    .as_ref()
                    .map(|value| {
                        resolve_dimension(
                            value,
                            animations,
                            self.id,
                            WidgetProperty::Height,
                            now,
                            units,
                        )
                    })
                    .unwrap_or(Dimension::AUTO),
            },
            max_size: TaffySize {
                width: self
                    .layout
                    .max_width
                    .as_ref()
                    .map(|value| {
                        resolve_dimension(
                            value,
                            animations,
                            self.id,
                            WidgetProperty::Width,
                            now,
                            units,
                        )
                    })
                    .unwrap_or(Dimension::AUTO),
                height: self
                    .layout
                    .max_height
                    .as_ref()
                    .map(|value| {
                        resolve_dimension(
                            value,
                            animations,
                            self.id,
                            WidgetProperty::Height,
                            now,
                            units,
                        )
                    })
                    .unwrap_or(Dimension::AUTO),
            },
            margin: to_taffy_rect_auto(
                self.layout
                    .margin
                    .resolve_widget(animations, self.id, WidgetProperty::Margin, now),
                units,
            ),
            padding: to_taffy_rect(
                self.layout
                    .padding
                    .as_ref()
                    .map(|padding| {
                        padding.resolve_widget(animations, self.id, WidgetProperty::Padding, now)
                    })
                    .unwrap_or_else(|| default_layout_padding(self, theme)),
                units,
            ),
            flex_grow: self
                .layout
                .grow
                .resolve_widget(animations, self.id, WidgetProperty::Grow, now)
                .max(0.0),
            flex_shrink: self
                .layout
                .shrink
                .resolve_widget(animations, self.id, WidgetProperty::Grow, now)
                .max(0.0),
            flex_basis: self
                .layout
                .basis
                .as_ref()
                .map(|value| {
                    resolve_dimension(
                        value,
                        animations,
                        self.id,
                        WidgetProperty::Width,
                        now,
                        units,
                    )
                })
                .unwrap_or(Dimension::AUTO),
            aspect_ratio: self.layout.aspect_ratio.as_ref().map(|value| {
                value
                    .resolve_widget(animations, self.id, WidgetProperty::Grow, now)
                    .max(0.0)
            }),
            position: match self.layout.position_type {
                PositionType::Relative => TaffyPosition::Relative,
                PositionType::Absolute => TaffyPosition::Absolute,
            },
            inset: taffy::Rect {
                left: self
                    .layout
                    .left
                    .as_ref()
                    .map(|value| {
                        resolve_length_percentage_auto(
                            value,
                            animations,
                            self.id,
                            WidgetProperty::Width,
                            now,
                            units,
                        )
                    })
                    .unwrap_or(LengthPercentageAuto::AUTO),
                right: self
                    .layout
                    .right
                    .as_ref()
                    .map(|value| {
                        resolve_length_percentage_auto(
                            value,
                            animations,
                            self.id,
                            WidgetProperty::Width,
                            now,
                            units,
                        )
                    })
                    .unwrap_or(LengthPercentageAuto::AUTO),
                top: self
                    .layout
                    .top
                    .as_ref()
                    .map(|value| {
                        resolve_length_percentage_auto(
                            value,
                            animations,
                            self.id,
                            WidgetProperty::Height,
                            now,
                            units,
                        )
                    })
                    .unwrap_or(LengthPercentageAuto::AUTO),
                bottom: self
                    .layout
                    .bottom
                    .as_ref()
                    .map(|value| {
                        resolve_length_percentage_auto(
                            value,
                            animations,
                            self.id,
                            WidgetProperty::Height,
                            now,
                            units,
                        )
                    })
                    .unwrap_or(LengthPercentageAuto::AUTO),
            },
            align_self: self.layout.align_self.map(map_align_self),
            justify_self: self.layout.justify_self.map(map_align_self),
            ..Default::default()
        };

        if matches!(parent_kind, Some(ContainerKind::Stack)) {
            style.grid_row.start = line(1);
            style.grid_row.end = span(self.layout.row_span.max(1) as u16);
            style.grid_column.start = line(1);
            style.grid_column.end = span(self.layout.column_span.max(1) as u16);
        } else {
            if let Some(start) = self.layout.row_start {
                style.grid_row.start = line(start as i16);
            }
            if self.layout.row_span > 1 {
                style.grid_row.end = span(self.layout.row_span as u16);
            }
            if let Some(start) = self.layout.column_start {
                style.grid_column.start = line(start as i16);
            }
            if self.layout.column_span > 1 {
                style.grid_column.end = span(self.layout.column_span as u16);
            }
        }

        if let ResolvedWidgetKind::Container { layout, .. } = &self.kind {
            apply_container_style(&mut style, layout, animations, self.id, units, now);
        }

        style
    }

    fn collect_primitives(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
    ) {
        let layout = context
            .taffy
            .layout(layout_node.node)
            .expect("layout node should exist");
        let layout_frame = Rect::new(
            visual_context.origin.x + layout.location.x,
            visual_context.origin.y + layout.location.y,
            layout.size.width,
            layout.size.height,
        );
        let offset = self.visual.offset.resolve_widget(
            context.animations,
            self.id,
            WidgetProperty::Offset,
            context.now,
        );
        let frame = Rect::new(
            layout_frame.x + offset.x,
            layout_frame.y + offset.y,
            layout_frame.width,
            layout_frame.height,
        );
        let disabled = match &self.kind {
            ResolvedWidgetKind::Button { disabled, .. }
            | ResolvedWidgetKind::Checkbox { disabled, .. }
            | ResolvedWidgetKind::Radio { disabled, .. }
            | ResolvedWidgetKind::Switch { disabled, .. }
            | ResolvedWidgetKind::Select { disabled, .. } => disabled.resolve(),
            _ => false,
        };
        let widget_state = if disabled {
            WidgetState {
                disabled: true,
                ..Default::default()
            }
        } else {
            context.widget_states.get(self.id)
        };
        let opacity = visual_context.opacity
            * self.visual.opacity.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Opacity,
                context.now,
                0.0,
                1.0,
            )
            * if disabled { 0.55 } else { 1.0 };
        let button_style = match &self.kind {
            ResolvedWidgetKind::Button { style, .. } => {
                Some(resolve_button_style(style, widget_state, context.theme))
            }
            _ => None,
        };
        let select_style = match &self.kind {
            ResolvedWidgetKind::Select { style, .. } => {
                Some(resolve_select_style(style, widget_state, context.theme))
            }
            _ => None,
        };
        let checkbox_style = match &self.kind {
            ResolvedWidgetKind::Checkbox { checked, style, .. } => Some(resolve_checkbox_style(
                style,
                widget_state,
                checked.resolve(),
                context.theme,
            )),
            _ => None,
        };
        let radio_style = match &self.kind {
            ResolvedWidgetKind::Radio { checked, style, .. } => Some(resolve_radio_style(
                style,
                widget_state,
                checked.resolve(),
                context.theme,
            )),
            _ => None,
        };
        let border_width = match &self.kind {
            ResolvedWidgetKind::Button { .. } => {
                let button_style = button_style
                    .as_ref()
                    .expect("button style should be resolved for button widgets");
                self.visual
                    .border_width
                    .as_ref()
                    .map(|width| {
                        width.resolve_widget_to_logical(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderWidth,
                            context.now,
                            context.units,
                        )
                    })
                    .unwrap_or_else(|| context.units.resolve_dp(button_style.border_width))
            }
            ResolvedWidgetKind::Select { .. } => self
                .visual
                .border_width
                .as_ref()
                .map(|width| {
                    width.resolve_widget_to_logical(
                        context.animations,
                        self.id,
                        WidgetProperty::BorderWidth,
                        context.now,
                        context.units,
                    )
                })
                .unwrap_or_else(|| {
                    context
                        .units
                        .resolve_dp(
                            select_style
                                .as_ref()
                                .expect("select style should be resolved for select widgets")
                                .border_width,
                        )
                }),
            ResolvedWidgetKind::Checkbox { .. } => {
                let checkbox_style = checkbox_style
                    .as_ref()
                    .expect("checkbox style should be resolved for checkbox widgets");
                self.visual
                    .border_width
                    .as_ref()
                    .map(|width| {
                        width.resolve_widget_to_logical(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderWidth,
                            context.now,
                            context.units,
                        )
                    })
                    .unwrap_or_else(|| context.units.resolve_dp(checkbox_style.border_width))
            }
            ResolvedWidgetKind::Radio { .. } => {
                let radio_style = radio_style
                    .as_ref()
                    .expect("radio style should be resolved for radio widgets");
                self.visual
                    .border_width
                    .as_ref()
                    .map(|width| {
                        width.resolve_widget_to_logical(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderWidth,
                            context.now,
                            context.units,
                        )
                    })
                    .unwrap_or_else(|| context.units.resolve_dp(radio_style.border_width))
            }
            ResolvedWidgetKind::Switch { style, .. } => {
                let switch_style = style;
                self.visual
                    .border_width
                    .as_ref()
                    .map(|width| {
                        width.resolve_widget_to_logical(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderWidth,
                            context.now,
                            context.units,
                        )
                    })
                    .unwrap_or_else(|| {
                        context
                            .units
                            .resolve_dp(switch_style.border_width.resolve())
                    })
            }
            _ => self
                .visual
                .border_width
                .as_ref()
                .map(|width| {
                    width.resolve_widget_to_logical(
                        context.animations,
                        self.id,
                        WidgetProperty::BorderWidth,
                        context.now,
                        context.units,
                    )
                })
                .unwrap_or(0.0),
        }
        .max(0.0);
        let border_radius = self
            .visual
            .border_radius
            .as_ref()
            .map(|radius| {
                radius.resolve_widget_to_logical(
                    context.animations,
                    self.id,
                    WidgetProperty::BorderRadius,
                    context.now,
                    context.units,
                )
            })
            .unwrap_or_else(|| match &self.kind {
                ResolvedWidgetKind::Button { .. } => context
                    .units
                    .resolve_dp(
                        button_style
                            .as_ref()
                            .expect("button style should be resolved for button widgets")
                            .radius,
                    ),
                ResolvedWidgetKind::Select { .. } => context
                    .units
                    .resolve_dp(
                        select_style
                            .as_ref()
                            .expect("select style should be resolved for select widgets")
                            .radius,
                    ),
                ResolvedWidgetKind::Checkbox { .. } => context.units.resolve_dp(
                    checkbox_style
                        .as_ref()
                        .expect("checkbox style should be resolved for checkbox widgets")
                        .radius,
                ),
                ResolvedWidgetKind::Radio { .. } => context.units.resolve_dp(
                    radio_style
                        .as_ref()
                        .expect("radio style should be resolved for radio widgets")
                        .radius,
                ),
                ResolvedWidgetKind::Switch { style, .. } => {
                    context.units.resolve_dp(style.radius.resolve())
                }
                _ => 0.0,
            })
            .max(0.0);
        let border_color = match &self.kind {
            ResolvedWidgetKind::Button { .. } => {
                let button_style = button_style
                    .as_ref()
                    .expect("button style should be resolved for button widgets");
                self.visual
                    .border_color
                    .as_ref()
                    .map(|color| {
                        color.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderColor,
                            context.now,
                        )
                    })
                    .unwrap_or_else(|| {
                        context.animations.resolve_color(
                            crate::animation::AnimationKey::Widget {
                                id: self.id.raw(),
                                property: WidgetProperty::BorderColor,
                            },
                            button_style.border_color,
                            Some(Transition::default()),
                            context.now,
                        )
                    })
            }
            ResolvedWidgetKind::Select { .. } => {
                let select_style = select_style
                    .as_ref()
                    .expect("select style should be resolved for select widgets");
                self.visual
                    .border_color
                    .as_ref()
                    .map(|color| {
                        color.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderColor,
                            context.now,
                        )
                    })
                    .unwrap_or_else(|| {
                        context.animations.resolve_color(
                            crate::animation::AnimationKey::Widget {
                                id: self.id.raw(),
                                property: WidgetProperty::BorderColor,
                            },
                            select_style.border,
                            Some(Transition::default()),
                            context.now,
                        )
                    })
            }
            ResolvedWidgetKind::Switch { style, checked, .. } => {
                let visual_state = base_interaction_state(widget_state);
                let switch_style = if checked.resolve() {
                    resolve_stateful_widget_color(&style.border_checked, visual_state)
                } else {
                    resolve_stateful_widget_color(&style.border, visual_state)
                };
                self.visual
                    .border_color
                    .as_ref()
                    .map(|color| {
                        color.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::BorderColor,
                            context.now,
                        )
                    })
                    .unwrap_or(switch_style)
            }
            _ => self
                .visual
                .border_color
                .as_ref()
                .map(|color| {
                    color.resolve_widget(
                        context.animations,
                        self.id,
                        WidgetProperty::BorderColor,
                        context.now,
                    )
                })
                .unwrap_or(Color::TRANSPARENT),
        }
        .with_alpha_factor(opacity);
        let background = match &self.kind {
            ResolvedWidgetKind::Button { .. } => self
                .background
                .as_ref()
                .map(|background| {
                    background.resolve_widget(
                        context.animations,
                        self.id,
                        WidgetProperty::Background,
                        context.now,
                    )
                })
                .unwrap_or_else(|| {
                    let button_style = button_style
                        .as_ref()
                        .expect("button style should be resolved for button widgets");
                    context.animations.resolve_color(
                        crate::animation::AnimationKey::Widget {
                            id: self.id.raw(),
                            property: WidgetProperty::Background,
                        },
                        button_style.background,
                        Some(Transition::default()),
                        context.now,
                    )
                }),
            ResolvedWidgetKind::Select { .. } => self
                .background
                .as_ref()
                .map(|background| {
                    background.resolve_widget(
                        context.animations,
                        self.id,
                        WidgetProperty::Background,
                        context.now,
                    )
                })
                .unwrap_or(
                    select_style
                        .as_ref()
                        .expect("select style should be resolved for select widgets")
                        .background,
                ),
            ResolvedWidgetKind::Switch {
                checked,
                active_background,
                inactive_background,
                style,
                ..
            } => context.animations.resolve_color(
                crate::animation::AnimationKey::Widget {
                    id: self.id.raw(),
                    property: WidgetProperty::BackgroundAlt,
                },
                {
                    let visual_state = base_interaction_state(widget_state);
                    if checked.resolve() {
                        active_background.as_ref().map(Value::resolve).unwrap_or(
                            resolve_stateful_widget_color(&style.track_checked, visual_state),
                        )
                    } else {
                        inactive_background
                            .as_ref()
                            .map(Value::resolve)
                            .unwrap_or(resolve_stateful_widget_color(&style.track, visual_state))
                    }
                },
                Some(default_switch_transition()),
                context.now,
            ),
            _ => self
                .background
                .as_ref()
                .map(|background| {
                    background.resolve_widget(
                        context.animations,
                        self.id,
                        WidgetProperty::Background,
                        context.now,
                    )
                })
                .unwrap_or(Color::TRANSPARENT),
        }
        .with_alpha_factor(opacity);

        let background_inset = border_width
            .min((frame.width * 0.5).get())
            .min((frame.height * 0.5).get());
        let background_frame = frame.inset(Insets::all(Dp::new(background_inset)));
        let background_radius = (border_radius - background_inset).max(0.0);
        let primitive_clip = Some(visual_context.clip_rect);
        let primitive_clip_mask = visual_context.clip_mask;
        let background_blur = self
            .visual
            .background_blur
            .resolve_widget_to_logical(
                context.animations,
                self.id,
                WidgetProperty::BackgroundBlur,
                context.now,
                context.units,
            )
            .max(0.0);
        let background_brush = self
            .visual
            .background_brush
            .as_ref()
            .map(|brush| brush.resolve_widget());
        let background_image = self
            .visual
            .background_image
            .as_ref()
            .map(|image| image.resolve_widget());

        if background_blur > 0.0
            && background_frame.width > Dp::ZERO
            && background_frame.height > Dp::ZERO
        {
            computed.scene.push_backdrop_blur(BackdropBlurPrimitive {
                rect: background_frame,
                corner_radius: background_radius,
                blur_radius: background_blur,
                clip_rect: primitive_clip,
                clip_mask: primitive_clip_mask,
            });
        }

        let preserve_solid_background = matches!(self.kind, ResolvedWidgetKind::Switch { .. });

        if background_frame.width > Dp::ZERO && background_frame.height > Dp::ZERO {
            let should_draw_base_background = background.a > 0
                && (background_image.is_some()
                    || background_brush.is_none()
                    || preserve_solid_background);
            if should_draw_base_background {
                computed.scene.push_shape(RenderPrimitive {
                    rect: background_frame,
                    color: background,
                    corner_radius: background_radius,
                    stroke_width: 0.0,
                    clip_rect: primitive_clip,
                    clip_mask: primitive_clip_mask,
                });
            }

            if let Some(image) = background_image.as_ref() {
                push_background_media_texture(
                    &image.source,
                    image.fit,
                    background_frame,
                    background_radius,
                    primitive_clip,
                    primitive_clip_mask,
                    context,
                    computed,
                );
            }

            if let Some(brush) = background_brush.clone() {
                computed.scene.push_brush(BrushPrimitive {
                    rect: background_frame,
                    brush,
                    corner_radius: background_radius,
                    clip_rect: primitive_clip,
                    clip_mask: primitive_clip_mask,
                });
            }
        }

        push_border_primitives(
            &mut computed.scene,
            frame,
            border_width,
            border_color,
            border_radius,
            primitive_clip,
            primitive_clip_mask,
        );
        let focus_ring = match &self.kind {
            ResolvedWidgetKind::Button { .. } => {
                button_style.as_ref().and_then(|style| style.focus_ring.clone())
            }
            ResolvedWidgetKind::Select { .. } => {
                select_style.as_ref().and_then(|style| style.focus_ring.clone())
            }
            ResolvedWidgetKind::Switch { style, .. } => {
                resolve_focus_ring(context.theme, style.focus_ring.as_ref(), widget_state)
            }
            _ => None,
        };
        push_focus_ring_primitives(
            &mut computed.scene,
            frame,
            border_radius,
            focus_ring.as_ref(),
            opacity,
        );

        if disabled {
            computed.hit_regions.push(HitRegion {
                rect: frame,
                clip_rect: primitive_clip,
                geometry: HitGeometry::Rect,
                interaction: HitInteraction::Disabled { id: self.id },
            });
        } else if self.interactions.has_any()
            && !matches!(&self.kind, ResolvedWidgetKind::Text { text } if text.user_select)
            && !matches!(&self.kind, ResolvedWidgetKind::Select { .. })
        {
            computed.hit_regions.push(HitRegion {
                rect: frame,
                clip_rect: primitive_clip,
                geometry: HitGeometry::Rect,
                interaction: HitInteraction::Widget {
                    id: self.id,
                    interactions: self.interactions.clone(),
                    focusable: matches!(
                        self.kind,
                        ResolvedWidgetKind::Button { .. }
                            | ResolvedWidgetKind::Checkbox { .. }
                            | ResolvedWidgetKind::Radio { .. }
                            | ResolvedWidgetKind::Switch { .. }
                            | ResolvedWidgetKind::Select { .. }
                    ),
                },
            });
        }

        match &self.kind {
            ResolvedWidgetKind::Container { layout, children } => {
                let content_bounds =
                    compute_container_content_bounds(self, children, layout_node, frame, context);
                let max_scroll = Point {
                    x: (content_bounds.right() - background_frame.right()).max(0.0),
                    y: (content_bounds.bottom() - background_frame.bottom()).max(0.0),
                };
                let requested_scroll = context
                    .scroll_offsets
                    .get(&self.id)
                    .copied()
                    .unwrap_or(Point::ZERO);
                let scroll_offset = Point {
                    x: if layout.overflow_x == Overflow::Scroll {
                        requested_scroll.x.clamp(0.0, max_scroll.x)
                    } else {
                        Dp::ZERO
                    },
                    y: if layout.overflow_y == Overflow::Scroll {
                        requested_scroll.y.clamp(0.0, max_scroll.y)
                    } else {
                        Dp::ZERO
                    },
                };
                let child_clip_rect = apply_overflow_clip(
                    visual_context.clip_rect,
                    background_frame,
                    layout.overflow_x,
                    layout.overflow_y,
                );
                let child_clip_mask = apply_overflow_clip_mask(
                    visual_context.clip_mask,
                    background_frame,
                    background_radius,
                    layout.overflow_x,
                    layout.overflow_y,
                );
                let scrollbar_geometry = compute_scrollbar_geometry(
                    background_frame,
                    content_bounds,
                    scroll_offset,
                    layout,
                    context.theme,
                    context.units,
                );
                let visible_frame = frame
                    .intersect(visual_context.clip_rect)
                    .unwrap_or(Rect::new(frame.x, frame.y, 0.0, 0.0));
                computed.scroll_regions.push(ScrollRegion {
                    id: self.id,
                    content_viewport: background_frame,
                    visible_frame,
                    content_bounds,
                    scroll_offset,
                    overflow_x: layout.overflow_x,
                    overflow_y: layout.overflow_y,
                    horizontal_track: scrollbar_geometry.horizontal_track,
                    horizontal_thumb: scrollbar_geometry.horizontal_thumb,
                    vertical_track: scrollbar_geometry.vertical_track,
                    vertical_thumb: scrollbar_geometry.vertical_thumb,
                });
                for (child, child_layout) in children.iter().zip(layout_node.children.iter()) {
                    child.collect_primitives(
                        child_layout,
                        VisualContext {
                            origin: Point {
                                x: frame.x - scroll_offset.x,
                                y: frame.y - scroll_offset.y,
                            },
                            opacity,
                            clip_rect: child_clip_rect,
                            clip_mask: child_clip_mask,
                        },
                        context,
                        computed,
                    );
                }
                push_scrollbar_primitives(
                    &mut computed.scene,
                    context.theme,
                    child_clip_rect,
                    opacity,
                    layout,
                    scrollbar_geometry,
                    self.id,
                    context.hovered_scrollbar,
                    context.active_scrollbar,
                );
            }
            ResolvedWidgetKind::Text { text } => {
                let padding = text
                    .layout
                    .padding
                    .as_ref()
                    .map(|padding| {
                        padding.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Padding,
                            context.now,
                        )
                    })
                    .unwrap_or(Insets::ZERO);
                push_text_primitives(
                    text,
                    frame,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    false,
                    false,
                    padding,
                    None,
                    (text.user_select && context.selected_text == Some(self.id))
                        .then_some(context.selected_text_state)
                        .flatten(),
                    context.theme.colors.on_surface,
                    opacity,
                    self.id,
                    primitive_clip,
                    primitive_clip_mask,
                );
                if text.user_select && !disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        clip_rect: primitive_clip,
                        geometry: HitGeometry::Rect,
                        interaction: HitInteraction::SelectableText {
                            id: self.id,
                            frame,
                            padding,
                            interactions: self.interactions.clone(),
                            text_style: text.clone(),
                            text: text.content.resolve(),
                        },
                    });
                }
            }
            ResolvedWidgetKind::Image { image } => {
                let source = image.source.resolve();
                let loading_background = image
                    .background
                    .as_ref()
                    .map(|background| {
                        background.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Background,
                            context.now,
                        )
                    })
                    .unwrap_or(Color::rgba(255, 255, 255, 0));
                push_media_texture_or_placeholder(
                    self.id,
                    &source,
                    image.fit,
                    frame,
                    background_frame,
                    background_radius,
                    primitive_clip,
                    primitive_clip_mask,
                    opacity,
                    loading_background,
                    context,
                    computed,
                    "image",
                );
            }
            ResolvedWidgetKind::Canvas {
                items,
                item_interactions,
            } => {
                let padding = self
                    .layout
                    .padding
                    .as_ref()
                    .map(|padding| {
                        padding.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Padding,
                            context.now,
                        )
                    })
                    .unwrap_or(Insets::ZERO);
                let canvas_frame = background_frame.inset(padding);
                let canvas_clip = primitive_clip.and_then(|clip| clip.intersect(canvas_frame));
                let canvas_clip_mask = if background_radius > 0.0
                    && canvas_frame.width > Dp::ZERO
                    && canvas_frame.height > Dp::ZERO
                {
                    Some(ClipMask {
                        rect: canvas_frame,
                        corner_radius: background_radius,
                    })
                } else {
                    primitive_clip_mask
                };
                let canvas_origin = Point::new(canvas_frame.x, canvas_frame.y);

                if canvas_frame.width > Dp::ZERO && canvas_frame.height > Dp::ZERO {
                    for item in items {
                        let rendered = item.tessellate(
                            canvas_origin,
                            opacity,
                            CanvasClipContext {
                                clip_rect: canvas_clip,
                                clip_mask: canvas_clip_mask,
                            },
                            context.media,
                            context.units,
                        );
                        let meshes = rendered.meshes;
                        for texture in rendered.textures {
                            computed.scene.push_texture(texture);
                        }
                        for text in rendered.texts {
                            computed.scene.push_text(text);
                        }
                        for mesh in &meshes {
                            computed.scene.push_mesh(mesh.clone());
                        }

                        if item_interactions.has_any() {
                            if let Some(bounds) = item.hit_bounds() {
                                let triangles = meshes
                                    .iter()
                                    .flat_map(|mesh| mesh.triangles.iter().copied())
                                    .collect::<Vec<_>>();
                                let geometry = if triangles.is_empty() {
                                    HitGeometry::Rect
                                } else {
                                    HitGeometry::Triangles(Arc::from(triangles))
                                };
                                computed.hit_regions.push(HitRegion {
                                    rect: Rect::new(
                                        canvas_frame.x + bounds.min_x,
                                        canvas_frame.y + bounds.min_y,
                                        bounds.width(),
                                        bounds.height(),
                                    ),
                                    clip_rect: canvas_clip,
                                    geometry,
                                    interaction: HitInteraction::CanvasItem {
                                        id: self.id,
                                        item_id: item.id(),
                                        item_interactions: item_interactions.clone(),
                                        cursor_style: item.style().cursor,
                                        canvas_origin,
                                        item_origin: Point::new(
                                            canvas_frame.x + bounds.min_x,
                                            canvas_frame.y + bounds.min_y,
                                        ),
                                    },
                                });
                            }
                        }
                    }
                }
            }
            #[cfg(feature = "video")]
            ResolvedWidgetKind::VideoSurface { video } => {
                let loading_background = video
                    .background
                    .as_ref()
                    .map(|background| {
                        background.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Background,
                            context.now,
                        )
                    })
                    .unwrap_or(Color::rgba(255, 255, 255, 0));
                push_video_texture_or_placeholder(
                    self.id,
                    video,
                    frame,
                    background_frame,
                    background_radius,
                    primitive_clip,
                    opacity,
                    loading_background,
                    context,
                    computed,
                );
            }
            ResolvedWidgetKind::Button { label, style, .. } => {
                let button_style = style.clone();
                let padding = Insets::symmetric(button_style.padding_x, button_style.padding_y);
                let button_foreground = context.animations.resolve_color(
                    crate::animation::AnimationKey::Widget {
                        id: self.id.raw(),
                        property: WidgetProperty::TextColor,
                    },
                    resolve_stateful_widget_color(&button_style.foreground, widget_state),
                    Some(Transition::default()),
                    context.now,
                );
                let label_text = text_with_typography(label.clone(), &button_style.text_style);
                push_text_primitives(
                    &label_text,
                    frame,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    false,
                    true,
                    padding,
                    None,
                    None,
                    button_foreground,
                    opacity,
                    self.id,
                    primitive_clip,
                    primitive_clip_mask,
                );
            }
            ResolvedWidgetKind::Checkbox {
                checked,
                label,
                on_change,
                ..
            } => {
                let checkbox_style = checkbox_style
                    .as_ref()
                    .expect("checkbox style should be resolved for checkbox widgets");
                push_checkbox_primitives(
                    frame,
                    checked.resolve(),
                    label.as_ref(),
                    checkbox_style,
                    opacity,
                    self.id,
                    primitive_clip,
                    primitive_clip_mask,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                );
                if !disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        clip_rect: primitive_clip,
                        geometry: HitGeometry::Rect,
                        interaction: HitInteraction::Checkbox {
                            id: self.id,
                            interactions: self.interactions.clone(),
                            on_change: on_change.clone(),
                            current: checked.resolve(),
                        },
                    });
                }
            }
            ResolvedWidgetKind::Radio {
                checked,
                label,
                on_change,
                ..
            } => {
                let radio_style = radio_style
                    .as_ref()
                    .expect("radio style should be resolved for radio widgets");
                push_radio_primitives(
                    frame,
                    checked.resolve(),
                    label.as_ref(),
                    radio_style,
                    opacity,
                    self.id,
                    primitive_clip,
                    primitive_clip_mask,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                );
                if !disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        clip_rect: primitive_clip,
                        geometry: HitGeometry::Rect,
                        interaction: HitInteraction::Radio {
                            id: self.id,
                            interactions: self.interactions.clone(),
                            on_change: on_change.clone(),
                            current: checked.resolve(),
                        },
                    });
                }
            }
            ResolvedWidgetKind::Switch {
                checked,
                on_change,
                active_thumb_color,
                inactive_thumb_color,
                style,
                ..
            } => {
                let switch_style = style;
                let padding = self
                    .layout
                    .padding
                    .as_ref()
                    .map(|padding| {
                        padding.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Padding,
                            context.now,
                        )
                    })
                    .unwrap_or(switch_style.padding);
                push_switch_primitives(
                    background_frame,
                    background_radius,
                    padding,
                    checked.resolve(),
                    active_thumb_color.as_ref().map(Value::resolve).unwrap_or(
                        resolve_stateful_widget_color(&switch_style.thumb_checked, widget_state),
                    ),
                    inactive_thumb_color.as_ref().map(Value::resolve).unwrap_or(
                        resolve_stateful_widget_color(&switch_style.thumb, widget_state),
                    ),
                    opacity,
                    self.id,
                    primitive_clip,
                    primitive_clip_mask,
                    context.animations,
                    &mut computed.scene,
                    context.now,
                );
                if !disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        clip_rect: primitive_clip,
                        geometry: HitGeometry::Rect,
                        interaction: HitInteraction::Switch {
                            id: self.id,
                            interactions: self.interactions.clone(),
                            on_change: on_change.clone(),
                            current: checked.resolve(),
                        },
                    });
                }
            }
            ResolvedWidgetKind::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                style: _,
                ..
            } => {
                let active = open
                    .as_ref()
                    .map(Value::resolve)
                    .or_else(|| context.select_open_states.get(&self.id).copied())
                    .unwrap_or(false);
                let select_style = select_style
                    .as_ref()
                    .expect("select style should be resolved for select widgets");
                let padding = Insets::symmetric(select_style.padding_x, select_style.padding_y);
                push_select_primitives(
                    frame,
                    selected_label.resolve(),
                    placeholder,
                    select_style,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    padding,
                    opacity,
                    self.id,
                    primitive_clip,
                    primitive_clip_mask,
                );
                if active && !disabled {
                    push_select_menu_primitives(
                        self.id,
                        frame,
                        context.viewport,
                        options,
                        on_open_change.as_ref(),
                        select_style,
                        context,
                        computed,
                        opacity,
                    );
                }
                if !disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        clip_rect: primitive_clip,
                        geometry: HitGeometry::Rect,
                        interaction: HitInteraction::SelectTrigger {
                            id: self.id,
                            interactions: self.interactions.clone(),
                            on_open_change: on_open_change.clone(),
                            is_open: active,
                        },
                    });
                }
            }
        }
    }

    fn collect_media_event_states(
        &self,
        media: &MediaManager,
        states: &mut Vec<MediaEventState<VM>>,
    ) {
        match &self.kind {
            ResolvedWidgetKind::Container { children, .. } => {
                for child in children {
                    child.collect_media_event_states(media, states);
                }
            }
            ResolvedWidgetKind::Image { image } => {
                if !self.media_events.has_any() {
                    return;
                }
                let source = image.source.resolve();
                let snapshot = media.image_snapshot(&source, None);
                if let Some(phase) = media_event_phase(snapshot.loading, snapshot.error.as_deref())
                {
                    states.push(MediaEventState {
                        widget_id: self.id,
                        media_phase: Some(phase),
                        handlers: self.media_events.clone(),
                    });
                }
            }
            #[cfg(feature = "video")]
            ResolvedWidgetKind::VideoSurface { video } => {
                if !self.media_events.has_any() {
                    return;
                }
                let snapshot = video.controller.surface_snapshot();
                if let Some(phase) = media_event_phase(snapshot.loading, snapshot.error.as_deref())
                {
                    states.push(MediaEventState {
                        widget_id: self.id,
                        media_phase: Some(phase),
                        handlers: self.media_events.clone(),
                    });
                }
            }
            _ => {}
        }
    }
}

fn media_event_phase(loading: bool, error: Option<&str>) -> Option<MediaEventPhase> {
    if loading {
        Some(MediaEventPhase::Loading)
    } else if let Some(error) = error {
        Some(MediaEventPhase::Error(error.to_string()))
    } else {
        Some(MediaEventPhase::Success)
    }
}

fn apply_container_style(
    style: &mut TaffyStyle,
    layout: &ContainerLayout,
    animations: &mut AnimationEngine,
    widget_id: WidgetId,
    units: UnitContext,
    now: std::time::Instant,
) {
    style.padding = to_taffy_rect(
        layout
            .padding
            .as_ref()
            .map(|padding| {
                padding.resolve_widget(animations, widget_id, WidgetProperty::Padding, now)
            })
            .unwrap_or(Insets::ZERO),
        units,
    );
    let gap = layout
        .gap
        .resolve_widget(animations, widget_id, WidgetProperty::Gap, now);
    style.gap = TaffySize {
        width: resolve_length_percentage(&gap, units).unwrap_or(LengthPercentage::ZERO),
        height: resolve_length_percentage(&gap, units).unwrap_or(LengthPercentage::ZERO),
    };

    match &layout.kind {
        ContainerKind::Flow => {
            style.display = Display::Flex;
            style.flex_direction = FlexDirection::Column;
            style.justify_content = Some(map_justify_content(layout.justify));
            style.align_items = map_align_items(layout.align);
        }
        ContainerKind::Flex { direction, wrap } => {
            style.display = Display::Flex;
            style.flex_direction = match direction {
                Axis::Horizontal => FlexDirection::Row,
                Axis::Vertical => FlexDirection::Column,
            };
            style.flex_wrap = match wrap {
                Wrap::NoWrap => FlexWrap::NoWrap,
                Wrap::Wrap => FlexWrap::Wrap,
            };
            style.justify_content = Some(map_justify_content(layout.justify));
            style.align_items = map_align_items(layout.align);
            style.align_content = Some(map_align_content(layout.align));
        }
        ContainerKind::Grid { columns, rows } => {
            style.display = Display::Grid;
            style.grid_template_columns = if columns.is_empty() {
                vec![GridTemplateComponent::Single(TrackSizingFunction::AUTO)]
            } else {
                columns
                    .iter()
                    .copied()
                    .map(map_track)
                    .map(GridTemplateComponent::Single)
                    .collect()
            };
            style.grid_template_rows = if rows.is_empty() {
                vec![GridTemplateComponent::Single(TrackSizingFunction::AUTO)]
            } else {
                rows.iter()
                    .copied()
                    .map(map_track)
                    .map(GridTemplateComponent::Single)
                    .collect()
            };
            style.justify_content = Some(map_justify_content(layout.justify));
            style.align_content = Some(map_align_content(layout.align));
            style.justify_items = map_justify_items(layout.justify);
            style.align_items = map_align_items(layout.align);
        }
        ContainerKind::Stack => {
            style.display = Display::Grid;
            style.grid_template_columns =
                vec![GridTemplateComponent::Single(TrackSizingFunction::AUTO)];
            style.grid_template_rows =
                vec![GridTemplateComponent::Single(TrackSizingFunction::AUTO)];
            style.justify_items = map_justify_items(layout.justify);
            style.align_items = map_align_items(layout.align);
        }
    }
}

fn compute_container_content_bounds<VM>(
    element: &ResolvedElement<VM>,
    children: &[ResolvedElement<VM>],
    layout_node: &LayoutNode,
    frame: Rect,
    context: &mut CollectContext<'_, '_>,
) -> Rect {
    let padding = match &element.kind {
        ResolvedWidgetKind::Container { layout, .. } => layout
            .padding
            .as_ref()
            .map(|padding| {
                padding.resolve_widget(
                    context.animations,
                    element.id,
                    WidgetProperty::Padding,
                    context.now,
                )
            })
            .unwrap_or(Insets::ZERO),
        _ => Insets::ZERO,
    };
    let mut bounds: Option<Rect> = None;

    for (child, child_layout) in children.iter().zip(layout_node.children.iter()) {
        let child_layout = context
            .taffy
            .layout(child_layout.node)
            .expect("child layout node should exist");
        let offset = child.visual.offset.resolve_widget(
            context.animations,
            child.id,
            WidgetProperty::Offset,
            context.now,
        );
        let child_frame = Rect::new(
            frame.x + child_layout.location.x + offset.x,
            frame.y + child_layout.location.y + offset.y,
            child_layout.size.width,
            child_layout.size.height,
        );
        bounds = Some(match bounds {
            Some(existing) => existing.union(child_frame),
            None => child_frame,
        });
    }

    bounds
        .map(|bounds| {
            Rect::new(
                bounds.x,
                bounds.y,
                bounds.width + padding.right,
                bounds.height + padding.bottom,
            )
        })
        .unwrap_or(Rect::new(frame.x, frame.y, 0.0, 0.0))
}

fn apply_overflow_clip(
    parent_clip: Rect,
    frame: Rect,
    overflow_x: Overflow,
    overflow_y: Overflow,
) -> Rect {
    let x = if matches!(overflow_x, Overflow::Hidden | Overflow::Scroll) {
        parent_clip.x.max(frame.x)
    } else {
        parent_clip.x
    };
    let y = if matches!(overflow_y, Overflow::Hidden | Overflow::Scroll) {
        parent_clip.y.max(frame.y)
    } else {
        parent_clip.y
    };
    let right = if matches!(overflow_x, Overflow::Hidden | Overflow::Scroll) {
        parent_clip.right().min(frame.right())
    } else {
        parent_clip.right()
    };
    let bottom = if matches!(overflow_y, Overflow::Hidden | Overflow::Scroll) {
        parent_clip.bottom().min(frame.bottom())
    } else {
        parent_clip.bottom()
    };

    Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0))
}

fn apply_overflow_clip_mask(
    parent_clip_mask: Option<ClipMask>,
    frame: Rect,
    corner_radius: f32,
    overflow_x: Overflow,
    overflow_y: Overflow,
) -> Option<ClipMask> {
    if matches!(overflow_x, Overflow::Hidden | Overflow::Scroll)
        && matches!(overflow_y, Overflow::Hidden | Overflow::Scroll)
        && corner_radius > 0.0
        && frame.width > Dp::ZERO
        && frame.height > Dp::ZERO
    {
        return Some(ClipMask {
            rect: frame,
            corner_radius,
        });
    }

    parent_clip_mask
}

#[derive(Clone, Copy, Default)]
struct ScrollbarGeometry {
    horizontal_track: Option<Rect>,
    horizontal_thumb: Option<Rect>,
    vertical_track: Option<Rect>,
    vertical_thumb: Option<Rect>,
}

fn compute_scrollbar_geometry(
    viewport: Rect,
    content_bounds: Rect,
    scroll_offset: Point,
    layout: &ContainerLayout,
    theme: &Theme,
    units: UnitContext,
) -> ScrollbarGeometry {
    let can_scroll_x =
        layout.overflow_x == Overflow::Scroll && content_bounds.right() > viewport.right();
    let can_scroll_y =
        layout.overflow_y == Overflow::Scroll && content_bounds.bottom() > viewport.bottom();
    if !can_scroll_x && !can_scroll_y {
        return ScrollbarGeometry::default();
    }

    let defaults = resolved_container_style(None, theme).scrollbar;
    let style = layout.scrollbar_style;
    let thickness = units.resolve_dp(
        style
            .thickness
            .or(defaults.thickness)
            .unwrap_or(dp(5.0))
            .max(dp(2.0)),
    );
    let inset_bounds = viewport.inset(style.insets.unwrap_or(Insets::ZERO));
    if inset_bounds.is_empty() {
        return ScrollbarGeometry::default();
    }

    let vertical_track = can_scroll_y.then(|| {
        Rect::new(
            (inset_bounds.right() - thickness).max(inset_bounds.x),
            inset_bounds.y,
            Dp::new(thickness).min(inset_bounds.width),
            (inset_bounds.height - if can_scroll_x { thickness } else { 0.0 }).max(0.0),
        )
    });
    let horizontal_track = can_scroll_x.then(|| {
        Rect::new(
            inset_bounds.x,
            (inset_bounds.bottom() - thickness).max(inset_bounds.y),
            (inset_bounds.width - if can_scroll_y { thickness } else { 0.0 }).max(0.0),
            Dp::new(thickness).min(inset_bounds.height),
        )
    });

    ScrollbarGeometry {
        horizontal_thumb: horizontal_track
            .filter(|track| !track.is_empty())
            .map(|track| {
                scrollbar_thumb_rect(
                    track,
                    viewport.width.get(),
                    scroll_offset.x.get(),
                    (content_bounds.right() - viewport.x)
                        .max(viewport.width)
                        .get(),
                    units.resolve_dp(
                        style
                            .min_thumb_length
                            .or(defaults.min_thumb_length)
                            .unwrap_or(dp(12.0))
                            .max(Dp::new(thickness)),
                    ),
                    Axis::Horizontal,
                )
            }),
        vertical_thumb: vertical_track
            .filter(|track| !track.is_empty())
            .map(|track| {
                scrollbar_thumb_rect(
                    track,
                    viewport.height.get(),
                    scroll_offset.y.get(),
                    (content_bounds.bottom() - viewport.y)
                        .max(viewport.height)
                        .get(),
                    units.resolve_dp(
                        style
                            .min_thumb_length
                            .or(defaults.min_thumb_length)
                            .unwrap_or(dp(12.0))
                            .max(Dp::new(thickness)),
                    ),
                    Axis::Vertical,
                )
            }),
        horizontal_track: horizontal_track.filter(|track| !track.is_empty()),
        vertical_track: vertical_track.filter(|track| !track.is_empty()),
    }
}

fn push_scrollbar_primitives(
    scene: &mut ScenePrimitives,
    theme: &Theme,
    clip_rect: Rect,
    opacity: f32,
    layout: &ContainerLayout,
    geometry: ScrollbarGeometry,
    widget_id: WidgetId,
    hovered_scrollbar: Option<ScrollbarHandle>,
    active_scrollbar: Option<ScrollbarHandle>,
) {
    if geometry.horizontal_track.is_none() && geometry.vertical_track.is_none() {
        return;
    }

    let track_clip = Some(clip_rect);
    let defaults = resolved_container_style(None, theme).scrollbar;
    let style = layout.scrollbar_style;
    let track_color = style
        .track_color
        .or(defaults.track_color)
        .unwrap_or(Color::TRANSPARENT)
        .with_alpha_factor(opacity);
    let thumb_color_for = |axis| {
        let handle = ScrollbarHandle {
            id: widget_id,
            axis,
        };
        let mut state = crate::ui::theme::WidgetState::default();
        if active_scrollbar == Some(handle) {
            state.pressed = true;
        } else if hovered_scrollbar == Some(handle) {
            state.hovered = true;
        }
        if state.pressed {
            style
                .active_thumb_color
                .or(style.thumb_color)
                .or(defaults.active_thumb_color)
                .or(defaults.thumb_color)
                .unwrap_or(Color::TRANSPARENT)
                .with_alpha_factor(opacity)
        } else if state.hovered {
            style
                .hover_thumb_color
                .or(style.thumb_color)
                .or(defaults.hover_thumb_color)
                .or(defaults.thumb_color)
                .unwrap_or(Color::TRANSPARENT)
                .with_alpha_factor(opacity)
        } else {
            style
                .thumb_color
                .or(defaults.thumb_color)
                .unwrap_or(Color::TRANSPARENT)
                .with_alpha_factor(opacity)
        }
    };
    let thickness = style
        .thickness
        .or(defaults.thickness)
        .unwrap_or(dp(12.0))
        .max(dp(2.0))
        .get();
    let radius = style
        .radius
        .or(defaults.radius)
        .unwrap_or(dp(999.0))
        .max(Dp::ZERO)
        .min(Dp::new(thickness * 0.5))
        .get();

    if let Some(track) = geometry.vertical_track {
        scene.push_overlay_shape(RenderPrimitive {
            rect: track,
            color: track_color,
            corner_radius: radius,
            stroke_width: 0.0,
            clip_rect: track_clip,
            clip_mask: None,
        });
        let thumb = geometry
            .vertical_thumb
            .expect("vertical thumb should exist with vertical track");
        scene.push_overlay_shape(RenderPrimitive {
            rect: thumb,
            color: thumb_color_for(ScrollbarAxis::Vertical),
            corner_radius: radius,
            stroke_width: 0.0,
            clip_rect: track_clip,
            clip_mask: None,
        });
    }

    if let Some(track) = geometry.horizontal_track {
        scene.push_overlay_shape(RenderPrimitive {
            rect: track,
            color: track_color,
            corner_radius: radius,
            stroke_width: 0.0,
            clip_rect: track_clip,
            clip_mask: None,
        });
        let thumb = geometry
            .horizontal_thumb
            .expect("horizontal thumb should exist with horizontal track");
        scene.push_overlay_shape(RenderPrimitive {
            rect: thumb,
            color: thumb_color_for(ScrollbarAxis::Horizontal),
            corner_radius: radius,
            stroke_width: 0.0,
            clip_rect: track_clip,
            clip_mask: None,
        });
    }
}

fn scrollbar_thumb_rect(
    track: Rect,
    viewport_extent: f32,
    scroll_offset: f32,
    content_extent: f32,
    min_thumb_length: f32,
    axis: Axis,
) -> Rect {
    let track_extent = match axis {
        Axis::Horizontal => track.width,
        Axis::Vertical => track.height,
    }
    .max(0.0)
    .get();
    let max_offset = (content_extent - viewport_extent).max(0.0);
    let mut thumb_extent = if content_extent <= 0.0 {
        track_extent
    } else {
        track_extent * (viewport_extent / content_extent)
    };
    thumb_extent = thumb_extent.clamp(min_thumb_length.min(track_extent), track_extent);
    let travel = (track_extent - thumb_extent).max(0.0);
    let thumb_offset = if max_offset <= 0.0 || travel <= 0.0 {
        0.0
    } else {
        (scroll_offset.clamp(0.0, max_offset) / max_offset) * travel
    };

    match axis {
        Axis::Horizontal => Rect::new(track.x + thumb_offset, track.y, thumb_extent, track.height),
        Axis::Vertical => Rect::new(track.x, track.y + thumb_offset, track.width, thumb_extent),
    }
}

fn map_align_items(align: Align) -> Option<TaffyAlignItems> {
    Some(match align {
        Align::Start => TaffyAlignItems::Start,
        Align::Center => TaffyAlignItems::Center,
        Align::End => TaffyAlignItems::End,
        Align::Stretch => TaffyAlignItems::Stretch,
    })
}

fn map_align_self(align: Align) -> TaffyAlignItems {
    match align {
        Align::Start => TaffyAlignItems::Start,
        Align::Center => TaffyAlignItems::Center,
        Align::End => TaffyAlignItems::End,
        Align::Stretch => TaffyAlignItems::Stretch,
    }
}

fn map_justify_content(justify: Justify) -> TaffyJustifyContent {
    match justify {
        Justify::Start => TaffyJustifyContent::Start,
        Justify::Center => TaffyJustifyContent::Center,
        Justify::End => TaffyJustifyContent::End,
        Justify::SpaceBetween => TaffyJustifyContent::SpaceBetween,
        Justify::SpaceAround => TaffyJustifyContent::SpaceAround,
        Justify::SpaceEvenly => TaffyJustifyContent::SpaceEvenly,
    }
}

fn map_align_content(align: Align) -> TaffyAlignContent {
    match align {
        Align::Start => TaffyAlignContent::Start,
        Align::Center => TaffyAlignContent::Center,
        Align::End => TaffyAlignContent::End,
        Align::Stretch => TaffyAlignContent::Stretch,
    }
}

fn map_justify_items(justify: Justify) -> Option<TaffyAlignItems> {
    match justify {
        Justify::Start => Some(TaffyAlignItems::Start),
        Justify::Center => Some(TaffyAlignItems::Center),
        Justify::End => Some(TaffyAlignItems::End),
        Justify::SpaceBetween | Justify::SpaceAround | Justify::SpaceEvenly => None,
    }
}

fn map_track(track: Track) -> TrackSizingFunction {
    match track {
        Track::Auto => TrackSizingFunction::AUTO,
        Track::Px(value) => TrackSizingFunction::from_length(value.get()),
        Track::Percent(value) => TrackSizingFunction::from_percent(value),
        Track::Fr(value) => TrackSizingFunction::from_fr(value),
    }
}

fn resolve_dimension(
    value: &Value<Length>,
    animations: &mut AnimationEngine,
    widget_id: WidgetId,
    property: WidgetProperty,
    now: std::time::Instant,
    units: UnitContext,
) -> Dimension {
    match value.resolve_widget(animations, widget_id, property, now) {
        Length::Auto => Dimension::AUTO,
        Length::Px(value) => Dimension::from_length(units.resolve_dp(value)),
        Length::Percent(value) => Dimension::from_percent(value),
    }
}

fn resolve_length_percentage(value: &Length, units: UnitContext) -> Option<LengthPercentage> {
    match value {
        Length::Auto => None,
        Length::Px(value) => Some(LengthPercentage::from_length(units.resolve_dp(*value))),
        Length::Percent(value) => Some(LengthPercentage::from_percent(*value)),
    }
}

fn resolve_length_percentage_auto(
    value: &Value<Length>,
    animations: &mut AnimationEngine,
    widget_id: WidgetId,
    property: WidgetProperty,
    now: std::time::Instant,
    units: UnitContext,
) -> LengthPercentageAuto {
    match value.resolve_widget(animations, widget_id, property, now) {
        Length::Auto => LengthPercentageAuto::AUTO,
        Length::Px(value) => LengthPercentageAuto::from_length(units.resolve_dp(value)),
        Length::Percent(value) => LengthPercentageAuto::from_percent(value),
    }
}

fn to_taffy_rect(
    insets: Insets,
    units: UnitContext,
) -> taffy::prelude::Rect<taffy::style::LengthPercentage> {
    taffy::prelude::Rect {
        left: length(units.resolve_dp(insets.left)),
        right: length(units.resolve_dp(insets.right)),
        top: length(units.resolve_dp(insets.top)),
        bottom: length(units.resolve_dp(insets.bottom)),
    }
}

fn to_taffy_rect_auto(
    insets: Insets,
    units: UnitContext,
) -> taffy::prelude::Rect<taffy::style::LengthPercentageAuto> {
    taffy::prelude::Rect {
        left: length(units.resolve_dp(insets.left)),
        right: length(units.resolve_dp(insets.right)),
        top: length(units.resolve_dp(insets.top)),
        bottom: length(units.resolve_dp(insets.bottom)),
    }
}

fn measure_node(
    node_context: Option<&mut MeasureContext>,
    known_dimensions: TaffySize<Option<f32>>,
    font_manager: &FontManager,
    theme: &Theme,
    media: &MediaManager,
    units: UnitContext,
) -> TaffySize<f32> {
    let measured = match node_context {
        Some(MeasureContext::Text(text)) => measure_text_content(text, font_manager, theme, units),
        Some(MeasureContext::Image(image)) => {
            let snapshot = media.image_snapshot(&image.source.resolve(), None);
            measure_media_content(
                known_dimensions,
                image.layout.aspect_ratio.as_ref().map(Value::resolve),
                snapshot.intrinsic_size,
            )
        }
        Some(MeasureContext::Canvas(items)) => canvas_bounds(items)
            .map(|bounds| (bounds.width(), bounds.height()))
            .unwrap_or((0.0, 0.0)),
        #[cfg(feature = "video")]
        Some(MeasureContext::VideoSurface(video)) => {
            let snapshot = video.controller.surface_snapshot();
            measure_media_content(
                known_dimensions,
                video.layout.aspect_ratio.as_ref().map(Value::resolve),
                snapshot.intrinsic_size,
            )
        }
        Some(MeasureContext::Button { label, style }) => {
            let button_style = resolve_button_style(style, Default::default(), theme);
            let label_text = text_with_typography(label.clone(), &style.text_style);
            let text_size = measure_text_content(&label_text, font_manager, theme, units);
            let horizontal = units.resolve_dp(button_style.padding_x) * 2.0;
            let vertical = units.resolve_dp(button_style.padding_y) * 2.0;
            (
                text_size.0 + horizontal,
                text_size
                    .1
                    .max(units.resolve_dp(button_style.min_height))
                    .max(text_size.1 + vertical),
            )
        }
        Some(MeasureContext::Switch { style }) => {
            let switch_style = style;
            (
                units.resolve_dp(switch_style.width),
                units.resolve_dp(switch_style.height),
            )
        }
        Some(MeasureContext::Checkbox { label, style }) => {
            let checkbox_style = resolve_checkbox_style(style, Default::default(), false, theme);
            measure_checkbox_content(label.as_ref(), &checkbox_style, font_manager, theme, units)
        }
        Some(MeasureContext::Radio { label, style }) => {
            let radio_style = resolve_radio_style(style, Default::default(), false, theme);
            measure_radio_content(label.as_ref(), &radio_style, font_manager, theme, units)
        }
        Some(MeasureContext::Select {
            selected_label,
            placeholder,
            style,
        }) => measure_select_content(
            selected_label.as_deref(),
            placeholder,
            &resolve_select_style(style, Default::default(), theme),
            font_manager,
            theme,
            units,
        ),
        Some(MeasureContext::None) | None => (0.0, 0.0),
    };

    TaffySize {
        width: known_dimensions.width.unwrap_or(measured.0),
        height: known_dimensions.height.unwrap_or(measured.1),
    }
}

fn measure_text_content(
    text: &Text,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32) {
    let default_style = &theme.typography.body;
    let (font_size, line_height, letter_spacing) = resolved_text_metrics(text, theme, units);
    font_manager.measure_text(
        &text.content.resolve(),
        TextFontRequest {
            preferred_font: text
                .font_family
                .as_deref()
                .or(default_style.font_family.as_deref()),
            weight: text.font_weight.unwrap_or(default_style.weight),
        },
        font_size,
        line_height,
        letter_spacing,
    )
}

fn text_from_content(content: impl Into<Value<String>>) -> Text {
    Text::new(content)
}

fn measure_checkbox_content(
    label: Option<&Value<String>>,
    checkbox_style: &ResolvedCheckboxStyle,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32) {
    let size = units.resolve_dp(checkbox_style.size);
    let Some(label) = label else {
        return (size, size);
    };

    let label = checkbox_label_with_theme(label, checkbox_style);
    let label_size = measure_text_content(&label, font_manager, theme, units);
    (
        size + units.resolve_dp(checkbox_style.label_gap) + label_size.0,
        size.max(label_size.1),
    )
}

fn checkbox_label_with_theme(
    label: &Value<String>,
    checkbox_style: &ResolvedCheckboxStyle,
) -> Text {
    let mut label = text_from_content(label.clone());
    if label.font_family.is_none() {
        label.font_family = checkbox_style.text_style.font_family.clone();
    }
    if label.font_size.is_none() {
        label.font_size = Some(checkbox_style.text_style.size);
    }
    if label.line_height.is_none() {
        label.line_height = checkbox_style.text_style.line_height;
    }
    if label.font_weight.is_none() {
        label.font_weight = Some(checkbox_style.text_style.weight);
    }
    if label.letter_spacing.is_none() {
        label.letter_spacing = checkbox_style.text_style.letter_spacing;
    }
    label
}

fn measure_radio_content(
    label: Option<&Value<String>>,
    radio_style: &ResolvedRadioStyle,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32) {
    let size = units.resolve_dp(radio_style.size);
    let Some(label) = label else {
        return (size, size);
    };

    let label = radio_label_with_theme(label, radio_style);
    let label_size = measure_text_content(&label, font_manager, theme, units);
    (
        size + units.resolve_dp(radio_style.label_gap) + label_size.0,
        size.max(label_size.1),
    )
}

fn radio_label_with_theme(label: &Value<String>, radio_style: &ResolvedRadioStyle) -> Text {
    let mut label = text_from_content(label.clone());
    if label.font_family.is_none() {
        label.font_family = radio_style.text_style.font_family.clone();
    }
    if label.font_size.is_none() {
        label.font_size = Some(radio_style.text_style.size);
    }
    if label.line_height.is_none() {
        label.line_height = radio_style.text_style.line_height;
    }
    if label.font_weight.is_none() {
        label.font_weight = Some(radio_style.text_style.weight);
    }
    if label.letter_spacing.is_none() {
        label.letter_spacing = radio_style.text_style.letter_spacing;
    }
    label
}

fn resolved_button_style(
    style: Option<&super::style::StyleResolver<WidgetButtonStyle>>,
    theme: &Theme,
    variant: ButtonVariantKind,
) -> WidgetButtonStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| WidgetButtonStyle::default_for(infer_theme_mode(theme), variant))
}

fn resolved_checkbox_style(
    style: Option<&super::style::StyleResolver<WidgetCheckboxStyle>>,
    theme: &Theme,
) -> WidgetCheckboxStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| WidgetCheckboxStyle::default_for(infer_theme_mode(theme)))
}

fn resolved_radio_style(
    style: Option<&super::style::StyleResolver<WidgetRadioStyle>>,
    theme: &Theme,
) -> WidgetRadioStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| WidgetRadioStyle::default_for(infer_theme_mode(theme)))
}

fn resolved_switch_style(
    style: Option<&super::style::StyleResolver<WidgetSwitchStyle>>,
    theme: &Theme,
) -> WidgetSwitchStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| WidgetSwitchStyle::default_for(infer_theme_mode(theme)))
}

fn resolved_select_style(
    style: Option<&super::style::StyleResolver<WidgetSelectStyle>>,
    theme: &Theme,
) -> WidgetSelectStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| WidgetSelectStyle::default_for(infer_theme_mode(theme)))
}

fn resolved_container_style(
    style: Option<&super::style::StyleResolver<super::style::ContainerStyle>>,
    theme: &Theme,
) -> super::style::ContainerStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| super::style::ContainerStyle::default_for(infer_theme_mode(theme)))
}

fn resolved_image_style(
    style: Option<&super::style::StyleResolver<super::style::ImageStyle>>,
    theme: &Theme,
) -> super::style::ImageStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| super::style::ImageStyle::default_for(infer_theme_mode(theme)))
}

fn resolved_canvas_style(
    style: Option<&super::style::StyleResolver<super::style::CanvasStyle>>,
    theme: &Theme,
) -> super::style::CanvasStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| super::style::CanvasStyle::default_for(infer_theme_mode(theme)))
}

fn resolved_text_widget_style(
    style: Option<&super::style::StyleResolver<TextWidgetStyle>>,
    theme: &Theme,
) -> TextWidgetStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| TextWidgetStyle::default_for(infer_theme_mode(theme)))
}

#[cfg(feature = "video")]
fn resolved_video_surface_style(
    style: Option<&super::style::StyleResolver<WidgetVideoSurfaceStyle>>,
    theme: &Theme,
) -> WidgetVideoSurfaceStyle {
    style
        .map(|resolver| resolver.resolve(infer_theme_mode(theme)))
        .unwrap_or_else(|| WidgetVideoSurfaceStyle::default_for(infer_theme_mode(theme)))
}

fn apply_surface_style(
    background: &mut Option<Value<Color>>,
    visual: &mut VisualStyle,
    surface: &super::style::WidgetSurfaceStyle,
) {
    *background = surface.background.clone();
    visual.background_brush = surface.background_brush.clone();
    visual.background_image = surface.background_image.clone();
    visual.background_blur = surface.background_blur.clone();
    visual.border_color = surface.border_color.clone();
    visual.border_radius = surface.border_radius.clone();
    visual.border_width = surface.border_width.clone();
    visual.opacity = surface.opacity.clone();
    visual.offset = surface.offset.clone();
}

fn apply_text_widget_style(text: &mut Text, style: &TextWidgetStyle) {
    text.background = style.surface.background.clone();
    text.color = Some(style.color.clone());
    text.font_family = style.typography.font_family.clone();
    text.font_size = Some(style.typography.size);
    text.line_height = style.typography.line_height;
    text.font_weight = Some(style.typography.weight);
    text.letter_spacing = style.typography.letter_spacing;
}

fn resolve_stateful_widget_color(
    value: &crate::ui::theme::Stateful<Value<Color>>,
    state: WidgetState,
) -> Color {
    value.resolve(state).resolve()
}

fn base_interaction_state(mut state: WidgetState) -> WidgetState {
    state.focused = false;
    state
}

fn resolve_focus_ring(
    theme: &Theme,
    override_style: Option<&FocusRingOverride>,
    state: WidgetState,
) -> Option<crate::theme::FocusRingStyle> {
    if state.disabled || !state.focused {
        return None;
    }

    let resolved = override_style
        .map(|style| style.resolve(theme))
        .unwrap_or_else(|| theme.focus_ring.clone());
    if !resolved.enabled || resolved.width <= Dp::ZERO {
        return None;
    }
    Some(resolved)
}

fn resolve_button_style(
    style: &WidgetButtonStyle,
    state: WidgetState,
    theme: &Theme,
) -> ResolvedButtonStyle {
    let visual_state = base_interaction_state(state);
    ResolvedButtonStyle {
        background: resolve_stateful_widget_color(&style.background, visual_state),
        border_color: resolve_stateful_widget_color(&style.border, visual_state),
        focus_ring: resolve_focus_ring(theme, style.focus_ring.as_ref(), state),
        border_width: style.border_width.resolve(),
        radius: style.radius.resolve(),
        padding_x: style.padding_x,
        padding_y: style.padding_y,
        min_height: style.min_height,
    }
}

fn resolve_checkbox_style(
    style: &WidgetCheckboxStyle,
    state: WidgetState,
    checked: bool,
    theme: &Theme,
) -> ResolvedCheckboxStyle {
    let mut control_state = base_interaction_state(state);
    control_state.selected = checked;
    ResolvedCheckboxStyle {
        background: if checked {
            resolve_stateful_widget_color(&style.background_checked, control_state)
        } else {
            resolve_stateful_widget_color(&style.background, control_state)
        },
        border: if checked {
            resolve_stateful_widget_color(&style.border_checked, control_state)
        } else {
            resolve_stateful_widget_color(&style.border, control_state)
        },
        focus_ring: resolve_focus_ring(theme, style.focus_ring.as_ref(), state),
        checkmark: resolve_stateful_widget_color(&style.checkmark, control_state),
        label: resolve_stateful_widget_color(&style.label, control_state),
        border_width: style.border_width.resolve(),
        radius: style.radius.resolve(),
        size: style.size,
        label_gap: style.label_gap,
        text_style: style.text_style.clone(),
    }
}

fn resolve_radio_style(
    style: &WidgetRadioStyle,
    state: WidgetState,
    checked: bool,
    theme: &Theme,
) -> ResolvedRadioStyle {
    let mut control_state = base_interaction_state(state);
    control_state.selected = checked;
    ResolvedRadioStyle {
        background: if checked {
            resolve_stateful_widget_color(&style.background_checked, control_state)
        } else {
            resolve_stateful_widget_color(&style.background, control_state)
        },
        border: if checked {
            resolve_stateful_widget_color(&style.border_checked, control_state)
        } else {
            resolve_stateful_widget_color(&style.border, control_state)
        },
        focus_ring: resolve_focus_ring(theme, style.focus_ring.as_ref(), state),
        indicator: resolve_stateful_widget_color(&style.indicator, control_state),
        label: resolve_stateful_widget_color(&style.label, control_state),
        border_width: style.border_width.resolve(),
        radius: style.radius.resolve(),
        size: style.size,
        label_gap: style.label_gap,
        text_style: style.text_style.clone(),
    }
}

fn resolve_select_style(
    style: &WidgetSelectStyle,
    state: WidgetState,
    theme: &Theme,
) -> ResolvedSelectStyle {
    let visual_state = base_interaction_state(state);
    ResolvedSelectStyle {
        background: resolve_stateful_widget_color(&style.background, visual_state),
        text: resolve_stateful_widget_color(&style.text, visual_state),
        placeholder: resolve_stateful_widget_color(&style.placeholder, visual_state),
        border: resolve_stateful_widget_color(&style.border, visual_state),
        focus_ring: resolve_focus_ring(theme, style.focus_ring.as_ref(), state),
        arrow: resolve_stateful_widget_color(&style.arrow, visual_state),
        menu_background: style.menu_background.resolve(),
        selected_option_background: style.selected_option_background.resolve(),
        border_width: style.border_width.resolve(),
        radius: style.radius.resolve(),
        padding_x: style.padding_x,
        padding_y: style.padding_y,
        min_height: style.min_height,
        option_height: style.option_height,
        menu_gap: style.menu_gap,
        text_style: style.text_style.clone(),
    }
}

fn default_select_menu_option_color(theme: &Theme, state: WidgetState) -> Color {
    let style = WidgetSelectStyle::default_for(infer_theme_mode(theme));
    resolve_stateful_widget_color(&style.option_background, base_interaction_state(state))
}

fn default_select_disabled_text_color(theme: &Theme) -> Color {
    let style = WidgetSelectStyle::default_for(infer_theme_mode(theme));
    let mut state = WidgetState::default();
    state.disabled = true;
    resolve_stateful_widget_color(&style.text, state)
}

fn default_layout_padding<VM>(element: &ResolvedElement<VM>, _theme: &Theme) -> Insets {
    match &element.kind {
        ResolvedWidgetKind::Button { style, .. } => {
            Insets::symmetric(style.padding_x, style.padding_y)
        }
        ResolvedWidgetKind::Select { style, .. } => {
            Insets::symmetric(style.padding_x, style.padding_y)
        }
        ResolvedWidgetKind::Switch { style, .. } => style.padding,
        ResolvedWidgetKind::Checkbox { .. } => Insets::ZERO,
        ResolvedWidgetKind::Radio { .. } => Insets::ZERO,
        ResolvedWidgetKind::Text { .. } => Insets::ZERO,
        ResolvedWidgetKind::Container { .. } => Insets::ZERO,
        ResolvedWidgetKind::Image { .. } => Insets::ZERO,
        ResolvedWidgetKind::Canvas { .. } => Insets::ZERO,
        #[cfg(feature = "video")]
        ResolvedWidgetKind::VideoSurface { .. } => Insets::ZERO,
    }
}

fn resolved_text_metrics(text: &Text, theme: &Theme, units: UnitContext) -> (f32, f32, f32) {
    let default_style = &theme.typography.body;
    let default_size = default_style.size.max(sp(1.0));
    let default_line_height_sp = text
        .line_height
        .or(default_style.line_height)
        .unwrap_or(text.font_size.unwrap_or(default_style.size) * 1.25);
    let font_size = units.resolve_sp(text.font_size.unwrap_or(default_size));
    let default_line_height = units.resolve_sp(default_line_height_sp);
    let default_font_size = units.resolve_sp(default_size);
    let scaled_line_height = if default_font_size > 0.0 {
        default_line_height * (font_size / default_font_size)
    } else {
        default_line_height
    };
    let line_height = default_line_height
        .max(scaled_line_height)
        .max(font_size + 4.0);
    let letter_spacing = units.resolve_sp(
        text.letter_spacing
            .unwrap_or(default_style.letter_spacing.unwrap_or(Sp::ZERO)),
    );
    (font_size, line_height, letter_spacing)
}

fn text_with_typography(
    content: impl Into<Value<String>>,
    style: &crate::ui::theme::TextStyle,
) -> Text {
    let mut text = text_from_content(content);
    text.font_family = style.font_family.clone();
    text.font_size = Some(style.size);
    text.line_height = style.line_height;
    text.font_weight = Some(style.weight);
    text.letter_spacing = style.letter_spacing;
    text
}

fn measure_media_content(
    known_dimensions: TaffySize<Option<f32>>,
    aspect_ratio: Option<f32>,
    intrinsic_size: IntrinsicSize,
) -> (f32, f32) {
    let ratio = aspect_ratio
        .filter(|ratio| ratio.is_finite() && *ratio > 0.0)
        .or_else(|| intrinsic_size.aspect_ratio());

    match (known_dimensions.width, known_dimensions.height, ratio) {
        (Some(width), Some(height), _) => (width, height),
        (Some(width), None, Some(ratio)) => (width, width / ratio),
        (None, Some(height), Some(ratio)) => (height * ratio, height),
        (Some(width), None, None) => (width, intrinsic_size.height),
        (None, Some(height), None) => (intrinsic_size.width, height),
        (None, None, _) => (intrinsic_size.width, intrinsic_size.height),
    }
}

fn push_media_texture_or_placeholder<VM>(
    widget_id: WidgetId,
    source: &crate::media::MediaSource,
    fit: ContentFit,
    frame: Rect,
    content_frame: Rect,
    content_corner_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    opacity: f32,
    loading_background: Color,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
    kind: &str,
) {
    let metadata = context.media.image_snapshot(source, None);
    let target_frame = resolve_media_rect(content_frame, metadata.intrinsic_size, fit);
    let snapshot = if let Some(raster_request) =
        RasterRequest::from_frame(target_frame, context.units.scale_factor())
    {
        context.media.image_snapshot(source, Some(raster_request))
    } else {
        metadata
    };

    if let Some(texture) = snapshot.texture.as_ref() {
        computed.scene.push_texture(TexturePrimitive {
            texture: Arc::clone(texture),
            frame: target_frame,
            corner_radius: content_corner_radius,
            clip_rect,
            clip_mask,
        });
        return;
    }

    push_media_placeholder(
        frame,
        content_frame,
        content_corner_radius,
        clip_rect,
        clip_mask,
        opacity,
        context,
        &mut computed.scene,
        widget_id,
        kind,
        snapshot.loading,
        snapshot.error.as_deref(),
        loading_background,
        snapshot.loading,
    );
}

fn push_background_media_texture<VM>(
    source: &crate::media::MediaSource,
    fit: ContentFit,
    content_frame: Rect,
    content_corner_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
) {
    let metadata = context.media.image_snapshot(source, None);
    let target_frame = resolve_media_rect(content_frame, metadata.intrinsic_size, fit);
    let snapshot = if let Some(raster_request) =
        RasterRequest::from_frame(target_frame, context.units.scale_factor())
    {
        context.media.image_snapshot(source, Some(raster_request))
    } else {
        metadata
    };

    if let Some(texture) = snapshot.texture.as_ref() {
        computed.scene.push_texture(TexturePrimitive {
            texture: Arc::clone(texture),
            frame: target_frame,
            corner_radius: content_corner_radius,
            clip_rect,
            clip_mask,
        });
    }
}

#[cfg(feature = "video")]
fn push_video_texture_or_placeholder<VM>(
    widget_id: WidgetId,
    video: &PublicVideoSurface,
    frame: Rect,
    content_frame: Rect,
    content_corner_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    opacity: f32,
    loading_background: Color,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
) {
    let snapshot = video.controller.surface_snapshot();
    let target_frame = resolve_media_rect(content_frame, snapshot.intrinsic_size, video.fit);
    let use_surface_background =
        snapshot.loading || (snapshot.texture.is_none() && snapshot.error.is_none());

    if let Some(texture) = snapshot.texture.as_ref() {
        computed.scene.push_texture(TexturePrimitive {
            texture: Arc::clone(texture),
            frame: target_frame,
            corner_radius: content_corner_radius,
            clip_rect,
            clip_mask,
        });
        return;
    }

    push_media_placeholder(
        frame,
        content_frame,
        content_corner_radius,
        clip_rect,
        clip_mask,
        opacity,
        context,
        &mut computed.scene,
        widget_id,
        "video",
        snapshot.loading,
        snapshot.error.as_deref(),
        loading_background,
        use_surface_background,
    );
}

fn push_media_placeholder(
    frame: Rect,
    content_frame: Rect,
    content_corner_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    opacity: f32,
    context: &mut CollectContext<'_, '_>,
    scene: &mut ScenePrimitives,
    widget_id: WidgetId,
    kind: &str,
    loading: bool,
    error: Option<&str>,
    loading_background: Color,
    use_loading_background: bool,
) {
    let placeholder =
        media_loading_fill_color(loading, error, loading_background, use_loading_background)
            .with_alpha_factor(opacity);
    if content_frame.width > Dp::ZERO && content_frame.height > Dp::ZERO {
        scene.push_shape(RenderPrimitive {
            rect: content_frame,
            color: placeholder,
            corner_radius: content_corner_radius,
            stroke_width: 0.0,
            clip_rect,
            clip_mask,
        });
    }

    let label = media_placeholder_label(kind, loading, error);
    let mut text = Text::new(label);
    text.font_size = Some((context.theme.typography.body_small.size - sp(1.0)).max(sp(12.0)));
    push_text_primitives(
        &text,
        frame,
        context.font_manager,
        context.theme,
        context.units,
        context.animations,
        context.now,
        scene,
        false,
        true,
        Insets::all(dp(12.0)),
        None,
        None,
        Color::hexa(0xE5E7EBFF),
        opacity,
        widget_id,
        clip_rect,
        clip_mask,
    );
}

fn media_loading_fill_color(
    loading: bool,
    error: Option<&str>,
    loading_background: Color,
    use_loading_background: bool,
) -> Color {
    if use_loading_background {
        loading_background
    } else {
        media_placeholder_color(loading, error)
    }
}

fn push_text_primitives(
    text: &Text,
    frame: Rect,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    show_caret: bool,
    center_horizontally: bool,
    padding: Insets,
    caret_content: Option<&str>,
    selection_state: Option<&TextEditState>,
    fallback_color: Color,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    let content = text.content.resolve();
    let default_style = &theme.typography.body;
    let text_request = TextFontRequest {
        preferred_font: text
            .font_family
            .as_deref()
            .or(default_style.font_family.as_deref()),
        weight: text.font_weight.unwrap_or(default_style.weight),
    };
    let resolved = font_manager.resolve_text(&content, text_request.clone());

    let color = text
        .color
        .as_ref()
        .map(|color| color.resolve_widget(animations, widget_id, WidgetProperty::TextColor, now))
        .unwrap_or(fallback_color);
    let (font_size, line_height, letter_spacing) = resolved_text_metrics(text, theme, units);
    let inner = frame.inset(padding);
    let current_layout = font_manager.measure_text_layout(
        &content,
        text_request.clone(),
        font_size,
        line_height,
        letter_spacing,
    );
    let content_frame = centered_text_frame(
        inner,
        current_layout.width,
        current_layout.height,
        line_height,
        center_horizontally,
    );

    if let Some((selection_start, selection_end)) = selection_state
        .cloned()
        .unwrap_or_else(|| TextEditState::caret_at(&content))
        .clamped_to(&content)
        .selection_range()
    {
        let selection_start = selection_start.min(content.len());
        let selection_end = selection_end.min(content.len());
        let selection_start_x = current_layout.x_for_index(selection_start);
        let selection_end_x = current_layout.x_for_index(selection_end);
        let selection_width = (selection_end_x - selection_start_x).max(0.0);
        if selection_width > 0.0 {
            scene.push_shape(RenderPrimitive {
                rect: Rect::new(
                    content_frame.x + selection_start_x,
                    content_frame.y,
                    selection_width,
                    content_frame.height.max(Dp::new(line_height)),
                ),
                color: theme.colors.selection.with_alpha_factor(opacity),
                corner_radius: 4.0,
                stroke_width: 0.0,
                clip_rect,
                clip_mask,
            });
        }
    }

    scene.push_text(TextPrimitive {
        content: content.clone(),
        frame: content_frame,
        color: color.with_alpha_factor(opacity),
        force_color: false,
        font_family: Some(resolved.primary_font),
        font_size,
        font_weight: text.font_weight.unwrap_or(default_style.weight),
        line_height,
        letter_spacing,
        clip_rect,
        clip_mask,
    });

    if show_caret {
        let caret_width = caret_content
            .map(|caret_text| {
                font_manager
                    .measure_text_raw(
                        caret_text,
                        text_request,
                        font_size,
                        line_height,
                        letter_spacing,
                    )
                    .0
            })
            .unwrap_or(current_layout.width);
        let caret_x = (inner.x + inner.width.min(caret_width) + CARET_END_GAP).max(inner.x);
        scene.push_overlay_shape(RenderPrimitive {
            rect: Rect::new(
                caret_x,
                content_frame.y,
                CARET_WIDTH,
                content_frame.height.max(Dp::new(line_height)),
            ),
            color: theme.colors.on_surface.with_alpha_factor(opacity),
            corner_radius: 0.0,
            stroke_width: 0.0,
            clip_rect,
            clip_mask,
        });
    }
}

fn measure_select_content(
    selected_label: Option<&str>,
    placeholder: &Value<String>,
    select_style: &ResolvedSelectStyle,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
) -> (f32, f32) {
    let display = selected_label
        .map(|label| select_display_text(text_from_content(label.to_string()), select_style))
        .unwrap_or_else(|| {
            select_display_text(text_from_content(placeholder.clone()), select_style)
        });
    let text_size = measure_text_content(&display, font_manager, theme, units);
    let horizontal = units.resolve_dp(select_style.padding_x) * 2.0 + units.resolve_dp(dp(24.0));
    let vertical = units.resolve_dp(select_style.padding_y) * 2.0;
    (
        SELECT_DEFAULT_WIDTH.max(text_size.0 + horizontal),
        text_size
            .1
            .max(units.resolve_dp(select_style.min_height))
            .max(text_size.1 + vertical),
    )
}

fn select_display_text(mut text: Text, select_style: &ResolvedSelectStyle) -> Text {
    if text.font_family.is_none() {
        text.font_family = select_style.text_style.font_family.clone();
    }
    if text.font_size.is_none() {
        text.font_size = Some(select_style.text_style.size);
    }
    if text.font_weight.is_none() {
        text.font_weight = Some(select_style.text_style.weight);
    }
    if text.letter_spacing.is_none() {
        text.letter_spacing = select_style.text_style.letter_spacing;
    }
    text
}

#[allow(clippy::too_many_arguments)]
fn push_select_primitives(
    frame: Rect,
    selected_label: Option<String>,
    placeholder: &Value<String>,
    select_style: &ResolvedSelectStyle,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    padding: Insets,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    let arrow_width = dp(24.0);
    let text_frame = Rect::new(
        frame.x,
        frame.y,
        (frame.width - arrow_width).max(Dp::ZERO),
        frame.height,
    );
    match selected_label {
        Some(label) => push_select_text(
            &select_display_text(text_from_content(label), select_style),
            text_frame,
            font_manager,
            theme,
            units,
            animations,
            now,
            scene,
            padding,
            select_style.text,
            opacity,
            widget_id,
            clip_rect,
            clip_mask,
            false,
        ),
        None => push_select_text(
            &select_display_text(text_from_content(placeholder.clone()), select_style),
            text_frame,
            font_manager,
            theme,
            units,
            animations,
            now,
            scene,
            padding,
            select_style.placeholder,
            opacity,
            widget_id,
            clip_rect,
            clip_mask,
            false,
        ),
    }

    push_select_icon(
        Rect::new(
            (frame.right() - arrow_width).max(frame.x),
            frame.y,
            arrow_width.min(frame.width),
            frame.height,
        ),
        font_manager,
        select_style,
        units,
        scene,
        opacity,
        clip_rect,
        clip_mask,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_select_menu_primitives<VM>(
    widget_id: WidgetId,
    trigger_frame: Rect,
    viewport: Rect,
    options: &[SelectOptionState<VM>],
    on_open_change: Option<&ValueCommand<VM, bool>>,
    select_style: &ResolvedSelectStyle,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
    opacity: f32,
) {
    if options.is_empty() {
        return;
    }

    let option_height = context
        .units
        .resolve_dp(select_style.option_height)
        .max(1.0);
    let menu_height = option_height * options.len() as f32;
    let menu_gap = context.units.resolve_dp(select_style.menu_gap);
    let below_space = (viewport.bottom().get() - trigger_frame.bottom().get() - menu_gap).max(0.0);
    let above_space = (trigger_frame.y.get() - viewport.y.get() - menu_gap).max(0.0);
    let open_down = below_space >= menu_height || below_space >= above_space;
    let available_height = if open_down { below_space } else { above_space };
    let visible_height = menu_height.min(available_height).max(0.0);
    if visible_height <= 0.0 {
        return;
    }

    let menu_y = if open_down {
        trigger_frame.bottom().get() + menu_gap
    } else {
        trigger_frame.y.get() - menu_gap - visible_height
    };
    let menu_frame = Rect::new(trigger_frame.x, menu_y, trigger_frame.width, visible_height);
    let Some(menu_clip) = viewport.intersect(menu_frame) else {
        return;
    };
    let menu_clip = Some(menu_clip);
    let menu_corner_radius = context.units.resolve_dp(select_style.radius);
    let menu_clip_mask = Some(ClipMask {
        rect: menu_frame,
        corner_radius: menu_corner_radius,
    });

    computed.scene.push_overlay_shape(RenderPrimitive {
        rect: menu_frame,
        color: select_style.menu_background.with_alpha_factor(opacity),
        corner_radius: menu_corner_radius,
        stroke_width: 0.0,
        clip_rect: menu_clip,
        clip_mask: None,
    });

    let option_padding = Insets::symmetric(select_style.padding_x, Dp::ZERO);
    let disabled_text = default_select_disabled_text_color(context.theme);
    let mut option_interactions = InteractionHandlers::default();
    option_interactions.cursor_style = Some(Value::Static(CursorStyle::Pointer));

    for (index, option) in options.iter().enumerate() {
        let option_frame = Rect::new(
            menu_frame.x,
            menu_frame.y + option_height * index as f32,
            menu_frame.width,
            option_height,
        );
        let selected = option.selected.resolve();
        let option_disabled = option.disabled.resolve();
        let mut option_state = context.widget_states.get_select_option(widget_id, index);
        option_state.disabled = option_disabled;
        let hovered_option_color = default_select_menu_option_color(context.theme, option_state);
        let option_color = if option_state.hovered || option_state.pressed {
            hovered_option_color
        } else if selected {
            select_style.selected_option_background
        } else {
            hovered_option_color
        };
        if selected || option_color.a > 0 {
            computed.scene.push_overlay_shape(RenderPrimitive {
                rect: option_frame,
                color: option_color.with_alpha_factor(opacity),
                corner_radius: 0.0,
                stroke_width: 0.0,
                clip_rect: menu_clip,
                clip_mask: menu_clip_mask,
            });
        }

        push_select_text(
            &select_display_text(text_from_content(option.label.clone()), select_style),
            option_frame,
            context.font_manager,
            context.theme,
            context.units,
            context.animations,
            context.now,
            &mut computed.scene,
            option_padding,
            if option_disabled {
                disabled_text
            } else {
                select_style.text
            },
            opacity,
            widget_id,
            menu_clip,
            None,
            true,
        );

        computed.overlay_hit_regions.push(HitRegion {
            rect: option_frame,
            clip_rect: menu_clip,
            geometry: HitGeometry::Rect,
            interaction: if option_disabled {
                HitInteraction::Disabled { id: widget_id }
            } else {
                HitInteraction::SelectOption {
                    id: widget_id,
                    option_index: index,
                    interactions: option_interactions.clone(),
                    on_select: option.on_select.clone(),
                    on_open_change: on_open_change.cloned(),
                }
            },
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn push_select_text(
    text: &Text,
    frame: Rect,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    padding: Insets,
    fallback_color: Color,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    overlay: bool,
) {
    let content = text.content.resolve();
    let default_style = &theme.typography.body;
    let text_request = TextFontRequest {
        preferred_font: text
            .font_family
            .as_deref()
            .or(default_style.font_family.as_deref()),
        weight: text.font_weight.unwrap_or(default_style.weight),
    };
    let resolved = font_manager.resolve_text(&content, text_request.clone());
    let color = text
        .color
        .as_ref()
        .map(|color| color.resolve_widget(animations, widget_id, WidgetProperty::TextColor, now))
        .unwrap_or(fallback_color)
        .with_alpha_factor(opacity);
    let (font_size, line_height, letter_spacing) = resolved_text_metrics(text, theme, units);
    let inner = frame.inset(padding);
    let layout = font_manager.measure_text_layout(
        &content,
        text_request,
        font_size,
        line_height,
        letter_spacing,
    );
    let content_frame = centered_text_frame(inner, layout.width, layout.height, line_height, false);
    let primitive = TextPrimitive {
        content,
        frame: content_frame,
        color,
        force_color: false,
        font_family: Some(resolved.primary_font),
        font_size,
        font_weight: text.font_weight.unwrap_or(default_style.weight),
        line_height,
        letter_spacing,
        clip_rect,
        clip_mask,
    };
    if overlay {
        scene.push_overlay_text(primitive);
    } else {
        scene.push_text(primitive);
    }
}

fn push_select_icon(
    frame: Rect,
    font_manager: &FontManager,
    select_style: &ResolvedSelectStyle,
    units: UnitContext,
    scene: &mut ScenePrimitives,
    opacity: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    let font_size = units
        .resolve_sp(select_style.text_style.size)
        .min(frame.width.get())
        .min(frame.height.get())
        .max(1.0);
    let line_height = font_size;
    let letter_spacing = 0.0;
    let text_request = TextFontRequest {
        preferred_font: Some(ICON_FONT_FAMILY),
        weight: select_style.text_style.weight,
    };
    let resolved = font_manager.resolve_text(SELECT_ARROW_ICON, text_request.clone());
    let layout = font_manager.measure_text_layout(
        SELECT_ARROW_ICON,
        text_request,
        font_size,
        line_height,
        letter_spacing,
    );
    let icon_frame = centered_text_frame(
        frame,
        layout.width.max(font_size),
        layout.height.max(line_height),
        line_height,
        true,
    );

    scene.push_text(TextPrimitive {
        content: SELECT_ARROW_ICON.to_string(),
        frame: icon_frame,
        color: select_style.arrow.with_alpha_factor(opacity),
        force_color: true,
        font_family: Some(resolved.primary_font),
        font_size,
        font_weight: select_style.text_style.weight,
        line_height,
        letter_spacing,
        clip_rect,
        clip_mask,
    });
}

fn default_switch_transition() -> crate::animation::Transition {
    crate::animation::Transition::ease_in_out(std::time::Duration::from_millis(180))
}

fn push_checkbox_primitives(
    frame: Rect,
    checked: bool,
    label: Option<&Value<String>>,
    checkbox_style: &ResolvedCheckboxStyle,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
) {
    let box_size = units.resolve_dp(checkbox_style.size);
    let box_frame = Rect::new(
        frame.x,
        frame.y + ((frame.height - box_size) * 0.5).max(Dp::ZERO),
        box_size,
        box_size,
    );
    let radius = units.resolve_dp(checkbox_style.radius);
    scene.push_shape(RenderPrimitive {
        rect: box_frame,
        color: checkbox_style.background.with_alpha_factor(opacity),
        corner_radius: radius,
        stroke_width: 0.0,
        clip_rect,
        clip_mask,
    });
    let border_width = units.resolve_dp(checkbox_style.border_width);
    push_border_primitives(
        scene,
        box_frame,
        border_width,
        checkbox_style.border.with_alpha_factor(opacity),
        radius,
        clip_rect,
        clip_mask,
    );
    push_focus_ring_primitives(
        scene,
        box_frame,
        radius,
        checkbox_style.focus_ring.as_ref(),
        opacity,
    );

    if checked {
        push_checkbox_checkmark_primitives(
            box_frame,
            checkbox_style,
            opacity,
            font_manager,
            units,
            clip_rect,
            clip_mask,
            scene,
        );
    }

    if let Some(label) = label {
        let label = checkbox_label_with_theme(label, checkbox_style);
        let label_x = box_frame.right() + checkbox_style.label_gap;
        let label_frame = Rect::new(
            label_x,
            frame.y + dp(1.0),
            (frame.right() - label_x).max(Dp::ZERO),
            frame.height,
        );
        push_text_primitives(
            &label,
            label_frame,
            font_manager,
            theme,
            units,
            animations,
            now,
            scene,
            false,
            false,
            Insets::ZERO,
            None,
            None,
            checkbox_style.label,
            opacity,
            widget_id,
            clip_rect,
            clip_mask,
        );
    }
}

fn push_checkbox_checkmark_primitives(
    box_frame: Rect,
    checkbox_style: &ResolvedCheckboxStyle,
    opacity: f32,
    font_manager: &FontManager,
    units: UnitContext,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    scene: &mut ScenePrimitives,
) {
    let font_size = units
        .resolve_sp(checkbox_style.text_style.size)
        .min(box_frame.width.get())
        .min(box_frame.height.get())
        .max(1.0);
    let line_height = font_size;
    let letter_spacing = 0.0;
    let text_request = TextFontRequest {
        preferred_font: Some(ICON_FONT_FAMILY),
        weight: checkbox_style.text_style.weight,
    };
    let resolved = font_manager.resolve_text(CHECKBOX_CHECKMARK_ICON, text_request.clone());
    let layout = font_manager.measure_text_layout(
        CHECKBOX_CHECKMARK_ICON,
        text_request,
        font_size,
        line_height,
        letter_spacing,
    );
    let mut icon_frame = centered_text_frame(
        box_frame,
        layout.width.max(font_size),
        layout.height.max(line_height),
        line_height,
        true,
    );

    // 将对勾图标向下移动1dp
    icon_frame.y += dp(1.0);

    scene.push_text(TextPrimitive {
        content: CHECKBOX_CHECKMARK_ICON.to_string(),
        frame: icon_frame,
        color: checkbox_style.checkmark.with_alpha_factor(opacity),
        force_color: true,
        font_family: Some(resolved.primary_font),
        font_size,
        font_weight: checkbox_style.text_style.weight,
        line_height,
        letter_spacing,
        clip_rect,
        clip_mask,
    });
}

fn push_radio_primitives(
    frame: Rect,
    checked: bool,
    label: Option<&Value<String>>,
    radio_style: &ResolvedRadioStyle,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
) {
    let size = units.resolve_dp(radio_style.size);
    let control_frame = Rect::new(
        frame.x,
        frame.y + ((frame.height - size) * 0.5).max(Dp::ZERO),
        size,
        size,
    );
    let radius = units
        .resolve_dp(radio_style.radius)
        .min(size * 0.5)
        .max(0.0);
    scene.push_shape(RenderPrimitive {
        rect: control_frame,
        color: radio_style.background.with_alpha_factor(opacity),
        corner_radius: radius,
        stroke_width: 0.0,
        clip_rect,
        clip_mask,
    });
    push_border_primitives(
        scene,
        control_frame,
        units.resolve_dp(radio_style.border_width),
        radio_style.border.with_alpha_factor(opacity),
        radius,
        clip_rect,
        clip_mask,
    );
    push_focus_ring_primitives(
        scene,
        control_frame,
        radius,
        radio_style.focus_ring.as_ref(),
        opacity,
    );

    if checked {
        let inset = dp(size * 0.28);
        let indicator_frame = control_frame.inset(Insets::all(inset));
        if indicator_frame.width > Dp::ZERO && indicator_frame.height > Dp::ZERO {
            let indicator_radius = (indicator_frame.width.min(indicator_frame.height).get() * 0.5)
                .min(radius)
                .max(0.0);
            scene.push_overlay_shape(RenderPrimitive {
                rect: indicator_frame,
                color: radio_style.indicator.with_alpha_factor(opacity),
                corner_radius: indicator_radius,
                stroke_width: 0.0,
                clip_rect,
                clip_mask,
            });
        }
    }

    if let Some(label) = label {
        let label = radio_label_with_theme(label, radio_style);
        let label_x = control_frame.right() + radio_style.label_gap;
        let label_frame = Rect::new(
            label_x,
            frame.y + dp(1.0),
            (frame.right() - label_x).max(Dp::ZERO),
            frame.height,
        );
        push_text_primitives(
            &label,
            label_frame,
            font_manager,
            theme,
            units,
            animations,
            now,
            scene,
            false,
            false,
            Insets::ZERO,
            None,
            None,
            radio_style.label,
            opacity,
            widget_id,
            clip_rect,
            clip_mask,
        );
    }
}

fn push_switch_primitives(
    background_frame: Rect,
    background_radius: f32,
    padding: Insets,
    checked: bool,
    active_thumb_color: Color,
    inactive_thumb_color: Color,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
    animations: &mut AnimationEngine,
    scene: &mut ScenePrimitives,
    now: std::time::Instant,
) {
    let inner = background_frame.inset(padding);
    if inner.width <= Dp::ZERO || inner.height <= Dp::ZERO {
        return;
    }

    let thumb_diameter = inner.height.min(inner.width);
    if thumb_diameter <= Dp::ZERO {
        return;
    }

    let travel = (inner.width - thumb_diameter).max(Dp::ZERO);
    let thumb_offset = animations.resolve_dp(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::SwitchThumbOffset,
        },
        if checked { travel } else { Dp::ZERO },
        Some(default_switch_transition()),
        now,
    );
    let thumb_color = animations.resolve_color(
        crate::animation::AnimationKey::Widget {
            id: widget_id.raw(),
            property: WidgetProperty::SwitchThumbColor,
        },
        if checked {
            active_thumb_color
        } else {
            inactive_thumb_color
        },
        Some(default_switch_transition()),
        now,
    );

    scene.push_overlay_shape(RenderPrimitive {
        rect: Rect::new(
            inner.x + thumb_offset,
            inner.y + ((inner.height - thumb_diameter) / 2.0),
            thumb_diameter,
            thumb_diameter,
        ),
        color: thumb_color.with_alpha_factor(opacity),
        corner_radius: (thumb_diameter.get() * 0.5).min(background_radius),
        stroke_width: 0.0,
        clip_rect,
        clip_mask,
    });
}

fn push_focus_ring_primitives(
    scene: &mut ScenePrimitives,
    frame: Rect,
    border_radius: f32,
    focus_ring: Option<&crate::theme::FocusRingStyle>,
    opacity: f32,
) {
    let Some(focus_ring) = focus_ring else {
        return;
    };
    if !focus_ring.enabled {
        return;
    }

    let width = focus_ring.width.get().max(0.0);
    if width <= 0.0 {
        return;
    }
    let gap = focus_ring.gap.get().max(0.0);
    let expansion = gap + (width * 0.5);
    let ring_frame = Rect::new(
        frame.x - expansion,
        frame.y - expansion,
        frame.width + expansion * 2.0,
        frame.height + expansion * 2.0,
    );
    if ring_frame.is_empty() {
        return;
    }

    scene.push_overlay_shape(RenderPrimitive {
        rect: ring_frame,
        color: focus_ring.color.with_alpha_factor(opacity),
        corner_radius: border_radius + expansion,
        stroke_width: width,
        clip_rect: None,
        clip_mask: None,
    });
}

fn centered_text_frame(
    inner: Rect,
    measured_width: f32,
    measured_height: f32,
    line_height: f32,
    center_horizontally: bool,
) -> Rect {
    let content_height = inner
        .height
        .min(measured_height.max(line_height))
        .max(Dp::new(line_height));
    let content_width = inner.width.min(measured_width).max(0.0);
    let content_x = if center_horizontally {
        inner.x + ((inner.width - content_width).max(0.0) * 0.5)
    } else {
        inner.x
    };

    Rect::new(
        content_x,
        inner.y + ((inner.height - content_height).max(0.0) * 0.5),
        content_width,
        content_height,
    )
}

fn push_border_primitives(
    scene: &mut ScenePrimitives,
    frame: Rect,
    border_width: f32,
    border_color: Color,
    border_radius: f32,
    clip_rect: Option<Rect>,
    clip_mask: Option<ClipMask>,
) {
    if border_color.a == 0 {
        return;
    }

    let thickness = border_width
        .min((frame.width * 0.5).get())
        .min((frame.height * 0.5).get())
        .max(0.0);
    if thickness <= 0.0 {
        return;
    }

    scene.push_shape(RenderPrimitive {
        rect: frame,
        color: border_color,
        corner_radius: border_radius,
        stroke_width: thickness,
        clip_rect,
        clip_mask,
    });
}

pub struct WidgetTree<VM> {
    root: Element<VM>,
}

impl<VM> WidgetTree<VM> {
    pub fn new(root: impl Into<Element<VM>>) -> Self {
        Self { root: root.into() }
    }

    #[allow(dead_code)]
    pub(crate) fn compute_scene(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        _focused_input: Option<WidgetId>,
        _focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        _caret_visible: bool,
    ) -> ComputedScene<VM> {
        self.compute_scene_with_widget_state(
            font_manager,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            &WidgetStateMap::default(),
            &HashMap::new(),
            scroll_offsets,
            viewport,
            _focused_input,
            _focused_text_state,
            selected_text,
            selected_text_state,
            _caret_visible,
        )
    }

    pub(crate) fn compute_scene_with_widget_state(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        _focused_input: Option<WidgetId>,
        _focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        _caret_visible: bool,
    ) -> ComputedScene<VM> {
        self.compute_scene_with_units_and_widget_state(
            font_manager,
            theme,
            media,
            UnitContext::default(),
            animations,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            _focused_input,
            _focused_text_state,
            selected_text,
            selected_text_state,
            _caret_visible,
        )
    }

    #[allow(dead_code)]
    pub(crate) fn compute_scene_with_units(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        units: UnitContext,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        _focused_input: Option<WidgetId>,
        _focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        _caret_visible: bool,
    ) -> ComputedScene<VM> {
        self.compute_scene_with_units_and_widget_state(
            font_manager,
            theme,
            media,
            units,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            &WidgetStateMap::default(),
            &HashMap::new(),
            scroll_offsets,
            viewport,
            _focused_input,
            _focused_text_state,
            selected_text,
            selected_text_state,
            _caret_visible,
        )
    }

    pub(crate) fn compute_scene_with_units_and_widget_state(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        units: UnitContext,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        _focused_input: Option<WidgetId>,
        _focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        _caret_visible: bool,
    ) -> ComputedScene<VM> {
        let layout =
            self.build_scene_layout(font_manager, theme, media, animations, units, viewport);
        self.collect_scene_from_layout(
            font_manager,
            &layout,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            _focused_input,
            _focused_text_state,
            selected_text,
            selected_text_state,
            _caret_visible,
        )
    }

    pub(crate) fn build_scene_layout(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        units: UnitContext,
        viewport: Rect,
    ) -> ResolvedSceneLayout<VM> {
        let mut taffy = TaffyTree::new();
        let now = std::time::Instant::now();
        let resolved_root = self.root.resolve(theme);
        let root_layout = resolved_root
            .build_layout_tree(
                &mut taffy, animations, theme, units, None, viewport, true, now,
            )
            .expect("widget tree layout should build");
        taffy
            .compute_layout_with_measure(
                root_layout.node,
                TaffySize {
                    width: AvailableSpace::Definite(viewport.width.get()),
                    height: AvailableSpace::Definite(viewport.height.get()),
                },
                |known_dimensions, _, _, node_context, _| {
                    measure_node(
                        node_context,
                        known_dimensions,
                        font_manager,
                        theme,
                        media,
                        units,
                    )
                },
            )
            .expect("widget tree layout should compute");

        ResolvedSceneLayout {
            resolved_root,
            layout_root: root_layout,
            taffy,
            units,
        }
    }

    pub(crate) fn collect_scene_from_layout(
        &self,
        font_manager: &FontManager,
        layout: &ResolvedSceneLayout<VM>,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        _focused_input: Option<WidgetId>,
        _focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        _caret_visible: bool,
    ) -> ComputedScene<VM> {
        let mut computed = ComputedScene::default();
        let mut context = CollectContext {
            taffy: &layout.taffy,
            font_manager,
            theme,
            media,
            selected_text,
            selected_text_state,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            units: layout.units,
            animations,
            now: std::time::Instant::now(),
        };
        layout.resolved_root.collect_primitives(
            &layout.layout_root,
            VisualContext {
                origin: Point {
                    x: viewport.x,
                    y: viewport.y,
                },
                opacity: 1.0,
                clip_rect: viewport,
                clip_mask: None,
            },
            &mut context,
            &mut computed,
        );
        computed
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn render_output(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> RenderedWidgetScene {
        self.render_output_with_widget_state(
            font_manager,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            &WidgetStateMap::default(),
            &HashMap::new(),
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            selected_text,
            selected_text_state,
            caret_visible,
        )
    }

    #[cfg(test)]
    pub(crate) fn render_output_with_widget_state(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> RenderedWidgetScene {
        self.render_output_with_units_and_widget_state(
            font_manager,
            theme,
            media,
            UnitContext::default(),
            animations,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            selected_text,
            selected_text_state,
            caret_visible,
        )
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn render_output_with_units(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        units: UnitContext,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> RenderedWidgetScene {
        self.render_output_with_units_and_widget_state(
            font_manager,
            theme,
            media,
            units,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            &WidgetStateMap::default(),
            &HashMap::new(),
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            selected_text,
            selected_text_state,
            caret_visible,
        )
    }

    #[cfg(test)]
    pub(crate) fn render_output_with_units_and_widget_state(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        units: UnitContext,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        focused_input: Option<WidgetId>,
        focused_text_state: Option<&TextEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&TextEditState>,
        caret_visible: bool,
    ) -> RenderedWidgetScene {
        let computed = self.compute_scene_with_units_and_widget_state(
            font_manager,
            theme,
            media,
            units,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            focused_text_state,
            selected_text,
            selected_text_state,
            caret_visible,
        );
        computed.rendered()
    }

    pub(crate) fn hit_path_from_computed(
        computed: &ComputedScene<VM>,
        point: Point,
    ) -> Vec<HitInteraction<VM>> {
        let mut path = Vec::new();
        let mut ids = Vec::new();

        for hit in computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .filter(|hit| {
                hit.rect.contains(point)
                    && hit
                        .clip_rect
                        .map(|clip_rect| clip_rect.contains(point))
                        .unwrap_or(true)
                    && hit.geometry.contains(point)
            })
        {
            let id = hit.interaction.target_id();

            if let Some(index) = ids.iter().position(|existing| *existing == id) {
                path[index] = hit.interaction.clone();
            } else {
                ids.push(id);
                path.push(hit.interaction.clone());
            }
        }

        path
    }

    #[allow(dead_code)]
    pub(crate) fn hit_test(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        cursor_position: Option<Point>,
        focused_input: Option<WidgetId>,
    ) -> Option<HitInteraction<VM>> {
        self.hit_test_with_widget_state(
            font_manager,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            &WidgetStateMap::default(),
            &HashMap::new(),
            scroll_offsets,
            viewport,
            cursor_position,
            focused_input,
        )
    }

    pub(crate) fn hit_test_with_widget_state(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        cursor_position: Option<Point>,
        focused_input: Option<WidgetId>,
    ) -> Option<HitInteraction<VM>> {
        self.hit_path_with_widget_state(
            font_manager,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            cursor_position,
            focused_input,
        )
        .pop()
    }

    pub(crate) fn hit_path_with_widget_state(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        widget_states: &WidgetStateMap,
        select_open_states: &HashMap<WidgetId, bool>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        cursor_position: Option<Point>,
        focused_input: Option<WidgetId>,
    ) -> Vec<HitInteraction<VM>> {
        let Some(point) = cursor_position else {
            return Vec::new();
        };
        let computed = self.compute_scene_with_widget_state(
            font_manager,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            widget_states,
            select_open_states,
            scroll_offsets,
            viewport,
            focused_input,
            None,
            None,
            None,
            false,
        );
        Self::hit_path_from_computed(&computed, point)
    }

    #[allow(dead_code)]
    pub(crate) fn hit_path(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        hovered_scrollbar: Option<ScrollbarHandle>,
        active_scrollbar: Option<ScrollbarHandle>,
        scroll_offsets: &HashMap<WidgetId, Point>,
        viewport: Rect,
        cursor_position: Option<Point>,
        focused_input: Option<WidgetId>,
    ) -> Vec<HitInteraction<VM>> {
        self.hit_path_with_widget_state(
            font_manager,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            &WidgetStateMap::default(),
            &HashMap::new(),
            scroll_offsets,
            viewport,
            cursor_position,
            focused_input,
        )
    }

    pub(crate) fn media_event_states(
        &self,
        media: &MediaManager,
        theme: &Theme,
    ) -> Vec<MediaEventState<VM>> {
        let mut states = Vec::new();
        self.root
            .resolve(theme)
            .collect_media_event_states(media, &mut states);
        states
    }
}

#[cfg(test)]
mod tests {
    use super::{centered_text_frame, resolved_text_metrics, SELECT_ARROW_ICON};
    use std::collections::HashMap;

    use crate::animation::{AnimationCoordinator, AnimationEngine};
    use crate::foundation::binding::{InvalidationSignal, ViewModelContext};
    use crate::foundation::color::Color;
    use crate::foundation::view_model::{Command, CommandContext, ValueCommand};
    use crate::media::{MediaManager, MediaSource};
    use crate::text::font::{FontCatalog, FontManager};
    use crate::ui::layout::{Axis, Insets, Overflow};
    use crate::ui::theme::{Stateful, Theme};
    use crate::ui::unit::{dp, sp, Dp, UnitContext};
    use crate::ui::widget::common::{ContainerKind, Rect, WidgetKind};
    use crate::ui::widget::{
        BackgroundGradientStop, BackgroundImage, BackgroundLinearGradient, BackgroundRadialGradient,
    };
    use crate::ui::widget::{
        ButtonStyle, Canvas, CanvasItem, CanvasPath, CanvasStroke, CanvasStyle, Checkbox, ClipMask,
        ContainerStyle, Element, Image, PathBuilder, Point, Radio, RadioGroup, RadioOption,
        ScrollbarAxis, ScrollbarHandle, Select, SelectOption, Stack, Switch, SwitchStyle, Text,
        TextEditState, TextWidgetStyle, WidgetStateMap, WidgetTree,
    };
    #[cfg(feature = "video")]
    use crate::video::backend::{
        BackendSharedState, VideoBackend, DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES,
    };
    #[cfg(feature = "video")]
    use crate::video::{PlaybackState, VideoController, VideoMetrics, VideoSize, VideoSurface};

    const ONE_BY_ONE_GIF: &[u8] = &[
        0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xFF, 0xFF, 0xFF, 0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02,
        0x01, 0x4C, 0x00, 0x3B,
    ];

    fn stateful<T: Clone>(value: T) -> Stateful<T> {
        Stateful {
            normal: value.clone(),
            hovered: value.clone(),
            pressed: value.clone(),
            disabled: value,
        }
    }

    fn text_style(
        mode: crate::theme::ResolvedThemeMode,
        size: Option<crate::ui::unit::Sp>,
    ) -> TextWidgetStyle {
        let mut style = TextWidgetStyle::default_for(mode);
        if let Some(size) = size {
            style.typography.size = size;
        }
        style
    }

    fn container_style(
        mode: crate::theme::ResolvedThemeMode,
        background: Option<Color>,
        brush: Option<crate::ui::widget::BackgroundBrush>,
        image: Option<BackgroundImage>,
        blur: Option<Dp>,
        border: Option<(Dp, Color)>,
        radius: Option<Dp>,
        offset: Option<Point>,
    ) -> ContainerStyle {
        let mut style = ContainerStyle::default_for(mode);
        style.surface.background = background.map(Into::into);
        style.surface.background_brush = brush.map(Into::into);
        style.surface.background_image = image.map(Into::into);
        if let Some(blur) = blur {
            style.surface.background_blur = blur.into();
        }
        if let Some((width, color)) = border {
            style.surface.border_width = Some(width.into());
            style.surface.border_color = Some(color.into());
        }
        if let Some(radius) = radius {
            style.surface.border_radius = Some(radius.into());
        }
        if let Some(offset) = offset {
            style.surface.offset = offset.into();
        }
        style
    }

    fn canvas_style(mode: crate::theme::ResolvedThemeMode, radius: Dp) -> CanvasStyle {
        let mut style = CanvasStyle::default_for(mode);
        style.surface.border_radius = Some(radius.into());
        style
    }

    fn button_style(
        mode: crate::theme::ResolvedThemeMode,
        radius: Option<Dp>,
        border_width: Option<Dp>,
        border_color: Option<Color>,
    ) -> ButtonStyle {
        let mut style =
            ButtonStyle::default_for(mode, crate::ui::widget::common::ButtonVariantKind::Primary);
        if let Some(radius) = radius {
            style.radius = radius.into();
        }
        if let Some(border_width) = border_width {
            style.border_width = border_width.into();
        }
        if let Some(border_color) = border_color {
            style.border = stateful(border_color.into());
        }
        style
    }

    fn switch_style(
        mode: crate::theme::ResolvedThemeMode,
        active_background: Color,
        inactive_background: Color,
        active_thumb: Option<Color>,
        inactive_thumb: Option<Color>,
    ) -> SwitchStyle {
        let mut style = SwitchStyle::default_for(mode);
        style.track_checked = stateful(active_background.into());
        style.track = stateful(inactive_background.into());
        if let Some(active_thumb) = active_thumb {
            style.thumb_checked = stateful(active_thumb.into());
        }
        if let Some(inactive_thumb) = inactive_thumb {
            style.thumb = stateful(inactive_thumb.into());
        }
        style
    }

    fn resolved_theme_mode(theme: &Theme) -> crate::theme::ResolvedThemeMode {
        super::infer_theme_mode(theme)
    }

    fn default_checkbox_style(
        theme: &Theme,
        state: crate::ui::theme::WidgetState,
        checked: bool,
    ) -> super::ResolvedCheckboxStyle {
        super::resolve_checkbox_style(
            &super::WidgetCheckboxStyle::default_for(resolved_theme_mode(theme)),
            state,
            checked,
            theme,
        )
    }

    fn default_radio_style(
        theme: &Theme,
        state: crate::ui::theme::WidgetState,
        checked: bool,
    ) -> super::ResolvedRadioStyle {
        super::resolve_radio_style(
            &super::WidgetRadioStyle::default_for(resolved_theme_mode(theme)),
            state,
            checked,
            theme,
        )
    }

    fn default_button_style(
        theme: &Theme,
        state: crate::ui::theme::WidgetState,
        variant: crate::ui::widget::common::ButtonVariantKind,
    ) -> super::ResolvedButtonStyle {
        super::resolve_button_style(
            &ButtonStyle::default_for(resolved_theme_mode(theme), variant),
            state,
            theme,
        )
    }

    fn default_switch_style(theme: &Theme) -> super::WidgetSwitchStyle {
        super::WidgetSwitchStyle::default_for(resolved_theme_mode(theme))
    }

    fn default_select_style(
        theme: &Theme,
        state: crate::ui::theme::WidgetState,
    ) -> super::ResolvedSelectStyle {
        super::resolve_select_style(
            &super::WidgetSelectStyle::default_for(resolved_theme_mode(theme)),
            state,
            theme,
        )
    }

    #[test]
    fn centers_text_using_actual_render_height() {
        let inner = Rect::new(12.0, 8.0, 180.0, 24.0);
        let frame = centered_text_frame(inner, 56.0, 18.0, 18.0, false);

        assert_eq!(frame.x, 12.0);
        assert_eq!(frame.y, 11.0);
        assert_eq!(frame.width, 56.0);
        assert_eq!(frame.height, 18.0);
    }

    #[test]
    fn centers_text_horizontally_when_requested() {
        let inner = Rect::new(12.0, 8.0, 180.0, 24.0);
        let frame = centered_text_frame(inner, 56.0, 18.0, 18.0, true);

        assert_eq!(frame.x, 74.0);
        assert_eq!(frame.y, 11.0);
        assert_eq!(frame.width, 56.0);
        assert_eq!(frame.height, 18.0);
    }

    #[test]
    fn text_background_matches_measured_text_width() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let background = crate::foundation::color::Color::RED;
        let tree: WidgetTree<()> =
            WidgetTree::new(Stack::new().size(dp(52.0), dp(52.0)).center().child(
                Text::new("A").style(move |mode| {
                    let mut style = text_style(mode, None);
                    style.surface.background = Some(background.into());
                    style
                }),
            ));

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        let text = rendered
            .primitives
            .texts
            .first()
            .expect("text primitive should exist");
        let background_shape = rendered
            .primitives
            .shapes
            .iter()
            .find(|primitive| primitive.color == background && primitive.rect.width.get() < 52.0)
            .expect("text background should exist");

        assert!((background_shape.rect.width.get() - text.frame.width.get()).abs() <= 1.0);
        assert!((background_shape.rect.height.get() - text.frame.height.get()).abs() <= 1.0);
    }

    #[test]
    fn larger_font_sizes_scale_default_line_height() {
        let theme = Theme::default();
        let mut text = Text::new("Background Effects Gallery");
        let style = text_style(resolved_theme_mode(&theme), Some(sp(30.0)));
        super::apply_text_widget_style(&mut text, &style);
        let (font_size, line_height, _) =
            resolved_text_metrics(&text, &theme, UnitContext::default());

        assert_eq!(font_size, 30.0);
        assert_eq!(line_height, 41.25);
    }

    #[test]
    fn image_loading_placeholder_uses_image_background() {
        let background = Color::hexa(0x11223344);

        assert_eq!(
            super::media_loading_fill_color(true, None, background, true),
            background
        );
    }

    #[test]
    fn image_loading_placeholder_defaults_to_transparent_white() {
        assert_eq!(
            super::media_loading_fill_color(true, None, Color::rgba(255, 255, 255, 0), true),
            Color::rgba(255, 255, 255, 0)
        );
    }

    #[test]
    fn image_error_placeholder_keeps_error_color() {
        assert_eq!(
            super::media_loading_fill_color(false, Some("boom"), Color::WHITE, false),
            crate::media::media_placeholder_color(false, Some("boom"))
        );
    }

    #[test]
    fn idle_media_placeholder_keeps_default_placeholder_color() {
        let background = Color::hexa(0xABCDEF12);

        assert_eq!(
            super::media_loading_fill_color(false, None, background, false),
            crate::media::media_placeholder_color(false, None)
        );
    }

    #[test]
    fn canvas_without_explicit_size_uses_item_bounds_for_layout() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let canvas: Element<()> = Canvas::new(vec![CanvasItem::Path(
            CanvasPath::new(
                1_u64,
                PathBuilder::new()
                    .move_to(0.0, 0.0)
                    .line_to(80.0, 0.0)
                    .line_to(80.0, 30.0)
                    .line_to(0.0, 30.0)
                    .close(),
            )
            .fill(Color::WHITE),
        )])
        .cursor(crate::ui::widget::CursorStyle::Pointer)
        .into();
        let canvas_id = canvas.id;
        let tree = WidgetTree::new(Stack::new().child(canvas));

        let computed = tree.compute_scene(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 200.0, 120.0),
            None,
            None,
            None,
            None,
            false,
        );

        let widget_region = computed
            .hit_regions
            .iter()
            .find(|region| matches!(region.interaction, super::HitInteraction::Widget { id, .. } if id == canvas_id))
            .expect("canvas widget region should exist");
        assert_eq!(widget_region.rect.width, 80.0);
        assert_eq!(widget_region.rect.height, 30.0);
    }

    #[test]
    fn background_brush_generates_brush_primitive() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> =
            WidgetTree::new(Stack::new().size(dp(120.0), dp(80.0)).style(|mode| {
                container_style(
                    mode,
                    None,
                    Some(
                        BackgroundLinearGradient::new(
                            Point::new(dp(0.0), dp(0.0)),
                            Point::new(dp(120.0), dp(80.0)),
                            vec![
                                BackgroundGradientStop::new(0.0, Color::hexa(0x38BDF8FF)),
                                BackgroundGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                            ],
                        )
                        .into(),
                    ),
                    None,
                    None,
                    None,
                    Some(dp(12.0)),
                    None,
                )
            }));

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 80.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert_eq!(rendered.primitives.brushes.len(), 1);
        assert!(matches!(
            rendered.primitives.brushes[0].brush,
            crate::ui::widget::BackgroundBrush::LinearGradient(_)
        ));
    }

    #[test]
    fn background_brush_takes_priority_over_background_color() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> =
            WidgetTree::new(Stack::new().size(dp(120.0), dp(80.0)).style(|mode| {
                container_style(
                    mode,
                    Some(Color::hexa(0xEF4444FF)),
                    Some(
                        BackgroundRadialGradient::new(
                            Point::new(dp(60.0), dp(40.0)),
                            dp(72.0),
                            vec![
                                BackgroundGradientStop::new(0.0, Color::hexa(0xFFFFFFAA)),
                                BackgroundGradientStop::new(1.0, Color::hexa(0x2563EB00)),
                            ],
                        )
                        .into(),
                    ),
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }));

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 80.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert_eq!(rendered.primitives.brushes.len(), 1);
        assert!(rendered
            .primitives
            .shapes
            .iter()
            .all(|shape| shape.color != Color::hexa(0xEF4444FF)));
    }

    #[test]
    fn background_blur_is_emitted_before_background_fill() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> =
            WidgetTree::new(Stack::new().size(dp(120.0), dp(80.0)).style(|mode| {
                container_style(
                    mode,
                    Some(Color::hexa(0x112233AA)),
                    None,
                    None,
                    Some(dp(18.0)),
                    None,
                    None,
                    None,
                )
            }));

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 80.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert_eq!(rendered.primitives.backdrop_blurs.len(), 1);
        assert!(matches!(
            rendered.primitives.commands.first(),
            Some(crate::ui::widget::RenderCommand::BackdropBlur(_))
        ));
    }

    #[test]
    fn background_image_produces_texture_primitive() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> =
            WidgetTree::new(Stack::new().size(dp(64.0), dp(64.0)).style(|mode| {
                container_style(
                    mode,
                    None,
                    None,
                    Some(BackgroundImage::from_bytes(ONE_BY_ONE_GIF)),
                    None,
                    None,
                    None,
                    None,
                )
            }));

        let rendered = wait_for_rendered_output(
            &tree,
            &font_manager,
            &theme,
            &media,
            &mut animations,
            Rect::new(0.0, 0.0, 64.0, 64.0),
        );

        assert_eq!(rendered.primitives.textures.len(), 1);
        assert_eq!(rendered.primitives.textures[0].frame.width, 64.0);
        assert_eq!(rendered.primitives.textures[0].frame.height, 64.0);
    }

    #[test]
    fn background_image_loading_failure_keeps_base_background_without_placeholder_text() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let fallback = Color::hexa(0x112233FF);
        let tree: WidgetTree<()> =
            WidgetTree::new(Stack::new().size(dp(80.0), dp(50.0)).style(move |mode| {
                container_style(
                    mode,
                    Some(fallback),
                    None,
                    Some(BackgroundImage::new(MediaSource::bytes(
                        b"not-an-image".as_slice(),
                    ))),
                    None,
                    None,
                    None,
                    None,
                )
            }));

        let rendered = wait_for_rendered_output(
            &tree,
            &font_manager,
            &theme,
            &media,
            &mut animations,
            Rect::new(0.0, 0.0, 80.0, 50.0),
        );

        assert!(rendered.primitives.textures.is_empty());
        assert!(rendered.primitives.texts.is_empty());
        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == fallback));
    }

    #[test]
    fn background_image_renders_between_blur_and_brush_overlay() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> =
            WidgetTree::new(Stack::new().size(dp(96.0), dp(72.0)).style(|mode| {
                container_style(
                    mode,
                    Some(Color::hexa(0x0F172AFF)),
                    Some(
                        BackgroundLinearGradient::new(
                            Point::new(dp(0.0), dp(0.0)),
                            Point::new(dp(96.0), dp(72.0)),
                            vec![
                                BackgroundGradientStop::new(0.0, Color::hexa(0xFFFFFF33)),
                                BackgroundGradientStop::new(1.0, Color::hexa(0x00000033)),
                            ],
                        )
                        .into(),
                    ),
                    Some(BackgroundImage::from_bytes(ONE_BY_ONE_GIF)),
                    Some(dp(10.0)),
                    Some((dp(1.0), Color::WHITE)),
                    None,
                    None,
                )
            }));

        let rendered = wait_for_rendered_output(
            &tree,
            &font_manager,
            &theme,
            &media,
            &mut animations,
            Rect::new(0.0, 0.0, 96.0, 72.0),
        );

        let commands = &rendered.primitives.commands;
        assert!(matches!(
            commands.get(0),
            Some(crate::ui::widget::RenderCommand::BackdropBlur(_))
        ));
        assert!(matches!(
            commands.get(1),
            Some(crate::ui::widget::RenderCommand::Shape(_))
        ));
        assert!(matches!(
            commands.get(2),
            Some(crate::ui::widget::RenderCommand::Texture(_))
        ));
        assert!(matches!(
            commands.get(3),
            Some(crate::ui::widget::RenderCommand::Brush(_))
        ));
    }

    #[test]
    fn background_image_texture_uses_corner_radius() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> =
            WidgetTree::new(Stack::new().size(dp(64.0), dp(64.0)).style(|mode| {
                container_style(
                    mode,
                    None,
                    None,
                    Some(BackgroundImage::from_bytes(ONE_BY_ONE_GIF)),
                    None,
                    None,
                    Some(dp(18.0)),
                    None,
                )
            }));

        let rendered = wait_for_rendered_output(
            &tree,
            &font_manager,
            &theme,
            &media,
            &mut animations,
            Rect::new(0.0, 0.0, 64.0, 64.0),
        );

        assert_eq!(rendered.primitives.textures.len(), 1);
        assert_eq!(rendered.primitives.textures[0].corner_radius, 18.0);
    }

    #[test]
    fn background_brush_keeps_clip_rect() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            Stack::new()
                .size(dp(100.0), dp(100.0))
                .overflow(Overflow::Hidden)
                .child(Stack::new().size(dp(120.0), dp(80.0)).style(|mode| {
                    container_style(
                        mode,
                        None,
                        Some(
                            BackgroundLinearGradient::new(
                                Point::new(dp(0.0), dp(0.0)),
                                Point::new(dp(120.0), dp(80.0)),
                                vec![
                                    BackgroundGradientStop::new(0.0, Color::hexa(0x14B8A6FF)),
                                    BackgroundGradientStop::new(1.0, Color::hexa(0x0F766EFF)),
                                ],
                            )
                            .into(),
                        ),
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                })),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 100.0, 100.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert_eq!(rendered.primitives.brushes.len(), 1);
        assert_eq!(
            rendered.primitives.brushes[0].clip_rect,
            Some(Rect::new(0.0, 0.0, 100.0, 100.0))
        );
    }

    #[test]
    fn canvas_renders_fill_and_stroke_meshes() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            Canvas::new(vec![CanvasItem::Path(
                CanvasPath::new(
                    1_u64,
                    PathBuilder::new()
                        .move_to(10.0, 10.0)
                        .line_to(100.0, 10.0)
                        .line_to(100.0, 60.0)
                        .line_to(10.0, 60.0)
                        .close(),
                )
                .fill(Color::hexa(0x22C55EFF))
                .stroke(CanvasStroke::new(dp(4.0), Color::WHITE)),
            )])
            .size(dp(120.0), dp(80.0)),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 80.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert_eq!(rendered.primitives.meshes.len(), 2);
        assert!(!rendered.primitives.commands.is_empty());
    }

    #[test]
    fn canvas_border_radius_clips_item_meshes() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            Canvas::new(vec![CanvasItem::Path(
                CanvasPath::new(1_u64, PathBuilder::new().rect(0.0, 0.0, 120.0, 80.0))
                    .fill(Color::hexa(0x22C55EFF)),
            )])
            .size(dp(120.0), dp(80.0))
            .style(|mode| canvas_style(mode, dp(18.0))),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 80.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(!rendered.primitives.meshes.is_empty());
        assert!(rendered.primitives.meshes.iter().all(|mesh| {
            mesh.clip_mask
                == Some(ClipMask {
                    rect: Rect::new(0.0, 0.0, 120.0, 80.0),
                    corner_radius: 18.0,
                })
        }));
    }

    #[test]
    fn canvas_hit_testing_prefers_topmost_item() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            Canvas::new(vec![
                CanvasItem::Path(
                    CanvasPath::new(
                        1_u64,
                        PathBuilder::new()
                            .move_to(0.0, 0.0)
                            .line_to(80.0, 0.0)
                            .line_to(80.0, 80.0)
                            .line_to(0.0, 80.0)
                            .close(),
                    )
                    .fill(Color::hexa(0x1D4ED8FF)),
                ),
                CanvasItem::Path(
                    CanvasPath::new(
                        2_u64,
                        PathBuilder::new()
                            .move_to(20.0, 20.0)
                            .line_to(90.0, 20.0)
                            .line_to(90.0, 90.0)
                            .line_to(20.0, 90.0)
                            .close(),
                    )
                    .fill(Color::hexa(0xF97316FF)),
                ),
            ])
            .size(dp(120.0), dp(120.0))
            .on_item_click(ValueCommand::new(|_: &mut (), _| {})),
        );

        let hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 120.0),
            Some(Point::new(dp(30.0), dp(30.0))),
            None,
        );

        assert!(matches!(
            hit,
            Some(super::HitInteraction::CanvasItem { item_id, .. }) if item_id == 2_u64.into()
        ));
    }

    fn test_media() -> MediaManager {
        MediaManager::new(InvalidationSignal::new())
    }

    fn wait_for_rendered_output(
        tree: &WidgetTree<()>,
        font_manager: &FontManager,
        theme: &Theme,
        media: &MediaManager,
        animations: &mut AnimationEngine,
        viewport: Rect,
    ) -> super::RenderedWidgetScene {
        for _ in 0..150 {
            let rendered = tree.render_output(
                font_manager,
                theme,
                media,
                animations,
                None,
                None,
                &HashMap::new(),
                viewport,
                None,
                None,
                None,
                None,
                false,
            );
            if !rendered.primitives.textures.is_empty() {
                return rendered;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        tree.render_output(
            font_manager,
            theme,
            media,
            animations,
            None,
            None,
            &HashMap::new(),
            viewport,
            None,
            None,
            None,
            None,
            false,
        )
    }

    fn test_context() -> ViewModelContext {
        ViewModelContext::new(InvalidationSignal::new(), AnimationCoordinator::default())
    }

    #[cfg(feature = "video")]
    fn test_video_controller(snapshot: crate::video::VideoSurfaceSnapshot) -> VideoController {
        struct StaticVideoBackend;

        impl VideoBackend for StaticVideoBackend {
            fn load(&self, _source: crate::video::VideoSource) -> Result<(), crate::TguiError> {
                Ok(())
            }

            fn play(&self) {}
            fn pause(&self) {}
            fn seek(&self, _position: std::time::Duration) {}
            fn set_volume(&self, _volume: f32) {}
            fn set_muted(&self, _muted: bool) {}
            fn set_buffer_memory_limit_bytes(&self, _bytes: u64) {}
            fn current_frame(&self) -> Option<std::sync::Arc<crate::media::TextureFrame>> {
                None
            }
            fn shutdown(&self) {}
        }

        let ctx = test_context();
        let shared = BackendSharedState {
            playback_state: ctx.observable(PlaybackState::Ready),
            metrics: ctx.observable(VideoMetrics {
                duration: Some(std::time::Duration::from_secs(30)),
                position: std::time::Duration::ZERO,
                buffered: Some(std::time::Duration::from_secs(30)),
                video_width: snapshot.intrinsic_size.width as u32,
                video_height: snapshot.intrinsic_size.height as u32,
            }),
            volume: ctx.observable(1.0),
            muted: ctx.observable(false),
            buffer_memory_limit_bytes: ctx.observable(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
            video_size: ctx.observable(VideoSize {
                width: snapshot.intrinsic_size.width as u32,
                height: snapshot.intrinsic_size.height as u32,
            }),
            error: ctx.observable(snapshot.error.clone()),
            surface: ctx.observable(snapshot),
        };
        VideoController::from_parts(shared, std::sync::Arc::new(StaticVideoBackend))
    }

    #[test]
    fn clipped_children_keep_clip_rect_and_do_not_hit_outside_parent() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree = WidgetTree::new(
            Stack::new().child(
                Stack::new()
                    .size(dp(100.0), dp(100.0))
                    .style(|mode| {
                        container_style(
                            mode,
                            Some(Color::hexa(0x1E293BFF)),
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                        )
                    })
                    .child(
                        Stack::new()
                            .size(dp(80.0), dp(80.0))
                            .style(|mode| {
                                container_style(
                                    mode,
                                    Some(Color::hexa(0x38BDF8FF)),
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    Some(Point::new(dp(60.0), dp(0.0))),
                                )
                            })
                            .on_click(Command::new(|_: &mut ()| {})),
                    ),
            ),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 100.0, 100.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert_eq!(
            rendered
                .primitives
                .shapes
                .last()
                .and_then(|primitive| primitive.clip_rect),
            Some(Rect::new(0.0, 0.0, 100.0, 100.0))
        );

        let hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Some(Point::new(dp(120.0), dp(20.0))),
            None,
        );
        assert!(hit.is_none());
    }

    #[test]
    fn wrapped_flex_align_start_packs_lines_from_cross_axis_start() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let child_color = crate::foundation::color::Color::hexa(0x22C55EFF);
        let tree: WidgetTree<()> = WidgetTree::new(
            crate::ui::widget::Flex::horizontal()
                .wrap(crate::ui::layout::Wrap::Wrap)
                .align(crate::ui::layout::Align::Start)
                .justify(crate::ui::layout::Justify::Start)
                .gap(dp(10.0))
                .child([
                    Stack::new().size(dp(60.0), dp(40.0)).style(move |mode| {
                        container_style(mode, Some(child_color), None, None, None, None, None, None)
                    }),
                    Stack::new().size(dp(60.0), dp(40.0)).style(move |mode| {
                        container_style(mode, Some(child_color), None, None, None, None, None, None)
                    }),
                    Stack::new().size(dp(60.0), dp(40.0)).style(move |mode| {
                        container_style(mode, Some(child_color), None, None, None, None, None, None)
                    }),
                ]),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 140.0, 240.0),
            None,
            None,
            None,
            None,
            false,
        );
        let child_rects: Vec<_> = rendered
            .primitives
            .shapes
            .iter()
            .filter(|shape| shape.color == child_color)
            .map(|shape| shape.rect)
            .collect();

        assert_eq!(child_rects.len(), 3);
        assert_eq!(child_rects[0], Rect::new(0.0, 0.0, 60.0, 40.0));
        assert_eq!(child_rects[1], Rect::new(70.0, 0.0, 60.0, 40.0));
        assert_eq!(child_rects[2], Rect::new(0.0, 50.0, 60.0, 40.0));
    }

    #[test]
    fn scroll_offsets_are_clamped_to_content_bounds() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let scroller: super::Element<()> = Stack::new()
            .size(dp(100.0), dp(100.0))
            .overflow_y(Overflow::Scroll)
            .style(|mode| {
                container_style(
                    mode,
                    Some(crate::foundation::color::Color::hexa(0x111827FF)),
                    None,
                    None,
                    None,
                    Some((dp(4.0), crate::foundation::color::Color::WHITE)),
                    None,
                    None,
                )
            })
            .child(Stack::new().size(dp(100.0), dp(300.0)).style(|mode| {
                container_style(
                    mode,
                    Some(crate::foundation::color::Color::hexa(0x22C55EFF)),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }))
            .into();
        let scroller_id = scroller.id;
        let tree = WidgetTree::new(Stack::new().child(scroller));

        let mut scroll_offsets = HashMap::new();
        scroll_offsets.insert(scroller_id, Point::new(dp(0.0), dp(500.0)));
        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &scroll_offsets,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            None,
            None,
            None,
            None,
            false,
        );

        let region = rendered
            .scroll_regions
            .into_iter()
            .find(|region| region.id == scroller_id)
            .expect("scroll region should exist");
        assert_eq!(region.content_viewport, Rect::new(4.0, 4.0, 92.0, 92.0));
        assert_eq!(region.scroll_offset.y, 204.0);
        assert_eq!(region.max_offset().y, 204.0);
    }

    #[test]
    fn scroll_content_bounds_include_container_bottom_padding() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let scroller: super::Element<()> = Stack::new()
            .size(dp(100.0), dp(100.0))
            .padding(Insets::all(dp(20.0)))
            .overflow_y(Overflow::Scroll)
            .child(Stack::new().size(dp(60.0), dp(120.0)).style(|mode| {
                container_style(
                    mode,
                    Some(crate::foundation::color::Color::hexa(0x22C55EFF)),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }))
            .into();
        let scroller_id = scroller.id;
        let tree = WidgetTree::new(scroller);

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 100.0, 100.0),
            None,
            None,
            None,
            None,
            false,
        );

        let region = rendered
            .scroll_regions
            .into_iter()
            .find(|region| region.id == scroller_id)
            .expect("scroll region should exist");
        assert_eq!(region.content_viewport, Rect::new(0.0, 0.0, 100.0, 100.0));
        assert_eq!(region.content_bounds.bottom(), dp(160.0));
        assert_eq!(region.max_offset().y, 60.0);
    }

    #[test]
    fn overflow_clips_children_to_inside_of_border() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree = WidgetTree::new(
            Stack::<()>::new()
                .size(dp(100.0), dp(100.0))
                .overflow(Overflow::Hidden)
                .style(|mode| {
                    container_style(
                        mode,
                        None,
                        None,
                        None,
                        None,
                        Some((dp(4.0), crate::foundation::color::Color::WHITE)),
                        None,
                        None,
                    )
                })
                .child(Stack::new().size(dp(100.0), dp(100.0)).style(|mode| {
                    container_style(
                        mode,
                        Some(crate::foundation::color::Color::BLACK),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                })),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 100.0, 100.0),
            None,
            None,
            None,
            None,
            false,
        );

        let child_shape = rendered
            .primitives
            .shapes
            .iter()
            .find(|primitive| primitive.color == crate::foundation::color::Color::BLACK)
            .expect("child shape should exist");
        assert_eq!(child_shape.clip_rect, Some(Rect::new(4.0, 4.0, 92.0, 92.0)));
    }

    #[test]
    fn rounded_overflow_clips_children_with_parent_corner_mask() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree = WidgetTree::new(
            Stack::<()>::new()
                .size(dp(100.0), dp(100.0))
                .style(|mode| {
                    container_style(
                        mode,
                        Some(crate::foundation::color::Color::WHITE),
                        None,
                        None,
                        None,
                        None,
                        Some(dp(18.0)),
                        None,
                    )
                })
                .overflow(Overflow::Hidden)
                .child(Stack::new().size(dp(100.0), dp(40.0)).style(|mode| {
                    container_style(
                        mode,
                        Some(crate::foundation::color::Color::BLACK),
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                    )
                })),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 100.0, 100.0),
            None,
            None,
            None,
            None,
            false,
        );

        let child_shape = rendered
            .primitives
            .shapes
            .iter()
            .find(|primitive| primitive.color == crate::foundation::color::Color::BLACK)
            .expect("child shape should exist");
        assert_eq!(
            child_shape.clip_mask,
            Some(ClipMask {
                rect: Rect::new(0.0, 0.0, 100.0, 100.0),
                corner_radius: 18.0,
            })
        );
    }

    #[test]
    fn scroll_containers_render_scrollbar_track_and_thumb() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let scroller: super::Element<()> = Stack::new()
            .size(dp(120.0), dp(120.0))
            .overflow_y(Overflow::Scroll)
            .style(|mode| {
                let mut style = ContainerStyle::default_for(mode);
                style.scrollbar.thumb_color = Some(crate::foundation::color::Color::BLACK);
                style.scrollbar.track_color = Some(crate::foundation::color::Color::WHITE);
                style.scrollbar.hover_thumb_color =
                    Some(crate::foundation::color::Color::hexa(0x112233FF));
                style.scrollbar.active_thumb_color =
                    Some(crate::foundation::color::Color::hexa(0x445566FF));
                style
            })
            .child(Stack::new().size(dp(120.0), dp(260.0)).style(|mode| {
                container_style(
                    mode,
                    Some(crate::foundation::color::Color::hexa(0x1D4ED8FF)),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            }))
            .into();
        let scroller_id = scroller.id;
        let tree = WidgetTree::new(scroller);

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 120.0),
            None,
            None,
            None,
            None,
            false,
        );

        let overlay_shapes = rendered.primitives.overlay_shapes;
        assert!(overlay_shapes
            .iter()
            .any(|primitive| primitive.color == crate::foundation::color::Color::WHITE));
        assert!(overlay_shapes
            .iter()
            .any(|primitive| primitive.color == crate::foundation::color::Color::BLACK));

        let hovered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            Some(ScrollbarHandle {
                id: scroller_id,
                axis: ScrollbarAxis::Vertical,
            }),
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 120.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert!(hovered
            .primitives
            .overlay_shapes
            .iter()
            .any(|primitive| primitive.color == crate::foundation::color::Color::hexa(0x112233FF)));

        let active = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            Some(ScrollbarHandle {
                id: scroller_id,
                axis: ScrollbarAxis::Vertical,
            }),
            Some(ScrollbarHandle {
                id: scroller_id,
                axis: ScrollbarAxis::Vertical,
            }),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 120.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert!(active
            .primitives
            .overlay_shapes
            .iter()
            .any(|primitive| primitive.color == crate::foundation::color::Color::hexa(0x445566FF)));
    }

    #[test]
    fn binding_driven_children_relayout_when_child_count_changes() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let context = test_context();
        let expanded = context.observable(false);
        let tree = WidgetTree::new(Stack::<()>::new().child(expanded.binding().map(|value| {
            if value {
                vec![
                    Element::from(Text::new("first")),
                    Element::from(Text::new("second")),
                ]
            } else {
                vec![Element::from(Text::new("first"))]
            }
        })));

        let mut animations = AnimationEngine::default();
        let compact = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 200.0, 120.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert_eq!(compact.primitives.texts.len(), 1);

        expanded.set(true);
        let expanded_render = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 200.0, 120.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert_eq!(expanded_render.primitives.texts.len(), 2);
    }

    #[test]
    fn hit_testing_tracks_currently_resolved_children() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let context = test_context();
        let visible = context.observable(true);
        let clickable: Element<()> = Stack::new()
            .size(dp(40.0), dp(40.0))
            .style(|mode| {
                container_style(
                    mode,
                    Some(crate::foundation::color::Color::WHITE),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
            })
            .on_click(Command::new(|_: &mut ()| {}))
            .into();
        let tree = WidgetTree::new(Stack::<()>::new().size(dp(100.0), dp(100.0)).child(
            visible.binding().map(move |value| {
                if value {
                    vec![clickable.clone()]
                } else {
                    Vec::<Element<()>>::new()
                }
            }),
        ));

        let mut animations = AnimationEngine::default();
        let hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Some(Point::new(dp(10.0), dp(10.0))),
            None,
        );
        assert!(matches!(hit, Some(super::HitInteraction::Widget { .. })));

        visible.set(false);
        let hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Some(Point::new(dp(10.0), dp(10.0))),
            None,
        );
        assert!(hit.is_none());
    }

    #[derive(Default)]
    struct ScopeChildVm {
        count: i32,
        checked: bool,
        selected_key: String,
        selected_value: String,
        canvas_hits: usize,
        context_hits: usize,
    }

    #[derive(Default)]
    struct ScopeRootVm {
        child: ScopeChildVm,
        other: ScopeChildVm,
        root_count: i32,
    }

    fn scope_child(root: &mut ScopeRootVm) -> &mut ScopeChildVm {
        &mut root.child
    }

    fn scope_other(root: &mut ScopeRootVm) -> &mut ScopeChildVm {
        &mut root.other
    }

    #[test]
    fn scoped_command_targets_child_view_model() {
        let child: Element<ScopeChildVm> = Stack::new()
            .on_click(Command::new(|vm: &mut ScopeChildVm| vm.count += 1))
            .into();
        let root = child.scope(scope_child);

        let command = root.interactions.on_click.expect("scoped command");
        let mut vm = ScopeRootVm::default();
        command.execute(&mut vm);

        assert_eq!(vm.child.count, 1);
        assert_eq!(vm.root_count, 0);
    }

    #[test]
    fn scoped_context_command_receives_child_context() {
        let command = Command::new_with_context(
            |vm: &mut ScopeChildVm, _ctx: &CommandContext<ScopeChildVm>| {
                vm.context_hits += 1;
            },
        )
        .scope(std::sync::Arc::new(scope_child));

        let mut vm = ScopeRootVm::default();
        command.execute(&mut vm);

        assert_eq!(vm.child.context_hits, 1);
    }

    #[test]
    fn checkbox_without_label_measures_to_theme_box_size() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(Checkbox::new(false));
        let expected = UnitContext::default().resolve_dp(
            default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), false).size,
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| { shape.rect.width == expected && shape.rect.height == expected }));
    }

    #[test]
    fn checkbox_label_extends_measure_and_hit_region() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(Checkbox::new(false).label("Accept"));
        let checkbox_style =
            default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), false);
        let size = UnitContext::default().resolve_dp(checkbox_style.size);
        let gap = UnitContext::default().resolve_dp(checkbox_style.label_gap);

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let label = rendered
            .primitives
            .texts
            .iter()
            .find(|text| text.content == "Accept")
            .expect("checkbox label should render");

        assert_eq!(label.frame.x, size + gap);
        assert!(label.frame.y >= Dp::ZERO);
        assert!(label.frame.y <= dp(12.0));
        let hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 40.0),
            Some(Point::new(label.frame.right() - 1.0, label.frame.y + 1.0)),
            None,
        );
        assert!(matches!(hit, Some(super::HitInteraction::Checkbox { .. })));
    }

    #[test]
    fn checked_checkbox_renders_checked_background_and_checkmark() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(Checkbox::new(true));
        let checked_style =
            default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), true);

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == checked_style.background));
        let checkmark = rendered
            .primitives
            .texts
            .iter()
            .find(|text| text.content == super::CHECKBOX_CHECKMARK_ICON)
            .expect("checked checkbox should render checkmark icon");
        assert_eq!(checkmark.color, Color::WHITE);
        assert!(checkmark.force_color);
        assert!(checkmark.font_family.is_some());
        let checkmark_center_x = checkmark.frame.x + checkmark.frame.width / 2.0;
        let checkmark_center_y = checkmark.frame.y + checkmark.frame.height / 2.0;
        assert!((checkmark_center_x - Dp::new(8.0)).abs().get() < 0.01);
        assert!((checkmark_center_y - Dp::new(21.0)).abs().get() < 0.01);
    }

    #[test]
    fn hovered_checkbox_uses_primary_border_without_changing_background() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let checkbox: Element<()> = Checkbox::new(false).into();
        let checkbox_id = checkbox.id;
        let tree: WidgetTree<()> = WidgetTree::new(checkbox);
        let mut states = WidgetStateMap::default();
        states.set(
            checkbox_id,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
        );

        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut AnimationEngine::default(),
            None,
            None,
            &states,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let hovered_style = default_checkbox_style(
            &theme,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
            false,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| { shape.stroke_width == 0.0 && shape.color == hovered_style.background }));
        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| { shape.stroke_width > 0.0 && shape.color == hovered_style.border }));
    }

    #[test]
    fn checkbox_checked_content_switches_without_animation() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let checkbox: Element<()> = Checkbox::new(false).into();
        let checkbox_id = checkbox.id;
        let unchecked_tree: WidgetTree<()> = WidgetTree::new(checkbox.clone());

        unchecked_tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert!(!animations.has_active_animations());

        let mut checked_checkbox: Element<()> = Checkbox::new(true).into();
        checked_checkbox.id = checkbox_id;
        let checked_tree: WidgetTree<()> = WidgetTree::new(checked_checkbox);
        let checked = checked_tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let checked_style =
            default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), true);
        let checked_fill = checked
            .primitives
            .shapes
            .iter()
            .find(|shape| shape.stroke_width == 0.0 && shape.color == checked_style.background)
            .expect("checked fill should render immediately");
        let control_size = UnitContext::default().resolve_dp(checked_style.size);
        assert_eq!(checked_fill.rect.width, control_size);
        assert_eq!(checked_fill.rect.height, control_size);
        assert!(!animations.has_active_animations());

        let unchecked = unchecked_tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert!(unchecked.primitives.shapes.iter().all(|shape| {
            shape.stroke_width == 0.0 && shape.color != checked_style.background
                || shape.stroke_width > 0.0
        }));
        assert!(!animations.has_active_animations());
    }

    #[test]
    fn focused_unchecked_checkbox_keeps_default_box_colors() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let checkbox: Element<()> = Checkbox::new(false).into();
        let checkbox_id = checkbox.id;
        let tree: WidgetTree<()> = WidgetTree::new(checkbox);
        let mut states = WidgetStateMap::default();
        states.set(
            checkbox_id,
            crate::ui::theme::WidgetState {
                focused: true,
                ..Default::default()
            },
        );
        let default_style =
            default_checkbox_style(&theme, crate::ui::theme::WidgetState::default(), false);

        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut AnimationEngine::default(),
            None,
            None,
            &states,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.stroke_width == 0.0 && shape.color == default_style.background));
        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.stroke_width > 0.0 && shape.color == default_style.border));
        assert!(rendered
            .primitives
            .overlay_shapes
            .iter()
            .any(|shape| shape.stroke_width == theme.focus_ring.width.get()
                && shape.color == theme.focus_ring.color
                && shape.rect.width > dp(16.0)));
        assert!(rendered
            .primitives
            .texts
            .iter()
            .all(|text| text.content != super::CHECKBOX_CHECKMARK_ICON));
    }

    #[test]
    fn disabled_checkbox_exposes_disabled_hit_for_cursor_only() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(Checkbox::new(false).disable(true));

        let hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            Some(Point::new(4.0, 4.0)),
            None,
        );

        assert!(matches!(hit, Some(super::HitInteraction::Disabled { .. })));
    }

    #[test]
    fn radio_without_label_measures_to_theme_control_size() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(Radio::new(false));
        let expected = UnitContext::default().resolve_dp(
            default_radio_style(&theme, crate::ui::theme::WidgetState::default(), false).size,
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| { shape.rect.width == expected && shape.rect.height == expected }));
    }

    #[test]
    fn radio_label_extends_measure_and_hit_region() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(Radio::new(false).label("Email"));
        let radio_style =
            default_radio_style(&theme, crate::ui::theme::WidgetState::default(), false);
        let size = UnitContext::default().resolve_dp(radio_style.size);
        let gap = UnitContext::default().resolve_dp(radio_style.label_gap);

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let label = rendered
            .primitives
            .texts
            .iter()
            .find(|text| text.content == "Email")
            .expect("radio label should render");

        assert_eq!(label.frame.x, size + gap);
        let hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 40.0),
            Some(Point::new(label.frame.right() - 1.0, label.frame.y + 1.0)),
            None,
        );
        assert!(matches!(hit, Some(super::HitInteraction::Radio { .. })));
    }

    #[test]
    fn checked_radio_renders_indicator() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(Radio::new(true));
        let checked_style =
            default_radio_style(&theme, crate::ui::theme::WidgetState::default(), true);

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered
            .primitives
            .overlay_shapes
            .iter()
            .any(|shape| shape.color == checked_style.indicator));
    }

    #[test]
    fn disabled_radio_exposes_disabled_hit_for_cursor_only() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(Radio::new(false).disable(true));

        let hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            Some(Point::new(4.0, 4.0)),
            None,
        );

        assert!(matches!(hit, Some(super::HitInteraction::Disabled { .. })));
    }

    #[test]
    fn radio_group_renders_selected_option_and_dispatches_key_value() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<ScopeChildVm> = WidgetTree::new(
            RadioGroup::new(
                vec![
                    ("email".to_string(), "Email".to_string()),
                    ("sms".to_string(), "SMS".to_string()),
                ],
                "email".to_string(),
            )
            .on_change(ValueCommand::new(
                |vm: &mut ScopeChildVm, (key, value): (String, String)| {
                    vm.selected_key = key;
                    vm.selected_value = value;
                },
            )),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 180.0, 80.0),
            None,
            None,
            None,
            None,
            false,
        );
        let indicator =
            default_radio_style(&theme, crate::ui::theme::WidgetState::default(), true).indicator;
        assert_eq!(
            rendered
                .primitives
                .overlay_shapes
                .iter()
                .filter(|shape| shape.color == indicator)
                .count(),
            1
        );

        let hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 180.0, 80.0),
            Some(Point::new(4.0, 30.0)),
            None,
        );
        let mut vm = ScopeChildVm::default();
        match hit {
            Some(super::HitInteraction::Radio {
                on_change: Some(command),
                current,
                ..
            }) => {
                assert!(!current);
                command.execute(&mut vm, true);
            }
            _ => panic!("second radio should be hit"),
        }

        assert_eq!(vm.selected_key, "sms");
        assert_eq!(vm.selected_value, "SMS");
    }

    #[test]
    fn radio_group_ignores_false_child_change_and_maps_direction() {
        let group: Element<ScopeChildVm> = RadioGroup::new(
            vec![
                ("email".to_string(), "Email".to_string()),
                ("sms".to_string(), "SMS".to_string()),
            ],
            "email".to_string(),
        )
        .horizontal()
        .on_change(ValueCommand::new(
            |vm: &mut ScopeChildVm, (key, value): (String, String)| {
                vm.selected_key = key;
                vm.selected_value = value;
            },
        ))
        .into();

        match &group.kind {
            WidgetKind::Container { layout, .. } => match &layout.kind {
                ContainerKind::Flex { direction, .. } => {
                    assert_eq!(*direction, Axis::Horizontal);
                }
                _ => panic!("radio group should render as flex"),
            },
            _ => panic!("radio group should render as container"),
        }

        let tree = WidgetTree::new(group);
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 180.0, 40.0),
            Some(Point::new(4.0, 4.0)),
            None,
        );
        let mut vm = ScopeChildVm::default();
        match hit {
            Some(super::HitInteraction::Radio {
                on_change: Some(command),
                current,
                ..
            }) => {
                assert!(current);
                command.execute(&mut vm, false);
            }
            _ => panic!("first radio should be hit"),
        }

        assert!(vm.selected_key.is_empty());
        assert!(vm.selected_value.is_empty());
    }

    #[test]
    fn radio_group_disabled_option_exposes_disabled_hit_for_cursor_only() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<ScopeChildVm> = WidgetTree::new(
            RadioGroup::new(
                vec![
                    RadioOption::new("email".to_string(), "Email".to_string()),
                    RadioOption::new("sms".to_string(), "SMS".to_string()).disable(true),
                ],
                "email".to_string(),
            )
            .on_change(ValueCommand::new(
                |vm: &mut ScopeChildVm, (key, value): (String, String)| {
                    vm.selected_key = key;
                    vm.selected_value = value;
                },
            )),
        );

        let disabled_hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 180.0, 80.0),
            Some(Point::new(4.0, 30.0)),
            None,
        );
        assert!(matches!(
            disabled_hit,
            Some(super::HitInteraction::Disabled { .. })
        ));

        let enabled_hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 180.0, 80.0),
            Some(Point::new(4.0, 4.0)),
            None,
        );
        assert!(matches!(
            enabled_hit,
            Some(super::HitInteraction::Radio { .. })
        ));
    }

    #[test]
    fn select_renders_placeholder_and_arrow_when_unselected() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            Select::<(), String, String>::new(
                vec![SelectOption::new("email".to_string(), "Email".to_string())],
                None::<String>,
            )
            .placeholder("Choose one")
            .size(dp(180.0), dp(40.0)),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 180.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered
            .primitives
            .texts
            .iter()
            .any(|text| text.content == "Choose one"));
        assert!(rendered
            .primitives
            .texts
            .iter()
            .any(|text| text.content == SELECT_ARROW_ICON));
    }

    #[test]
    fn disabled_select_exposes_disabled_hit_for_cursor_only() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            Select::<(), String, String>::new(
                vec![SelectOption::new("email".to_string(), "Email".to_string())],
                None::<String>,
            )
            .disable(true)
            .size(dp(180.0), dp(40.0)),
        );

        let hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 180.0, 40.0),
            Some(Point::new(10.0, 10.0)),
            None,
        );
        assert!(matches!(hit, Some(super::HitInteraction::Disabled { .. })));
    }

    #[test]
    fn focused_select_opens_upward_and_hits_enabled_and_disabled_options() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let select: Element<ScopeChildVm> = Select::new(
            vec![
                SelectOption::new("email".to_string(), "Email".to_string()),
                SelectOption::new("sms".to_string(), "SMS".to_string()).disable(true),
                SelectOption::new("phone".to_string(), "Phone".to_string()),
            ],
            Some("email".to_string()),
        )
        .on_change(ValueCommand::new(
            |vm: &mut ScopeChildVm, (key, value): (String, String)| {
                vm.selected_key = key;
                vm.selected_value = value;
            },
        ))
        .open(true)
        .size(dp(180.0), dp(40.0))
        .position_absolute()
        .top(dp(50.0))
        .into();
        let tree = WidgetTree::new(Stack::new().child(select));
        let widget_states = WidgetStateMap::default();

        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &widget_states,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 220.0, 90.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert!(rendered
            .primitives
            .overlay_shapes
            .iter()
            .any(|shape| shape.rect.y < dp(50.0) && shape.rect.height > dp(40.0)));

        let enabled_hit = tree.hit_test_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &widget_states,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 220.0, 90.0),
            Some(Point::new(8.0, 10.0)),
            None,
        );
        let mut vm = ScopeChildVm::default();
        match enabled_hit {
            Some(super::HitInteraction::SelectOption {
                on_select: Some(command),
                ..
            }) => command.execute(&mut vm),
            _ => panic!("enabled select option should be hit"),
        }
        assert_eq!(vm.selected_key, "email");
        assert_eq!(vm.selected_value, "Email");

        let disabled_hit = tree.hit_test_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &widget_states,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 220.0, 90.0),
            Some(Point::new(8.0, 45.0)),
            None,
        );
        assert!(matches!(
            disabled_hit,
            Some(super::HitInteraction::Disabled { .. })
        ));
    }

    #[test]
    fn select_dropdown_escapes_parent_overflow_clip() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let select: Element<ScopeChildVm> = Select::new(
            vec![
                SelectOption::new("email".to_string(), "Email".to_string()),
                SelectOption::new("sms".to_string(), "SMS".to_string()),
            ],
            None::<String>,
        )
        .placeholder("Choose")
        .on_change(ValueCommand::new(
            |vm: &mut ScopeChildVm, (key, value): (String, String)| {
                vm.selected_key = key;
                vm.selected_value = value;
            },
        ))
        .open(true)
        .size(dp(180.0), dp(40.0))
        .into();
        let tree = WidgetTree::new(
            Stack::new()
                .size(dp(180.0), dp(45.0))
                .overflow(Overflow::Hidden)
                .child(select),
        );
        let widget_states = WidgetStateMap::default();

        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &widget_states,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 180.0, 140.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert!(rendered
            .primitives
            .overlay_shapes
            .iter()
            .any(|shape| shape.rect.y > dp(40.0) && shape.rect.bottom() > dp(45.0)));

        let hit = tree.hit_test_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &widget_states,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 180.0, 140.0),
            Some(Point::new(8.0, 58.0)),
            None,
        );
        let mut vm = ScopeChildVm::default();
        match hit {
            Some(super::HitInteraction::SelectOption {
                on_select: Some(command),
                ..
            }) => command.execute(&mut vm),
            _ => panic!("select option outside parent clip should be hit"),
        }
        assert_eq!(vm.selected_key, "email");
    }

    #[test]
    fn select_dropdown_stays_above_later_media_placeholder() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let select: Element<ScopeChildVm> = Select::new(
            vec![
                SelectOption::new("email".to_string(), "Email".to_string()),
                SelectOption::new("sms".to_string(), "SMS".to_string()),
            ],
            None::<String>,
        )
        .open(true)
        .size(dp(180.0), dp(40.0))
        .into();
        let image_frame = Rect::new(0.0, 40.0, 180.0, 40.0);
        let tree = WidgetTree::new(
            crate::ui::widget::Flex::new(Axis::Vertical)
                .gap(dp(0.0))
                .child([
                    select,
                    Image::from_bytes(vec![0_u8; 4])
                        .size(dp(180.0), dp(40.0))
                        .into(),
                ]),
        );
        let widget_states = WidgetStateMap::default();

        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &widget_states,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 180.0, 140.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(
            rendered
                .primitives
                .overlay_shapes
                .iter()
                .all(|shape| shape.rect != image_frame),
            "media placeholders should not render in the overlay layer"
        );
        assert!(
            rendered
                .primitives
                .shapes
                .iter()
                .any(|shape| shape.rect == image_frame),
            "media placeholder should still render in the normal scene"
        );
    }

    #[test]
    fn select_dropdown_highlights_hovered_option() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let select: Element<ScopeChildVm> = Select::new(
            vec![
                SelectOption::new("email".to_string(), "Email".to_string()),
                SelectOption::new("sms".to_string(), "SMS".to_string()),
            ],
            None::<String>,
        )
        .open(true)
        .size(dp(180.0), dp(32.0))
        .into();
        let select_id = select.id;
        let tree = WidgetTree::new(Stack::new().child(select));
        let mut widget_states = WidgetStateMap::default();
        widget_states.set_select_option(
            select_id,
            1,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
        );

        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &widget_states,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 180.0, 140.0),
            None,
            None,
            None,
            None,
            false,
        );
        let hovered_options = rendered
            .primitives
            .overlay_shapes
            .iter()
            .filter(|shape| {
                shape.rect.y > dp(60.0)
                    && shape.rect.height
                        == UnitContext::default().resolve_dp(
                            default_select_style(&theme, crate::ui::theme::WidgetState::default())
                                .option_height,
                        )
                    && shape.color.a > 0
            })
            .collect::<Vec<_>>();

        assert_eq!(hovered_options.len(), 1);
    }

    #[test]
    fn select_dropdown_hover_highlight_preserves_menu_corner_clip() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let select: Element<ScopeChildVm> = Select::new(
            vec![
                SelectOption::new("email".to_string(), "Email".to_string()),
                SelectOption::new("sms".to_string(), "SMS".to_string()),
            ],
            None::<String>,
        )
        .open(true)
        .size(dp(180.0), dp(32.0))
        .into();
        let select_id = select.id;
        let tree = WidgetTree::new(Stack::new().child(select));
        let mut widget_states = WidgetStateMap::default();
        widget_states.set_select_option(
            select_id,
            0,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
        );

        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &widget_states,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 180.0, 140.0),
            None,
            None,
            None,
            None,
            false,
        );
        let select_style =
            default_select_style(&theme, crate::ui::theme::WidgetState::default());
        let option_height = UnitContext::default().resolve_dp(select_style.option_height);
        let menu_radius = select_style.radius.get();
        let highlight = rendered
            .primitives
            .overlay_shapes
            .iter()
            .find(|shape| shape.rect.y > dp(20.0) && shape.rect.height == option_height)
            .expect("hovered option highlight should render");

        assert_eq!(
            highlight.clip_mask,
            Some(ClipMask {
                rect: Rect::new(
                    highlight.rect.x,
                    highlight.rect.y,
                    highlight.rect.width,
                    option_height * 2.0,
                ),
                corner_radius: menu_radius,
            })
        );
    }

    #[test]
    fn scoped_value_commands_cover_switch_canvas_and_media() {
        let mut vm = ScopeRootVm::default();
        let switch: Element<ScopeChildVm> = Switch::new(false)
            .on_change(ValueCommand::new(|vm: &mut ScopeChildVm, value| {
                vm.checked = value;
            }))
            .into();
        let switch = switch.scope(scope_child);
        match switch.kind {
            WidgetKind::Switch {
                on_change: Some(command),
                ..
            } => command.execute(&mut vm, true),
            _ => panic!("switch command should be scoped"),
        }
        assert!(vm.child.checked);

        vm.child.checked = false;
        let checkbox: Element<ScopeChildVm> = Checkbox::new(false)
            .on_change(ValueCommand::new(|vm: &mut ScopeChildVm, value| {
                vm.checked = value;
            }))
            .into();
        let checkbox = checkbox.scope(scope_child);
        match checkbox.kind {
            WidgetKind::Checkbox {
                on_change: Some(command),
                ..
            } => command.execute(&mut vm, true),
            _ => panic!("checkbox command should be scoped"),
        }
        assert!(vm.child.checked);

        vm.child.checked = false;
        let radio: Element<ScopeChildVm> = Radio::new(false)
            .on_change(ValueCommand::new(|vm: &mut ScopeChildVm, value| {
                vm.checked = value;
            }))
            .into();
        let radio = radio.scope(scope_child);
        match radio.kind {
            WidgetKind::Radio {
                on_change: Some(command),
                ..
            } => command.execute(&mut vm, true),
            _ => panic!("radio command should be scoped"),
        }
        assert!(vm.child.checked);

        let canvas: Element<ScopeChildVm> = Canvas::new(Vec::<CanvasItem>::new())
            .on_item_click(ValueCommand::new(|vm: &mut ScopeChildVm, _event| {
                vm.canvas_hits += 1;
            }))
            .into();
        let canvas = canvas.scope(scope_child);
        match canvas.kind {
            WidgetKind::Canvas {
                item_interactions, ..
            } => item_interactions
                .on_click
                .expect("canvas item command")
                .execute(
                    &mut vm,
                    crate::ui::widget::CanvasPointerEvent {
                        item_id: 1_u64.into(),
                        button: None,
                        canvas_position: Point::ZERO,
                        scene_position: Point::ZERO,
                        local_position: Point::ZERO,
                    },
                ),
            _ => panic!("canvas command should be scoped"),
        }
        assert_eq!(vm.child.canvas_hits, 1);

        let image = Image::from_path("missing-test-image.png")
            .on_loading(Command::new(|vm: &mut ScopeChildVm| vm.count += 10))
            .scope(scope_child);
        let media_command = image.media_events.on_loading.expect("media command");
        media_command.execute(&mut vm);
        assert_eq!(vm.child.count, 10);
    }

    #[test]
    fn scoped_dynamic_children_resolve_to_root_commands() {
        let context = test_context();
        let show = context.observable(true);
        let child_a: Element<ScopeChildVm> = Stack::new()
            .on_click(Command::new(|vm: &mut ScopeChildVm| vm.count += 1))
            .into();
        let child_b: Element<ScopeChildVm> = Stack::new()
            .on_click(Command::new(|vm: &mut ScopeChildVm| vm.count += 10))
            .into();

        let tree = WidgetTree::new(Stack::<ScopeRootVm>::new().child(show.binding().map(
            move |visible| {
                if visible {
                    vec![child_a.clone().scope(scope_child)]
                } else {
                    vec![child_b.clone().scope(scope_other)]
                }
            },
        )));

        let resolved = match &tree.root.kind {
            WidgetKind::Container { children, .. } => children[0].resolve(),
            _ => panic!("root should be a container"),
        };

        let command = resolved[0]
            .interactions
            .on_click
            .clone()
            .expect("dynamic scoped command");
        let mut vm = ScopeRootVm::default();
        command.execute(&mut vm);
        assert_eq!(vm.child.count, 1);
        assert_eq!(vm.other.count, 0);

        show.set(false);
        let resolved = match &tree.root.kind {
            WidgetKind::Container { children, .. } => children[0].resolve(),
            _ => panic!("root should be a container"),
        };
        let command = resolved[0]
            .interactions
            .on_click
            .clone()
            .expect("dynamic scoped command");
        command.execute(&mut vm);
        assert_eq!(vm.child.count, 1);
        assert_eq!(vm.other.count, 10);
    }

    #[cfg(feature = "video")]
    #[test]
    fn video_surface_renders_placeholder_without_frame() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let controller = test_video_controller(crate::video::VideoSurfaceSnapshot {
            intrinsic_size: crate::media::IntrinsicSize::from_pixels(16, 9),
            texture: None,
            loading: true,
            error: None,
        });
        let tree: WidgetTree<()> =
            WidgetTree::new(VideoSurface::new(controller).size(dp(160.0), dp(90.0)));

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 90.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered.primitives.textures.is_empty());
        assert!(rendered
            .primitives
            .texts
            .iter()
            .any(|text| text.content.contains("loading video")));
    }

    #[cfg(feature = "video")]
    #[test]
    fn video_surface_idle_placeholder_uses_surface_background() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let background = Color::hexa(0x123456FF);
        let radius = dp(12.0);
        let controller = test_video_controller(crate::video::VideoSurfaceSnapshot {
            intrinsic_size: crate::media::IntrinsicSize::ZERO,
            texture: None,
            loading: false,
            error: None,
        });
        let tree: WidgetTree<()> = WidgetTree::new(
            VideoSurface::new(controller)
                .size(dp(160.0), dp(90.0))
                .style(move |mode| {
                    let mut style = VideoSurfaceStyle::default_for(mode);
                    style.surface.background = Some(background.into());
                    style.surface.border_radius = Some(radius.into());
                    style
                }),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 90.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered.primitives.textures.is_empty());
        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == background && shape.corner_radius == radius.get()));
        assert!(rendered
            .primitives
            .texts
            .iter()
            .any(|text| text.content.contains("video unavailable")));
    }

    #[cfg(feature = "video")]
    #[test]
    fn video_surface_renders_texture_when_frame_exists() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let texture = std::sync::Arc::new(crate::media::TextureFrame::new(
            32,
            18,
            vec![255; 32 * 18 * 4],
        ));
        let controller = test_video_controller(crate::video::VideoSurfaceSnapshot {
            intrinsic_size: crate::media::IntrinsicSize::from_pixels(32, 18),
            texture: Some(texture),
            loading: false,
            error: None,
        });
        let tree: WidgetTree<()> = WidgetTree::new(
            VideoSurface::new(controller)
                .width(dp(160.0))
                .aspect_ratio(32.0 / 18.0),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 90.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert_eq!(rendered.primitives.textures.len(), 1);
        assert_eq!(rendered.primitives.textures[0].frame.width, 160.0);
        assert_eq!(rendered.primitives.textures[0].frame.height, 90.0);
    }

    #[test]
    fn binding_driven_children_can_switch_component_types() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let context = test_context();
        let show_button = context.observable(false);
        let tree = WidgetTree::new(Stack::<()>::new().child(show_button.binding().map(|value| {
            if value {
                vec![super::Element::from(crate::ui::widget::Button::new(
                    "toggle button",
                ))]
            } else {
                vec![Element::from(Text::new("toggle text"))]
            }
        })));

        let mut animations = AnimationEngine::default();
        let text_render = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 220.0, 120.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert_eq!(text_render.primitives.shapes.len(), 0);

        show_button.set(true);
        let button_render = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 220.0, 120.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert!(!button_render.primitives.shapes.is_empty());
    }

    #[test]
    fn button_label_is_horizontally_centered_but_text_is_not() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();

        let text_tree: WidgetTree<()> = WidgetTree::new(
            Text::new("Center")
                .padding(Insets::all(dp(16.0)))
                .size(dp(160.0), dp(48.0)),
        );
        let text_render = text_tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 48.0),
            None,
            None,
            None,
            None,
            false,
        );

        let button_tree: WidgetTree<()> =
            WidgetTree::new(crate::ui::widget::Button::new("Center").size(dp(160.0), dp(48.0)));
        let button_render = button_tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 48.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert_eq!(text_render.primitives.texts.len(), 1);
        assert_eq!(button_render.primitives.texts.len(), 1);
        assert!(
            button_render.primitives.texts[0].frame.x > text_render.primitives.texts[0].frame.x
        );
    }

    #[test]
    fn disabled_button_exposes_disabled_hit_for_cursor_only() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            crate::ui::widget::Button::new("disabled")
                .disable(true)
                .size(dp(120.0), dp(40.0)),
        );

        let hit = tree.hit_test(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            Some(Point::new(dp(10.0), dp(10.0))),
            None,
        );
        assert!(matches!(hit, Some(super::HitInteraction::Disabled { .. })));
    }

    #[test]
    fn button_uses_theme_radius_by_default() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> =
            WidgetTree::new(crate::ui::widget::Button::new("radius").size(dp(120.0), dp(40.0)));
        let default_style = default_button_style(
            &theme,
            crate::ui::theme::WidgetState::default(),
            crate::ui::widget::common::ButtonVariantKind::Primary,
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.corner_radius == default_style.radius.get()));
        assert!(!rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.stroke_width > 0.0 && shape.color == default_style.border_color));
    }

    #[test]
    fn primary_button_uses_hover_background_when_hovered() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let button: Element<()> = crate::ui::widget::Button::new("hover")
            .size(dp(120.0), dp(40.0))
            .into();
        let button_id = button.id;
        let tree: WidgetTree<()> = WidgetTree::new(button);
        let mut hovered_state = WidgetStateMap::default();
        hovered_state.set(
            button_id,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
        );

        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &hovered_state,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let hovered_style = default_button_style(
            &theme,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
            crate::ui::widget::common::ButtonVariantKind::Primary,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.stroke_width == 0.0 && shape.color == hovered_style.background));
    }

    #[test]
    fn primary_button_hover_background_uses_transition() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let button: Element<()> = crate::ui::widget::Button::new("hover")
            .size(dp(120.0), dp(40.0))
            .into();
        let button_id = button.id;
        let tree: WidgetTree<()> = WidgetTree::new(button);
        let mut hovered_state = WidgetStateMap::default();
        hovered_state.set(
            button_id,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
        );

        let normal = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let start_background = normal
            .primitives
            .shapes
            .iter()
            .find(|shape| shape.stroke_width == 0.0)
            .expect("button should render a filled background")
            .color;

        let hovered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &hovered_state,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let immediate_background = hovered
            .primitives
            .shapes
            .iter()
            .find(|shape| shape.stroke_width == 0.0)
            .expect("hovered button should render a filled background")
            .color;

        std::thread::sleep(std::time::Duration::from_millis(100));
        let mid = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &hovered_state,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let mid_background = mid
            .primitives
            .shapes
            .iter()
            .find(|shape| shape.stroke_width == 0.0)
            .expect("hovered button should keep a filled background")
            .color;

        std::thread::sleep(std::time::Duration::from_millis(140));
        let settled = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &hovered_state,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let settled_background = settled
            .primitives
            .shapes
            .iter()
            .find(|shape| shape.stroke_width == 0.0)
            .expect("hovered button should render a filled background after transition")
            .color;
        let start_style = default_button_style(
            &theme,
            crate::ui::theme::WidgetState::default(),
            crate::ui::widget::common::ButtonVariantKind::Primary,
        );
        let hovered_style = default_button_style(
            &theme,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
            crate::ui::widget::common::ButtonVariantKind::Primary,
        );

        assert_eq!(start_background, start_style.background);
        assert_eq!(immediate_background, start_background);
        assert_ne!(mid_background, start_background);
        assert_ne!(mid_background, hovered_style.background);
        assert_eq!(settled_background, hovered_style.background);
    }

    #[test]
    fn pressed_button_background_takes_priority_over_focus_fill() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let button: Element<()> = crate::ui::widget::Button::new("focus")
            .size(dp(120.0), dp(40.0))
            .into();
        let button_id = button.id;
        let tree: WidgetTree<()> = WidgetTree::new(button);
        let mut state = WidgetStateMap::default();
        state.set(
            button_id,
            crate::ui::theme::WidgetState {
                hovered: true,
                pressed: true,
                focused: true,
                ..Default::default()
            },
        );

        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &state,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let pressed_style = default_button_style(
            &theme,
            crate::ui::theme::WidgetState {
                hovered: true,
                pressed: true,
                focused: true,
                ..Default::default()
            },
            crate::ui::widget::common::ButtonVariantKind::Primary,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.stroke_width == 0.0 && shape.color == pressed_style.background));
    }

    #[test]
    fn focused_secondary_button_keeps_default_border() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let button: Element<()> = crate::ui::widget::Button::new("focus")
            .secondary()
            .size(dp(120.0), dp(40.0))
            .into();
        let button_id = button.id;
        let tree: WidgetTree<()> = WidgetTree::new(button);
        let mut state = WidgetStateMap::default();
        state.set(
            button_id,
            crate::ui::theme::WidgetState {
                hovered: true,
                pressed: true,
                focused: true,
                ..Default::default()
            },
        );

        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &state,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let focused_style = default_button_style(
            &theme,
            crate::ui::theme::WidgetState {
                hovered: true,
                pressed: true,
                focused: true,
                ..Default::default()
            },
            crate::ui::widget::common::ButtonVariantKind::Secondary,
        );
        let default_style = default_button_style(
            &theme,
            Default::default(),
            crate::ui::widget::common::ButtonVariantKind::Secondary,
        );
        let hovered_pressed_style = default_button_style(
            &theme,
            crate::ui::theme::WidgetState {
                hovered: true,
                pressed: true,
                ..Default::default()
            },
            crate::ui::widget::common::ButtonVariantKind::Secondary,
        );

        assert_eq!(focused_style.border_color, hovered_pressed_style.border_color);

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.stroke_width > 0.0 && shape.color == hovered_pressed_style.border_color));
        assert!(rendered
            .primitives
            .overlay_shapes
            .iter()
            .any(|shape| shape.stroke_width == theme.focus_ring.width.get()
                && shape.color == theme.focus_ring.color
                && shape.rect.width > dp(120.0)));
        assert_eq!(default_style.border_color, default_style.border_color);
    }

    #[test]
    fn focused_ghost_button_keeps_default_visuals() {
        let theme = Theme::default();
        let focused_style = default_button_style(
            &theme,
            crate::ui::theme::WidgetState {
                focused: true,
                ..Default::default()
            },
            crate::ui::widget::common::ButtonVariantKind::Ghost,
        );
        let default_style = default_button_style(
            &theme,
            Default::default(),
            crate::ui::widget::common::ButtonVariantKind::Ghost,
        );

        assert_eq!(focused_style.background, default_style.background);
        assert_eq!(focused_style.border_color, default_style.border_color);
    }

    #[test]
    fn secondary_button_uses_theme_border_by_default() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            crate::ui::widget::Button::new("secondary")
                .secondary()
                .size(dp(120.0), dp(40.0)),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let default_style = default_button_style(
            &theme,
            crate::ui::theme::WidgetState::default(),
            crate::ui::widget::common::ButtonVariantKind::Secondary,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == default_style.border_color
                && shape.stroke_width == default_style.border_width.get()));
    }

    #[test]
    fn danger_button_has_no_default_border() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            crate::ui::widget::Button::new("danger")
                .danger()
                .size(dp(120.0), dp(40.0)),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let default_style = default_button_style(
            &theme,
            crate::ui::theme::WidgetState::default(),
            crate::ui::widget::common::ButtonVariantKind::Danger,
        );

        assert!(!rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.stroke_width > 0.0 && shape.color == default_style.border_color));
    }

    #[test]
    fn explicit_button_transparent_border_overrides_theme_border() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            crate::ui::widget::Button::new("border")
                .style(|mode| button_style(mode, None, Some(dp(0.0)), Some(Color::TRANSPARENT)))
                .size(dp(120.0), dp(40.0)),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let default_style = default_button_style(
            &theme,
            crate::ui::theme::WidgetState::default(),
            crate::ui::widget::common::ButtonVariantKind::Primary,
        );

        assert!(!rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.stroke_width > 0.0 && shape.color == default_style.border_color));
    }

    #[test]
    fn explicit_button_radius_overrides_theme_radius() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            crate::ui::widget::Button::new("radius")
                .style(|mode| button_style(mode, Some(dp(12.0)), None, None))
                .size(dp(120.0), dp(40.0)),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.corner_radius == 12.0));
    }

    #[test]
    fn switch_renders_custom_track_and_thumb_colors() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let active_background = Color::hexa(0x10B981FF);
        let inactive_background = Color::hexa(0x475569FF);
        let active_thumb = Color::hexa(0xECFDF5FF);
        let tree: WidgetTree<()> = WidgetTree::new(
            Switch::new(true)
                .size(dp(52.0), dp(30.0))
                .style(move |mode| {
                    switch_style(
                        mode,
                        active_background,
                        inactive_background,
                        Some(active_thumb),
                        None,
                    )
                }),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == active_background));
        assert!(rendered
            .primitives
            .overlay_shapes
            .iter()
            .any(|shape| shape.color == active_thumb));

        let inactive_tree: WidgetTree<()> = WidgetTree::new(
            Switch::new(false)
                .size(dp(52.0), dp(30.0))
                .style(move |mode| {
                    switch_style(
                        mode,
                        active_background,
                        inactive_background,
                        None,
                        Some(Color::WHITE),
                    )
                }),
        );
        let inactive_render = inactive_tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut AnimationEngine::default(),
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert!(inactive_render
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == inactive_background));
    }

    #[test]
    fn switch_uses_theme_defaults_when_styles_are_not_explicitly_set() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(Switch::new(false));
        let default_style = default_switch_style(&theme);
        let default_radius = default_style.radius.resolve().get();
        let default_track = super::resolve_stateful_widget_color(
            &default_style.track,
            crate::ui::theme::WidgetState::default(),
        );
        let default_thumb = super::resolve_stateful_widget_color(
            &default_style.thumb,
            crate::ui::theme::WidgetState::default(),
        );
        let default_border = super::resolve_stateful_widget_color(
            &default_style.border,
            crate::ui::theme::WidgetState::default(),
        );
        let default_border_width = default_style.border_width.resolve().get();

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == default_track));
        assert!(rendered
            .primitives
            .overlay_shapes
            .iter()
            .any(|shape| shape.color == default_thumb));
        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == default_track && shape.corner_radius == default_radius));
        assert!(rendered.primitives.shapes.iter().any(
            |shape| shape.color == default_border && shape.stroke_width == default_border_width
        ));

        let checked_tree: WidgetTree<()> = WidgetTree::new(Switch::new(true));
        let checked_track = super::resolve_stateful_widget_color(
            &default_style.track_checked,
            crate::ui::theme::WidgetState::default(),
        );
        let checked_thumb = super::resolve_stateful_widget_color(
            &default_style.thumb_checked,
            crate::ui::theme::WidgetState::default(),
        );
        let checked_rendered = checked_tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut AnimationEngine::default(),
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(checked_rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == checked_track && shape.corner_radius == default_radius));
        assert!(checked_rendered
            .primitives
            .overlay_shapes
            .iter()
            .any(|shape| shape.color == checked_thumb));

        let hovered_switch: Element<()> = Switch::new(true).into();
        let hovered_switch_id = hovered_switch.id;
        let hovered_tree: WidgetTree<()> = WidgetTree::new(hovered_switch);
        let mut hovered_state = WidgetStateMap::default();
        hovered_state.set(
            hovered_switch_id,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
        );
        let hovered_rendered = hovered_tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut AnimationEngine::default(),
            None,
            None,
            &hovered_state,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let hovered_checked_thumb = super::resolve_stateful_widget_color(
            &default_style.thumb_checked,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
        );
        let hovered_checked_track = super::resolve_stateful_widget_color(
            &default_style.track_checked,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
        );
        assert!(hovered_rendered
            .primitives
            .overlay_shapes
            .iter()
            .any(|shape| shape.color == hovered_checked_thumb));
        assert!(hovered_rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == hovered_checked_track));
    }

    #[test]
    fn checked_switch_thumb_uses_white_across_hover_states() {
        let theme = Theme::dark();

        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();

        let tree: WidgetTree<()> = WidgetTree::new(Switch::new(true));
        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut AnimationEngine::default(),
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert!(rendered
            .primitives
            .overlay_shapes
            .iter()
            .any(|shape| shape.color == Color::WHITE));

        let hovered_switch: Element<()> = Switch::new(true).into();
        let hovered_switch_id = hovered_switch.id;
        let hovered_tree: WidgetTree<()> = WidgetTree::new(hovered_switch);
        let mut hovered_state = WidgetStateMap::default();
        hovered_state.set(
            hovered_switch_id,
            crate::ui::theme::WidgetState {
                hovered: true,
                ..Default::default()
            },
        );
        let hovered_rendered = hovered_tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut AnimationEngine::default(),
            None,
            None,
            &hovered_state,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert!(hovered_rendered
            .primitives
            .overlay_shapes
            .iter()
            .any(|shape| shape.color == Color::WHITE));
    }

    #[test]
    fn focused_switch_keeps_pressed_colors_and_renders_focus_ring() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let switch: Element<()> = Switch::new(true).into();
        let switch_id = switch.id;
        let tree: WidgetTree<()> = WidgetTree::new(switch);
        let mut state = WidgetStateMap::default();
        state.set(
            switch_id,
            crate::ui::theme::WidgetState {
                hovered: true,
                pressed: true,
                focused: true,
                ..Default::default()
            },
        );

        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut AnimationEngine::default(),
            None,
            None,
            &state,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 80.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let switch_style = default_switch_style(&theme);
        let base_state = crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            ..Default::default()
        };
        let focused_state = crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            focused: true,
            ..Default::default()
        };

        assert!(rendered.primitives.shapes.iter().any(|shape| shape.color
            == super::resolve_stateful_widget_color(&switch_style.track_checked, base_state)));
        assert_eq!(
            super::resolve_stateful_widget_color(&switch_style.track_checked, focused_state),
            super::resolve_stateful_widget_color(&switch_style.track_checked, base_state)
        );
        assert_eq!(
            super::resolve_stateful_widget_color(&switch_style.border_checked, focused_state),
            super::resolve_stateful_widget_color(&switch_style.border_checked, base_state)
        );
        assert!(rendered
            .primitives
            .overlay_shapes
            .iter()
            .any(|shape| shape.stroke_width == theme.focus_ring.width.get()
                && shape.color == theme.focus_ring.color
                && shape.rect.width > dp(42.0)));
    }

    #[test]
    fn button_focus_ring_override_changes_overlay_without_affecting_layout() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let button: Element<()> = crate::ui::widget::Button::new("focus")
            .style(|mode| {
                let mut style = ButtonStyle::default_for(
                    mode,
                    crate::ui::widget::common::ButtonVariantKind::Primary,
                );
                style.focus_ring = Some(crate::ui::widget::FocusRingOverride {
                    color: Some(Color::hexa(0x22C55EFF)),
                    width: Some(dp(3.0)),
                    gap: Some(dp(4.0)),
                    enabled: Some(true),
                });
                style
            })
            .size(dp(120.0), dp(40.0))
            .into();
        let button_id = button.id;
        let tree: WidgetTree<()> = WidgetTree::new(button);
        let mut state = WidgetStateMap::default();
        state.set(
            button_id,
            crate::ui::theme::WidgetState {
                focused: true,
                ..Default::default()
            },
        );

        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &state,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered
            .primitives
            .overlay_shapes
            .iter()
            .any(|shape| shape.stroke_width == 3.0
                && shape.color == Color::hexa(0x22C55EFF)
                && shape.rect.width > dp(120.0)
                && shape.rect.height > dp(40.0)));
    }

    #[test]
    fn focus_ring_overlay_is_not_clipped() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let button: Element<()> = crate::ui::widget::Button::new("focus")
            .size(dp(120.0), dp(40.0))
            .into();
        let button_id = button.id;
        let tree: WidgetTree<()> = WidgetTree::new(button);
        let mut state = WidgetStateMap::default();
        state.set(
            button_id,
            crate::ui::theme::WidgetState {
                focused: true,
                ..Default::default()
            },
        );

        let rendered = tree.render_output_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &state,
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );

        let ring = rendered
            .primitives
            .overlay_shapes
            .iter()
            .find(|shape| shape.stroke_width == theme.focus_ring.width.get())
            .expect("focused button should render focus ring overlay");
        assert_eq!(ring.clip_rect, None);
        assert_eq!(ring.clip_mask, None);
    }

    #[test]
    fn neutral_components_remain_transparent_by_default() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();

        let tree: WidgetTree<()> = WidgetTree::new(
            Stack::new()
                .size(dp(120.0), dp(80.0))
                .child(Image::from_bytes(ONE_BY_ONE_GIF).size(dp(40.0), dp(40.0)))
                .child(Canvas::new(Vec::<CanvasItem>::new()).size(dp(40.0), dp(20.0))),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 120.0, 80.0),
            None,
            None,
            None,
            None,
            false,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .all(|shape| shape.color.a == 0));
    }

    #[test]
    fn switch_thumb_animates_between_positions() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let context = test_context();
        let checked = context.observable(false);
        let tree: WidgetTree<()> = WidgetTree::new(Switch::new(checked.binding().animated(
            crate::animation::Transition::ease_in_out(std::time::Duration::from_millis(180)),
        )));

        let mut animations = AnimationEngine::default();
        let initial = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 60.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let start_x = initial.primitives.overlay_shapes[0].rect.x;

        checked.set(true);
        let toggled = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 60.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let immediate_x = toggled.primitives.overlay_shapes[0].rect.x;

        std::thread::sleep(std::time::Duration::from_millis(100));
        let mid = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 60.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let mid_x = mid.primitives.overlay_shapes[0].rect.x;

        std::thread::sleep(std::time::Duration::from_millis(140));
        let end = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 60.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let end_x = end.primitives.overlay_shapes[0].rect.x;

        assert_eq!(immediate_x, start_x);
        assert!(mid_x > start_x);
        assert!(mid_x < end_x);
    }

    #[test]
    fn selectable_text_renders_selection_highlight() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let text: Element<()> = Text::new("hello").user_select(true).into();
        let text_id = text.id;
        let tree = WidgetTree::new(text);

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 40.0),
            None,
            None,
            Some(text_id),
            Some(&TextEditState {
                cursor: 5,
                anchor: 1,
                composition: None,
                scroll_x: Dp::ZERO,
                scroll_y: Dp::ZERO,
                preferred_column_x: None,
            }),
            false,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|primitive| { primitive.color == theme.colors.selection.with_alpha_factor(1.0) }));
    }
}

pub enum WidgetCommand<VM> {
    Command(Command<VM>),
    Value(ValueCommand<VM, String>, String),
}

pub struct WidgetEventResult<VM> {
    pub command: Option<WidgetCommand<VM>>,
    pub focus: Option<WidgetId>,
    pub request_redraw: bool,
}

pub fn rect(x: Dp, y: Dp, width: Dp, height: Dp) -> Rect {
    Rect::new(x, y, width, height)
}
