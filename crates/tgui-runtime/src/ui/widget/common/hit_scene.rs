use super::*;
use crate::ui::widget::style::{DividerStyle, ProgressBarStyle, SpinnerStyle};

#[derive(Clone, Debug, PartialEq)]
pub struct FocusScopeOptions {
    pub(crate) trap: bool,
    pub(crate) auto_focus_first: bool,
    pub(crate) active: Value<bool>,
    /// Internal component gate used by retained visual transitions. An inactive
    /// scope normally only removes descendants from keyboard focus order. Tabs,
    /// carousels, drawers, and similar view stacks additionally need the outgoing
    /// visual subtree to stop receiving *all* input as soon as logical selection
    /// changes, while its opacity/offset are still animating.
    pub(crate) suppress_interactions_when_inactive: bool,
}

impl FocusScopeOptions {
    pub fn new() -> Self {
        Self {
            trap: false,
            auto_focus_first: false,
            active: Value::Static(true),
            suppress_interactions_when_inactive: false,
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

    pub(crate) fn suppress_interactions_when_inactive(mut self) -> Self {
        self.suppress_interactions_when_inactive = true;
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

    pub(crate) fn is_active_untracked(&self) -> bool {
        self.active.resolve_untracked()
    }

    pub(crate) fn suppresses_interactions(&self, active: bool) -> bool {
        self.suppress_interactions_when_inactive && !active
    }
}

impl Default for FocusScopeOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)] // Single-key variants are kept for widgets that should not handle both keys.
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
        orientation: SliderOrientation,
    },
    ProgressBar {
        id: WidgetId,
        show_label: bool,
        label: Option<Value<String>>,
        style: ProgressBarStyle,
    },
    Spinner {
        id: WidgetId,
        style: SpinnerStyle,
        size_override: Option<Value<Dp>>,
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
    /// Immediate-child content bounds in coordinates local to this node.
    ///
    /// Scene-only recollection keeps the Taffy layout stable, so scroll containers with
    /// static child offsets can reuse this value instead of walking every child each frame.
    /// `None` inside the lock represents a container without children; an uninitialized lock
    /// means either the bounds have not been computed yet or a dynamic child offset prevents
    /// caching.
    pub(crate) cached_child_content_bounds: std::sync::OnceLock<Option<Rect>>,
    /// Monotonic immediate-child intervals used to binary-search the visible slice during
    /// scene recollection. `Some(None)` records that this layout cannot use the fast path.
    pub(crate) cached_child_cull_index: std::sync::OnceLock<Option<ChildCullIndex>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChildCullAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug)]
pub(crate) struct ChildCullIndex {
    pub(crate) axis: ChildCullAxis,
    pub(crate) intervals: Vec<ChildCullInterval>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ChildCullInterval {
    pub(crate) start: Dp,
    pub(crate) end: Dp,
}

impl LayoutNode {
    #[cfg(feature = "bench-support")]
    pub(crate) fn clear_cached_child_content_bounds(&mut self) {
        self.cached_child_content_bounds = std::sync::OnceLock::new();
        for child in &mut self.children {
            child.clear_cached_child_content_bounds();
        }
    }

    pub(crate) fn clear_cached_layout_metadata(&mut self) {
        self.cached_child_content_bounds = std::sync::OnceLock::new();
        self.cached_child_cull_index = std::sync::OnceLock::new();
        for child in &mut self.children {
            child.clear_cached_layout_metadata();
        }
    }

