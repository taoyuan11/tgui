use super::*;
use crate::foundation::binding::ScrollViewController;
use crate::foundation::binding::{ToastPlacement, ToastQueue};
use crate::foundation::form::ValidationVisualState;
use crate::ui::widget::core::Element;
use crate::ui::widget::overlay::{OverlayLayer, PlacementOptions};
use crate::ui::widget::portal::{PortalAnchor, PortalTarget};
use crate::ui::widget::r#virtual::{
    ErasedVirtualItemSource, ItemLayout, VirtualArrangement, VirtualRuntimeState,
};

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
pub(crate) struct ScrollViewConfig {
    pub show_scrollbar: Value<bool>,
    pub controller: Option<ScrollViewController>,
}

impl Default for ScrollViewConfig {
    fn default() -> Self {
        Self {
            show_scrollbar: Value::Static(true),
            controller: None,
        }
    }
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
    pub scroll_view: Option<ScrollViewConfig>,
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
            scroll_view: None,
        }
    }
}

pub(crate) enum ChildSource<VM> {
    Static(Vec<Element<VM>>),
    Dynamic(Arc<dyn Fn() -> Vec<Element<VM>> + Send + Sync>),
}

impl<VM> ChildSource<VM> {
    pub(crate) fn resolve(&self, owner: Option<WidgetId>) -> Vec<Element<VM>> {
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
    resolver: &Arc<dyn Fn() -> Vec<Element<VM>> + Send + Sync>,
) -> Vec<Element<VM>> {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        const CHILD_RESOLVER_STACK_SIZE: usize = 8 * 1024 * 1024;
        const CHILD_RESOLVER_STACK_RED_ZONE: usize = CHILD_RESOLVER_STACK_SIZE;
        stacker::maybe_grow(
            CHILD_RESOLVER_STACK_RED_ZONE,
            CHILD_RESOLVER_STACK_SIZE,
            || resolver(),
        )
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
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
    Virtual {
        arrangement: VirtualArrangement,
        item_layout: ItemLayout,
        source: ErasedVirtualItemSource<VM>,
        content_cross_extent: Option<Value<Dp>>,
        overflow_x: Overflow,
        overflow_y: Overflow,
        style: Option<StyleResolver<ContainerStyle>>,
        runtime_state: VirtualRuntimeState,
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
    Icon {
        icon: crate::ui::widget::icon::BuiltinSvgIcon,
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
        validation: Value<ValidationVisualState>,
        style: Option<StyleResolver<WidgetCheckboxStyle>>,
    },
    Radio {
        checked: Value<bool>,
        label: Option<Value<String>>,
        on_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        validation: Value<ValidationVisualState>,
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
        validation: Value<ValidationVisualState>,
        style: Option<StyleResolver<WidgetSwitchStyle>>,
    },
    Select {
        selected_label: Value<Option<String>>,
        placeholder: Value<String>,
        options: Vec<SelectOptionState<VM>>,
        open: Option<Value<bool>>,
        on_open_change: Option<ValueCommand<VM, bool>>,
        disabled: Value<bool>,
        validation: Value<ValidationVisualState>,
        style: Option<StyleResolver<WidgetSelectStyle>>,
    },
    SelectOptionRow {
        owner_id: WidgetId,
        option_index: usize,
        option: SelectOptionState<VM>,
        on_open_change: Option<ValueCommand<VM, bool>>,
        style: SelectOptionRowStyle,
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
        validation: Value<ValidationVisualState>,
        style: Option<StyleResolver<WidgetSliderStyle>>,
    },
    ProgressBar {
        value: Value<f32>,
        indeterminate: Value<bool>,
        show_label: bool,
        label: Option<Value<String>>,
        style: Option<StyleResolver<crate::ui::widget::style::ProgressBarStyle>>,
    },
    Divider {
        orientation: DividerOrientation,
        dashed: Value<bool>,
        color_override: Option<Value<Color>>,
        thickness_override: Option<Value<Dp>>,
        inset_override: Option<Value<Dp>>,
        label: Option<Value<String>>,
        style: Option<StyleResolver<crate::ui::widget::style::DividerStyle>>,
    },
    Spinner {
        style: Option<StyleResolver<crate::ui::widget::style::SpinnerStyle>>,
        size_override: Option<Value<Dp>>,
        thickness_override: Option<Value<Dp>>,
        track_override: Option<bool>,
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
        validation: Value<ValidationVisualState>,
    },
    ToastHost {
        queue: ToastQueue<VM>,
        placement: ToastPlacement,
        max_visible: Option<usize>,
        style: Option<StyleResolver<crate::ui::widget::style::ToastStyle>>,
    },
    Portal {
        content: Box<Element<VM>>,
        open: Value<bool>,
        target: PortalTarget,
        anchor: Option<PortalAnchor>,
        options: PlacementOptions,
        layer: OverlayLayer,
        on_open_change: Option<ValueCommand<VM, bool>>,
        return_focus_to: Option<WidgetId>,
        close_on_outside_click: bool,
        close_on_escape: bool,
        focus_scope: Option<crate::ui::widget::FocusScopeOptions>,
    },
}

pub(crate) struct SelectOptionState<VM> {
    pub label: Value<String>,
    pub selected: Value<bool>,
    pub disabled: Value<bool>,
    pub on_select: Option<Command<VM>>,
}

#[derive(Clone)]
pub(crate) struct SelectOptionRowStyle {
    pub text: Color,
    pub disabled_text: Color,
    pub selected_background: Color,
    pub option_height: Dp,
    pub padding_x: Dp,
    pub text_style: crate::ui::theme::TextStyle,
    pub clip_mask: Option<ClipMask>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabPlacement {
    Top,
    Bottom,
    Left,
    Right,
}

/// 分隔线的朝向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DividerOrientation {
    Horizontal,
    Vertical,
}

impl Default for DividerOrientation {
    fn default() -> Self {
        Self::Horizontal
    }
}

impl DividerOrientation {
    pub(crate) fn is_horizontal(self) -> bool {
        matches!(self, Self::Horizontal)
    }
}

impl Default for TabPlacement {
    fn default() -> Self {
        Self::Top
    }
}

impl TabPlacement {
    pub(crate) fn is_horizontal(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

pub(crate) struct TabTriggerState<VM> {
    pub group_id: WidgetId,
    pub index: usize,
    pub placement: TabPlacement,
    pub key: String,
    pub label: String,
    pub on_change: Option<ValueCommand<VM, (String, String)>>,
    pub reorderable: Value<bool>,
    pub on_reorder: Option<ValueCommand<VM, crate::ui::widget::TabsReorderEvent>>,
}

impl<VM> Clone for TabTriggerState<VM> {
    fn clone(&self) -> Self {
        Self {
            group_id: self.group_id,
            index: self.index,
            placement: self.placement,
            key: self.key.clone(),
            label: self.label.clone(),
            on_change: self.on_change.clone(),
            reorderable: self.reorderable.clone(),
            on_reorder: self.on_reorder.clone(),
        }
    }
}

impl<VM: 'static> TabTriggerState<VM> {
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> TabTriggerState<RootVm> {
        TabTriggerState {
            group_id: self.group_id,
            index: self.index,
            placement: self.placement,
            key: self.key,
            label: self.label,
            on_change: self
                .on_change
                .map(|command| command.scope(selector.clone())),
            reorderable: self.reorderable,
            on_reorder: self.on_reorder.map(|command| command.scope(selector)),
        }
    }
}

pub(crate) struct ListItemState<VM> {
    pub list_id: WidgetId,
    pub row_index: usize,
    pub item_index: usize,
    pub key: WidgetKey,
    pub selected_keys: Value<Vec<WidgetKey>>,
    pub selection_mode: crate::ui::widget::ListSelectionMode,
    pub disabled: Value<bool>,
    pub item_extent: Dp,
    pub item_spacing: Dp,
    pub item_background: Value<Color>,
    pub item_hover_background: Value<Color>,
    pub item_selected_background: Value<Color>,
    pub item_disabled_background: Value<Color>,
    pub on_selection_change: Option<ValueCommand<VM, crate::ui::widget::ListSelectionChange>>,
    pub on_item_action: Option<ValueCommand<VM, crate::ui::widget::ListItemAction>>,
    pub sibling_keys: std::sync::Arc<[WidgetKey]>,
    pub sibling_disabled: std::sync::Arc<[bool]>,
}

pub(crate) struct TreeRootState {
    pub tree_id: WidgetId,
    pub node_count: usize,
    pub selection_mode: crate::ui::widget::TreeSelectionMode,
    pub selected_keys: crate::ui::layout::Value<Vec<WidgetKey>>,
    pub checkable: crate::ui::layout::Value<bool>,
}

impl Clone for TreeRootState {
    fn clone(&self) -> Self {
        Self {
            tree_id: self.tree_id,
            node_count: self.node_count,
            selection_mode: self.selection_mode,
            selected_keys: self.selected_keys.clone(),
            checkable: self.checkable.clone(),
        }
    }
}

pub(crate) struct TreeNodeState<VM> {
    pub tree_id: WidgetId,
    pub row_index: usize,
    pub node_index: usize,
    pub key: WidgetKey,
    pub parent_key: Option<WidgetKey>,
    pub depth: usize,
    pub position_in_set: usize,
    pub set_size: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub check_state: crate::ui::widget::TreeCheckState,
    pub selected_keys: crate::ui::layout::Value<Vec<WidgetKey>>,
    pub expanded_keys: crate::ui::layout::Value<Vec<WidgetKey>>,
    pub checked_keys: crate::ui::layout::Value<Vec<WidgetKey>>,
    pub selection_mode: crate::ui::widget::TreeSelectionMode,
    pub checkable: crate::ui::layout::Value<bool>,
    pub disabled: crate::ui::layout::Value<bool>,
    pub item_extent: Dp,
    pub item_spacing: Dp,
    pub item_padding: crate::ui::layout::Insets,
    pub indent_width: Dp,
    pub disclosure_width: Dp,
    pub checkbox_width: Dp,
    pub disclosure_icon_size: Sp,
    pub checkbox_icon_size: Sp,
    pub indent_line_color: crate::ui::layout::Value<Color>,
    pub disclosure_icon_color: crate::ui::layout::Value<Color>,
    pub disclosure_hover_background: crate::ui::layout::Value<Color>,
    pub checkbox_unchecked_color: crate::ui::layout::Value<Color>,
    pub checkbox_checked_color: crate::ui::layout::Value<Color>,
    pub checkbox_indeterminate_color: crate::ui::layout::Value<Color>,
    pub checkbox_disabled_color: crate::ui::layout::Value<Color>,
    pub item_background: crate::ui::layout::Value<Color>,
    pub item_hover_background: crate::ui::layout::Value<Color>,
    pub item_selected_background: crate::ui::layout::Value<Color>,
    pub item_disabled_background: crate::ui::layout::Value<Color>,
    pub on_selection_change: Option<ValueCommand<VM, crate::ui::widget::TreeSelectionChange>>,
    pub on_expand_change: Option<ValueCommand<VM, crate::ui::widget::TreeExpandChange>>,
    pub on_check_change: Option<ValueCommand<VM, crate::ui::widget::TreeCheckChange>>,
    pub on_node_action: Option<ValueCommand<VM, crate::ui::widget::TreeNodeAction>>,
    pub on_drop: Option<ValueCommand<VM, crate::ui::widget::TreeDropEvent>>,
    pub sibling_keys: std::sync::Arc<[WidgetKey]>,
    pub sibling_disabled: std::sync::Arc<[bool]>,
    pub visible_keys: std::sync::Arc<[WidgetKey]>,
    pub visible_disabled: std::sync::Arc<[bool]>,
    pub child_keys: std::sync::Arc<[WidgetKey]>,
    pub descendant_keys: std::sync::Arc<[WidgetKey]>,
    pub check_target_keys: std::sync::Arc<[WidgetKey]>,
    pub draggable: bool,
}

pub(crate) struct DataGridRootState {
    pub grid_id: WidgetId,
    pub row_count: usize,
    pub column_count: usize,
    pub selection_mode: crate::ui::widget::DataGridSelectionMode,
    pub selected_keys: crate::ui::layout::Value<Vec<WidgetKey>>,
}

impl Clone for DataGridRootState {
    fn clone(&self) -> Self {
        Self {
            grid_id: self.grid_id,
            row_count: self.row_count,
            column_count: self.column_count,
            selection_mode: self.selection_mode,
            selected_keys: self.selected_keys.clone(),
        }
    }
}

pub(crate) struct DataGridCellState<VM> {
    pub grid_id: WidgetId,
    pub scroll_container_id: WidgetId,
    pub virtual_row_index: usize,
    pub row_index: usize,
    pub column_index: usize,
    pub row_key: WidgetKey,
    pub column_key: WidgetKey,
    pub pin: crate::ui::widget::DataGridColumnPin,
    pub pin_offset: Dp,
    pub selected_keys: crate::ui::layout::Value<Vec<WidgetKey>>,
    pub selection_mode: crate::ui::widget::DataGridSelectionMode,
    pub disabled: crate::ui::layout::Value<bool>,
    pub editable: bool,
    pub edit_value: String,
    pub on_selection_change: Option<ValueCommand<VM, crate::ui::widget::DataGridSelectionChange>>,
    pub on_cell_action: Option<ValueCommand<VM, crate::ui::widget::DataGridCellAction>>,
    pub on_cell_edit_commit: Option<ValueCommand<VM, crate::ui::widget::DataGridCellEditCommit>>,
    pub sibling_keys: std::sync::Arc<[WidgetKey]>,
    pub sibling_disabled: std::sync::Arc<[bool]>,
}

impl<VM> Clone for DataGridCellState<VM> {
    fn clone(&self) -> Self {
        Self {
            grid_id: self.grid_id,
            scroll_container_id: self.scroll_container_id,
            virtual_row_index: self.virtual_row_index,
            row_index: self.row_index,
            column_index: self.column_index,
            row_key: self.row_key.clone(),
            column_key: self.column_key.clone(),
            pin: self.pin,
            pin_offset: self.pin_offset,
            selected_keys: self.selected_keys.clone(),
            selection_mode: self.selection_mode,
            disabled: self.disabled.clone(),
            editable: self.editable,
            edit_value: self.edit_value.clone(),
            on_selection_change: self.on_selection_change.clone(),
            on_cell_action: self.on_cell_action.clone(),
            on_cell_edit_commit: self.on_cell_edit_commit.clone(),
            sibling_keys: self.sibling_keys.clone(),
            sibling_disabled: self.sibling_disabled.clone(),
        }
    }
}

impl<VM: 'static> DataGridCellState<VM> {
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> DataGridCellState<RootVm> {
        DataGridCellState {
            grid_id: self.grid_id,
            scroll_container_id: self.scroll_container_id,
            virtual_row_index: self.virtual_row_index,
            row_index: self.row_index,
            column_index: self.column_index,
            row_key: self.row_key,
            column_key: self.column_key,
            pin: self.pin,
            pin_offset: self.pin_offset,
            selected_keys: self.selected_keys,
            selection_mode: self.selection_mode,
            disabled: self.disabled,
            editable: self.editable,
            edit_value: self.edit_value,
            on_selection_change: self
                .on_selection_change
                .map(|command| command.scope(selector.clone())),
            on_cell_action: self
                .on_cell_action
                .map(|command| command.scope(selector.clone())),
            on_cell_edit_commit: self
                .on_cell_edit_commit
                .map(|command| command.scope(selector)),
            sibling_keys: self.sibling_keys,
            sibling_disabled: self.sibling_disabled,
        }
    }
}

pub(crate) struct DataGridHeaderState<VM> {
    pub grid_id: WidgetId,
    pub scroll_container_id: WidgetId,
    pub column_index: usize,
    pub column_key: WidgetKey,
    pub label: String,
    pub pin: crate::ui::widget::DataGridColumnPin,
    pub pin_offset: Dp,
    pub sortable: bool,
    pub resizable: bool,
    pub reorderable: bool,
    pub sort: crate::ui::layout::Value<Vec<crate::ui::widget::DataGridSort>>,
    pub width: Dp,
    pub min_width: Dp,
    pub max_width: Option<Dp>,
    pub on_sort_change: Option<ValueCommand<VM, crate::ui::widget::DataGridSortChange>>,
    pub on_column_width_change:
        Option<ValueCommand<VM, crate::ui::widget::DataGridColumnWidthChange>>,
    pub on_column_reorder: Option<ValueCommand<VM, crate::ui::widget::DataGridColumnReorderEvent>>,
}

impl<VM> Clone for DataGridHeaderState<VM> {
    fn clone(&self) -> Self {
        Self {
            grid_id: self.grid_id,
            scroll_container_id: self.scroll_container_id,
            column_index: self.column_index,
            column_key: self.column_key.clone(),
            label: self.label.clone(),
            pin: self.pin,
            pin_offset: self.pin_offset,
            sortable: self.sortable,
            resizable: self.resizable,
            reorderable: self.reorderable,
            sort: self.sort.clone(),
            width: self.width,
            min_width: self.min_width,
            max_width: self.max_width,
            on_sort_change: self.on_sort_change.clone(),
            on_column_width_change: self.on_column_width_change.clone(),
            on_column_reorder: self.on_column_reorder.clone(),
        }
    }
}

impl<VM: 'static> DataGridHeaderState<VM> {
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> DataGridHeaderState<RootVm> {
        DataGridHeaderState {
            grid_id: self.grid_id,
            scroll_container_id: self.scroll_container_id,
            column_index: self.column_index,
            column_key: self.column_key,
            label: self.label,
            pin: self.pin,
            pin_offset: self.pin_offset,
            sortable: self.sortable,
            resizable: self.resizable,
            reorderable: self.reorderable,
            sort: self.sort,
            width: self.width,
            min_width: self.min_width,
            max_width: self.max_width,
            on_sort_change: self
                .on_sort_change
                .map(|command| command.scope(selector.clone())),
            on_column_width_change: self
                .on_column_width_change
                .map(|command| command.scope(selector.clone())),
            on_column_reorder: self
                .on_column_reorder
                .map(|command| command.scope(selector)),
        }
    }
}

pub(crate) struct DataGridResizeHandleState<VM> {
    pub grid_id: WidgetId,
    pub column_index: usize,
    pub column_key: WidgetKey,
    pub width: Dp,
    pub min_width: Dp,
    pub max_width: Option<Dp>,
    pub on_column_width_change:
        Option<ValueCommand<VM, crate::ui::widget::DataGridColumnWidthChange>>,
}

impl<VM> Clone for DataGridResizeHandleState<VM> {
    fn clone(&self) -> Self {
        Self {
            grid_id: self.grid_id,
            column_index: self.column_index,
            column_key: self.column_key.clone(),
            width: self.width,
            min_width: self.min_width,
            max_width: self.max_width,
            on_column_width_change: self.on_column_width_change.clone(),
        }
    }
}

impl<VM: 'static> DataGridResizeHandleState<VM> {
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> DataGridResizeHandleState<RootVm> {
        DataGridResizeHandleState {
            grid_id: self.grid_id,
            column_index: self.column_index,
            column_key: self.column_key,
            width: self.width,
            min_width: self.min_width,
            max_width: self.max_width,
            on_column_width_change: self
                .on_column_width_change
                .map(|command| command.scope(selector)),
        }
    }
}

