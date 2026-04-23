use std::collections::HashMap;
use std::sync::Arc;

use taffy::prelude::{
    auto, evenly_sized_tracks, length, line, percent, AlignItems as TaffyAlignItems,
    AvailableSpace, Display, FlexDirection, FlexWrap, JustifyContent as TaffyJustifyContent,
    Style as TaffyStyle, TaffyTree,
};
use taffy::Size as TaffySize;

use crate::animation::{AnimationEngine, WidgetProperty};
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::media::{
    media_placeholder_color, media_placeholder_label, resolve_media_rect, ContentFit,
    IntrinsicSize, MediaManager, RasterRequest,
};
use crate::text::font::{FontManager, TextFontRequest};
use crate::ui::layout::{Align, Axis, Insets, LayoutStyle, Overflow, Value, Wrap};
use crate::ui::theme::Theme;
use crate::ui::unit::{dp, sp, Dp, UnitContext};
#[cfg(feature = "video")]
use crate::video::VideoSurface as PublicVideoSurface;

use super::canvas::{canvas_bounds, CanvasItem};
use super::common::{
    ComputedScene, ContainerKind, ContainerLayout, HitGeometry, HitInteraction, HitRegion,
    InputEditState, InputSnapshot, InteractionHandlers, LayoutNode, MeasureContext,
    MediaEventHandlers, MediaEventPhase, MediaEventState, Point, Rect, RenderPrimitive,
    RenderedWidgetScene, ScenePrimitives, ScrollRegion, ScrollbarAxis, ScrollbarHandle,
    TextPrimitive, TexturePrimitive, VisualStyle, WidgetId, WidgetKind,
};
use super::text::Text;

/// Caret width in logical pixels.
/// 光标的像素宽度
const CARET_WIDTH: f32 = 2.0;

/// Caret end gap in logical pixels.
/// 光标末尾间隔
const CARET_END_GAP: f32 = 1.0;

/// Caret width in logical pixels when input is empty.
/// 当输入为空时光标像素宽度
const INPUT_EMPTY_CARET_INSET: f32 = 1.0;

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
    },
    Button {
        label: Text,
        disabled: Value<bool>,
    },
    Input {
        text: Text,
        placeholder: Text,
        on_change: Option<ValueCommand<VM, String>>,
        disabled: Value<bool>,
    },
}

struct CollectContext<'a, 'b> {
    taffy: &'a TaffyTree<MeasureContext>,
    font_manager: &'a FontManager,
    theme: &'a Theme,
    media: &'a MediaManager,
    focused_input: Option<WidgetId>,
    focused_input_state: Option<&'a InputEditState>,
    selected_text: Option<WidgetId>,
    selected_text_state: Option<&'a InputEditState>,
    caret_visible: bool,
    hovered_scrollbar: Option<ScrollbarHandle>,
    active_scrollbar: Option<ScrollbarHandle>,
    scroll_offsets: &'a HashMap<WidgetId, Point>,
    units: UnitContext,
    animations: &'b mut AnimationEngine,
    now: std::time::Instant,
}

#[derive(Clone, Copy)]
struct VisualContext {
    origin: Point,
    opacity: f32,
    clip_rect: Rect,
}

impl<VM> Element<VM> {
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

    fn resolve(&self) -> ResolvedElement<VM> {
        let kind = match &self.kind {
            WidgetKind::Container { layout, children } => ResolvedWidgetKind::Container {
                layout: layout.clone(),
                children: children
                    .iter()
                    .flat_map(|child| child.resolve())
                    .map(|child| child.resolve())
                    .collect(),
            },
            WidgetKind::Text { text } => ResolvedWidgetKind::Text { text: text.clone() },
            WidgetKind::Image { image } => ResolvedWidgetKind::Image {
                image: image.clone(),
            },
            WidgetKind::Canvas {
                items,
                item_interactions,
            } => ResolvedWidgetKind::Canvas {
                items: items.resolve(),
                item_interactions: item_interactions.clone(),
            },
            #[cfg(feature = "video")]
            WidgetKind::VideoSurface { video } => ResolvedWidgetKind::VideoSurface {
                video: video.clone(),
            },
            WidgetKind::Button { label, disabled } => ResolvedWidgetKind::Button {
                label: label.clone(),
                disabled: disabled.clone(),
            },
            WidgetKind::Input {
                text,
                placeholder,
                on_change,
                disabled,
            } => ResolvedWidgetKind::Input {
                text: text.clone(),
                placeholder: placeholder.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
            },
        };

        ResolvedElement {
            id: self.id,
            layout: self.layout.clone(),
            visual: self.visual.clone(),
            interactions: self.interactions.clone(),
            media_events: self.media_events.clone(),
            background: self.background.clone(),
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
            ResolvedWidgetKind::VideoSurface { video } => {
                MeasureContext::VideoSurface(video.clone())
            }
            ResolvedWidgetKind::Button { label, .. } => MeasureContext::Button(label.clone()),
            ResolvedWidgetKind::Input {
                text, placeholder, ..
            } => MeasureContext::Input {
                text: text.clone(),
                placeholder: placeholder.clone(),
            },
        }
    }

