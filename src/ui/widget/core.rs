use taffy::prelude::{
    auto, evenly_sized_tracks, length, line, percent, AlignItems as TaffyAlignItems,
    AvailableSpace, Display, FlexDirection, FlexWrap, JustifyContent as TaffyJustifyContent,
    Style as TaffyStyle, TaffyTree,
};
use taffy::Size as TaffySize;
use winit::event::ElementState;
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::animation::{AnimationEngine, WidgetProperty};
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::text::font::{FontManager, TextFontRequest};
use crate::ui::layout::{Align, Axis, Insets, Justify, LayoutStyle, Wrap};
use crate::ui::theme::Theme;

use super::common::{
    ComputedScene, ContainerKind, ContainerLayout, HitInteraction, HitRegion, InteractionHandlers,
    LayoutNode, MeasureContext, Point, Rect, RenderPrimitive, ScenePrimitives, TextPrimitive,
    Value, VisualStyle, WidgetId, WidgetKind,
};
use super::text::Text;

pub struct Element<VM> {
    pub(crate) id: WidgetId,
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) interactions: InteractionHandlers<VM>,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) kind: WidgetKind<VM>,
}

struct CollectContext<'a, 'b> {
    taffy: &'a TaffyTree<MeasureContext>,
    font_manager: &'a FontManager,
    theme: &'a Theme,
    focused_input: Option<WidgetId>,
    animations: &'b mut AnimationEngine,
    now: std::time::Instant,
}

#[derive(Clone, Copy)]
struct VisualContext {
    origin: Point,
    opacity: f32,
}

impl<VM> Element<VM> {
    pub fn border(
        mut self,
        width: impl Into<Value<f32>>,
        color: impl Into<Value<Color>>,
    ) -> Self {
        self.visual.border_width = width.into();
        self.visual.border_color = color.into();
        self
    }

    pub fn border_color(mut self, color: impl Into<Value<Color>>) -> Self {
        self.visual.border_color = color.into();
        self
    }

    pub fn border_radius(mut self, radius: impl Into<Value<f32>>) -> Self {
        self.visual.border_radius = radius.into();
        self
    }

