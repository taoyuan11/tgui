use std::sync::atomic::{AtomicU64, Ordering};

use taffy::prelude::{
    AlignItems as TaffyAlignItems, AvailableSpace, Display, FlexDirection, FlexWrap,
    JustifyContent as TaffyJustifyContent, Style as TaffyStyle, TaffyTree, auto,
    evenly_sized_tracks, length, line, percent,
};
use taffy::{NodeId as TaffyNodeId, Size as TaffySize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::foundation::binding::Binding;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::text::font::{FontManager, FontWeight, TextFontRequest};
use crate::ui::layout::{Align, Axis, Insets, Justify, LayoutStyle, Wrap};
use crate::ui::theme::Theme;

static NEXT_WIDGET_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WidgetId(u64);

impl WidgetId {
    fn next() -> Self {
        Self(NEXT_WIDGET_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    fn inset(self, insets: Insets) -> Self {
        let width = (self.width - insets.left - insets.right).max(0.0);
        let height = (self.height - insets.top - insets.bottom).max(0.0);
        Self {
            x: self.x + insets.left,
            y: self.y + insets.top,
            width,
            height,
        }
    }
}

#[derive(Clone)]
pub enum Value<T> {
    Static(T),
    Bound(Binding<T>),
}

impl<T: Clone> Value<T> {
    pub fn resolve(&self) -> T {
        match self {
            Self::Static(value) => value.clone(),
            Self::Bound(binding) => binding.get(),
        }
    }
}

impl<T> From<T> for Value<T> {
    fn from(value: T) -> Self {
        Self::Static(value)
    }
}

impl<T> From<Binding<T>> for Value<T> {
    fn from(value: Binding<T>) -> Self {
        Self::Bound(value)
    }
}

#[derive(Clone, Copy)]
pub struct RenderPrimitive {
    pub rect: Rect,
    pub color: wgpu::Color,
}

#[derive(Clone)]
pub struct TextPrimitive {
    pub content: String,
    pub frame: Rect,
    pub color: wgpu::Color,
    pub font_family: Option<String>,
    pub font_size: f32,
    pub font_weight: FontWeight,
    pub line_height: f32,
    pub letter_spacing: f32,
}

#[derive(Clone, Default)]
pub struct ScenePrimitives {
    pub shapes: Vec<RenderPrimitive>,
    pub texts: Vec<TextPrimitive>,
}

#[derive(Clone)]
pub struct Text {
    layout: LayoutStyle,
    content: Value<String>,
    font_family: Option<String>,
    background: Option<Value<wgpu::Color>>,
    color: Option<Value<wgpu::Color>>,
    font_size: Option<f32>,
    font_weight: FontWeight,
    letter_spacing: f32,
}

impl Text {
    pub fn new(content: impl Into<Value<String>>) -> Self {
        Self {
            layout: LayoutStyle::default(),
            content: content.into(),
            font_family: None,
            background: None,
            color: None,
            font_size: None,
            font_weight: FontWeight::NORMAL,
            letter_spacing: 0.0,
        }
    }

    pub fn margin(mut self, insets: Insets) -> Self {
        self.layout.margin = insets;
        self
    }

    pub fn padding(mut self, insets: Insets) -> Self {
        self.layout.padding = insets;
        self
    }

    pub fn font(mut self, font_family: impl Into<String>) -> Self {
        self.font_family = Some(font_family.into());
        self
    }

    pub fn background(mut self, color: impl Into<Value<wgpu::Color>>) -> Self {
        self.background = Some(color.into());
        self
    }

    pub fn color(mut self, color: impl Into<Value<wgpu::Color>>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size.max(1.0));
        self
    }

    pub fn font_weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = weight;
        self
    }

    pub fn letter_spacing(mut self, spacing: f32) -> Self {
        self.letter_spacing = spacing;
        self
    }
}

#[derive(Clone, Copy, Debug)]
enum ContainerKind {
    Flow,
    Stack,
    Row,
    Column,
    Grid { columns: usize },
    Flex { direction: Axis, wrap: Wrap },
}

#[derive(Clone, Copy, Debug)]
struct ContainerLayout {
    kind: ContainerKind,
    padding: Insets,
    gap: f32,
    justify: Justify,
    align: Align,
    align_x: Option<Align>,
    align_y: Option<Align>,
}

impl ContainerLayout {
    fn flow() -> Self {
        Self {
            kind: ContainerKind::Flow,
            padding: Insets::ZERO,
            gap: 0.0,
            justify: Justify::Start,
            align: Align::Start,
            align_x: None,
            align_y: None,
        }
    }
}

enum WidgetKind<VM> {
    Container {
        layout: ContainerLayout,
        children: Vec<Element<VM>>,
    },
    Text {
        text: Text,
    },
    Button {
        label: Text,
        on_click: Option<Command<VM>>,
    },
    Input {
        text: Text,
        placeholder: Text,
        on_change: Option<ValueCommand<VM, String>>,
    },
}

pub struct Element<VM> {
    id: WidgetId,
    layout: LayoutStyle,
    background: Option<Value<wgpu::Color>>,
    kind: WidgetKind<VM>,
}

#[derive(Clone)]
enum MeasureContext {
    None,
    Text(Text),
    Button(Text),
    Input { text: Text, placeholder: Text },
}

struct LayoutNode {
    node: TaffyNodeId,
    children: Vec<LayoutNode>,
}

#[derive(Clone)]
enum HitInteraction<VM> {
    Command(Command<VM>),
    FocusInput {
        id: WidgetId,
        on_change: Option<ValueCommand<VM, String>>,
        text: String,
    },
}

#[derive(Clone)]
struct HitRegion<VM> {
    rect: Rect,
    interaction: HitInteraction<VM>,
}

struct ComputedScene<VM> {
    scene: ScenePrimitives,
    hit_regions: Vec<HitRegion<VM>>,
}

impl<VM> Default for ComputedScene<VM> {
    fn default() -> Self {
        Self {
            scene: ScenePrimitives::default(),
            hit_regions: Vec::new(),
        }
    }
}

impl<VM> Element<VM> {
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
            let child_ids = child_layouts.iter().map(|child| child.node).collect::<Vec<_>>();
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
        taffy: &TaffyTree<MeasureContext>,
        parent_origin: Point,
        font_manager: &FontManager,
        theme: &Theme,
        computed: &mut ComputedScene<VM>,
        focused_input: Option<WidgetId>,
    ) {
        let layout = taffy.layout(layout_node.node).expect("layout node should exist");
        let frame = Rect::new(
            parent_origin.x + layout.location.x,
            parent_origin.y + layout.location.y,
            layout.size.width,
            layout.size.height,
        );

        let background = match &self.kind {
            WidgetKind::Button { .. } => self
                .background
                .as_ref()
                .map(Value::resolve)
                .unwrap_or(theme.palette.accent),
            WidgetKind::Input { .. } => self
                .background
                .as_ref()
                .map(Value::resolve)
                .unwrap_or(theme.palette.input_background),
            _ => self
                .background
                .as_ref()
                .map(Value::resolve)
                .unwrap_or(wgpu::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                }),
        };

        if background.a > 0.0 {
            computed.scene.shapes.push(RenderPrimitive {
                rect: frame,
                color: background,
            });
        }

        match &self.kind {
            WidgetKind::Container { children, .. } => {
                for (child, child_layout) in children.iter().zip(layout_node.children.iter()) {
                    child.collect_primitives(
                        child_layout,
                        taffy,
                        Point {
                            x: frame.x,
                            y: frame.y,
                        },
                        font_manager,
                        theme,
                        computed,
                        focused_input,
                    );
                }
            }
            WidgetKind::Text { text } => {
                push_text_primitives(
                    text,
                    frame,
                    font_manager,
                    theme,
                    &mut computed.scene,
                    false,
                    text.layout.padding,
                );
            }
            WidgetKind::Button { label, on_click } => {
                push_text_primitives(
                    label,
                    frame,
                    font_manager,
                    theme,
                    &mut computed.scene,
                    false,
                    self.layout.padding,
                );
                if let Some(command) = on_click.clone() {
                    computed.hit_regions.push(HitRegion {
                        rect: frame,
                        interaction: HitInteraction::Command(command),
                    });
                }
            }
            WidgetKind::Input {
                text,
                placeholder,
                on_change,
            } => {
                let active = focused_input == Some(self.id);
                let has_text = !text.content.resolve().is_empty();
                let text_to_draw = if has_text { text } else { placeholder };
                push_text_primitives(
                    text_to_draw,
                    frame,
                    font_manager,
                    theme,
                    &mut computed.scene,
                    active,
                    self.layout.padding,
                );
                computed.hit_regions.push(HitRegion {
                    rect: frame,
                    interaction: HitInteraction::FocusInput {
                        id: self.id,
                        on_change: on_change.clone(),
                        text: text.content.resolve(),
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
            style.justify_content =
                layout.align_y.map(map_axis_align_content).or_else(|| map_justify_content(layout.justify));
            style.align_items = map_align_items(layout.align_x.unwrap_or(layout.align));
        }
        ContainerKind::Row => {
            style.display = Display::Flex;
            style.flex_direction = FlexDirection::Row;
            style.justify_content =
                layout.align_x.map(map_axis_align_content).or_else(|| map_justify_content(layout.justify));
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
                    style.justify_content =
                        layout.align_x.map(map_axis_align_content).or_else(|| map_justify_content(layout.justify));
                    style.align_items = map_align_items(layout.align_y.unwrap_or(layout.align));
                }
                Axis::Vertical => {
                    style.justify_content =
                        layout.align_y.map(map_axis_align_content).or_else(|| map_justify_content(layout.justify));
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

fn to_taffy_rect_auto(
    insets: Insets,
) -> taffy::prelude::Rect<taffy::style::LengthPercentageAuto> {
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
            (text_size.0.max(placeholder_size.0), text_size.1.max(placeholder_size.1))
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
    scene: &mut ScenePrimitives,
    show_caret: bool,
    padding: Insets,
) {
    let content = text.content.resolve();
    let resolved = font_manager.resolve_text(
        &content,
        TextFontRequest {
            preferred_font: text
                .font_family
                .as_deref()
                .or(theme.typography.font_family.as_deref()),
            weight: text.font_weight,
        },
    );

    let color = text
        .color
        .as_ref()
        .map(Value::resolve)
        .unwrap_or(theme.palette.text);
    let font_size = text
        .font_size
        .unwrap_or(theme.typography.font_size.max(1.0));
    let line_height = (font_size * 1.25).max(font_size + 4.0);
    let inner = frame.inset(padding);
    let (measured_width, measured_height) = font_manager.measure_text(
        &content,
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
    );
    let content_frame = Rect::new(
        inner.x,
        inner.y + ((inner.height - measured_height).max(0.0) * 0.5),
        inner.width.min(measured_width).max(0.0),
        inner.height.min(measured_height.max(line_height)).max(line_height),
    );

    scene.texts.push(TextPrimitive {
        content,
        frame: content_frame,
        color,
        font_family: Some(resolved.primary_font),
        font_size,
        font_weight: text.font_weight,
        line_height,
        letter_spacing: text.letter_spacing,
    });

    if show_caret {
        let caret_x = (content_frame.x + content_frame.width - 2.0).max(content_frame.x);
        scene.shapes.push(RenderPrimitive {
            rect: Rect::new(caret_x, frame.y + 8.0, 2.0, frame.height - 16.0),
            color: theme.palette.text,
        });
    }
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
        self.root.collect_primitives(
            &root_layout,
            &taffy,
            Point {
                x: viewport.x,
                y: viewport.y,
            },
            font_manager,
            theme,
            &mut computed,
            focused_input,
        );
        computed
    }

    pub(crate) fn render_primitives(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        viewport: Rect,
        focused_input: Option<WidgetId>,
    ) -> ScenePrimitives {
        self.compute_scene(font_manager, theme, viewport, focused_input)
            .scene
    }

    pub(crate) fn handle_window_event(
        &self,
        font_manager: &FontManager,
        theme: &Theme,
        viewport: Rect,
        event: &WindowEvent,
        cursor_position: Option<Point>,
        focused_input: Option<WidgetId>,
    ) -> WidgetEventResult<VM> {
        let computed = self.compute_scene(font_manager, theme, viewport, focused_input);

        match event {
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(point) = cursor_position {
                    for hit in computed.hit_regions.iter().rev() {
                        if hit.rect.contains(point) {
                            return match &hit.interaction {
                                HitInteraction::Command(command) => WidgetEventResult::new(
                                    Some(WidgetCommand::Command(command.clone())),
                                    None,
                                    true,
                                ),
                                HitInteraction::FocusInput { id, .. } => {
                                    WidgetEventResult::new(None, Some(*id), true)
                                }
                            };
                        }
                    }
                }
                WidgetEventResult::new(None, None, false)
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let Some(focused) = focused_input {
                    if event.state != ElementState::Pressed {
                        return WidgetEventResult::new(None, Some(focused), false);
                    }

                    for hit in &computed.hit_regions {
                        if let HitInteraction::FocusInput {
                            id,
                            on_change,
                            text,
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
                                if matches!(
                                    event.physical_key,
                                    PhysicalKey::Code(KeyCode::Backspace)
                                ) {
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
            _ => WidgetEventResult::new(None, focused_input, false),
        }
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

pub struct Container<VM> {
    element: Element<VM>,
}

impl<VM> Container<VM> {
    pub fn new() -> Self {
        Self::with_layout(ContainerLayout::flow())
    }

    fn with_layout(layout: ContainerLayout) -> Self {
        Self {
            element: Element {
                id: WidgetId::next(),
                layout: LayoutStyle::default(),
                background: None,
                kind: WidgetKind::Container {
                    layout,
                    children: Vec::new(),
                },
            },
        }
    }

    pub fn background(mut self, color: impl Into<Value<wgpu::Color>>) -> Self {
        self.element.background = Some(color.into());
        self
    }

    pub fn child(mut self, child: impl Into<Element<VM>>) -> Self {
        if let WidgetKind::Container { children, .. } = &mut self.element.kind {
            children.push(child.into());
        }
        self
    }

    pub fn padding(mut self, padding: Insets) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.padding = padding;
        }
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.gap = gap;
        }
        self
    }

    pub fn justify(mut self, justify: Justify) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.justify = justify;
        }
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.align = align;
            layout.align_x = Some(align);
            layout.align_y = Some(align);
        }
        self
    }

    pub fn align_x(mut self, align: Align) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.align_x = Some(align);
        }
        self
    }

    pub fn align_y(mut self, align: Align) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.element.kind {
            layout.align_y = Some(align);
        }
        self
    }
}

impl<VM> From<Container<VM>> for Element<VM> {
    fn from(value: Container<VM>) -> Self {
        value.element
    }
}

pub struct Stack<VM>(Container<VM>);
pub struct Row<VM>(Container<VM>);
pub struct Column<VM>(Container<VM>);
pub struct Grid<VM>(Container<VM>);
pub struct Flex<VM>(Container<VM>);

macro_rules! impl_layout_container {
    ($name:ident) => {
        impl<VM> $name<VM> {
            pub fn size(mut self, width: f32, height: f32) -> Self {
                self.0.element.layout.width = Some(width);
                self.0.element.layout.height = Some(height);
                self.0.element.layout.fill_width = false;
                self.0.element.layout.fill_height = false;
                self
            }
            pub fn width(mut self, width: f32) -> Self {
                self.0.element.layout.width = Some(width);
                self.0.element.layout.fill_width = false;
                self
            }
            pub fn height(mut self, height: f32) -> Self {
                self.0.element.layout.height = Some(height);
                self.0.element.layout.fill_height = false;
                self
            }
            pub fn fill_width(mut self) -> Self {
                self.0.element.layout.fill_width = true;
                self.0.element.layout.width = None;
                self
            }
            pub fn fill_height(mut self) -> Self {
                self.0.element.layout.fill_height = true;
                self.0.element.layout.height = None;
                self
            }
            pub fn fill_size(mut self) -> Self {
                self.0.element.layout.fill_width = true;
                self.0.element.layout.fill_height = true;
                self.0.element.layout.width = None;
                self.0.element.layout.height = None;
                self
            }
            pub fn margin(mut self, insets: Insets) -> Self {
                self.0.element.layout.margin = insets;
                self
            }
            pub fn grow(mut self, grow: f32) -> Self {
                self.0.element.layout.grow = grow;
                self
            }
            pub fn background(self, color: impl Into<Value<wgpu::Color>>) -> Self {
                Self(self.0.background(color))
            }
            pub fn child(self, child: impl Into<Element<VM>>) -> Self {
                Self(self.0.child(child))
            }
            pub fn padding(self, padding: Insets) -> Self {
                Self(self.0.padding(padding))
            }
            pub fn gap(self, gap: f32) -> Self {
                Self(self.0.gap(gap))
            }
            pub fn justify(self, justify: Justify) -> Self {
                Self(self.0.justify(justify))
            }
            pub fn align(self, align: Align) -> Self {
                Self(self.0.align(align))
            }
            pub fn align_x(self, align: Align) -> Self {
                Self(self.0.align_x(align))
            }
            pub fn align_y(self, align: Align) -> Self {
                Self(self.0.align_y(align))
            }
        }
        impl<VM> From<$name<VM>> for Element<VM> {
            fn from(value: $name<VM>) -> Self {
                value.0.into()
            }
        }
    };
}

impl<VM> Stack<VM> {
    pub fn new() -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Stack,
            ..ContainerLayout::flow()
        }))
    }
}