    fn build_layout_tree(
        &self,
        taffy: &mut TaffyTree<MeasureContext>,
        animations: &mut AnimationEngine,
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
                    units,
                    Some(layout.kind),
                    viewport,
                    false,
                    now,
                )?);
            }
        }

        let style = self.taffy_style(parent_kind, viewport, is_root, animations, units, now);
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
        units: UnitContext,
        now: std::time::Instant,
    ) -> TaffyStyle {
        let mut style = TaffyStyle {
            size: TaffySize {
                width: if is_root {
                    length(viewport.width)
                } else if self.layout.fill_width {
                    percent(1.0)
                } else {
                    self.layout
                        .width
                        .as_ref()
                        .map(|value| {
                            length(value.resolve_widget_to_logical(
                                animations,
                                self.id,
                                WidgetProperty::Width,
                                now,
                                units,
                            ))
                        })
                        .unwrap_or_else(auto)
                },
                height: if is_root {
                    length(viewport.height)
                } else if self.layout.fill_height {
                    percent(1.0)
                } else {
                    self.layout
                        .height
                        .as_ref()
                        .map(|value| {
                            length(value.resolve_widget_to_logical(
                                animations,
                                self.id,
                                WidgetProperty::Height,
                                now,
                                units,
                            ))
                        })
                        .unwrap_or_else(auto)
                },
            },
            margin: to_taffy_rect_auto(
                self.layout
                    .margin
                    .resolve_widget(animations, self.id, WidgetProperty::Margin, now),
                units,
            ),
            padding: to_taffy_rect(
                self.layout.padding.resolve_widget(
                    animations,
                    self.id,
                    WidgetProperty::Padding,
                    now,
                ),
                units,
            ),
            flex_grow: self
                .layout
                .grow
                .resolve_widget(animations, self.id, WidgetProperty::Grow, now)
                .max(0.0),
            ..Default::default()
        };

        if matches!(parent_kind, Some(ContainerKind::Stack)) {
            style.grid_row.start = line(1);
            style.grid_column.start = line(1);
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
            | ResolvedWidgetKind::Input { disabled, .. } => disabled.resolve(),
            _ => false,
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
        let border_width = self
            .visual
            .border_width
            .resolve_widget_to_logical(
                context.animations,
                self.id,
                WidgetProperty::BorderWidth,
                context.now,
                context.units,
            )
            .max(0.0);
        let border_radius = self
            .visual
            .border_radius
            .resolve_widget_to_logical(
                context.animations,
                self.id,
                WidgetProperty::BorderRadius,
                context.now,
                context.units,
            )
            .max(0.0);
        let border_color = self
            .visual
            .border_color
            .resolve_widget(
                context.animations,
                self.id,
                WidgetProperty::BorderColor,
                context.now,
            )
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
                .unwrap_or(context.theme.palette.accent),
            ResolvedWidgetKind::Input { .. } => self
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
                .unwrap_or(context.theme.palette.input_background),
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

        if background.a > 0
            && background_frame.width > Dp::ZERO
            && background_frame.height > Dp::ZERO
        {
            computed.scene.push_shape(RenderPrimitive {
                rect: background_frame,
                color: background,
                corner_radius: background_radius,
                stroke_width: 0.0,
                clip_rect: primitive_clip,
            });
        }

        push_border_primitives(
            &mut computed.scene,
            frame,
            border_width,
            border_color,
            border_radius,
            primitive_clip,
        );

        if self.interactions.has_any()
            && !disabled
            && !matches!(&self.kind, ResolvedWidgetKind::Text { text } if text.user_select)
        {
            computed.hit_regions.push(HitRegion {
                rect: frame,
                clip_rect: primitive_clip,
                geometry: HitGeometry::Rect,
                interaction: HitInteraction::Widget {
                    id: self.id,
                    interactions: self.interactions.clone(),
                    focusable: matches!(self.kind, ResolvedWidgetKind::Button { .. }),
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
                let scrollbar_geometry = compute_scrollbar_geometry(
                    background_frame,
                    content_bounds,
                    scroll_offset,
                    layout,
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
                        },
                        context,
                        computed,
                    );
                }
                push_scrollbar_primitives(
                    &mut computed.scene,
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
                let padding = text.layout.padding.resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::Padding,
                    context.now,
                );
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
                    padding,
                    None,
                    (text.user_select && context.selected_text == Some(self.id))
                        .then_some(context.selected_text_state)
                        .flatten(),
                    context.theme.palette.text,
                    opacity,
                    self.id,
                    primitive_clip,
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
                    primitive_clip,
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
                let padding = self.layout.padding.resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::Padding,
                    context.now,
                );
                let canvas_frame = background_frame.inset(padding);
                let canvas_clip = primitive_clip.and_then(|clip| clip.intersect(canvas_frame));
                let canvas_origin = Point::new(canvas_frame.x, canvas_frame.y);

                if canvas_frame.width > Dp::ZERO && canvas_frame.height > Dp::ZERO {
                    for item in items {
                        let meshes = item.tessellate(canvas_origin, opacity, canvas_clip);
                        for mesh in &meshes {
                            computed.scene.push_mesh(mesh.clone());
                        }

                        if item_interactions.has_any() {
                            if let Some(bounds) = item.bounds() {
                                let triangles = meshes
                                    .iter()
                                    .flat_map(|mesh| mesh.triangles.iter().copied())
                                    .collect::<Vec<_>>();
                                if !triangles.is_empty() {
                                    computed.hit_regions.push(HitRegion {
                                        rect: Rect::new(
                                            canvas_frame.x + bounds.min_x,
                                            canvas_frame.y + bounds.min_y,
                                            bounds.width(),
                                            bounds.height(),
                                        ),
                                        clip_rect: canvas_clip,
                                        geometry: HitGeometry::Triangles(Arc::from(triangles)),
                                        interaction: HitInteraction::CanvasItem {
                                            id: self.id,
                                            item_id: item.id(),
                                            item_interactions: item_interactions.clone(),
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
                    primitive_clip,
                    opacity,
                    loading_background,
                    context,
                    computed,
                );
            }
            ResolvedWidgetKind::Button { label, .. } => {
                let padding = self.layout.padding.resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::Padding,
                    context.now,
                );
                push_text_primitives(
                    label,
                    frame,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    false,
                    padding,
                    None,
                    None,
                    if disabled {
                        context.theme.palette.text_muted
                    } else {
                        context.theme.palette.text
                    },
                    opacity,
                    self.id,
                    primitive_clip,
                );
            }
            ResolvedWidgetKind::Input {
                text,
                placeholder,
                on_change,
                ..
            } => {
                let active = context.focused_input == Some(self.id);
                let current_text = text.content.resolve();
                let padding = self.layout.padding.resolve_widget(
                    context.animations,
                    self.id,
                    WidgetProperty::Padding,
                    context.now,
                );
                let ime_cursor_area = push_input_primitives(
                    frame,
                    text,
                    placeholder,
                    &current_text,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    padding,
                    opacity,
                    self.id,
                    active.then_some(context.focused_input_state).flatten(),
                    active && context.caret_visible && !disabled,
                    primitive_clip,
                );
                if active {
                    computed.ime_cursor_area = ime_cursor_area;
                }
                if !disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        clip_rect: primitive_clip,
                        geometry: HitGeometry::Rect,
                        interaction: HitInteraction::FocusInput {
                            id: self.id,
                            frame,
                            padding,
                            interactions: self.interactions.clone(),
                            on_change: on_change.clone(),
                            text_style: text.clone(),
                            text: current_text,
                        },
                    });
                }
            }
        }
    }

    fn input_snapshot(&self, id: WidgetId) -> Option<InputSnapshot<VM>> {
        match &self.kind {
            ResolvedWidgetKind::Container { children, .. } => {
                children.iter().find_map(|child| child.input_snapshot(id))
            }
            ResolvedWidgetKind::Input { disabled, .. } if self.id == id && disabled.resolve() => {
                None
            }
            ResolvedWidgetKind::Input {
                text, on_change, ..
            } if self.id == id => Some(InputSnapshot {
                id,
                on_change: on_change.clone(),
                text: text.content.resolve(),
            }),
            _ => None,
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
            .resolve_widget(animations, widget_id, WidgetProperty::Padding, now),
        units,
    );
    let gap = layout
        .gap
        .resolve_widget_to_logical(animations, widget_id, WidgetProperty::Gap, now, units)
        .max(0.0);
    style.gap = TaffySize {
        width: length(gap),
        height: length(gap),
    };

    match layout.kind {
        ContainerKind::Flow | ContainerKind::Column => {
            style.display = Display::Flex;
            style.flex_direction = FlexDirection::Column;
            style.justify_content = Some(map_axis_align_content(
                layout.align_y.unwrap_or(layout.align),
            ));
            style.align_items = map_align_items(layout.align_x.unwrap_or(layout.align));
        }
        ContainerKind::Row => {
            style.display = Display::Flex;
            style.flex_direction = FlexDirection::Row;
            style.justify_content = Some(map_axis_align_content(
                layout.align_x.unwrap_or(layout.align),
            ));
            style.align_items = map_align_items(layout.align_y.unwrap_or(layout.align));
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
            match direction {
                Axis::Horizontal => {
                    style.justify_content = Some(map_axis_align_content(
                        layout.align_x.unwrap_or(layout.align),
                    ));
                    style.align_items = map_align_items(layout.align_y.unwrap_or(layout.align));
                }
                Axis::Vertical => {
                    style.justify_content = Some(map_axis_align_content(
                        layout.align_y.unwrap_or(layout.align),
                    ));
                    style.align_items = map_align_items(layout.align_x.unwrap_or(layout.align));
                }
            }
        }
        ContainerKind::Grid { columns } => {
            style.display = Display::Grid;
            style.grid_template_columns = evenly_sized_tracks(columns.max(1) as u16);
            style.justify_items = map_align_items(layout.align_x.unwrap_or(layout.align));
            style.align_items = map_align_items(layout.align_y.unwrap_or(layout.align));
        }
        ContainerKind::Stack => {
            style.display = Display::Grid;
            style.grid_template_columns = vec![auto()];
            style.grid_template_rows = vec![auto()];
            style.justify_items = map_align_items(layout.align_x.unwrap_or(layout.align));
            style.align_items = map_align_items(layout.align_y.unwrap_or(layout.align));
        }
    }
}

fn compute_container_content_bounds<VM>(
    _element: &ResolvedElement<VM>,
    children: &[ResolvedElement<VM>],
    layout_node: &LayoutNode,
    frame: Rect,
    context: &mut CollectContext<'_, '_>,
) -> Rect {
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

    bounds.unwrap_or(Rect::new(frame.x, frame.y, 0.0, 0.0))
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
    units: UnitContext,
) -> ScrollbarGeometry {
    let can_scroll_x =
        layout.overflow_x == Overflow::Scroll && content_bounds.right() > viewport.right();
    let can_scroll_y =
        layout.overflow_y == Overflow::Scroll && content_bounds.bottom() > viewport.bottom();
    if !can_scroll_x && !can_scroll_y {
        return ScrollbarGeometry::default();
    }

    let style = layout.scrollbar_style;
    let thickness = units.resolve_dp(style.thickness.max(dp(2.0)));
    let inset_bounds = viewport.inset(style.insets);
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
                    units.resolve_dp(style.min_thumb_length.max(Dp::new(thickness))),
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
                    units.resolve_dp(style.min_thumb_length.max(Dp::new(thickness))),
                    Axis::Vertical,
                )
            }),
        horizontal_track: horizontal_track.filter(|track| !track.is_empty()),
        vertical_track: vertical_track.filter(|track| !track.is_empty()),
    }
}