pub(crate) struct SplitterHandleState<VM> {
    pub axis: Axis,
    pub index: usize,
    pub sizes: Vec<f32>,
    pub constraints: Vec<(f32, f32)>,
    pub step: f32,
    pub on_resize: Option<ValueCommand<VM, crate::ui::widget::SplitterResize>>,
}

impl<VM> Clone for SplitterHandleState<VM> {
    fn clone(&self) -> Self {
        Self {
            axis: self.axis,
            index: self.index,
            sizes: self.sizes.clone(),
            constraints: self.constraints.clone(),
            step: self.step,
            on_resize: self.on_resize.clone(),
        }
    }
}

impl<VM: 'static> SplitterHandleState<VM> {
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> SplitterHandleState<RootVm> {
        SplitterHandleState {
            axis: self.axis,
            index: self.index,
            sizes: self.sizes,
            constraints: self.constraints,
            step: self.step,
            on_resize: self.on_resize.map(|command| command.scope(selector)),
        }
    }
}

pub(crate) struct CarouselAutoPlayState<VM> {
    pub id: WidgetId,
    pub frame: Rect,
    pub selected: usize,
    pub count: usize,
    pub interval: std::time::Duration,
    pub on_change: Option<ValueCommand<VM, usize>>,
}

impl<VM> Clone for CarouselAutoPlayState<VM> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            frame: self.frame,
            selected: self.selected,
            count: self.count,
            interval: self.interval,
            on_change: self.on_change.clone(),
        }
    }
}

