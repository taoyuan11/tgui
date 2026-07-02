use std::collections::HashSet;
use std::sync::Arc;

use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::StyleContext;
use crate::ui::layout::{pct, Align, Insets, LayoutStyle, Value};
use crate::ui::unit::{dp, sp, Dp, Sp};

use super::common::{
    CursorStyle, FocusScopeOptions, InteractionHandlers, LifecycleEventHandlers,
    MediaEventHandlers, TreeNodeState, TreeRootState, VisualStyle, WidgetId, WidgetKey,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;
use super::r#virtual::{ItemLayout, ItemSource, VirtualList};
use super::style::palette::palette_from_theme;
use super::style::{ContainerStyle, StyleResolver, StyleSheet, WidgetSurfaceStyle};
use super::{
    ContextMenuDescriptor, Flex, GestureRecognizer, LongPressEvent, MenuItem, MenuItemState, Stack,
    Text,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeSelectionMode {
    None,
    Single,
    Multiple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeSelectionTrigger {
    Click,
    Keyboard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeExpandTrigger {
    Click,
    Keyboard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeCheckTrigger {
    Click,
    Keyboard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeCheckState {
    Unchecked,
    Checked,
    Indeterminate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeDropPosition {
    Before,
    After,
    Inside,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeSelectionChange {
    pub selected_keys: Vec<WidgetKey>,
    pub focused_key: Option<WidgetKey>,
    pub anchor_key: Option<WidgetKey>,
    pub changed_key: Option<WidgetKey>,
    pub trigger: TreeSelectionTrigger,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeExpandChange {
    pub expanded_keys: Vec<WidgetKey>,
    pub key: WidgetKey,
    pub expanded: bool,
    pub trigger: TreeExpandTrigger,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeCheckChange {
    pub checked_keys: Vec<WidgetKey>,
    pub key: WidgetKey,
    pub checked: bool,
    pub check_state: TreeCheckState,
    pub affected_keys: Vec<WidgetKey>,
    pub trigger: TreeCheckTrigger,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeNodeAction {
    pub index: usize,
    pub key: WidgetKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeDropEvent {
    pub dragged_key: WidgetKey,
    pub target_key: WidgetKey,
    pub position: TreeDropPosition,
}

#[derive(Clone, Debug)]
pub struct TreeNodeContext<T> {
    pub index: usize,
    pub key: WidgetKey,
    pub item: T,
    pub depth: usize,
    pub parent_key: Option<WidgetKey>,
    pub has_children: bool,
    pub expanded: bool,
    pub selected: bool,
    pub disabled: bool,
    pub check_state: TreeCheckState,
}

#[derive(Clone)]
pub struct TreeNode<T> {
    key: Option<WidgetKey>,
    value: T,
    disabled: Value<bool>,
    children: Vec<TreeNode<T>>,
}

impl<T> TreeNode<T> {
    pub fn new(value: T) -> Self {
        Self {
            key: None,
            value,
            disabled: Value::Static(false),
            children: Vec::new(),
        }
    }

    pub fn keyed(key: impl Into<WidgetKey>, value: T) -> Self {
        Self::new(value).key(key)
    }

    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn disable(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
        self
    }

    pub fn child(mut self, child: impl Into<TreeNode<T>>) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn children<I>(mut self, children: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<TreeNode<T>>,
    {
        self.children.extend(children.into_iter().map(Into::into));
        self
    }
}

impl<T> From<T> for TreeNode<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TreeStyle {
    pub surface: WidgetSurfaceStyle,
    pub item_height: Dp,
    pub item_padding: Insets,
    pub item_radius: Dp,
    pub indent_width: Dp,
    pub disclosure_width: Dp,
    pub checkbox_width: Dp,
    pub disclosure_icon_size: Sp,
    pub checkbox_icon_size: Sp,
    pub indent_line_color: Value<Color>,
    pub disclosure_icon_color: Value<Color>,
    pub disclosure_hover_background: Value<Color>,
    pub checkbox_unchecked_color: Value<Color>,
    pub checkbox_checked_color: Value<Color>,
    pub checkbox_indeterminate_color: Value<Color>,
    pub checkbox_disabled_color: Value<Color>,
    pub item_background: Value<Color>,
    pub item_hover_background: Value<Color>,
    pub item_selected_background: Value<Color>,
    pub item_disabled_background: Value<Color>,
}

impl TreeStyle {
    pub fn default_for_theme(theme: &crate::ui::theme::Theme) -> Self {
        let palette = palette_from_theme(theme);
        Self {
            surface: WidgetSurfaceStyle {
                background: Some(Value::Static(theme.colors.surface)),
                border_color: Some(Value::Static(theme.colors.outline_muted)),
                border_width: Some(Value::Static(theme.border.thin)),
                border_radius: Some(Value::Static(theme.radius.lg)),
                ..WidgetSurfaceStyle::default()
            },
            item_height: theme.spacing.xl + theme.spacing.md,
            item_padding: Insets::symmetric(theme.spacing.sm, theme.spacing.xs),
            item_radius: theme.radius.md,
            indent_width: theme.spacing.xl + theme.spacing.xs,
            disclosure_width: theme.spacing.xl,
            checkbox_width: theme.spacing.xl,
            disclosure_icon_size: sp(20.0),
            checkbox_icon_size: sp(20.0),
            indent_line_color: Value::Static(theme.colors.outline_muted.with_alpha_factor(0.6)),
            disclosure_icon_color: Value::Static(palette.on_surface_muted),
            disclosure_hover_background: Value::Static(theme.colors.surface_low),
            checkbox_unchecked_color: Value::Static(theme.colors.outline),
            checkbox_checked_color: Value::Static(theme.colors.primary),
            checkbox_indeterminate_color: Value::Static(theme.colors.primary.lighten(0.08)),
            checkbox_disabled_color: Value::Static(theme.colors.on_disabled),
            item_background: Value::Static(Color::TRANSPARENT),
            item_hover_background: Value::Static(theme.colors.surface_low),
            item_selected_background: Value::Static(theme.colors.primary_container),
            item_disabled_background: Value::Static(Color::TRANSPARENT),
        }
    }
}

fn resolve_tree_style(
    style: Option<&StyleResolver<TreeStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
) -> TreeStyle {
    let mut base = TreeStyle::default_for_theme(context.theme);
    context.theme.components.tree.apply(&mut base, context);
    style_sheet.apply_tree(&mut base, context, visual);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

struct TreeRow<T> {
    source_index: usize,
    key: WidgetKey,
    value: T,
    disabled: Value<bool>,
    depth: usize,
    parent_key: Option<WidgetKey>,
    position_in_set: usize,
    set_size: usize,
    has_children: bool,
    expanded: bool,
    child_keys: Arc<[WidgetKey]>,
    descendant_keys: Arc<[WidgetKey]>,
    check_target_keys: Arc<[WidgetKey]>,
}

impl<T: Clone> Clone for TreeRow<T> {
    fn clone(&self) -> Self {
        Self {
            source_index: self.source_index,
            key: self.key.clone(),
            value: self.value.clone(),
            disabled: self.disabled.clone(),
            depth: self.depth,
            parent_key: self.parent_key.clone(),
            position_in_set: self.position_in_set,
            set_size: self.set_size,
            has_children: self.has_children,
            expanded: self.expanded,
            child_keys: self.child_keys.clone(),
            descendant_keys: self.descendant_keys.clone(),
            check_target_keys: self.check_target_keys.clone(),
        }
    }
}

struct TreeRowSource<T> {
    nodes: Arc<[TreeNode<T>]>,
    expanded_keys: Value<Vec<WidgetKey>>,
}

impl<T: Clone> Clone for TreeRowSource<T> {
    fn clone(&self) -> Self {
        Self {
            nodes: self.nodes.clone(),
            expanded_keys: self.expanded_keys.clone(),
        }
    }
}

impl<T: Clone> TreeRowSource<T> {
    fn new(nodes: Arc<[TreeNode<T>]>, expanded_keys: Value<Vec<WidgetKey>>) -> Self {
        Self {
            nodes,
            expanded_keys,
        }
    }

    fn rows(&self) -> Vec<TreeRow<T>> {
        let expanded_now: HashSet<_> = self.expanded_keys.resolve().into_iter().collect();
        flatten_tree_rows(&self.nodes, &expanded_now)
    }

    fn visible_keys_and_disabled(&self) -> (Arc<[WidgetKey]>, Arc<[bool]>) {
        let rows = self.rows();
        let visible_keys: Arc<[WidgetKey]> = rows
            .iter()
            .map(|row| row.key.clone())
            .collect::<Vec<_>>()
            .into();
        let visible_disabled: Arc<[bool]> = rows
            .iter()
            .map(|row| row.disabled.resolve())
            .collect::<Vec<_>>()
            .into();
        (visible_keys, visible_disabled)
    }
}

impl<T> ItemSource<TreeRow<T>> for TreeRowSource<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn len(&self) -> usize {
        self.rows().len()
    }

    fn item(&self, index: usize) -> Option<TreeRow<T>> {
        self.rows().get(index).cloned()
    }

    fn key(&self, index: usize) -> Option<WidgetKey> {
        self.rows().get(index).map(|row| row.key.clone())
    }
}

pub struct Tree<T, VM> {
    nodes: Vec<TreeNode<T>>,
    render: Arc<dyn Fn(TreeNodeContext<T>) -> Element<VM> + Send + Sync>,
    expanded_keys: Value<Vec<WidgetKey>>,
    selected_keys: Value<Vec<WidgetKey>>,
    checked_keys: Value<Vec<WidgetKey>>,
    selection_mode: TreeSelectionMode,
    checkable: Value<bool>,
    loading: Value<bool>,
    empty_view: Option<Element<VM>>,
    loading_view: Option<Element<VM>>,
    item_layout: ItemLayout,
    style: Option<StyleResolver<TreeStyle>>,
    on_selection_change: Option<ValueCommand<VM, TreeSelectionChange>>,
    on_expand_change: Option<ValueCommand<VM, TreeExpandChange>>,
    on_check_change: Option<ValueCommand<VM, TreeCheckChange>>,
    on_node_action: Option<ValueCommand<VM, TreeNodeAction>>,
    on_drop: Option<ValueCommand<VM, TreeDropEvent>>,
    context_menu: Vec<MenuItemState<VM>>,
    draggable: bool,
    layout: LayoutStyle,
    focusable: Option<bool>,
    tab_index: Option<i32>,
    focus_scope: Option<FocusScopeOptions>,
    visual: VisualStyle,
    interactions: InteractionHandlers<VM>,
    lifecycle_events: LifecycleEventHandlers<VM>,
    media_events: MediaEventHandlers<VM>,
    key: Option<WidgetKey>,
}

impl<T, VM> Tree<T, VM>
where
    T: Clone + Send + Sync + 'static,
    VM: 'static,
{
    pub fn new<I>(
        nodes: Vec<I>,
        render: impl Fn(TreeNodeContext<T>) -> Element<VM> + Send + Sync + 'static,
    ) -> Self
    where
        I: Into<TreeNode<T>>,
    {
        let interactions = InteractionHandlers {
            cursor_style: Some(Value::Static(CursorStyle::Pointer)),
            ..Default::default()
        };
        Self {
            nodes: nodes.into_iter().map(Into::into).collect(),
            render: Arc::new(render),
            expanded_keys: Value::Static(Vec::new()),
            selected_keys: Value::Static(Vec::new()),
            checked_keys: Value::Static(Vec::new()),
            selection_mode: TreeSelectionMode::Single,
            checkable: Value::Static(false),
            loading: Value::Static(false),
            empty_view: None,
            loading_view: None,
            item_layout: ItemLayout::Fixed {
                item_extent: dp(40.0),
                spacing: Dp::ZERO,
                overscan: 2,
            },
            style: None,
            on_selection_change: None,
            on_expand_change: None,
            on_check_change: None,
            on_node_action: None,
            on_drop: None,
            context_menu: Vec::new(),
            draggable: false,
            layout: LayoutStyle::default(),
            focusable: None,
            tab_index: None,
            focus_scope: None,
            visual: VisualStyle::default(),
            interactions,
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events: MediaEventHandlers::default(),
            key: None,
        }
    }

    pub fn expanded_keys(mut self, keys: impl Into<Value<Vec<WidgetKey>>>) -> Self {
        self.expanded_keys = keys.into();
        self
    }

    pub fn selected_keys(mut self, keys: impl Into<Value<Vec<WidgetKey>>>) -> Self {
        self.selected_keys = keys.into();
        self
    }

    pub fn selected_key(mut self, key: impl Into<Value<Option<WidgetKey>>>) -> Self {
        self.selected_keys = match key.into() {
            Value::Static(key) => Value::Static(key.into_iter().collect()),
            Value::Signal(signal) => Value::Signal(signal.map(|key| key.into_iter().collect())),
        };
        self
    }

    pub fn checked_keys(mut self, keys: impl Into<Value<Vec<WidgetKey>>>) -> Self {
        self.checked_keys = keys.into();
        self
    }

    pub fn selection_mode(mut self, mode: TreeSelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    pub fn checkable(mut self, checkable: impl Into<Value<bool>>) -> Self {
        self.checkable = checkable.into();
        self
    }

    pub fn on_selection_change(mut self, command: ValueCommand<VM, TreeSelectionChange>) -> Self {
        self.on_selection_change = Some(command);
        self
    }

    pub fn on_expand_change(mut self, command: ValueCommand<VM, TreeExpandChange>) -> Self {
        self.on_expand_change = Some(command);
        self
    }

    pub fn on_check_change(mut self, command: ValueCommand<VM, TreeCheckChange>) -> Self {
        self.on_check_change = Some(command);
        self
    }

    pub fn on_node_action(mut self, command: ValueCommand<VM, TreeNodeAction>) -> Self {
        self.on_node_action = Some(command);
        self
    }

    pub fn on_drop(mut self, command: ValueCommand<VM, TreeDropEvent>) -> Self {
        self.on_drop = Some(command);
        self
    }

    pub fn context_menu(mut self, items: Vec<MenuItem<VM>>) -> Self {
        self.context_menu = items.into_iter().map(MenuItemState::from_public).collect();
        self
    }

    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }

    pub fn empty(mut self, view: impl Into<Element<VM>>) -> Self {
        self.empty_view = Some(view.into());
        self
    }

    pub fn loading(mut self, loading: impl Into<Value<bool>>) -> Self {
        self.loading = loading.into();
        self
    }

    pub fn loading_view(mut self, view: impl Into<Element<VM>>) -> Self {
        self.loading_view = Some(view.into());
        self
    }

    pub fn item_layout(mut self, layout: ItemLayout) -> Self {
        self.item_layout = layout;
        self
    }

    pub fn spacing(mut self, spacing: Dp) -> Self {
        self.item_layout = match self.item_layout {
            ItemLayout::Fixed {
                item_extent,
                overscan,
                ..
            } => ItemLayout::Fixed {
                item_extent,
                spacing,
                overscan,
            },
            ItemLayout::Estimated {
                estimate, overscan, ..
            } => ItemLayout::Estimated {
                estimate,
                spacing,
                overscan,
            },
            ItemLayout::Measured {
                estimate, overscan, ..
            } => ItemLayout::Measured {
                estimate,
                spacing,
                overscan,
            },
        };
        self
    }

    pub fn overscan(mut self, overscan: usize) -> Self {
        self.item_layout = match self.item_layout {
            ItemLayout::Fixed {
                item_extent,
                spacing,
                ..
            } => ItemLayout::Fixed {
                item_extent,
                spacing,
                overscan,
            },
            ItemLayout::Estimated {
                estimate, spacing, ..
            } => ItemLayout::Estimated {
                estimate,
                spacing,
                overscan,
            },
            ItemLayout::Measured {
                estimate, spacing, ..
            } => ItemLayout::Measured {
                estimate,
                spacing,
                overscan,
            },
        };
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut TreeStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| TreeStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> TreeStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
        set_layout_lengths(&mut self.layout, width, height);
        self
    }

    pub fn width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.width, width);
        self
    }

    pub fn height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.height, height);
        self
    }

    pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.min_width, width);
        self
    }

    pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.min_height, height);
        self
    }

    pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.max_width, width);
        self
    }

    pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.max_height, height);
        self
    }

    pub fn margin(mut self, margin: impl Into<Value<Insets>>) -> Self {
        self.layout.margin = margin.into();
        self
    }

    pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
        self.layout.grow = grow.into();
        self
    }

    pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
        self.layout.shrink = shrink.into();
        self
    }

    pub fn align_self(mut self, align: Align) -> Self {
        self.layout.align_self = Some(align);
        self
    }

    pub fn position_absolute(mut self) -> Self {
        self.layout.position_type = crate::ui::layout::PositionType::Absolute;
        self
    }

    pub fn left(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.layout.left, value);
        self
    }

    pub fn top(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.layout.top, value);
        self
    }

    pub fn right(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.layout.right, value);
        self
    }

    pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.layout.bottom, value);
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = Some(focusable);
        self
    }

    pub fn tab_index(mut self, tab_index: i32) -> Self {
        self.tab_index = Some(tab_index);
        self
    }

    pub fn focus_scope(mut self, options: FocusScopeOptions) -> Self {
        self.focus_scope = Some(options);
        self
    }

    pub fn auto_focus_first(mut self, auto_focus_first: bool) -> Self {
        self.focus_scope = Some(
            self.focus_scope
                .take()
                .unwrap_or_default()
                .auto_focus_first(auto_focus_first),
        );
        self
    }

    pub fn on_mount(mut self, command: Command<VM>) -> Self {
        self.lifecycle_events.on_mount = Some(command);
        self
    }

    pub fn on_unmount(mut self, command: Command<VM>) -> Self {
        self.lifecycle_events.on_unmount = Some(command);
        self
    }

    pub fn on_update(mut self, command: Command<VM>) -> Self {
        self.lifecycle_events.on_update = Some(command);
        self
    }
}