    #[cfg(feature = "bench-support")]
    pub(crate) fn disable_cached_child_culling(&mut self) {
        self.cached_child_cull_index = std::sync::OnceLock::from(None);
        for child in &mut self.children {
            child.disable_cached_child_culling();
        }
    }
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
    SplitterHandle {
        id: WidgetId,
        state: crate::ui::widget::common::SplitterHandleState<VM>,
        interactions: InteractionHandlers<VM>,
        pair_extent: Dp,
    },
    Slider {
        id: WidgetId,
        interactions: InteractionHandlers<VM>,
        on_change: Option<ValueCommand<VM, f32>>,
        on_change_end: Option<ValueCommand<VM, f32>>,
        value: f32,
        min: f32,
        max: f32,
        step: f32,
        orientation: SliderOrientation,
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
        /// Owner used only for hover/pressed visual state. This differs from `id` for nested menu
        /// levels so equal local option indices cannot alias across the submenu path.
        state_id: WidgetId,
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

pub(crate) type HitPath<VM> = smallvec::SmallVec<[HitInteraction<VM>; 8]>;

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
            Self::SplitterHandle {
                id,
                state,
                interactions,
                pair_extent,
            } => Self::SplitterHandle {
                id: *id,
                state: state.clone(),
                interactions: interactions.clone(),
                pair_extent: *pair_extent,
            },
            Self::Slider {
                id,
                interactions,
                on_change,
                on_change_end,
                value,
                min,
                max,
                step,
                orientation,
                track_rect,
                thumb_rect,
            } => Self::Slider {
                id: *id,
                interactions: interactions.clone(),
                on_change: on_change.clone(),
                on_change_end: on_change_end.clone(),
                value: *value,
                min: *min,
                max: *max,
                step: *step,
                orientation: *orientation,
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
                state_id,
                option_index,
                interactions,
                on_select,
                on_open_change,
            } => Self::SelectOption {
                id: *id,
                state_id: *state_id,
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
    /// Keyboard activation capabilities used by the runtime focus-navigation index.
    ///
    /// Handler payloads and control state are read from the current `HitRegion` at dispatch time,
    /// so replacing a command or toggling a control cannot leave a stale cached interaction.
    pub(crate) fn keyboard_activation(&self) -> Option<(WidgetId, bool, bool)> {
        let (id, enter, space) = match self {
            Self::Widget {
                id,
                interactions,
                default_activation,
                ..
            } if interactions.on_click.is_some() => (
                *id,
                default_activation.handles_enter(),
                default_activation.handles_space(),
            ),
            Self::Checkbox { id, on_change, .. }
            | Self::Radio { id, on_change, .. }
            | Self::Switch { id, on_change, .. }
                if on_change.is_some() =>
            {
                (*id, false, true)
            }
            Self::SelectTrigger { id, .. } => (*id, true, false),
            Self::TabTrigger { id, on_change, .. } if on_change.is_some() => (*id, true, true),
            _ => return None,
        };
        (enter || space).then_some((id, enter, space))
    }

    pub(crate) fn widget_id(&self) -> WidgetId {
        match self.target_id() {
            HitTargetId::Widget(id) => id,
            HitTargetId::SelectOption { widget_id, .. }
            | HitTargetId::CanvasItem { widget_id, .. } => widget_id,
        }
    }

    pub(crate) fn translated(mut self, delta: Point) -> Self {
        if delta == Point::ZERO {
            return self;
        }
        match &mut self {
            Self::SelectableText { frame, .. } | Self::TextInput { frame, .. } => {
                translate_hit_rect(frame, delta);
            }
            Self::Slider {
                track_rect,
                thumb_rect,
                ..
            } => {
                translate_hit_rect(track_rect, delta);
                translate_hit_rect(thumb_rect, delta);
            }
            Self::CanvasItem {
                canvas_origin,
                item_origin,
                text_hits,
                ..
            } => {
                canvas_origin.x += delta.x;
                canvas_origin.y += delta.y;
                item_origin.x += delta.x;
                item_origin.y += delta.y;
                *text_hits = Arc::from(
                    text_hits
                        .iter()
                        .cloned()
                        .map(|hit| CanvasTextHitRegion {
                            hit: hit.hit,
                            quad: hit.quad.map(|point| Point {
                                x: point.x + delta.x,
                                y: point.y + delta.y,
                            }),
                        })
                        .collect::<Vec<_>>(),
                );
            }
            _ => {}
        }
        self
    }

    pub(crate) fn interactions(&self) -> Option<&InteractionHandlers<VM>> {
        match self {
            Self::Widget { interactions, .. }
            | Self::SelectableText { interactions, .. }
            | Self::Switch { interactions, .. }
            | Self::Checkbox { interactions, .. }
            | Self::Radio { interactions, .. }
            | Self::SelectTrigger { interactions, .. }
            | Self::TabTrigger { interactions, .. }
            | Self::ListItem { interactions, .. }
            | Self::TreeNode { interactions, .. }
            | Self::TreeDisclosure { interactions, .. }
            | Self::TreeCheckbox { interactions, .. }
            | Self::DataGridCell { interactions, .. }
            | Self::DataGridHeader { interactions, .. }
            | Self::DataGridResizeHandle { interactions, .. }
            | Self::SplitterHandle { interactions, .. }
            | Self::Slider { interactions, .. }
            | Self::TextInput { interactions, .. }
            | Self::SelectOption { interactions, .. } => Some(interactions),
            Self::Occluder { .. } | Self::Disabled { .. } | Self::CanvasItem { .. } => None,
        }
    }

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
            | Self::SplitterHandle { id, .. }
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

fn translate_hit_rect(rect: &mut Rect, delta: Point) {
    rect.x += delta.x;
    rect.y += delta.y;
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

    pub(crate) fn focus_overlay_patch_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Rect, Self::Rect) => true,
            (Self::Quad(left), Self::Quad(right)) => left == right,
            (Self::Triangles(left), Self::Triangles(right)) => left.as_ref() == right.as_ref(),
            _ => false,
        }
    }
}

pub(crate) struct HitRegion<VM> {
    pub rect: Rect,
    pub clip_rect: Option<Rect>,
    pub geometry: HitGeometry,
    pub transform_chain: TransformChain,
    pub scope_path: Vec<WidgetId>,
    pub focus: Option<FocusTargetMeta<VM>>,
    pub interaction: HitInteraction<VM>,
    pub gpu_scroll_container: Option<WidgetId>,
}

impl<VM> Clone for HitRegion<VM> {
    fn clone(&self) -> Self {
        Self {
            rect: self.rect,
            clip_rect: self.clip_rect,
            geometry: self.geometry.clone(),
            transform_chain: self.transform_chain.clone(),
            scope_path: self.scope_path.clone(),
            focus: self.focus.clone(),
            interaction: self.interaction.clone(),
            gpu_scroll_container: self.gpu_scroll_container,
        }
    }
}

impl<VM> HitRegion<VM> {
    /// Equality for metadata consumed by cached keyboard focus navigation.
    ///
    /// Geometry, transforms, visual state, handlers, and mutable control payloads are excluded:
    /// dispatch fetches those from the current scene. Paint-only and transform-only patches can
    /// therefore retain the cache while tab order, scope membership, and key capability cannot.
    pub(crate) fn focus_navigation_metadata_eq(&self, other: &Self) -> bool {
        let focus_eq = match (&self.focus, &other.focus) {
            (None, None) => true,
            (Some(left), Some(right)) => {
                left.widget_id == right.widget_id
                    && left.tab_index == right.tab_index
                    && left.order == right.order
                    && left.scope_path == right.scope_path
            }
            (None, Some(_)) | (Some(_), None) => false,
        };
        focus_eq
            && self.interaction.keyboard_activation() == other.interaction.keyboard_activation()
    }