impl<VM: 'static> CarouselAutoPlayState<VM> {
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> CarouselAutoPlayState<RootVm> {
        CarouselAutoPlayState {
            id: self.id,
            frame: self.frame,
            selected: self.selected,
            count: self.count,
            interval: self.interval,
            on_change: self.on_change.map(|command| command.scope(selector)),
        }
    }
}

impl<VM> Clone for ListItemState<VM> {
    fn clone(&self) -> Self {
        Self {
            list_id: self.list_id,
            row_index: self.row_index,
            item_index: self.item_index,
            key: self.key.clone(),
            selected_keys: self.selected_keys.clone(),
            selection_mode: self.selection_mode,
            disabled: self.disabled.clone(),
            item_extent: self.item_extent,
            item_spacing: self.item_spacing,
            item_background: self.item_background.clone(),
            item_hover_background: self.item_hover_background.clone(),
            item_selected_background: self.item_selected_background.clone(),
            item_disabled_background: self.item_disabled_background.clone(),
            on_selection_change: self.on_selection_change.clone(),
            on_item_action: self.on_item_action.clone(),
            sibling_keys: self.sibling_keys.clone(),
            sibling_disabled: self.sibling_disabled.clone(),
        }
    }
}

impl<VM> Clone for TreeNodeState<VM> {
    fn clone(&self) -> Self {
        Self {
            tree_id: self.tree_id,
            row_index: self.row_index,
            node_index: self.node_index,
            key: self.key.clone(),
            parent_key: self.parent_key.clone(),
            depth: self.depth,
            position_in_set: self.position_in_set,
            set_size: self.set_size,
            has_children: self.has_children,
            expanded: self.expanded,
            check_state: self.check_state,
            selected_keys: self.selected_keys.clone(),
            expanded_keys: self.expanded_keys.clone(),
            checked_keys: self.checked_keys.clone(),
            selection_mode: self.selection_mode,
            checkable: self.checkable.clone(),
            disabled: self.disabled.clone(),
            item_extent: self.item_extent,
            item_spacing: self.item_spacing,
            item_padding: self.item_padding,
            indent_width: self.indent_width,
            disclosure_width: self.disclosure_width,
            checkbox_width: self.checkbox_width,
            disclosure_icon_size: self.disclosure_icon_size,
            checkbox_icon_size: self.checkbox_icon_size,
            indent_line_color: self.indent_line_color.clone(),
            disclosure_icon_color: self.disclosure_icon_color.clone(),
            disclosure_hover_background: self.disclosure_hover_background.clone(),
            checkbox_unchecked_color: self.checkbox_unchecked_color.clone(),
            checkbox_checked_color: self.checkbox_checked_color.clone(),
            checkbox_indeterminate_color: self.checkbox_indeterminate_color.clone(),
            checkbox_disabled_color: self.checkbox_disabled_color.clone(),
            item_background: self.item_background.clone(),
            item_hover_background: self.item_hover_background.clone(),
            item_selected_background: self.item_selected_background.clone(),
            item_disabled_background: self.item_disabled_background.clone(),
            on_selection_change: self.on_selection_change.clone(),
            on_expand_change: self.on_expand_change.clone(),
            on_check_change: self.on_check_change.clone(),
            on_node_action: self.on_node_action.clone(),
            on_drop: self.on_drop.clone(),
            sibling_keys: self.sibling_keys.clone(),
            sibling_disabled: self.sibling_disabled.clone(),
            visible_keys: self.visible_keys.clone(),
            visible_disabled: self.visible_disabled.clone(),
            child_keys: self.child_keys.clone(),
            descendant_keys: self.descendant_keys.clone(),
            check_target_keys: self.check_target_keys.clone(),
            draggable: self.draggable,
        }
    }
}