    pub fn border_width(mut self, width: impl Into<Value<f32>>) -> Self {
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

    fn measure_context(&self) -> MeasureContext {
        match &self.kind {
            WidgetKind::Container { .. } => MeasureContext::None,
            WidgetKind::Text { text } => MeasureContext::Text(text.clone()),
            WidgetKind::Button { label, .. } => MeasureContext::Button(label.clone()),
            WidgetKind::Input {
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
        parent_kind: Option<ContainerKind>,
        viewport: Rect,
        is_root: bool,
    ) -> Result<LayoutNode, taffy::TaffyError> {
        let mut child_layouts = Vec::new();
        if let WidgetKind::Container { layout, children } = &self.kind {
            child_layouts.reserve(children.len());
            for child in children {
                child_layouts.push(child.build_layout_tree(
                    taffy,
                    Some(layout.kind),
                    viewport,
                    false,
                )?);
            }
        }

        let style = self.taffy_style(parent_kind, viewport, is_root);
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
    ) -> TaffyStyle {
        let mut style = TaffyStyle {
            size: TaffySize {
                width: if is_root {
                    length(viewport.width)
                } else if self.layout.fill_width {
                    percent(1.0)
                } else {
                    self.layout.width.map(length).unwrap_or_else(auto)
                },
                height: if is_root {
                    length(viewport.height)
                } else if self.layout.fill_height {
                    percent(1.0)
                } else {
                    self.layout.height.map(length).unwrap_or_else(auto)
                },
            },
            margin: to_taffy_rect_auto(self.layout.margin),
            padding: to_taffy_rect(self.layout.padding),
            flex_grow: self.layout.grow.max(0.0),
            ..Default::default()
        };

        if matches!(parent_kind, Some(ContainerKind::Stack)) {
            style.grid_row.start = line(1);
            style.grid_column.start = line(1);
        }

        if let WidgetKind::Container { layout, .. } = &self.kind {
            apply_container_style(&mut style, *layout);
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
        let opacity = visual_context.opacity
            * self.visual.opacity.resolve_widget_clamped(
                context.animations,
                self.id,
                WidgetProperty::Opacity,
                context.now,
                0.0,
                1.0,
            );
        let border_width = self
            .visual
            .border_width
            .resolve_widget(
                context.animations,
                self.id,
                WidgetProperty::BorderWidth,
                context.now,
            )
            .max(0.0);
        let border_radius = self
            .visual
            .border_radius
            .resolve_widget(
                context.animations,
                self.id,
                WidgetProperty::BorderRadius,
                context.now,
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
            WidgetKind::Button { .. } => self
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
            WidgetKind::Input { .. } => self
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

        let background_inset = border_width.min(frame.width * 0.5).min(frame.height * 0.5);
        let background_frame = frame.inset(Insets::all(background_inset));
        let background_radius = (border_radius - background_inset).max(0.0);

        if background.a > 0 && background_frame.width > 0.0 && background_frame.height > 0.0 {
            computed.scene.shapes.push(RenderPrimitive {
                rect: background_frame,
                color: background,
                corner_radius: background_radius,
                stroke_width: 0.0,
            });
        }

        push_border_primitives(
            &mut computed.scene,
            frame,
            border_width,
            border_color,
            border_radius,
        );

        if self.interactions.has_any() {
            computed.hit_regions.push(HitRegion {
                rect: frame,
                interaction: HitInteraction::Widget {
                    id: self.id,
                    interactions: self.interactions.clone(),
                    focusable: matches!(self.kind, WidgetKind::Button { .. }),
                },
            });
        }

        match &self.kind {
            WidgetKind::Container { children, .. } => {
                for (child, child_layout) in children.iter().zip(layout_node.children.iter()) {
                    child.collect_primitives(
                        child_layout,
                        VisualContext {
                            origin: Point {
                                x: frame.x,
                                y: frame.y,
                            },
                            opacity,
                        },
                        context,
                        computed,
                    );
                }
            }
            WidgetKind::Text { text } => {
                push_text_primitives(
                    text,
                    frame,
                    context.font_manager,
                    context.theme,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    false,
                    text.layout.padding,
                    None,
                    context.theme.palette.text,
                    opacity,
                    self.id,
                );
            }
            WidgetKind::Button { label } => {
                push_text_primitives(
                    label,
                    frame,
                    context.font_manager,
                    context.theme,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    false,
                    self.layout.padding,
                    None,
                    context.theme.palette.text,
                    opacity,
                    self.id,
                );
            }
            WidgetKind::Input {
                text,
                placeholder,
                on_change,
            } => {
                let active = context.focused_input == Some(self.id);
                let current_text = text.content.resolve();
                let has_text = !current_text.is_empty();
                let text_to_draw = if has_text { text } else { placeholder };
                let fallback_color = if has_text {
                    context.theme.palette.text
                } else {
                    context.theme.palette.text_muted
                };
                push_text_primitives(
                    text_to_draw,
                    frame,
                    context.font_manager,
                    context.theme,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    active,
                    self.layout.padding,
                    Some(current_text.as_str()),
                    fallback_color,
                    opacity,
                    self.id,
                );
                computed.hit_regions.push(HitRegion {
                    rect: frame,
                    interaction: HitInteraction::FocusInput {
                        id: self.id,
                        interactions: self.interactions.clone(),
                        on_change: on_change.clone(),
                        text: current_text,
                    },
                });
            }
        }
    }
}

fn apply_container_style(style: &mut TaffyStyle, layout: ContainerLayout) {
    style.padding = to_taffy_rect(layout.padding);
    style.gap = TaffySize {
        width: length(layout.gap),
        height: length(layout.gap),
    };

    match layout.kind {
        ContainerKind::Flow | ContainerKind::Column => {
            style.display = Display::Flex;
            style.flex_direction = FlexDirection::Column;
            style.justify_content = layout
                .align_y
                .map(map_axis_align_content)
                .or_else(|| map_justify_content(layout.justify));
            style.align_items = map_align_items(layout.align_x.unwrap_or(layout.align));
        }
        ContainerKind::Row => {
            style.display = Display::Flex;
            style.flex_direction = FlexDirection::Row;
            style.justify_content = layout
                .align_x
                .map(map_axis_align_content)
                .or_else(|| map_justify_content(layout.justify));
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
                    style.justify_content = layout
                        .align_x
                        .map(map_axis_align_content)
                        .or_else(|| map_justify_content(layout.justify));
                    style.align_items = map_align_items(layout.align_y.unwrap_or(layout.align));
                }
                Axis::Vertical => {
                    style.justify_content = layout
                        .align_y
                        .map(map_axis_align_content)
                        .or_else(|| map_justify_content(layout.justify));
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

fn map_align_items(align: Align) -> Option<TaffyAlignItems> {
    Some(match align {
        Align::Start => TaffyAlignItems::Start,
        Align::Center => TaffyAlignItems::Center,
        Align::End => TaffyAlignItems::End,
        Align::Stretch => TaffyAlignItems::Stretch,
    })
}

fn map_justify_content(justify: Justify) -> Option<TaffyJustifyContent> {
    Some(match justify {
        Justify::Start => TaffyJustifyContent::Start,
        Justify::Center => TaffyJustifyContent::Center,
        Justify::End => TaffyJustifyContent::End,
        Justify::SpaceBetween => TaffyJustifyContent::SpaceBetween,
    })
}

fn map_axis_align_content(align: Align) -> TaffyJustifyContent {
    match align {
        Align::Start => TaffyJustifyContent::Start,
        Align::Center => TaffyJustifyContent::Center,
        Align::End => TaffyJustifyContent::End,
        Align::Stretch => TaffyJustifyContent::Start,
    }
}

fn to_taffy_rect(insets: Insets) -> taffy::prelude::Rect<taffy::style::LengthPercentage> {
    taffy::prelude::Rect {
        left: length(insets.left),
        right: length(insets.right),
        top: length(insets.top),
        bottom: length(insets.bottom),
    }
}

fn to_taffy_rect_auto(insets: Insets) -> taffy::prelude::Rect<taffy::style::LengthPercentageAuto> {
    taffy::prelude::Rect {
        left: length(insets.left),
        right: length(insets.right),
        top: length(insets.top),
        bottom: length(insets.bottom),
    }
}

fn measure_node(
    node_context: Option<&mut MeasureContext>,
    known_dimensions: TaffySize<Option<f32>>,
    font_manager: &FontManager,
    theme: &Theme,
) -> TaffySize<f32> {
    let measured = match node_context {
        Some(MeasureContext::Text(text)) => measure_text_content(text, font_manager, theme),
        Some(MeasureContext::Button(label)) => measure_text_content(label, font_manager, theme),
        Some(MeasureContext::Input { text, placeholder }) => {
            let text_size = measure_text_content(text, font_manager, theme);
            let placeholder_size = measure_text_content(placeholder, font_manager, theme);
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

fn measure_text_content(text: &Text, font_manager: &FontManager, theme: &Theme) -> (f32, f32) {
    let font_size = text
        .font_size
        .unwrap_or(theme.typography.font_size.max(1.0));
    let line_height = (font_size * 1.25).max(font_size + 4.0);
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
        text.letter_spacing,
    )
}

fn push_text_primitives(
    text: &Text,
    frame: Rect,
    font_manager: &FontManager,
    theme: &Theme,
    animations: &mut AnimationEngine,
    now: std::time::Instant,
    scene: &mut ScenePrimitives,
    show_caret: bool,
    padding: Insets,
    caret_content: Option<&str>,
    fallback_color: Color,
    opacity: f32,
    widget_id: WidgetId,
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
    let font_size = text
        .font_size
        .unwrap_or(theme.typography.font_size.max(1.0));
    let line_height = (font_size * 1.25).max(font_size + 4.0);
    let inner = frame.inset(padding);
    let (measured_width, measured_height) = font_manager.measure_text_raw(
        &content,
        text_request.clone(),
        font_size,
        line_height,
        text.letter_spacing,
    );
    let content_frame = centered_text_frame(inner, measured_width, measured_height, line_height);

    scene.texts.push(TextPrimitive {
        content: content.clone(),
        frame: content_frame,
        color: color.with_alpha_factor(opacity),
        font_family: Some(resolved.primary_font),
        font_size,
        font_weight: text.font_weight,
        line_height,
        letter_spacing: text.letter_spacing,
    });

    if show_caret {
        let caret_text = caret_content.unwrap_or(content.as_str());
        let (caret_width, _) = if caret_text.is_empty() {
            (0.0, line_height)
        } else {
            font_manager.measure_text_raw(
                caret_text,
                text_request,
                font_size,
                line_height,
                text.letter_spacing,
            )
        };
        let caret_x = (inner.x + inner.width.min(caret_width) + 1.0).max(inner.x);
        scene.overlay_shapes.push(RenderPrimitive {
            rect: Rect::new(
                caret_x,
                content_frame.y,
                2.0,
                content_frame.height.max(line_height),
            ),
            color: theme.palette.text.with_alpha_factor(opacity),
            corner_radius: 0.0,
            stroke_width: 0.0,
        });
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
        .max(line_height);

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
) {
    if border_color.a == 0 {
        return;
    }

    let thickness = border_width.min(frame.width * 0.5).min(frame.height * 0.5).max(0.0);
    if thickness <= 0.0 {
        return;
    }

    scene.shapes.push(RenderPrimitive {
        rect: frame,
        color: border_color,
        corner_radius: border_radius,
        stroke_width: thickness,
    });
}

pub struct WidgetTree<VM> {
    root: Element<VM>,
}

impl<VM> WidgetTree<VM> {
    pub fn new(root: impl Into<Element<VM>>) -> Self {
        Self { root: root.into() }
    }

    fn compute_scene(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        animations: &mut AnimationEngine,
        viewport: Rect,
        focused_input: Option<WidgetId>,
    ) -> ComputedScene<VM> {
        let mut taffy = TaffyTree::new();
        let root_layout = self
            .root
            .build_layout_tree(&mut taffy, None, viewport, true)
            .expect("widget tree layout should build");
        taffy
            .compute_layout_with_measure(
                root_layout.node,
                TaffySize {
                    width: AvailableSpace::Definite(viewport.width),
                    height: AvailableSpace::Definite(viewport.height),
                },
                |known_dimensions, _, _, node_context, _| {
                    measure_node(node_context, known_dimensions, font_manager, theme)
                },
            )
            .expect("widget tree layout should compute");

        let mut computed = ComputedScene::default();
        let now = std::time::Instant::now();
        let mut context = CollectContext {
            taffy: &taffy,
            font_manager,
            theme,
            focused_input,
            animations,
            now,
        };
        self.root.collect_primitives(
            &root_layout,
            VisualContext {
                origin: Point {
                    x: viewport.x,
                    y: viewport.y,
                },
                opacity: 1.0,
            },
            &mut context,
            &mut computed,
        );
        computed
    }

    pub(crate) fn render_primitives(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        animations: &mut AnimationEngine,
        viewport: Rect,
        focused_input: Option<WidgetId>,
    ) -> ScenePrimitives {
        self.compute_scene(font_manager, theme, animations, viewport, focused_input)
            .scene
    }

    fn hit_path_from_computed(
        computed: &ComputedScene<VM>,
        point: Point,
    ) -> Vec<HitInteraction<VM>> {
        let mut path = Vec::new();
        let mut ids = Vec::new();

        for hit in computed
            .hit_regions
            .iter()
            .filter(|hit| hit.rect.contains(point))
        {
            let id = match &hit.interaction {
                HitInteraction::Widget { id, .. } | HitInteraction::FocusInput { id, .. } => *id,
            };

            if let Some(index) = ids.iter().position(|existing| *existing == id) {
                path[index] = hit.interaction.clone();
            } else {
                ids.push(id);
                path.push(hit.interaction.clone());
            }
        }

        path
    }

    pub(crate) fn hit_test(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        animations: &mut AnimationEngine,
        viewport: Rect,
        cursor_position: Option<Point>,
        focused_input: Option<WidgetId>,
    ) -> Option<HitInteraction<VM>> {
        self.hit_path(
            font_manager,
            theme,
            animations,
            viewport,
            cursor_position,
            focused_input,
        )
        .pop()
    }

    pub(crate) fn hit_path(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        animations: &mut AnimationEngine,
        viewport: Rect,
        cursor_position: Option<Point>,
        focused_input: Option<WidgetId>,
    ) -> Vec<HitInteraction<VM>> {
        let Some(point) = cursor_position else {
            return Vec::new();
        };
        let computed = self.compute_scene(font_manager, theme, animations, viewport, focused_input);
        Self::hit_path_from_computed(&computed, point)
    }

    pub(crate) fn handle_keyboard_event(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        animations: &mut AnimationEngine,
        viewport: Rect,
        event: &winit::event::KeyEvent,
        focused_input: Option<WidgetId>,
    ) -> WidgetEventResult<VM> {
        let computed = self.compute_scene(font_manager, theme, animations, viewport, focused_input);

        if let Some(focused) = focused_input {
            if event.state != ElementState::Pressed {
                return WidgetEventResult::new(None, Some(focused), false);
            }

            for hit in &computed.hit_regions {
                if let HitInteraction::FocusInput {
                    id,
                    on_change,
                    text,
                    ..
                } = &hit.interaction
                {
                    if *id == focused {
                        let on_change = match on_change.clone() {
                            Some(on_change) => on_change,
                            None => {
                                return WidgetEventResult::new(None, Some(focused), false);
                            }
                        };
                        let mut next_value = text.clone();
                        if matches!(event.physical_key, PhysicalKey::Code(KeyCode::Backspace)) {
                            next_value.pop();
                            return WidgetEventResult::new(
                                Some(WidgetCommand::Value(on_change, next_value)),
                                Some(focused),
                                true,
                            );
                        }

                        if let Some(input) = event.text.as_ref() {
                            let appended = input
                                .chars()
                                .filter(|ch| !ch.is_control())
                                .collect::<String>();
                            if appended.is_empty() {
                                return WidgetEventResult::new(None, Some(focused), false);
                            }
                            next_value.push_str(&appended);
                            return WidgetEventResult::new(
                                Some(WidgetCommand::Value(on_change, next_value)),
                                Some(focused),
                                true,
                            );
                        }
                    }
                }
            }
        }

        WidgetEventResult::new(None, focused_input, false)
    }
}

#[cfg(test)]
mod tests {
    use super::centered_text_frame;
    use crate::ui::widget::common::Rect;

    #[test]
    fn centers_text_using_actual_render_height() {
        let inner = Rect::new(12.0, 8.0, 180.0, 24.0);
        let frame = centered_text_frame(inner, 56.0, 18.0, 18.0);

        assert_eq!(frame.x, 12.0);
        assert_eq!(frame.y, 11.0);
        assert_eq!(frame.width, 56.0);
        assert_eq!(frame.height, 18.0);
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

impl<VM> WidgetEventResult<VM> {
    fn new(
        command: Option<WidgetCommand<VM>>,
        focus: Option<WidgetId>,
        request_redraw: bool,
    ) -> Self {
        Self {
            command,
            focus,
            request_redraw,
        }
    }
}

pub fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::new(x, y, width, height)
}