    /// Equality for the geometry and interaction shape retained by the focus-ring-only patch.
    /// Callback payloads are intentionally not compared (they are opaque commands), while a
    /// change in target kind, focus metadata, or keyboard capability rejects the bounded path.
    pub(crate) fn focus_overlay_patch_metadata_eq(&self, other: &Self) -> bool {
        let focus_eq = match (&self.focus, &other.focus) {
            (None, None) => true,
            (Some(left), Some(right)) => {
                left.widget_id == right.widget_id
                    && left.tab_index == right.tab_index
                    && left.scope_path == right.scope_path
            }
            _ => false,
        };
        self.rect == other.rect
            && self.clip_rect == other.clip_rect
            && self.geometry.focus_overlay_patch_eq(&other.geometry)
            && self.transform_chain == other.transform_chain
            && self.scope_path == other.scope_path
            && self.gpu_scroll_container == other.gpu_scroll_container
            && focus_eq
            && self.interaction.keyboard_activation() == other.interaction.keyboard_activation()
            && self.interaction.target_id() == other.interaction.target_id()
            && std::mem::discriminant(&self.interaction)
                == std::mem::discriminant(&other.interaction)
    }

    pub(crate) fn transform_delta(
        &self,
        transform_records: &std::collections::HashMap<WidgetId, TransformRecord>,
    ) -> Point {
        let mut delta = Point::ZERO;
        for id in &self.transform_chain {
            if let Some(record) = transform_records.get(id) {
                let record_delta = (*record).delta();
                delta.x += record_delta.x;
                delta.y += record_delta.y;
            }
        }
        delta
    }

    pub(crate) fn hit_delta_if_contains(
        &self,
        point: Point,
        transform_records: &std::collections::HashMap<WidgetId, TransformRecord>,
    ) -> Option<Point> {
        let delta = self.transform_delta(transform_records);
        let local_point = Point {
            x: point.x - delta.x,
            y: point.y - delta.y,
        };
        (self.rect.contains(local_point)
            && self
                .clip_rect
                .map(|clip_rect| clip_rect.contains(point))
                .unwrap_or(true)
            && self.geometry.contains(local_point))
        .then_some(delta)
    }

    pub(crate) fn contains_without_transform(&self, point: Point) -> bool {
        self.rect.contains(point)
            && self
                .clip_rect
                .map(|clip_rect| clip_rect.contains(point))
                .unwrap_or(true)
            && self.geometry.contains(point)
    }

    pub(crate) fn interaction_translated(&self, delta: Point) -> HitInteraction<VM> {
        self.interaction.clone().translated(delta)
    }

    pub(crate) fn supports_retained_transform(&self) -> bool {
        matches!(self.geometry, HitGeometry::Rect)
            && self.focus.is_none()
            && self.gpu_scroll_container.is_none()
            && matches!(
                self.interaction,
                HitInteraction::Widget { .. }
                    | HitInteraction::Occluder { .. }
                    | HitInteraction::Disabled { .. }
            )
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