impl<VM> Row<VM> {
    pub fn new() -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Row,
            ..ContainerLayout::flow()
        }))
    }
}

impl<VM> Column<VM> {
    pub fn new() -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Column,
            ..ContainerLayout::flow()
        }))
    }
}

impl<VM> Grid<VM> {
    pub fn new(columns: usize) -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Grid {
                columns: columns.max(1),
            },
            ..ContainerLayout::flow()
        }))
    }
}

impl<VM> Flex<VM> {
    pub fn new(direction: Axis) -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Flex {
                direction,
                wrap: Wrap::NoWrap,
            },
            ..ContainerLayout::flow()
        }))
    }

    pub fn wrap(mut self, wrap: Wrap) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.0.element.kind {
            layout.kind = match layout.kind {
                ContainerKind::Flex { direction, .. } => ContainerKind::Flex { direction, wrap },
                other => other,
            };
        }
        self
    }
}

impl_layout_container!(Stack);
impl_layout_container!(Row);
impl_layout_container!(Column);
impl_layout_container!(Grid);
impl_layout_container!(Flex);

impl<VM> From<Text> for Element<VM> {
    fn from(value: Text) -> Self {
        Element {
            id: WidgetId::next(),
            layout: value.layout,
            background: value.background.clone(),
            kind: WidgetKind::Text { text: value },
        }
    }
}