impl<T, VM> From<Tree<T, VM>> for Element<VM>
where
    T: Clone + Send + Sync + 'static,
    VM: 'static,
{
    fn from(tree: Tree<T, VM>) -> Self {
        tree.into_element()
    }
}

impl<T, VM> Tree<T, VM>
where
    T: Clone + Send + Sync + 'static,
    VM: 'static,
{
    fn into_element(self) -> Element<VM> {
        if self.loading.resolve() {
            return self
                .loading_view
                .unwrap_or_else(|| Stack::new().child(Text::new("Loading...")).into());
        }
        if self.nodes.is_empty() {
            return self
                .empty_view
                .unwrap_or_else(|| Stack::new().child(Text::new("No items")).into());
        }

        let tree_id = WidgetId::next();
        let style_resolver = self.style.clone();
        let row_visual = self.visual.clone();
        let root_style_resolver = self.style.clone();
        let root_visual = self.visual.clone();
        let source = TreeRowSource::new(self.nodes.into(), self.expanded_keys.clone());
        let row_count = source.len();
        if row_count == 0 {
            return self
                .empty_view
                .unwrap_or_else(|| Stack::new().child(Text::new("No items")).into());
        }
        let source_for_render = source.clone();
        let render = self.render.clone();
        let selected_keys = self.selected_keys.clone();
        let root_selected_keys = selected_keys.clone();
        let expanded_keys = self.expanded_keys.clone();
        let checked_keys = self.checked_keys.clone();
        let checkable = self.checkable.clone();
        let root_checkable = checkable.clone();
        let selection_mode = self.selection_mode;
        let on_selection_change = self.on_selection_change.clone();
        let on_expand_change = self.on_expand_change.clone();
        let on_check_change = self.on_check_change.clone();
        let on_node_action = self.on_node_action.clone();
        let on_drop = self.on_drop.clone();
        let context_menu = Arc::new(self.context_menu);
        let item_layout = self.item_layout;
        let item_extent = item_layout.estimate().max(Dp::ZERO);
        let item_spacing = item_layout.spacing().max(Dp::ZERO);
        let draggable = self.draggable;
        let mut tree: Element<VM> = VirtualList::new_with_style_context(
            source,
            move |visible_index, row, context, style_sheet| {
                let row_style = Arc::new(resolve_tree_style(
                    style_resolver.as_ref(),
                    &context,
                    style_sheet,
                    &row_visual,
                ));
                let selected = selected_keys.resolve().contains(&row.key);
                let disabled_now = row.disabled.resolve();
                let check_state = tree_check_state(&row.check_target_keys, &checked_keys.resolve());
                let context = TreeNodeContext {
                    index: row.source_index,
                    key: row.key.clone(),
                    item: row.value.clone(),
                    depth: row.depth,
                    parent_key: row.parent_key.clone(),
                    has_children: row.has_children,
                    expanded: row.expanded,
                    selected,
                    disabled: disabled_now,
                    check_state,
                };
                let child = render(context);
                let (visible_keys, visible_disabled) =
                    source_for_render.visible_keys_and_disabled();
                build_tree_row(
                    tree_id,
                    visible_index,
                    row.clone(),
                    child,
                    selected_keys.clone(),
                    expanded_keys.clone(),
                    checked_keys.clone(),
                    checkable.clone(),
                    selection_mode,
                    visible_keys.clone(),
                    visible_disabled.clone(),
                    row_style.clone(),
                    item_layout,
                    item_extent,
                    item_spacing,
                    on_selection_change.clone(),
                    on_expand_change.clone(),
                    on_check_change.clone(),
                    on_node_action.clone(),
                    on_drop.clone(),
                    context_menu.clone(),
                    draggable,
                )
            },
        )
        .widget_id(tree_id)
        .item_layout(self.item_layout)
        .width(pct(100.0))
        .style_full_with_style_sheet(move |context, style_sheet, _visual, state| {
            let mut style = resolve_tree_style(
                root_style_resolver.as_ref(),
                context,
                style_sheet,
                &root_visual,
            );
            style_sheet.apply_tree_state(&mut style, context, &root_visual, state);
            let mut container = ContainerStyle::default_for_theme(context.theme);
            container.surface = style.surface;
            container
        })
        .into();
        tree.key = self.key;
        tree.layout = self.layout;
        tree.focus.focusable = self.focusable;
        tree.focus.tab_index = self.tab_index;
        tree.focus.scope = self.focus_scope;
        tree.visual = self.visual;
        tree.interactions = self.interactions;
        tree.lifecycle_events = self.lifecycle_events;
        tree.media_events = self.media_events;
        tree.tree_root = Some(TreeRootState {
            tree_id,
            node_count: row_count,
            selection_mode,
            selected_keys: root_selected_keys,
            checkable: root_checkable,
        });
        tree
    }
}

