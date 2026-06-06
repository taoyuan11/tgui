use super::*;
use crate::ui::widget::style::{DividerStyle, ProgressBarStyle, SpinnerStyle};

#[derive(Clone, Debug, PartialEq)]
pub struct FocusScopeOptions {
    pub(crate) trap: bool,
    pub(crate) auto_focus_first: bool,
    pub(crate) active: Value<bool>,
}

impl FocusScopeOptions {
    pub fn new() -> Self {
        Self {
            trap: false,
            auto_focus_first: false,
            active: Value::Static(true),
        }
    }

    pub fn trap(mut self, trap: bool) -> Self {
        self.trap = trap;
        self
    }

    pub fn auto_focus_first(mut self, auto_focus_first: bool) -> Self {
        self.auto_focus_first = auto_focus_first;
        self
    }

    pub fn active(mut self, active: impl Into<Value<bool>>) -> Self {
        self.active = active.into();
        self
    }

    pub fn is_trap(&self) -> bool {
        self.trap
    }

    pub fn is_auto_focus_first(&self) -> bool {
        self.auto_focus_first
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.resolve()
    }
}

impl Default for FocusScopeOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DefaultActivation {
    None,
    Enter,
    Space,
    EnterAndSpace,
}

impl DefaultActivation {
    pub(crate) fn handles_enter(self) -> bool {
        matches!(self, Self::Enter | Self::EnterAndSpace)
    }

    pub(crate) fn handles_space(self) -> bool {
        matches!(self, Self::Space | Self::EnterAndSpace)
    }
}

pub(crate) struct FocusTargetMeta<VM> {
    pub(crate) widget_id: WidgetId,
    pub(crate) tab_index: Option<i32>,
    pub(crate) order: usize,
    pub(crate) scope_path: Vec<WidgetId>,
    pub(crate) on_focus: Option<Command<VM>>,
    pub(crate) on_blur: Option<Command<VM>>,
}