impl<VM: 'static> TreeNodeState<VM> {
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> TreeNodeState<RootVm> {
        TreeNodeState {
            tree_id: self.tree_id,
            row_index: self.row_index,
            node_index: self.node_index,
            key: self.key,
            parent_key: self.parent_key,
            depth: self.depth,
            position_in_set: self.position_in_set,
            set_size: self.set_size,
            has_children: self.has_children,
            expanded: self.expanded,
            check_state: self.check_state,
            selected_keys: self.selected_keys,
            expanded_keys: self.expanded_keys,
            checked_keys: self.checked_keys,
            selection_mode: self.selection_mode,
            checkable: self.checkable,
            disabled: self.disabled,
            item_extent: self.item_extent,
            item_spacing: self.item_spacing,
            item_padding: self.item_padding,
            indent_width: self.indent_width,
            disclosure_width: self.disclosure_width,
            checkbox_width: self.checkbox_width,
            disclosure_icon_size: self.disclosure_icon_size,
            checkbox_icon_size: self.checkbox_icon_size,
            indent_line_color: self.indent_line_color,
            disclosure_icon_color: self.disclosure_icon_color,
            disclosure_hover_background: self.disclosure_hover_background,
            checkbox_unchecked_color: self.checkbox_unchecked_color,
            checkbox_checked_color: self.checkbox_checked_color,
            checkbox_indeterminate_color: self.checkbox_indeterminate_color,
            checkbox_disabled_color: self.checkbox_disabled_color,
            item_background: self.item_background,
            item_hover_background: self.item_hover_background,
            item_selected_background: self.item_selected_background,
            item_disabled_background: self.item_disabled_background,
            on_selection_change: self
                .on_selection_change
                .map(|command| command.scope(selector.clone())),
            on_expand_change: self
                .on_expand_change
                .map(|command| command.scope(selector.clone())),
            on_check_change: self
                .on_check_change
                .map(|command| command.scope(selector.clone())),
            on_node_action: self
                .on_node_action
                .map(|command| command.scope(selector.clone())),
            on_drop: self.on_drop.map(|command| command.scope(selector)),
            sibling_keys: self.sibling_keys,
            sibling_disabled: self.sibling_disabled,
            visible_keys: self.visible_keys,
            visible_disabled: self.visible_disabled,
            child_keys: self.child_keys,
            descendant_keys: self.descendant_keys,
            check_target_keys: self.check_target_keys,
            draggable: self.draggable,
        }
    }
}