#[allow(clippy::too_many_arguments)]
fn build_tree_row<T, VM: 'static>(
    tree_id: WidgetId,
    visible_index: usize,
    row: TreeRow<T>,
    child: Element<VM>,
    selected_keys: Value<Vec<WidgetKey>>,
    expanded_keys: Value<Vec<WidgetKey>>,
    checked_keys: Value<Vec<WidgetKey>>,
    checkable: Value<bool>,
    selection_mode: TreeSelectionMode,
    visible_keys: Arc<[WidgetKey]>,
    visible_disabled: Arc<[bool]>,
    style: Arc<TreeStyle>,
    item_layout: ItemLayout,
    item_extent: Dp,
    item_spacing: Dp,
    on_selection_change: Option<ValueCommand<VM, TreeSelectionChange>>,
    on_expand_change: Option<ValueCommand<VM, TreeExpandChange>>,
    on_check_change: Option<ValueCommand<VM, TreeCheckChange>>,
    on_node_action: Option<ValueCommand<VM, TreeNodeAction>>,
    on_drop: Option<ValueCommand<VM, TreeDropEvent>>,
    context_menu: Arc<Vec<MenuItemState<VM>>>,
    draggable: bool,
) -> Element<VM>
where
    T: Clone + Send + Sync + 'static,
{
    let disabled_now = row.disabled.resolve();
    let check_state = tree_check_state(&row.check_target_keys, &checked_keys.resolve());
    let mut content = Flex::horizontal()
        .align(Align::Center)
        .child(
            Stack::<VM>::new()
                .width(style.indent_width * row.depth as f32)
                .shrink(0.0),
        )
        .child(Stack::<VM>::new().width(style.disclosure_width).shrink(0.0));
    if checkable.resolve() {
        content = content.child(Stack::<VM>::new().width(style.checkbox_width).shrink(0.0));
    }
    let content_slot = Flex::horizontal()
        .align(Align::Center)
        .basis(dp(0.0))
        .height(pct(100.0))
        .min_width(dp(0.0))
        .grow(1.0)
        .shrink(1.0)
        .child(child);
    let content = content
        .child(content_slot)
        .width(pct(100.0))
        .height(pct(100.0));
    let row_container = Flex::vertical()
        .align(Align::Stretch)
        .child(content)
        .padding(style.item_padding);
    let row_container = match item_layout {
        ItemLayout::Fixed { item_extent, .. } => row_container.height(item_extent),
        ItemLayout::Estimated { estimate, .. } => row_container.height(estimate),
        ItemLayout::Measured { .. } => row_container.min_height(style.item_height),
    };
    let mut element: Element<VM> = row_container.into();
    element.key = Some(row.key.clone());
    element.focus.focusable = Some(!disabled_now);
    element.focus.tab_index = Some(if visible_index == 0 { 0 } else { -1 });
    element.interactions.cursor_style = Some(Value::Static(if disabled_now {
        CursorStyle::Default
    } else if draggable {
        CursorStyle::Grab
    } else {
        CursorStyle::Pointer
    }));
    element.visual.border_radius = Some(Value::Static(style.item_radius));
    element.tree_node = Some(TreeNodeState {
        tree_id,
        row_index: visible_index,
        node_index: row.source_index,
        key: row.key.clone(),
        parent_key: row.parent_key.clone(),
        depth: row.depth,
        position_in_set: row.position_in_set,
        set_size: row.set_size,
        has_children: row.has_children,
        expanded: row.expanded,
        check_state,
        selected_keys,
        expanded_keys,
        checked_keys,
        selection_mode,
        checkable,
        disabled: row.disabled.clone(),
        item_extent,
        item_spacing,
        item_padding: style.item_padding,
        indent_width: style.indent_width,
        disclosure_width: style.disclosure_width,
        checkbox_width: style.checkbox_width,
        disclosure_icon_size: style.disclosure_icon_size,
        checkbox_icon_size: style.checkbox_icon_size,
        indent_line_color: style.indent_line_color.clone(),
        disclosure_icon_color: style.disclosure_icon_color.clone(),
        disclosure_hover_background: style.disclosure_hover_background.clone(),
        checkbox_unchecked_color: style.checkbox_unchecked_color.clone(),
        checkbox_checked_color: style.checkbox_checked_color.clone(),
        checkbox_indeterminate_color: style.checkbox_indeterminate_color.clone(),
        checkbox_disabled_color: style.checkbox_disabled_color.clone(),
        item_background: style.item_background.clone(),
        item_hover_background: style.item_hover_background.clone(),
        item_selected_background: style.item_selected_background.clone(),
        item_disabled_background: style.item_disabled_background.clone(),
        on_selection_change,
        on_expand_change,
        on_check_change,
        on_node_action,
        on_drop,
        sibling_keys: visible_keys.clone(),
        sibling_disabled: visible_disabled.clone(),
        visible_keys,
        visible_disabled,
        child_keys: row.child_keys,
        descendant_keys: row.descendant_keys,
        check_target_keys: row.check_target_keys,
        draggable,
    });
    if !context_menu.is_empty() {
        let on_show = ValueCommand::new(|_: &mut VM, _: LongPressEvent| {});
        element.interactions.gesture = Some(match element.interactions.gesture.take() {
            Some(existing) => existing.on_long_press(on_show),
            None => GestureRecognizer::new().on_long_press(on_show),
        });
        element.context_menu = Some(Box::new(ContextMenuDescriptor {
            items: context_menu.as_ref().to_vec(),
            on_open_change: None,
            disabled: Value::Static(false),
            style: None,
        }));
    }
    element
}