fn push_scrollbar_primitives(
    scene: &mut ScenePrimitives,
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

    let style = layout.scrollbar_style;
    let track_clip = Some(clip_rect);
    let track_color = style.track_color.with_alpha_factor(opacity);
    let thumb_color_for = |axis| {
        let handle = ScrollbarHandle {
            id: widget_id,
            axis,
        };
        if active_scrollbar == Some(handle) {
            style.active_thumb_color.with_alpha_factor(opacity)
        } else if hovered_scrollbar == Some(handle) {
            style.hover_thumb_color.with_alpha_factor(opacity)
        } else {
            style.thumb_color.with_alpha_factor(opacity)
        }
    };
    let thickness = style.thickness.max(dp(2.0)).get();
    let radius = style
        .radius
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
        });
    }

    if let Some(track) = geometry.horizontal_track {
        scene.push_overlay_shape(RenderPrimitive {
            rect: track,
            color: track_color,
            corner_radius: radius,
            stroke_width: 0.0,
            clip_rect: track_clip,
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
        Align::SpaceBetween => TaffyAlignItems::Start,
        Align::Stretch => TaffyAlignItems::Stretch,
    })
}

fn map_axis_align_content(align: Align) -> TaffyJustifyContent {
    match align {
        Align::Start => TaffyJustifyContent::Start,
        Align::Center => TaffyJustifyContent::Center,
        Align::End => TaffyJustifyContent::End,
        Align::SpaceBetween => TaffyJustifyContent::SpaceBetween,
        Align::Stretch => TaffyJustifyContent::Start,
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
                image.aspect_ratio,
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
                video.aspect_ratio,
                snapshot.intrinsic_size,
            )
        }
        Some(MeasureContext::Button(label)) => {
            measure_text_content(label, font_manager, theme, units)
        }
        Some(MeasureContext::Input { text, placeholder }) => {
            let text_size = measure_text_content(text, font_manager, theme, units);
            let placeholder_size = measure_text_content(placeholder, font_manager, theme, units);
            (
                text_size.0.max(placeholder_size.0),
                text_size.1.max(placeholder_size.1),
            )
        }
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
    let (font_size, line_height, letter_spacing) = resolved_text_metrics(text, theme, units);
    font_manager.measure_text(
        &text.content.resolve(),
        TextFontRequest {
            preferred_font: text
                .font_family
                .as_deref()
                .or(theme.typography.font_family.as_deref()),
            weight: text.font_weight,
        },
        font_size,
        line_height,
        letter_spacing,
    )
}