impl<VM: 'static> ListItemState<VM> {
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> ListItemState<RootVm> {
        ListItemState {
            list_id: self.list_id,
            row_index: self.row_index,
            item_index: self.item_index,
            key: self.key,
            selected_keys: self.selected_keys,
            selection_mode: self.selection_mode,
            disabled: self.disabled,
            item_extent: self.item_extent,
            item_spacing: self.item_spacing,
            item_background: self.item_background,
            item_hover_background: self.item_hover_background,
            item_selected_background: self.item_selected_background,
            item_disabled_background: self.item_disabled_background,
            on_selection_change: self
                .on_selection_change
                .map(|command| command.scope(selector.clone())),
            on_item_action: self.on_item_action.map(|command| command.scope(selector)),
            sibling_keys: self.sibling_keys,
            sibling_disabled: self.sibling_disabled,
        }
    }
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
            Self::Virtual {
                arrangement,
                item_layout,
                source,
                content_cross_extent,
                overflow_x,
                overflow_y,
                style,
                runtime_state,
            } => Self::Virtual {
                arrangement: *arrangement,
                item_layout: *item_layout,
                source: source.clone(),
                content_cross_extent: content_cross_extent.clone(),
                overflow_x: *overflow_x,
                overflow_y: *overflow_y,
                style: style.clone(),
                runtime_state: runtime_state.clone(),
            },
            Self::Text { text } => Self::Text { text: text.clone() },
            #[cfg(feature = "audio")]
            Self::Audio { audio } => Self::Audio {
                audio: audio.clone(),
            },
            Self::Image { image } => Self::Image {
                image: image.clone(),
            },
            Self::Icon { icon } => Self::Icon { icon: icon.clone() },
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
                validation,
                style,
            } => Self::Checkbox {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: style.clone(),
            },
            Self::Radio {
                checked,
                label,
                on_change,
                disabled,
                validation,
                style,
            } => Self::Radio {
                checked: checked.clone(),
                label: label.clone(),
                on_change: on_change.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
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
                validation,
                style,
            } => Self::Switch {
                checked: checked.clone(),
                on_change: on_change.clone(),
                active_background: active_background.clone(),
                inactive_background: inactive_background.clone(),
                active_thumb_color: active_thumb_color.clone(),
                inactive_thumb_color: inactive_thumb_color.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: style.clone(),
            },
            Self::Select {
                selected_label,
                placeholder,
                options,
                open,
                on_open_change,
                disabled,
                validation,
                style,
            } => Self::Select {
                selected_label: selected_label.clone(),
                placeholder: placeholder.clone(),
                options: options.clone(),
                open: open.clone(),
                on_open_change: on_open_change.clone(),
                disabled: disabled.clone(),
                validation: validation.clone(),
                style: style.clone(),
            },
            Self::SelectOptionRow {
                owner_id,
                option_index,
                option,
                on_open_change,
                style,
            } => Self::SelectOptionRow {
                owner_id: *owner_id,
                option_index: *option_index,
                option: option.clone(),
                on_open_change: on_open_change.clone(),
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
                validation,
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
                validation: validation.clone(),
                style: style.clone(),
            },
            Self::ProgressBar {
                value,
                indeterminate,
                show_label,
                label,
                style,
            } => Self::ProgressBar {
                value: value.clone(),
                indeterminate: indeterminate.clone(),
                show_label: *show_label,
                label: label.clone(),
                style: style.clone(),
            },
            Self::Divider {
                orientation,
                dashed,
                color_override,
                thickness_override,
                inset_override,
                label,
                style,
            } => Self::Divider {
                orientation: *orientation,
                dashed: dashed.clone(),
                color_override: color_override.clone(),
                thickness_override: thickness_override.clone(),
                inset_override: inset_override.clone(),
                label: label.clone(),
                style: style.clone(),
            },
            Self::Spinner {
                style,
                size_override,
                thickness_override,
                track_override,
            } => Self::Spinner {
                style: style.clone(),
                size_override: size_override.clone(),
                thickness_override: thickness_override.clone(),
                track_override: *track_override,
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
                validation,
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
                validation: validation.clone(),
            },
            Self::ToastHost {
                queue,
                placement,
                max_visible,
                style,
            } => Self::ToastHost {
                queue: queue.clone(),
                placement: *placement,
                max_visible: *max_visible,
                style: style.clone(),
            },
            Self::Portal {
                content,
                open,
                target,
                anchor,
                options,
                layer,
                on_open_change,
                return_focus_to,
                close_on_outside_click,
                close_on_escape,
                focus_scope,
            } => Self::Portal {
                content: content.clone(),
                open: open.clone(),
                target: target.clone(),
                anchor: anchor.clone(),
                options: options.clone(),
                layer: *layer,
                on_open_change: on_open_change.clone(),
                return_focus_to: *return_focus_to,
                close_on_outside_click: *close_on_outside_click,
                close_on_escape: *close_on_escape,
                focus_scope: focus_scope.clone(),
            },
        }
    }
}