pub struct Button<VM> {
    element: Element<VM>,
}

impl<VM> Button<VM> {
    pub fn new(label: Text) -> Self {
        Self {
            element: Element {
                id: WidgetId::next(),
                layout: LayoutStyle {
                    padding: Insets::symmetric(12.0, 8.0),
                    ..LayoutStyle::default()
                },
                background: None,
                kind: WidgetKind::Button {
                    label,
                    on_click: None,
                },
            },
        }
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.element.layout.width = Some(width);
        self.element.layout.height = Some(height);
        self.element.layout.fill_width = false;
        self.element.layout.fill_height = false;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.element.layout.width = Some(width);
        self.element.layout.fill_width = false;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.element.layout.height = Some(height);
        self.element.layout.fill_height = false;
        self
    }

    pub fn fill_width(mut self) -> Self {
        self.element.layout.fill_width = true;
        self.element.layout.width = None;
        self
    }

    pub fn fill_height(mut self) -> Self {
        self.element.layout.fill_height = true;
        self.element.layout.height = None;
        self
    }

    pub fn fill_size(mut self) -> Self {
        self.element.layout.fill_width = true;
        self.element.layout.fill_height = true;
        self.element.layout.width = None;
        self.element.layout.height = None;
        self
    }

    pub fn margin(mut self, insets: Insets) -> Self {
        self.element.layout.margin = insets;
        self
    }