fn resolved_text_metrics(text: &Text, theme: &Theme, units: UnitContext) -> (f32, f32, f32) {
    let font_size = units.resolve_sp(
        text.font_size
            .unwrap_or(theme.typography.font_size.max(sp(1.0))),
    );
    let line_height = (font_size * 1.25).max(font_size + 4.0);
    let letter_spacing = units.resolve_sp(text.letter_spacing);
    (font_size, line_height, letter_spacing)
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
    clip_rect: Option<Rect>,
    opacity: f32,
    loading_background: Color,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
    kind: &str,
) {
    let metadata = context.media.image_snapshot(source, None);
    let target_frame = resolve_media_rect(content_frame, metadata.intrinsic_size, fit);
    let snapshot = if let Some(raster_request) = RasterRequest::from_frame(target_frame) {
        context.media.image_snapshot(source, Some(raster_request))
    } else {
        metadata
    };

    if let Some(texture) = snapshot.texture.as_ref() {
        computed.scene.push_texture(TexturePrimitive {
            texture: Arc::clone(texture),
            frame: target_frame,
            clip_rect,
        });
        return;
    }

    push_media_placeholder(
        frame,
        content_frame,
        clip_rect,
        opacity,
        context,
        &mut computed.scene,
        widget_id,
        kind,
        snapshot.loading,
        snapshot.error.as_deref(),
        loading_background,
    );
}

#[cfg(feature = "video")]
fn push_video_texture_or_placeholder<VM>(
    widget_id: WidgetId,
    video: &PublicVideoSurface,
    frame: Rect,
    content_frame: Rect,
    clip_rect: Option<Rect>,
    opacity: f32,
    loading_background: Color,
    context: &mut CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
) {
    let snapshot = video.controller.surface_snapshot();
    let target_frame = resolve_media_rect(content_frame, snapshot.intrinsic_size, video.fit);

    if let Some(texture) = snapshot.texture.as_ref() {
        computed.scene.push_texture(TexturePrimitive {
            texture: Arc::clone(texture),
            frame: target_frame,
            clip_rect,
        });
        return;
    }

    push_media_placeholder(
        frame,
        content_frame,
        clip_rect,
        opacity,
        context,
        &mut computed.scene,
        widget_id,
        "video",
        snapshot.loading,
        snapshot.error.as_deref(),
        loading_background,
    );
}

fn push_media_placeholder(
    frame: Rect,
    content_frame: Rect,
    clip_rect: Option<Rect>,
    opacity: f32,
    context: &mut CollectContext<'_, '_>,
    scene: &mut ScenePrimitives,
    widget_id: WidgetId,
    kind: &str,
    loading: bool,
    error: Option<&str>,
    loading_background: Color,
) {
    let placeholder =
        media_loading_fill_color(loading, error, loading_background).with_alpha_factor(opacity);
    if content_frame.width > Dp::ZERO && content_frame.height > Dp::ZERO {
        scene.push_overlay_shape(RenderPrimitive {
            rect: content_frame,
            color: placeholder,
            corner_radius: 0.0,
            stroke_width: 0.0,
            clip_rect,
        });
    }

    let label = media_placeholder_label(kind, loading, error);
    let text =
        Text::new(label).font_size((context.theme.typography.font_size - sp(1.0)).max(sp(12.0)));
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
        Insets::all(dp(12.0)),
        None,
        None,
        Color::hexa(0xE5E7EBFF),
        opacity,
        widget_id,
        clip_rect,
    );
}