fn flatten_tree_rows<T: Clone>(
    nodes: &[TreeNode<T>],
    expanded_keys: &HashSet<WidgetKey>,
) -> Vec<TreeRow<T>> {
    let mut rows = Vec::new();
    let mut next_index = 0usize;
    flatten_children(nodes, None, 0, expanded_keys, &mut next_index, &mut rows);
    rows
}

#[allow(clippy::too_many_arguments)]
fn flatten_children<T: Clone>(
    nodes: &[TreeNode<T>],
    parent_key: Option<WidgetKey>,
    depth: usize,
    expanded_keys: &HashSet<WidgetKey>,
    next_index: &mut usize,
    rows: &mut Vec<TreeRow<T>>,
) {
    let set_size = nodes.len();
    for (position, node) in nodes.iter().enumerate() {
        let source_index = *next_index;
        *next_index += 1;
        let key = node
            .key
            .clone()
            .unwrap_or_else(|| WidgetKey::from(source_index));
        let has_children = !node.children.is_empty();
        let expanded = has_children && expanded_keys.contains(&key);
        let child_keys = collect_immediate_child_keys(&node.children, source_index + 1);
        let mut descendant_keys = Vec::new();
        let mut descendant_index = source_index + 1;
        collect_descendant_keys(&node.children, &mut descendant_index, &mut descendant_keys);
        let mut check_target_keys = Vec::new();
        let mut check_index = source_index + 1;
        collect_check_target_keys(&key, node, &mut check_index, &mut check_target_keys);
        let row = TreeRow {
            source_index,
            key: key.clone(),
            value: node.value.clone(),
            disabled: node.disabled.clone(),
            depth,
            parent_key: parent_key.clone(),
            position_in_set: position + 1,
            set_size,
            has_children,
            expanded,
            child_keys: child_keys.into(),
            descendant_keys: descendant_keys.into(),
            check_target_keys: check_target_keys.into(),
        };
        rows.push(row);
        if expanded {
            flatten_children(
                &node.children,
                Some(key),
                depth + 1,
                expanded_keys,
                next_index,
                rows,
            );
        } else {
            advance_source_index(&node.children, next_index);
        }
    }
}

