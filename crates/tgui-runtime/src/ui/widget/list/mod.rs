use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::StyleContext;
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};
use crate::ui::theme::Density;
use crate::ui::unit::{dp, Dp};

use super::common::{
    CursorStyle, FocusScopeOptions, InteractionHandlers, LifecycleEventHandlers, ListItemState,
    ListSelectionMetadata, MediaEventHandlers, VisualStyle, WidgetId, WidgetKey,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;
use super::r#virtual::{ItemLayout, ItemSource, VirtualList};
use super::style::palette::palette_from_theme;
use super::style::{ContainerStyle, StyleResolver, StyleSheet};
use super::{
    ContextMenuDescriptor, Flex, GestureRecognizer, LongPressEvent, MenuItem, MenuItemState, Stack,
    Text,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListSelectionMode {
    None,
    Single,
    Multiple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListSelectionTrigger {
    Click,
    Keyboard,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListSelectionChange {
    pub selected_keys: Vec<WidgetKey>,
    pub focused_key: Option<WidgetKey>,
    pub anchor_key: Option<WidgetKey>,
    pub changed_key: Option<WidgetKey>,
    pub trigger: ListSelectionTrigger,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItemAction {
    pub index: usize,
    pub key: WidgetKey,
}

#[derive(Clone, Debug)]
pub struct ListItemContext<T> {
    pub index: usize,
    pub key: WidgetKey,
    pub item: T,
    pub selected: bool,
    pub disabled: bool,
}

#[derive(Clone)]
pub struct ListItem<T> {
    key: Option<WidgetKey>,
    value: T,
    disabled: Value<bool>,
}

impl<T> ListItem<T> {
    pub fn new(value: T) -> Self {
        Self {
            key: None,
            value,
            disabled: Value::Static(false),
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
}

impl<T> From<T> for ListItem<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

#[derive(Clone)]
pub struct ListSection<T, VM> {
    header: Element<VM>,
    items: Vec<ListItem<T>>,
}

impl<T, VM> ListSection<T, VM> {
    pub fn new(header: impl Into<Element<VM>>, items: Vec<ListItem<T>>) -> Self {
        Self {
            header: header.into(),
            items,
        }
    }
}

enum ListRow<T, VM> {
    Header(Element<VM>),
    Item {
        source_index: usize,
        key: WidgetKey,
        value: T,
        disabled: Value<bool>,
    },
}

struct ListRowSource<T, VM> {
    rows: Arc<[ListRow<T, VM>]>,
}

impl<T: Clone, VM> Clone for ListRow<T, VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Header(header) => Self::Header(header.clone()),
            Self::Item {
                source_index,
                key,
                value,
                disabled,
            } => Self::Item {
                source_index: *source_index,
                key: key.clone(),
                value: value.clone(),
                disabled: disabled.clone(),
            },
        }
    }
}

impl<T, VM> Clone for ListRowSource<T, VM> {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
        }
    }
}

impl<T, VM> ItemSource<ListRow<T, VM>> for ListRowSource<T, VM>
where
    T: Clone + Send + Sync + 'static,
    VM: 'static,
{
    fn len(&self) -> usize {
        self.rows.len()
    }

    fn item(&self, index: usize) -> Option<ListRow<T, VM>> {
        self.rows.get(index).cloned()
    }

    fn key(&self, index: usize) -> Option<WidgetKey> {
        match self.rows.get(index)? {
            ListRow::Header(_) => Some(WidgetKey::from(format!("section-{index}"))),
            ListRow::Item { key, .. } => Some(key.clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListStyle {
    pub surface: super::style::WidgetSurfaceStyle,
    pub item_height: Dp,
    pub item_padding: Insets,
    pub item_radius: Dp,
    pub item_background: Value<Color>,
    pub item_hover_background: Value<Color>,
    pub item_selected_background: Value<Color>,
    pub item_disabled_background: Value<Color>,
    pub group_header_background: Value<Color>,
    pub group_header_text: Value<Color>,
}

impl ListStyle {
    pub fn default_for_theme(theme: &crate::ui::theme::Theme) -> Self {
        let palette = palette_from_theme(theme);
        let (item_height, item_padding, item_radius) = match theme.density {
            Density::Compact => (
                dp(32.0),
                Insets::symmetric(theme.spacing.sm, theme.spacing.xs),
                theme.radius.md,
            ),
            Density::Comfortable => (
                dp(40.0),
                Insets::symmetric(theme.spacing.sm + theme.spacing.xs, theme.spacing.sm),
                theme.radius.lg,
            ),
            Density::Spacious => (
                dp(48.0),
                Insets::symmetric(theme.spacing.md, theme.spacing.sm + theme.spacing.xs),
                theme.radius.xl,
            ),
        };
        Self {
            surface: super::style::WidgetSurfaceStyle::default(),
            item_height,
            item_padding,
            item_radius,
            item_background: Value::Static(Color::TRANSPARENT),
            item_hover_background: Value::Static(palette.on_surface.with_alpha_factor(0.06)),
            item_selected_background: Value::Static(palette.primary.with_alpha_factor(0.12)),
            item_disabled_background: Value::Static(Color::TRANSPARENT),
            group_header_background: Value::Static(Color::TRANSPARENT),
            group_header_text: Value::Static(palette.on_surface_muted),
        }
    }
}

fn resolve_list_style(
    style: Option<&StyleResolver<ListStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
) -> ListStyle {
    let mut base = ListStyle::default_for_theme(context.theme);
    context.theme.components.list.apply(&mut base, context);
    style_sheet.apply_list(&mut base, context, visual);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

pub struct List<T, VM> {
    rows: Vec<ListRow<T, VM>>,
    render: Arc<dyn Fn(ListItemContext<T>) -> Element<VM> + Send + Sync>,
    selected_keys: Value<Vec<WidgetKey>>,
    selection_mode: ListSelectionMode,
    loading: Value<bool>,
    empty_view: Option<Element<VM>>,
    loading_view: Option<Element<VM>>,
    item_layout: ItemLayout,
    style: Option<StyleResolver<ListStyle>>,
    on_selection_change: Option<ValueCommand<VM, ListSelectionChange>>,
    on_item_action: Option<ValueCommand<VM, ListItemAction>>,
    context_menu: Vec<MenuItemState<VM>>,
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

impl<T, VM: 'static> List<T, VM>
where
    T: Clone + Send + Sync + 'static,
    VM: 'static,
{
    pub fn new<I>(
        items: Vec<I>,
        render: impl Fn(ListItemContext<T>) -> Element<VM> + Send + Sync + 'static,
    ) -> Self
    where
        I: Into<ListItem<T>>,
    {
        let rows = items
            .into_iter()
            .map(Into::into)
            .enumerate()
            .map(|(index, item)| ListRow::Item {
                source_index: index,
                key: item.key.unwrap_or_else(|| WidgetKey::from(index)),
                value: item.value,
                disabled: item.disabled,
            })
            .collect();
        Self::from_rows(rows, render)
    }

    pub fn sections(
        sections: Vec<ListSection<T, VM>>,
        render: impl Fn(ListItemContext<T>) -> Element<VM> + Send + Sync + 'static,
    ) -> Self {
        let mut rows = Vec::new();
        let mut item_index = 0usize;
        for section in sections {
            rows.push(ListRow::Header(section.header));
            for item in section.items {
                rows.push(ListRow::Item {
                    source_index: item_index,
                    key: item.key.unwrap_or_else(|| WidgetKey::from(item_index)),
                    value: item.value,
                    disabled: item.disabled,
                });
                item_index += 1;
            }
        }
        Self::from_rows(rows, render)
    }

    fn from_rows(
        rows: Vec<ListRow<T, VM>>,
        render: impl Fn(ListItemContext<T>) -> Element<VM> + Send + Sync + 'static,
    ) -> Self {
        let interactions = InteractionHandlers {
            cursor_style: Some(Value::Static(CursorStyle::Pointer)),
            ..Default::default()
        };
        let render = Arc::new(render);
        Self {
            rows,
            render,
            selected_keys: Value::Static(Vec::new()),
            selection_mode: ListSelectionMode::Single,
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
            on_item_action: None,
            context_menu: Vec::new(),
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

    pub fn selection_mode(mut self, mode: ListSelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    pub fn selected_keys(mut self, keys: impl Into<Value<Vec<WidgetKey>>>) -> Self {
        self.selected_keys = keys.into();
        self
    }

    pub fn selected_key(mut self, key: impl Into<Value<Option<WidgetKey>>>) -> Self {
        let key = key.into();
        self.selected_keys = match key {
            Value::Static(key) => Value::Static(key.into_iter().collect()),
            Value::Signal(signal) => Value::Signal(signal.map(|key| key.into_iter().collect())),
        };
        self
    }

    pub fn on_selection_change(mut self, command: ValueCommand<VM, ListSelectionChange>) -> Self {
        self.on_selection_change = Some(command);
        self
    }

    pub fn on_item_action(mut self, command: ValueCommand<VM, ListItemAction>) -> Self {
        self.on_item_action = Some(command);
        self
    }

    pub fn context_menu(mut self, items: Vec<MenuItem<VM>>) -> Self {
        self.context_menu = items.into_iter().map(MenuItemState::from_public).collect();
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
        mutator: impl Fn(&mut ListStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| ListStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> ListStyle + Send + Sync + 'static,
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

impl<T, VM> From<List<T, VM>> for Element<VM>
where
    T: Clone + Send + Sync + 'static,
    VM: 'static,
{
    fn from(list: List<T, VM>) -> Self {
        list.into_element()
    }
}

impl<T, VM> List<T, VM>
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
        if self
            .rows
            .iter()
            .all(|row| matches!(row, ListRow::Header(_)))
        {
            return self
                .empty_view
                .unwrap_or_else(|| Stack::new().child(Text::new("No items")).into());
        }

        let list_id = WidgetId::next();
        let style_resolver = self.style.clone();
        let row_visual = self.visual.clone();
        let root_style_resolver = self.style.clone();
        let root_visual = self.visual.clone();
        let (selected_keys, selected_key_membership) = match self.selected_keys {
            Value::Static(keys) => {
                let shared: Arc<[WidgetKey]> = keys.into();
                let membership = Arc::new(shared.iter().cloned().collect::<HashSet<_>>());
                (Value::Static(shared), Value::Static(membership))
            }
            Value::Signal(signal) => {
                let shared = signal.map_memo(|keys| Arc::<[WidgetKey]>::from(keys));
                let membership =
                    shared.map_memo(|keys| Arc::new(keys.iter().cloned().collect::<HashSet<_>>()));
                (Value::Signal(shared), Value::Signal(membership))
            }
        };
        let row_keys: Arc<[WidgetKey]> = self
            .rows
            .iter()
            .filter_map(|row| match row {
                ListRow::Item { key, .. } => Some(key.clone()),
                ListRow::Header(_) => None,
            })
            .collect::<Vec<_>>()
            .into();
        let sibling_index_by_key = Arc::new(row_keys.iter().cloned().enumerate().fold(
            HashMap::with_capacity(row_keys.len()),
            |mut indexes, (index, key)| {
                // Preserve the former `.position()` semantics for malformed
                // sources with duplicate keys: range selection uses the first.
                indexes.entry(key).or_insert(index);
                indexes
            },
        ));
        let row_disabled: Arc<[bool]> = self
            .rows
            .iter()
            .filter_map(|row| match row {
                ListRow::Item { disabled, .. } => Some(disabled.resolve()),
                ListRow::Header(_) => None,
            })
            .collect::<Vec<_>>()
            .into();
        let source = ListRowSource {
            rows: self.rows.into(),
        };
        let on_selection_change = self.on_selection_change.clone();
        let on_item_action = self.on_item_action.clone();
        let selection_mode = self.selection_mode;
        let item_extent = self.item_layout.estimate().max(Dp::ZERO);
        let item_spacing = self.item_layout.spacing().max(Dp::ZERO);
        let render = self.render.clone();
        let context_menu = Arc::new(self.context_menu);
        let item_layout = self.item_layout;
        let selection = Arc::new(ListSelectionMetadata {
            selected_keys,
            selected_key_membership,
            sibling_keys: row_keys,
            sibling_index_by_key,
            sibling_disabled: row_disabled,
        });
        let mut list: Element<VM> = VirtualList::new_with_style_context(
            source,
            move |_visible, row, context, style_sheet| {
                let row_style = Arc::new(resolve_list_style(
                    style_resolver.as_ref(),
                    &context,
                    style_sheet,
                    &row_visual,
                ));
                match row {
                    ListRow::Header(header) => header.clone(),
                    ListRow::Item {
                        source_index,
                        key,
                        value,
                        disabled,
                    } => {
                        let selected = selection
                            .selected_key_membership
                            .resolve_ref(|membership| membership.contains(key));
                        let disabled_now = disabled.resolve();
                        let context = ListItemContext {
                            index: *source_index,
                            key: key.clone(),
                            item: value.clone(),
                            selected,
                            disabled: disabled_now,
                        };
                        let child = render(context.clone());
                        let row_container = Flex::vertical()
                            .align(Align::Stretch)
                            .child(child)
                            .padding(row_style.item_padding);
                        let row_container = match item_layout {
                            ItemLayout::Fixed { item_extent, .. } => {
                                row_container.height(item_extent)
                            }
                            ItemLayout::Estimated { estimate, .. } => {
                                row_container.height(estimate)
                            }
                            ItemLayout::Measured { .. } => {
                                row_container.min_height(row_style.item_height)
                            }
                        };
                        let mut row: Element<VM> = row_container.into();
                        row.key = Some(key.clone());
                        row.interactions.cursor_style = Some(Value::Static(if disabled_now {
                            CursorStyle::Default
                        } else {
                            CursorStyle::Pointer
                        }));
                        row.visual.border_radius = Some(Value::Static(row_style.item_radius));
                        row.list_item = Some(ListItemState {
                            list_id,
                            row_index: _visible,
                            item_index: *source_index,
                            key: key.clone(),
                            selection: selection.clone(),
                            selection_mode,
                            disabled: disabled.clone(),
                            item_extent,
                            item_spacing,
                            item_background: row_style.item_background.clone(),
                            item_hover_background: row_style.item_hover_background.clone(),
                            item_selected_background: row_style.item_selected_background.clone(),
                            item_disabled_background: row_style.item_disabled_background.clone(),
                            on_selection_change: on_selection_change.clone(),
                            on_item_action: on_item_action.clone(),
                        });
                        if !context_menu.is_empty() {
                            let on_show = ValueCommand::new(|_: &mut VM, _: LongPressEvent| {});
                            row.interactions.gesture =
                                Some(match row.interactions.gesture.take() {
                                    Some(existing) => existing.on_long_press(on_show),
                                    None => GestureRecognizer::new().on_long_press(on_show),
                                });
                            let descriptor = ContextMenuDescriptor {
                                items: context_menu.as_ref().to_vec(),
                                on_open_change: None,
                                disabled: Value::Static(false),
                                style: None,
                            };
                            row.context_menu = Some(Box::new(descriptor));
                        }
                        row
                    }
                }
            },
        )
        .widget_id(list_id)
        .item_layout(self.item_layout)
        .style_full_with_style_sheet(move |context, style_sheet, _visual, state| {
            let mut style = resolve_list_style(
                root_style_resolver.as_ref(),
                context,
                style_sheet,
                &root_visual,
            );
            style_sheet.apply_list_state(&mut style, context, &root_visual, state);
            let mut container = ContainerStyle::default_for_theme(context.theme);
            container.surface = style.surface;
            container
        })
        .into();
        list.key = self.key;
        list.layout = self.layout;
        list.focus.focusable = self.focusable;
        list.focus.tab_index = self.tab_index;
        list.focus.scope = self.focus_scope;
        list.visual = self.visual;
        list.interactions = self.interactions;
        list.lifecycle_events = self.lifecycle_events;
        list.media_events = self.media_events;
        list
    }
}