fn media_loading_fill_color(
    loading: bool,
    error: Option<&str>,
    loading_background: Color,
) -> Color {
    if loading {
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
    padding: Insets,
    caret_content: Option<&str>,
    selection_state: Option<&InputEditState>,
    fallback_color: Color,
    opacity: f32,
    widget_id: WidgetId,
    clip_rect: Option<Rect>,
) {
    let content = text.content.resolve();
    let text_request = TextFontRequest {
        preferred_font: text
            .font_family
            .as_deref()
            .or(theme.typography.font_family.as_deref()),
        weight: text.font_weight,
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
    );

    if let Some((selection_start, selection_end)) = selection_state
        .cloned()
        .unwrap_or_else(|| InputEditState::caret_at(&content))
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
                color: theme.palette.accent.with_alpha_factor(0.35 * opacity),
                corner_radius: 4.0,
                stroke_width: 0.0,
                clip_rect,
            });
        }
    }

    scene.push_text(TextPrimitive {
        content: content.clone(),
        frame: content_frame,
        color: color.with_alpha_factor(opacity),
        font_family: Some(resolved.primary_font),
        font_size,
        font_weight: text.font_weight,
        line_height,
        letter_spacing,
        clip_rect,
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
            color: theme.palette.text.with_alpha_factor(opacity),
            corner_radius: 0.0,
            stroke_width: 0.0,
            clip_rect,
        });
    }
}