impl<VM> Clone for FocusTargetMeta<VM> {
    fn clone(&self) -> Self {
        Self {
            widget_id: self.widget_id,
            tab_index: self.tab_index,
            order: self.order,
            scope_path: self.scope_path.clone(),
            on_focus: self.on_focus.clone(),
            on_blur: self.on_blur.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FocusScopeState {
    pub(crate) scope_id: WidgetId,
    pub(crate) path: Vec<WidgetId>,
    pub(crate) options: FocusScopeOptions,
    pub(crate) active: bool,
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
    ProgressBar {
        id: WidgetId,
        value: Value<f32>,
        indeterminate: Value<bool>,
        show_label: bool,
        label: Option<Value<String>>,
        style: ProgressBarStyle,
    },
    Spinner {
        id: WidgetId,
        style: SpinnerStyle,
        size_override: Option<Value<Dp>>,
        thickness_override: Option<Value<Dp>>,
        track_override: Option<bool>,
    },
    Divider {
        id: WidgetId,
        orientation: DividerOrientation,
        thickness_override: Option<Value<Dp>>,
        label: Option<Value<String>>,
        style: DividerStyle,
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
    pub absolute_offset: Option<Point>,
    pub absolute_size: Option<(Dp, Dp)>,
}

pub(crate) enum HitInteraction<VM> {
    Occluder {
        id: WidgetId,
    },
    Disabled {
        id: WidgetId,
    },
    Widget {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        focusable: bool,
        default_activation: DefaultActivation,
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
    TabTrigger {
        id: WidgetId,
        group_id: WidgetId,
        index: usize,
        placement: TabPlacement,
        key: String,
        label: String,
        interactions: InteractionHandlers<VM>,
        on_change: Option<ValueCommand<VM, (String, String)>>,
        reorderable: bool,
        on_reorder: Option<ValueCommand<VM, crate::ui::widget::TabsReorderEvent>>,
    },
    ListItem {
        id: WidgetId,
        state: crate::ui::widget::common::ListItemState<VM>,
        interactions: InteractionHandlers<VM>,
    },
    TreeNode {
        id: WidgetId,
        state: crate::ui::widget::common::TreeNodeState<VM>,
        interactions: InteractionHandlers<VM>,
    },
    TreeDisclosure {
        id: WidgetId,
        state: crate::ui::widget::common::TreeNodeState<VM>,
        interactions: InteractionHandlers<VM>,
    },
    TreeCheckbox {
        id: WidgetId,
        state: crate::ui::widget::common::TreeNodeState<VM>,
        interactions: InteractionHandlers<VM>,
    },
    DataGridCell {
        id: WidgetId,
        state: crate::ui::widget::common::DataGridCellState<VM>,
        interactions: InteractionHandlers<VM>,
    },
    DataGridHeader {
        id: WidgetId,
        state: crate::ui::widget::common::DataGridHeaderState<VM>,
        interactions: InteractionHandlers<VM>,
    },
    DataGridResizeHandle {
        id: WidgetId,
        state: crate::ui::widget::common::DataGridResizeHandleState<VM>,
        interactions: InteractionHandlers<VM>,
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
            Self::Occluder { id } => Self::Occluder { id: *id },
            Self::Disabled { id } => Self::Disabled { id: *id },
            Self::Widget {
                id,
                interactions,
                focusable,
                default_activation,
            } => Self::Widget {
                id: *id,
                interactions: interactions.clone(),
                focusable: *focusable,
                default_activation: *default_activation,
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
            Self::TabTrigger {
                id,
                group_id,
                index,
                placement,
                key,
                label,
                interactions,
                on_change,
                reorderable,
                on_reorder,
            } => Self::TabTrigger {
                id: *id,
                group_id: *group_id,
                index: *index,
                placement: *placement,
                key: key.clone(),
                label: label.clone(),
                interactions: interactions.clone(),
                on_change: on_change.clone(),
                reorderable: *reorderable,
                on_reorder: on_reorder.clone(),
            },
            Self::ListItem {
                id,
                state,
                interactions,
            } => Self::ListItem {
                id: *id,
                state: state.clone(),
                interactions: interactions.clone(),
            },
            Self::TreeNode {
                id,
                state,
                interactions,
            } => Self::TreeNode {
                id: *id,
                state: state.clone(),
                interactions: interactions.clone(),
            },
            Self::TreeDisclosure {
                id,
                state,
                interactions,
            } => Self::TreeDisclosure {
                id: *id,
                state: state.clone(),
                interactions: interactions.clone(),
            },
            Self::TreeCheckbox {
                id,
                state,
                interactions,
            } => Self::TreeCheckbox {
                id: *id,
                state: state.clone(),
                interactions: interactions.clone(),
            },
            Self::DataGridCell {
                id,
                state,
                interactions,
            } => Self::DataGridCell {
                id: *id,
                state: state.clone(),
                interactions: interactions.clone(),
            },
            Self::DataGridHeader {
                id,
                state,
                interactions,
            } => Self::DataGridHeader {
                id: *id,
                state: state.clone(),
                interactions: interactions.clone(),
            },
            Self::DataGridResizeHandle {
                id,
                state,
                interactions,
            } => Self::DataGridResizeHandle {
                id: *id,
                state: state.clone(),
                interactions: interactions.clone(),
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

impl<VM> HitInteraction<VM> {
    pub(crate) fn target_id(&self) -> HitTargetId {
        match self {
            Self::Occluder { id }
            | Self::Disabled { id }
            | Self::Widget { id, .. }
            | Self::SelectableText { id, .. }
            | Self::Switch { id, .. }
            | Self::Checkbox { id, .. }
            | Self::Radio { id, .. }
            | Self::SelectTrigger { id, .. }
            | Self::TabTrigger { id, .. }
            | Self::ListItem { id, .. }
            | Self::TreeNode { id, .. }
            | Self::TreeDisclosure { id, .. }
            | Self::TreeCheckbox { id, .. }
            | Self::DataGridCell { id, .. }
            | Self::DataGridHeader { id, .. }
            | Self::DataGridResizeHandle { id, .. }
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
    pub scope_path: Vec<WidgetId>,
    pub focus: Option<FocusTargetMeta<VM>>,
    pub interaction: HitInteraction<VM>,
}

impl<VM> Clone for HitRegion<VM> {
    fn clone(&self) -> Self {
        Self {
            rect: self.rect,
            clip_rect: self.clip_rect,
            geometry: self.geometry.clone(),
            scope_path: self.scope_path.clone(),
            focus: self.focus.clone(),
            interaction: self.interaction.clone(),
        }
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