    pub fn padding(mut self, insets: Insets) -> Self {
        self.element.layout.padding = insets;
        self
    }

    pub fn grow(mut self, grow: f32) -> Self {
        self.element.layout.grow = grow;
        self
    }

    pub fn background(mut self, color: impl Into<Value<wgpu::Color>>) -> Self {
        self.element.background = Some(color.into());
        self
    }

    pub fn on_click(mut self, command: Command<VM>) -> Self {
        if let WidgetKind::Button { on_click, .. } = &mut self.element.kind {
            *on_click = Some(command);
        }
        self
    }
}

impl<VM> From<Button<VM>> for Element<VM> {
    fn from(value: Button<VM>) -> Self {
        value.element
    }
}

pub struct Input<VM> {
    element: Element<VM>,
}

impl<VM> Input<VM> {
    pub fn new(text: Text) -> Self {
        Self {
            element: Element {
                id: WidgetId::next(),
                layout: LayoutStyle {
                    padding: Insets::symmetric(12.0, 8.0),
                    ..LayoutStyle::default()
                },
                background: None,
                kind: WidgetKind::Input {
                    text,
                    placeholder: Text::new(String::new()),
                    on_change: None,
                },
            },
        }
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.element.layout.width = Some(width);
        self.element.layout.height = Some(height);
        self.element.layout.fill_width = false;
        self.element.layout.fill_height = false;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.element.layout.width = Some(width);
        self.element.layout.fill_width = false;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.element.layout.height = Some(height);
        self.element.layout.fill_height = false;
        self
    }