fn push_input_primitives(
    frame: Rect,
    text: &Text,
    placeholder: &Text,
    current_text: &str,
    font_manager: &FontManager,
    theme: &Theme,
    units: UnitContext,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    padding: Insets,
    opacity: f32,
    widget_id: WidgetId,
    edit_state: Option<&InputEditState>,
    show_caret: bool,
    clip_rect: Option<Rect>,
) -> Option<Rect> {
    let (font_size, line_height, letter_spacing) = resolved_text_metrics(text, theme, units);
    let text_request = TextFontRequest {
        preferred_font: text
            .font_family
            .as_deref()
            .or(theme.typography.font_family.as_deref()),
        weight: text.font_weight,
    };
    let inner = frame.inset(padding);
    let state = edit_state
        .cloned()
        .unwrap_or_else(|| InputEditState::caret_at(current_text))
        .clamped_to(current_text);

    let composition = state.composition.clone();
    let show_placeholder = current_text.is_empty()
        && composition
            .as_ref()
            .map(|composition| composition.text.is_empty())
            .unwrap_or(true);

    if show_placeholder {
        let placeholder_color = placeholder
            .color
            .as_ref()
            .map(|color| {
                color.resolve_widget(animations, widget_id, WidgetProperty::TextColor, now)
            })
            .unwrap_or(theme.palette.text_muted)
            .with_alpha_factor(opacity);
        let placeholder_request = TextFontRequest {
            preferred_font: placeholder
                .font_family
                .as_deref()
                .or(theme.typography.font_family.as_deref()),
            weight: placeholder.font_weight,
        };
        let placeholder_content = placeholder.content.resolve();
        let resolved = font_manager.resolve_text(&placeholder_content, placeholder_request.clone());
        let (placeholder_size, placeholder_line_height, placeholder_letter_spacing) =
            resolved_text_metrics(placeholder, theme, units);
        let (measured_width, measured_height) = font_manager.measure_text_raw(
            &placeholder_content,
            placeholder_request,
            placeholder_size,
            placeholder_line_height,
            placeholder_letter_spacing,
        );
        let content_frame = centered_text_frame(
            inner,
            measured_width,
            measured_height,
            placeholder_line_height,
        );
        scene.push_text(TextPrimitive {
            content: placeholder_content,
            frame: content_frame,
            color: placeholder_color,
            font_family: Some(resolved.primary_font),
            font_size: placeholder_size,
            font_weight: placeholder.font_weight,
            line_height: placeholder_line_height,
            letter_spacing: placeholder_letter_spacing,
            clip_rect,
        });

        let caret_rect = Rect::new(
            inner.x + INPUT_EMPTY_CARET_INSET,
            content_frame.y,
            CARET_WIDTH,
            content_frame.height.max(Dp::new(placeholder_line_height)),
        );
        if show_caret {
            scene.push_overlay_shape(RenderPrimitive {
                rect: caret_rect,
                color: theme.palette.text.with_alpha_factor(opacity),
                corner_radius: 0.0,
                stroke_width: 0.0,
                clip_rect,
            });
        }

        return Some(caret_rect);
    }

    let composition_range = composition
        .as_ref()
        .map(|composition| composition.replace_range)
        .unwrap_or((state.cursor, state.cursor));
    let composition_start = composition_range.0.min(current_text.len());
    let composition_end = composition_range.1.min(current_text.len());
    let prefix_text = &current_text[..composition_start];
    let suffix_text = &current_text[composition_end..];
    let preedit_text = composition
        .as_ref()
        .map(|composition| composition.text.as_str())
        .unwrap_or("");

    let display_text = if composition.is_some() {
        format!("{prefix_text}{preedit_text}{suffix_text}")
    } else {
        current_text.to_string()
    };
    let (display_width, display_height) = font_manager.measure_text_raw(
        &display_text,
        text_request.clone(),
        font_size,
        line_height,
        letter_spacing,
    );
    let content_frame = centered_text_frame(inner, display_width, display_height, line_height);

    let base_color = text
        .color
        .as_ref()
        .map(|color| color.resolve_widget(animations, widget_id, WidgetProperty::TextColor, now))
        .unwrap_or(theme.palette.text)
        .with_alpha_factor(opacity);
    let preedit_color = theme.palette.text_muted.with_alpha_factor(opacity);
    let resolved = font_manager.resolve_text(current_text, text_request.clone());

    let current_layout = composition.is_none().then(|| {
        font_manager.measure_text_layout(
            current_text,
            text_request.clone(),
            font_size,
            line_height,
            letter_spacing,
        )
    });
    let prefix_width = measure_segment(
        font_manager,
        prefix_text,
        text_request.clone(),
        font_size,
        line_height,
        letter_spacing,
    );
    let preedit_width = measure_segment(
        font_manager,
        preedit_text,
        text_request.clone(),
        font_size,
        line_height,
        letter_spacing,
    );
    let full_text_width = current_layout
        .as_ref()
        .map(|layout| layout.width)
        .unwrap_or_else(|| {
            measure_segment(
                font_manager,
                current_text,
                text_request.clone(),
                font_size,
                line_height,
                letter_spacing,
            )
        });

    if composition.is_none() {
        if let Some((selection_start, selection_end)) = state.selection_range() {
            let selection_start = selection_start.min(current_text.len());
            let selection_end = selection_end.min(current_text.len());
            let selection_start_x = current_layout
                .as_ref()
                .map(|layout| layout.x_for_index(selection_start))
                .unwrap_or(0.0);
            let selection_end_x = current_layout
                .as_ref()
                .map(|layout| layout.x_for_index(selection_end))
                .unwrap_or(selection_start_x);
            let selection_x = content_frame.x + selection_start_x;
            let selection_width = (selection_end_x - selection_start_x).max(0.0);
            if selection_width > 0.0 {
                scene.push_shape(RenderPrimitive {
                    rect: Rect::new(
                        selection_x,
                        content_frame.y,
                        selection_width,
                        content_frame.height.max(Dp::new(line_height)),
                    ),
                    color: theme.palette.accent.with_alpha_factor(0.35 * opacity),
                    corner_radius: 4.0,
                    stroke_width: 0.0,
                    clip_rect,
                });
            }
        }
    }

    if composition.is_none() {
        scene.push_text(TextPrimitive {
            content: current_text.to_string(),
            frame: Rect::new(
                content_frame.x,
                content_frame.y,
                full_text_width,
                content_frame.height,
            ),
            color: base_color,
            font_family: Some(resolved.primary_font),
            font_size,
            font_weight: text.font_weight,
            line_height,
            letter_spacing,
            clip_rect,
        });
    } else {
        let mut cursor_x = content_frame.x;
        if !prefix_text.is_empty() {
            scene.push_text(TextPrimitive {
                content: prefix_text.to_string(),
                frame: Rect::new(
                    cursor_x,
                    content_frame.y,
                    prefix_width,
                    content_frame.height,
                ),
                color: base_color,
                font_family: Some(resolved.primary_font.clone()),
                font_size,
                font_weight: text.font_weight,
                line_height,
                letter_spacing,
                clip_rect,
            });
            cursor_x += prefix_width;
        }

        if !preedit_text.is_empty() {
            scene.push_text(TextPrimitive {
                content: preedit_text.to_string(),
                frame: Rect::new(
                    cursor_x,
                    content_frame.y,
                    preedit_width,
                    content_frame.height,
                ),
                color: preedit_color,
                font_family: Some(resolved.primary_font.clone()),
                font_size,
                font_weight: text.font_weight,
                line_height,
                letter_spacing,
                clip_rect,
            });
            scene.push_overlay_shape(RenderPrimitive {
                rect: Rect::new(
                    cursor_x,
                    (content_frame.y + content_frame.height - 1.0).max(content_frame.y),
                    preedit_width.max(1.0),
                    1.0,
                ),
                color: preedit_color,
                corner_radius: 0.0,
                stroke_width: 0.0,
                clip_rect,
            });
            cursor_x += preedit_width;
        }

        if !suffix_text.is_empty() {
            let suffix_width = measure_segment(
                font_manager,
                suffix_text,
                text_request.clone(),
                font_size,
                line_height,
                letter_spacing,
            );
            scene.push_text(TextPrimitive {
                content: suffix_text.to_string(),
                frame: Rect::new(
                    cursor_x,
                    content_frame.y,
                    suffix_width,
                    content_frame.height,
                ),
                color: base_color,
                font_family: Some(resolved.primary_font),
                font_size,
                font_weight: text.font_weight,
                line_height,
                letter_spacing,
                clip_rect,
            });
        }
    }

    let caret_boundary = composition
        .as_ref()
        .map(|composition| {
            let visual_cursor = composition
                .cursor
                .map(|(_, end)| end.min(composition.text.len()))
                .unwrap_or(composition.text.len());
            prefix_width
                + measure_segment(
                    font_manager,
                    &composition.text[..visual_cursor],
                    text_request.clone(),
                    font_size,
                    line_height,
                    letter_spacing,
                )
        })
        .unwrap_or_else(|| {
            current_layout
                .as_ref()
                .map(|layout| layout.x_for_index(state.cursor.min(current_text.len())))
                .unwrap_or(0.0)
        });
    let caret_padding = if composition.is_none() && state.cursor >= current_text.len() {
        CARET_END_GAP
    } else {
        CARET_WIDTH - CARET_WIDTH - CARET_WIDTH
    };
    let caret_rect = Rect::new(
        (content_frame.x + caret_boundary + caret_padding).max(inner.x),
        content_frame.y,
        CARET_WIDTH,
        content_frame.height.max(Dp::new(line_height)),
    );

    let hide_caret = composition
        .as_ref()
        .map(|composition| composition.cursor.is_none())
        .unwrap_or(false);
    if show_caret && !hide_caret {
        scene.push_overlay_shape(RenderPrimitive {
            rect: caret_rect,
            color: theme.palette.text.with_alpha_factor(opacity),
            corner_radius: 0.0,
            stroke_width: 0.0,
            clip_rect,
        });
    }

    Some(if composition.is_some() || full_text_width > 0.0 {
        caret_rect
    } else {
        Rect::new(
            inner.x + CARET_END_GAP,
            content_frame.y,
            CARET_WIDTH,
            content_frame.height.max(Dp::new(line_height)),
        )
    })
}

fn measure_segment(
    font_manager: &FontManager,
    text: &str,
    request: TextFontRequest<'_>,
    font_size: f32,
    line_height: f32,
    letter_spacing: f32,
) -> f32 {
    if text.is_empty() {
        0.0
    } else {
        font_manager
            .measure_text_raw(text, request, font_size, line_height, letter_spacing)
            .0
    }
}