fn advance_source_index<T>(nodes: &[TreeNode<T>], next_index: &mut usize) {
    for node in nodes {
        *next_index += 1;
        advance_source_index(&node.children, next_index);
    }
}

fn collect_immediate_child_keys<T>(nodes: &[TreeNode<T>], start_index: usize) -> Vec<WidgetKey> {
    let mut keys = Vec::new();
    let mut next_index = start_index;
    for node in nodes {
        let source_index = next_index;
        next_index += 1;
        keys.push(
            node.key
                .clone()
                .unwrap_or_else(|| WidgetKey::from(source_index)),
        );
        advance_source_index(&node.children, &mut next_index);
    }
    keys
}

fn collect_descendant_keys<T>(
    nodes: &[TreeNode<T>],
    next_index: &mut usize,
    output: &mut Vec<WidgetKey>,
) {
    for node in nodes {
        let source_index = *next_index;
        *next_index += 1;
        let key = node
            .key
            .clone()
            .unwrap_or_else(|| WidgetKey::from(source_index));
        output.push(key);
        collect_descendant_keys(&node.children, next_index, output);
    }
}

fn collect_check_target_keys<T>(
    self_key: &WidgetKey,
    node: &TreeNode<T>,
    next_index: &mut usize,
    output: &mut Vec<WidgetKey>,
) {
    if !node.disabled.resolve() {
        output.push(self_key.clone());
    }
    collect_enabled_descendant_keys(&node.children, next_index, output);
}

fn collect_enabled_descendant_keys<T>(
    nodes: &[TreeNode<T>],
    next_index: &mut usize,
    output: &mut Vec<WidgetKey>,
) {
    for node in nodes {
        let source_index = *next_index;
        *next_index += 1;
        let key = node
            .key
            .clone()
            .unwrap_or_else(|| WidgetKey::from(source_index));
        if !node.disabled.resolve() {
            output.push(key.clone());
        }
        collect_enabled_descendant_keys(&node.children, next_index, output);
    }
}

pub(crate) fn tree_check_state(keys: &[WidgetKey], checked: &[WidgetKey]) -> TreeCheckState {
    if keys.is_empty() {
        return TreeCheckState::Unchecked;
    }
    let checked_count = keys
        .iter()
        .filter(|key| checked.iter().any(|checked_key| checked_key == *key))
        .count();
    if checked_count == keys.len() {
        TreeCheckState::Checked
    } else if checked_count == 0 {
        TreeCheckState::Unchecked
    } else {
        TreeCheckState::Indeterminate
    }
}