    pub fn fill_width(mut self) -> Self {
        self.element.layout.fill_width = true;
        self.element.layout.width = None;
        self
    }

    pub fn fill_height(mut self) -> Self {
        self.element.layout.fill_height = true;
        self.element.layout.height = None;
        self
    }

    pub fn fill_size(mut self) -> Self {
        self.element.layout.fill_width = true;
        self.element.layout.fill_height = true;
        self.element.layout.width = None;
        self.element.layout.height = None;
        self
    }

    pub fn margin(mut self, insets: Insets) -> Self {
        self.element.layout.margin = insets;
        self
    }

    pub fn padding(mut self, insets: Insets) -> Self {
        self.element.layout.padding = insets;
        self
    }

    pub fn grow(mut self, grow: f32) -> Self {
        self.element.layout.grow = grow;
        self
    }

    pub fn placeholder(mut self, placeholder: Text) -> Self {
        if let WidgetKind::Input {
            placeholder: value, ..
        } = &mut self.element.kind
        {
            *value = placeholder;
        }
        self
    }

    pub fn background(mut self, color: impl Into<Value<wgpu::Color>>) -> Self {
        self.element.background = Some(color.into());
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, String>) -> Self {
        if let WidgetKind::Input { on_change, .. } = &mut self.element.kind {
            *on_change = Some(command);
        }
        self
    }
}

impl<VM> From<Input<VM>> for Element<VM> {
    fn from(value: Input<VM>) -> Self {
        value.element
    }
}

pub fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::new(x, y, width, height)
}