fn centered_text_frame(
    inner: Rect,
    measured_width: f32,
    measured_height: f32,
    line_height: f32,
) -> Rect {
    let content_height = inner
        .height
        .min(measured_height.max(line_height))
        .max(Dp::new(line_height));

    Rect::new(
        inner.x,
        inner.y + ((inner.height - content_height).max(0.0) * 0.5),
        inner.width.min(measured_width).max(0.0),
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
    });
}

pub struct WidgetTree<VM> {
    root: Element<VM>,
}

impl<VM> WidgetTree<VM> {
    pub fn new(root: impl Into<Element<VM>>) -> Self {
        Self { root: root.into() }
    }

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
        focused_input: Option<WidgetId>,
        focused_input_state: Option<&InputEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&InputEditState>,
        caret_visible: bool,
    ) -> ComputedScene<VM> {
        self.compute_scene_with_units(
            font_manager,
            theme,
            media,
            UnitContext::default(),
            animations,
            hovered_scrollbar,
            active_scrollbar,
            scroll_offsets,
            viewport,
            focused_input,
            focused_input_state,
            selected_text,
            selected_text_state,
            caret_visible,
        )
    }

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
        focused_input: Option<WidgetId>,
        focused_input_state: Option<&InputEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&InputEditState>,
        caret_visible: bool,
    ) -> ComputedScene<VM> {
        let mut taffy = TaffyTree::new();
        let now = std::time::Instant::now();
        let resolved_root = self.root.resolve();
        let root_layout = resolved_root
            .build_layout_tree(&mut taffy, animations, units, None, viewport, true, now)
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

        let mut computed = ComputedScene::default();
        let mut context = CollectContext {
            taffy: &taffy,
            font_manager,
            theme,
            media,
            focused_input,
            focused_input_state,
            selected_text,
            selected_text_state,
            caret_visible,
            hovered_scrollbar,
            active_scrollbar,
            scroll_offsets,
            units,
            animations,
            now,
        };
        resolved_root.collect_primitives(
            &root_layout,
            VisualContext {
                origin: Point {
                    x: viewport.x,
                    y: viewport.y,
                },
                opacity: 1.0,
                clip_rect: viewport,
            },
            &mut context,
            &mut computed,
        );
        computed
    }

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
        focused_input_state: Option<&InputEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&InputEditState>,
        caret_visible: bool,
    ) -> RenderedWidgetScene {
        self.render_output_with_units(
            font_manager,
            theme,
            media,
            UnitContext::default(),
            animations,
            hovered_scrollbar,
            active_scrollbar,
            scroll_offsets,
            viewport,
            focused_input,
            focused_input_state,
            selected_text,
            selected_text_state,
            caret_visible,
        )
    }

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
        focused_input_state: Option<&InputEditState>,
        selected_text: Option<WidgetId>,
        selected_text_state: Option<&InputEditState>,
        caret_visible: bool,
    ) -> RenderedWidgetScene {
        let computed = self.compute_scene_with_units(
            font_manager,
            theme,
            media,
            units,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            scroll_offsets,
            viewport,
            focused_input,
            focused_input_state,
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

        for hit in computed.hit_regions.iter().filter(|hit| {
            hit.rect.contains(point)
                && hit
                    .clip_rect
                    .map(|clip_rect| clip_rect.contains(point))
                    .unwrap_or(true)
                && hit.geometry.contains(point)
        }) {
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
        self.hit_path(
            font_manager,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
            scroll_offsets,
            viewport,
            cursor_position,
            focused_input,
        )
        .pop()
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
        let Some(point) = cursor_position else {
            return Vec::new();
        };
        let computed = self.compute_scene(
            font_manager,
            theme,
            media,
            animations,
            hovered_scrollbar,
            active_scrollbar,
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

    pub(crate) fn input_snapshot(&self, id: WidgetId) -> Option<InputSnapshot<VM>> {
        self.root.resolve().input_snapshot(id)
    }

    pub(crate) fn media_event_states(&self, media: &MediaManager) -> Vec<MediaEventState<VM>> {
        let mut states = Vec::new();
        self.root
            .resolve()
            .collect_media_event_states(media, &mut states);
        states
    }
}

#[cfg(test)]
mod tests {
    use super::centered_text_frame;
    use std::collections::HashMap;

    use crate::animation::{AnimationCoordinator, AnimationEngine};
    use crate::foundation::binding::{InvalidationSignal, ViewModelContext};
    use crate::foundation::color::Color;
    use crate::foundation::view_model::{Command, ValueCommand};
    use crate::media::MediaManager;
    use crate::text::font::{FontCatalog, FontManager};
    use crate::ui::layout::Overflow;
    use crate::ui::theme::Theme;
    use crate::ui::unit::dp;
    use crate::ui::widget::common::Rect;
    use crate::ui::widget::{
        Canvas, CanvasItem, CanvasPath, CanvasStroke, Element, Image, Input, InputEditState,
        PathBuilder, Point, ScrollbarAxis, ScrollbarHandle, Stack, Text, WidgetTree,
    };
    #[cfg(feature = "video")]
    use crate::video::backend::{
        BackendSharedState, VideoBackend, DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES,
    };
    #[cfg(feature = "video")]
    use crate::video::{PlaybackState, VideoController, VideoMetrics, VideoSize, VideoSurface};

    #[test]
    fn centers_text_using_actual_render_height() {
        let inner = Rect::new(12.0, 8.0, 180.0, 24.0);
        let frame = centered_text_frame(inner, 56.0, 18.0, 18.0);

        assert_eq!(frame.x, 12.0);
        assert_eq!(frame.y, 11.0);
        assert_eq!(frame.width, 56.0);
        assert_eq!(frame.height, 18.0);
    }

    #[test]
    fn image_loading_placeholder_uses_image_background() {
        let background = Color::hexa(0x11223344);

        assert_eq!(
            super::media_loading_fill_color(true, None, background),
            background
        );
    }

    #[test]
    fn image_loading_placeholder_defaults_to_transparent_white() {
        assert_eq!(
            super::media_loading_fill_color(true, None, Color::rgba(255, 255, 255, 0)),
            Color::rgba(255, 255, 255, 0)
        );
    }

    #[test]
    fn image_error_placeholder_keeps_error_color() {
        assert_eq!(
            super::media_loading_fill_color(false, Some("boom"), Color::WHITE),
            crate::media::media_placeholder_color(false, Some("boom"))
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
                    .background(crate::foundation::color::Color::hexa(0x1E293BFF))
                    .child(
                        Stack::new()
                            .size(dp(80.0), dp(80.0))
                            .offset(Point::new(dp(60.0), dp(0.0)))
                            .background(crate::foundation::color::Color::hexa(0x38BDF8FF))
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
    fn scroll_offsets_are_clamped_to_content_bounds() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let scroller: super::Element<()> = Stack::new()
            .size(dp(100.0), dp(100.0))
            .border(dp(4.0), crate::foundation::color::Color::WHITE)
            .overflow_y(Overflow::Scroll)
            .background(crate::foundation::color::Color::hexa(0x111827FF))
            .child(
                Stack::new()
                    .size(dp(100.0), dp(300.0))
                    .background(crate::foundation::color::Color::hexa(0x22C55EFF)),
            )
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
    fn overflow_clips_children_to_inside_of_border() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree = WidgetTree::new(
            Stack::<()>::new()
                .size(dp(100.0), dp(100.0))
                .border(dp(4.0), crate::foundation::color::Color::WHITE)
                .overflow(Overflow::Hidden)
                .child(
                    Stack::new()
                        .size(dp(100.0), dp(100.0))
                        .background(crate::foundation::color::Color::BLACK),
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

        let child_shape = rendered
            .primitives
            .shapes
            .iter()
            .find(|primitive| primitive.color == crate::foundation::color::Color::BLACK)
            .expect("child shape should exist");
        assert_eq!(child_shape.clip_rect, Some(Rect::new(4.0, 4.0, 92.0, 92.0)));
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
            .scrollbar_thumb_color(crate::foundation::color::Color::BLACK)
            .scrollbar_track_color(crate::foundation::color::Color::WHITE)
            .scrollbar_hover_thumb_color(crate::foundation::color::Color::hexa(0x112233FF))
            .scrollbar_active_thumb_color(crate::foundation::color::Color::hexa(0x445566FF))
            .child(
                Stack::new()
                    .size(dp(120.0), dp(260.0))
                    .background(crate::foundation::color::Color::hexa(0x1D4ED8FF)),
            )
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
            .background(crate::foundation::color::Color::WHITE)
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

    #[test]
    fn input_and_media_traversal_use_current_children() {
        let context = test_context();
        let show_input = context.observable(true);
        let input: Element<()> = Input::new(Text::new("hello")).into();
        let input_id = input.id;
        let placeholder: Element<()> = Text::new("placeholder").into();
        let input_tree = WidgetTree::new(Stack::<()>::new().child(show_input.binding().map(
            move |value| {
                if value {
                    vec![input.clone()]
                } else {
                    vec![placeholder.clone()]
                }
            },
        )));

        assert!(input_tree.input_snapshot(input_id).is_some());
        show_input.set(false);
        assert!(input_tree.input_snapshot(input_id).is_none());

        let show_media = context.observable(true);
        let image: Element<()> = Image::from_path("missing-test-image.png").into();
        let image = image.on_loading(Command::new(|_: &mut ()| {}));
        let media_placeholder: Element<()> = Text::new("no media").into();
        let media_tree = WidgetTree::new(Stack::<()>::new().child(show_media.binding().map(
            move |value| {
                if value {
                    vec![image.clone()]
                } else {
                    vec![media_placeholder.clone()]
                }
            },
        )));
        let media = test_media();

        assert_eq!(media_tree.media_event_states(&media).len(), 1);
        show_media.set(false);
        assert_eq!(media_tree.media_event_states(&media).len(), 0);
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
                    Text::new("toggle button"),
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
    fn disabled_button_does_not_participate_in_hit_testing() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            crate::ui::widget::Button::new(Text::new("disabled"))
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
        assert!(hit.is_none());
    }

    #[test]
    fn disabled_input_is_not_focusable_or_snapshotted() {
        let input: Element<()> = Input::new(Text::new("hello")).disable(true).into();
        let input_id = input.id;
        let tree = WidgetTree::new(input);

        assert!(tree.input_snapshot(input_id).is_none());

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
            Rect::new(0.0, 0.0, 160.0, 40.0),
            Some(Point::new(dp(10.0), dp(10.0))),
            None,
        );
        assert!(hit.is_none());
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
            Some(&InputEditState {
                cursor: 5,
                anchor: 1,
                composition: None,
            }),
            false,
        );

        assert!(rendered
            .primitives
            .shapes
            .iter()
            .any(|primitive| { primitive.color == theme.palette.accent.with_alpha_factor(0.35) }));
    }

    #[test]
    fn focused_input_renders_plain_text_as_single_primitive() {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let input: Element<()> = Input::new(Text::new("hello")).width(dp(160.0)).into();
        let input_id = input.id;
        let tree = WidgetTree::new(input);

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 40.0),
            Some(input_id),
            Some(&InputEditState {
                cursor: 2,
                anchor: 2,
                composition: None,
            }),
            None,
            None,
            true,
        );

        let texts: Vec<_> = rendered
            .primitives
            .texts
            .iter()
            .filter(|primitive| primitive.content == "hello")
            .collect();
        assert_eq!(texts.len(), 1);
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
