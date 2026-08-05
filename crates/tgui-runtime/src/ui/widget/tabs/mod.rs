use crate::foundation::view_model::{Command, CommandContext, ValueCommand};
use crate::theme::{StateValue, StyleContext, WidgetState};
use crate::ui::layout::{Align, Axis, Insets, LayoutStyle, Length, Overflow, Value};
use crate::ui::widget::button::Button;
use crate::ui::widget::common::{Point, TabPlacement, TabTriggerState, VisualStyle, WidgetId};
use crate::ui::widget::container::{set_layout_inset, set_layout_length, set_layout_lengths};
use crate::ui::widget::container::{Flex, IntoLengthValue};
use crate::ui::widget::core::Element;
use crate::ui::widget::menu::{Menu, MenuItem};
use crate::ui::widget::scroll_view::ScrollView;
use crate::ui::widget::style::{ButtonStyle, ContainerStyle, StyleResolver, StyleSheet, TabsStyle};
use crate::ui::widget::{FocusScopeOptions, For, Stack};
use std::collections::HashMap;
use std::sync::Arc;

const TABS_MORE_VISIBLE_BUDGET: usize = 4;
const TAB_PANEL_SHIFT_DP: f32 = 8.0;

/// Tab strip overflow behavior.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabsOverflowMode {
    /// Keep the strip scrollable when tab triggers exceed the available space.
    #[default]
    Scroll,
    /// Move lower-priority triggers into an uncontrolled More menu.
    More,
}

/// Payload emitted when a reorderable tab trigger is dragged onto another tab.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabsReorderEvent {
    pub from_index: usize,
    pub to_index: usize,
    pub key: String,
    pub target_key: String,
    pub placement: TabPlacement,
}

/// 单个 tab 声明。
#[derive(Clone)]
pub struct TabItem<VM> {
    key: String,
    label: String,
    panel: Element<VM>,
    disabled: Value<bool>,
}

impl<VM> TabItem<VM> {
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        panel: impl Into<Element<VM>>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            panel: panel.into(),
            disabled: Value::Static(false),
        }
    }

    pub fn disabled(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
        self
    }
}

/// 一组可切换的 tab panel。
pub struct Tabs<VM> {
    id: WidgetId,
    key: Option<crate::ui::widget::WidgetKey>,
    items: Vec<TabItem<VM>>,
    selected: Value<String>,
    placement: TabPlacement,
    overflow_mode: TabsOverflowMode,
    reorderable: Value<bool>,
    style: Option<StyleResolver<TabsStyle>>,
    on_change: Option<ValueCommand<VM, (String, String)>>,
    on_reorder: Option<ValueCommand<VM, TabsReorderEvent>>,
    layout: LayoutStyle,
}

pub type TabView<VM> = Tabs<VM>;

macro_rules! impl_tabs_layout_api {
    () => {
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

        pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.layout.margin = insets.into();
            self
        }

        pub fn padding(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.layout.padding = Some(insets.into());
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

        pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
            self.layout.basis = Some(basis.into_length_value());
            self
        }

        pub fn align_self(mut self, align: Align) -> Self {
            self.layout.align_self = Some(align);
            self
        }

        pub fn justify_self(mut self, align: Align) -> Self {
            self.layout.justify_self = Some(align);
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

        pub fn inset(mut self, value: impl IntoLengthValue + Copy) -> Self {
            set_layout_inset(&mut self.layout.left, value);
            set_layout_inset(&mut self.layout.top, value);
            set_layout_inset(&mut self.layout.right, value);
            set_layout_inset(&mut self.layout.bottom, value);
            self
        }
    };
}

impl<VM> Tabs<VM> {
    pub fn new(items: Vec<TabItem<VM>>, selected: impl Into<Value<String>>) -> Self {
        Self {
            id: WidgetId::next(),
            key: None,
            items,
            selected: selected.into(),
            placement: TabPlacement::Top,
            overflow_mode: TabsOverflowMode::Scroll,
            reorderable: Value::Static(false),
            style: None,
            on_change: None,
            on_reorder: None,
            layout: LayoutStyle::default(),
        }
    }

    pub fn tab(mut self, item: TabItem<VM>) -> Self {
        self.items.push(item);
        self
    }

    pub fn items(mut self, items: Vec<TabItem<VM>>) -> Self {
        self.items.extend(items);
        self
    }

    pub fn placement(mut self, placement: TabPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn overflow_mode(mut self, mode: TabsOverflowMode) -> Self {
        self.overflow_mode = mode;
        self
    }

    pub fn reorderable(mut self, reorderable: impl Into<Value<bool>>) -> Self {
        self.reorderable = reorderable.into();
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut TabsStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| TabsStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> TabsStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, (String, String)>) -> Self {
        self.on_change = Some(command);
        self
    }

    pub fn on_reorder(mut self, command: ValueCommand<VM, TabsReorderEvent>) -> Self {
        self.on_reorder = Some(command);
        self
    }

    pub fn key(mut self, key: impl Into<crate::ui::widget::WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    impl_tabs_layout_api!();
}

impl<VM: 'static> From<Tabs<VM>> for Element<VM> {
    fn from(tabs: Tabs<VM>) -> Self {
        let Tabs {
            id,
            key,
            items,
            selected,
            placement,
            overflow_mode,
            reorderable,
            style,
            on_change,
            on_reorder,
            layout,
        } = tabs;
        let strip = build_tab_strip(
            id,
            &items,
            selected.clone(),
            placement,
            overflow_mode,
            reorderable,
            style.clone(),
            on_change,
            on_reorder,
        );
        let panel = build_panel(items, selected, style.clone());

        let root = match placement {
            TabPlacement::Top => Flex::vertical().child(strip).child(panel),
            TabPlacement::Bottom => Flex::vertical().child(panel).child(strip),
            TabPlacement::Left => Flex::horizontal().child(strip).child(panel),
            TabPlacement::Right => Flex::horizontal().child(panel).child(strip),
        }
        .gap(crate::ui::unit::dp(0.0));

        let mut element: Element<VM> = root.into();
        element.id = id;
        element.key = key;
        element.layout = layout;
        element
    }
}

fn build_tab_strip<VM: 'static>(
    group_id: WidgetId,
    items: &[TabItem<VM>],
    selected: Value<String>,
    placement: TabPlacement,
    overflow_mode: TabsOverflowMode,
    reorderable: Value<bool>,
    style: Option<StyleResolver<TabsStyle>>,
    on_change: Option<ValueCommand<VM, (String, String)>>,
    on_reorder: Option<ValueCommand<VM, TabsReorderEvent>>,
) -> Element<VM> {
    let scroll_container_id = (overflow_mode == TabsOverflowMode::Scroll).then(WidgetId::next);
    let axis = if placement.is_horizontal() {
        Axis::Horizontal
    } else {
        Axis::Vertical
    };
    let specs: Vec<TabTriggerSpec> = items
        .iter()
        .enumerate()
        .map(|(index, item)| TabTriggerSpec {
            index,
            key: item.key.clone(),
            label: item.label.clone(),
            disabled: item.disabled.clone(),
        })
        .collect();

    let strip_style = style.clone();
    let list = Flex::new(axis)
        .runtime_layout(move |_, container, context, style_sheet, visual| {
            let resolved = resolve_tabs_style_with_sheet(
                strip_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            container.gap = Value::Static(Length::Px(resolved.tab_gap));
        })
        .align(Align::Start);
    let entry_specs = specs.clone();
    let entry_selected = selected.clone();
    let render_selected = selected.clone();
    let render_style = style.clone();
    let render_on_change = on_change.clone();
    let render_on_reorder = on_reorder.clone();
    let render_reorderable = reorderable.clone();
    let list = list.child(For::new_with_resolver(
        move || tab_trigger_entries(&entry_specs, &entry_selected.resolve(), overflow_mode),
        TabTriggerEntry::stable_key,
        move |_index, entry| match entry {
            TabTriggerEntry::Tab { item, tab_stop } => build_tab_trigger(
                group_id,
                scroll_container_id,
                item,
                *tab_stop,
                &render_selected,
                placement,
                render_reorderable.clone(),
                render_style.clone(),
                render_on_change.clone(),
                render_on_reorder.clone(),
            ),
            TabTriggerEntry::More(items) => build_more_trigger(
                items.clone(),
                &render_selected,
                placement,
                render_style.clone(),
                render_on_change.clone(),
            ),
        },
    ));
    match overflow_mode {
        TabsOverflowMode::Scroll => {
            let strip = ScrollView::new()
                .focusable(false)
                .overflow_x(if placement.is_horizontal() {
                    Overflow::Scroll
                } else {
                    Overflow::Hidden
                })
                .overflow_y(if placement.is_horizontal() {
                    Overflow::Hidden
                } else {
                    Overflow::Scroll
                })
                .show_scrollbar(false)
                .style_full_with_style_sheet({
                    let style = style.clone();
                    move |context, style_sheet, visual, state| {
                        tab_bar_container_style(
                            resolve_tabs_style_with_sheet(
                                style.as_ref(),
                                context,
                                style_sheet,
                                visual,
                                state,
                            ),
                            context,
                        )
                    }
                });
            let strip = if placement.is_horizontal() {
                strip.width(Length::Percent(1.0))
            } else {
                strip.height(Length::Percent(1.0))
            };
            let mut element: Element<VM> = strip.child(list).into();
            element.id = scroll_container_id.expect("scroll tabs should own a scroll container");
            element
        }
        TabsOverflowMode::More => Flex::new(axis)
            .style_full_with_style_sheet({
                let style = style.clone();
                move |context, style_sheet, visual, state| {
                    tab_bar_container_style(
                        resolve_tabs_style_with_sheet(
                            style.as_ref(),
                            context,
                            style_sheet,
                            visual,
                            state,
                        ),
                        context,
                    )
                }
            })
            .child(list)
            .into(),
    }
}

/// 构建 tab 触发按钮所需的轻量信息（不含 panel 内容）。
#[derive(Clone)]
struct TabTriggerSpec {
    index: usize,
    key: String,
    label: String,
    disabled: Value<bool>,
}

#[derive(Clone)]
enum TabTriggerEntry {
    Tab {
        item: TabTriggerSpec,
        tab_stop: bool,
    },
    More(Vec<TabTriggerSpec>),
}

impl TabTriggerEntry {
    fn stable_key(&self) -> String {
        match self {
            Self::Tab { item, .. } => format!("__tgui_tab_trigger_tab_{}", item.key),
            Self::More(_) => "__tgui_tab_trigger_more".to_string(),
        }
    }
}

fn tab_trigger_entries(
    specs: &[TabTriggerSpec],
    selected: &str,
    overflow_mode: TabsOverflowMode,
) -> Vec<TabTriggerEntry> {
    let enabled = specs
        .iter()
        .map(|spec| !spec.disabled.resolve())
        .collect::<Vec<_>>();
    let tab_stop_index = specs
        .iter()
        .enumerate()
        .find_map(|(index, spec)| (enabled[index] && spec.key == selected).then_some(spec.index))
        .or_else(|| {
            specs
                .iter()
                .enumerate()
                .find_map(|(index, spec)| enabled[index].then_some(spec.index))
        });
    let visibility_selection = tab_stop_index
        .and_then(|index| specs.iter().find(|spec| spec.index == index))
        .map(|spec| spec.key.as_str())
        .unwrap_or(selected);
    let (visible, overflow) = split_tabs_for_overflow(specs, visibility_selection, overflow_mode);
    let mut entries = visible
        .into_iter()
        .cloned()
        .map(|item| TabTriggerEntry::Tab {
            tab_stop: Some(item.index) == tab_stop_index,
            item,
        })
        .collect::<Vec<_>>();
    if !overflow.is_empty() {
        entries.push(TabTriggerEntry::More(
            overflow.into_iter().cloned().collect(),
        ));
    }
    entries
}

fn build_tab_trigger<VM: 'static>(
    group_id: WidgetId,
    scroll_container_id: Option<WidgetId>,
    item: &TabTriggerSpec,
    tab_stop: bool,
    selected: &Value<String>,
    placement: TabPlacement,
    reorderable: Value<bool>,
    style: Option<StyleResolver<TabsStyle>>,
    on_change: Option<ValueCommand<VM, (String, String)>>,
    on_reorder: Option<ValueCommand<VM, TabsReorderEvent>>,
) -> Element<VM> {
    let active = tab_active_value(selected, &item.key);
    let trigger_style = style.clone();
    let mut button = Button::new(item.label.clone())
        .ghost()
        .disable(item.disabled.clone())
        .runtime_layout(move |layout, context, style_sheet, visual| {
            let resolved = resolve_tabs_style_with_sheet(
                trigger_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            layout.min_width = Some(Value::Static(Length::Px(resolved.tab_min_width)));
        })
        .style_full_with_style_sheet({
            let style = style.clone();
            let active = active.clone();
            move |context, style_sheet, visual, state| {
                tab_button_style(
                    resolve_tabs_style_with_sheet(
                        style.as_ref(),
                        context,
                        style_sheet,
                        visual,
                        state,
                    ),
                    context,
                    active.clone(),
                )
            }
        });
    if let Some(command) = on_change.clone() {
        let key = item.key.clone();
        let label = item.label.clone();
        button = button.on_click(Command::new_with_context(
            move |vm: &mut VM, ctx: &CommandContext<VM>| {
                command.execute_with_context(vm, (key.clone(), label.clone()), ctx);
            },
        ));
    }

    let mut element: Element<VM> = button.into();
    element.focus.focusable = Some(true);
    element.focus.tab_index = Some(if tab_stop { 0 } else { -1 });
    element.with_tab_trigger_state(TabTriggerState {
        group_id,
        scroll_container_id,
        index: item.index,
        placement,
        key: item.key.clone(),
        label: item.label.clone(),
        selected: selected.clone(),
        active,
        on_change,
        reorderable,
        on_reorder,
    })
}

fn split_tabs_for_overflow<'a>(
    specs: &'a [TabTriggerSpec],
    selected: &str,
    overflow_mode: TabsOverflowMode,
) -> (Vec<&'a TabTriggerSpec>, Vec<&'a TabTriggerSpec>) {
    if overflow_mode == TabsOverflowMode::Scroll || specs.len() <= TABS_MORE_VISIBLE_BUDGET {
        return (specs.iter().collect(), Vec::new());
    }

    let visible_budget = TABS_MORE_VISIBLE_BUDGET.saturating_sub(1).max(1);
    let mut visible_indexes = Vec::new();
    for spec in specs.iter().take(visible_budget) {
        visible_indexes.push(spec.index);
    }
    if let Some(selected_spec) = specs.iter().find(|spec| spec.key == selected) {
        if !visible_indexes.contains(&selected_spec.index) {
            if visible_indexes.len() >= visible_budget {
                visible_indexes.pop();
            }
            visible_indexes.push(selected_spec.index);
        }
    }
    visible_indexes.sort_unstable();

    let mut visible = Vec::new();
    let mut overflow = Vec::new();
    for spec in specs {
        if visible_indexes.contains(&spec.index) {
            visible.push(spec);
        } else {
            overflow.push(spec);
        }
    }
    (visible, overflow)
}

fn build_more_trigger<VM: 'static>(
    overflow_specs: Vec<TabTriggerSpec>,
    selected: &Value<String>,
    placement: TabPlacement,
    style: Option<StyleResolver<TabsStyle>>,
    on_change: Option<ValueCommand<VM, (String, String)>>,
) -> Element<VM> {
    let active = any_tab_active_value(
        selected,
        overflow_specs
            .iter()
            .map(|item| item.key.clone())
            .collect::<Vec<_>>(),
    );
    let mut more_items = Vec::new();
    for item in overflow_specs {
        let mut menu_item = MenuItem::new(item.label.clone()).disable(item.disabled.clone());
        if let Some(command) = on_change.clone() {
            let key = item.key.clone();
            let label = item.label.clone();
            menu_item = menu_item.on_select(Command::new_with_context(
                move |vm: &mut VM, ctx: &CommandContext<VM>| {
                    command.execute_with_context(vm, (key.clone(), label.clone()), ctx);
                },
            ));
        }
        more_items.push(menu_item);
    }

    let trigger_style = style.clone();
    let trigger = Button::new("More")
        .ghost()
        .runtime_layout(move |layout, context, style_sheet, visual| {
            let resolved = resolve_tabs_style_with_sheet(
                trigger_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            layout.min_width = Some(Value::Static(Length::Px(resolved.tab_min_width)));
        })
        .style_full_with_style_sheet({
            let style = style.clone();
            let active = active.clone();
            move |context, style_sheet, visual, state| {
                tab_button_style(
                    resolve_tabs_style_with_sheet(
                        style.as_ref(),
                        context,
                        style_sheet,
                        visual,
                        state,
                    ),
                    context,
                    active.clone(),
                )
            }
        });
    Menu::new(trigger)
        .items(more_items)
        .placement(if placement.is_horizontal() {
            crate::ui::widget::overlay::Placement::bottom()
        } else {
            crate::ui::widget::overlay::Placement::right()
        })
        .into()
}

fn build_panel<VM: 'static>(
    items: Vec<TabItem<VM>>,
    selected: Value<String>,
    style: Option<StyleResolver<TabsStyle>>,
) -> Element<VM> {
    let panel_index = selected_tab_index_value(&items, selected);
    let panel_layout_style = style.clone();
    let mut panel = Flex::vertical()
        .grow(1.0)
        .runtime_layout(move |_, container, context, style_sheet, visual| {
            let resolved = resolve_tabs_style_with_sheet(
                panel_layout_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            container.padding = Some(Value::Static(resolved.panel_padding));
        })
        .overflow(Overflow::Hidden)
        .style_full_with_style_sheet({
            let style = style.clone();
            move |context, style_sheet, visual, state| {
                panel_container_style(
                    resolve_tabs_style_with_sheet(
                        style.as_ref(),
                        context,
                        style_sheet,
                        visual,
                        state,
                    ),
                    context,
                )
            }
        });

    for (index, item) in items.into_iter().enumerate() {
        let active = index_active_value(&panel_index, index);
        let opacity = active_opacity_value(active.clone());
        let offset = tab_panel_offset_value(&panel_index, index);
        let slot = Stack::new()
            .position_absolute()
            .inset(crate::ui::unit::dp(0.0))
            .width(crate::ui::layout::Length::Percent(1.0))
            .height(crate::ui::layout::Length::Percent(1.0))
            .runtime_layout(move |_, _, context, _, visual| {
                visual.opacity = opacity
                    .clone()
                    .with_default_transition(context.motion_normal_transition());
                visual.offset = offset
                    .clone()
                    .with_default_transition(context.motion_fast_transition());
            })
            .focus_scope(
                FocusScopeOptions::new()
                    .active(active)
                    .suppress_interactions_when_inactive()
                    .hide_from_accessibility_when_inactive(),
            )
            .child(item.panel);
        panel = panel.child(slot);
    }
    panel.into()
}

fn selected_tab_index_value<VM>(items: &[TabItem<VM>], selected: Value<String>) -> Value<usize> {
    let indexes = Arc::new(
        items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.key.clone(), index))
            .collect::<HashMap<_, _>>(),
    );
    match selected {
        Value::Static(selected) => Value::Static(indexes.get(&selected).copied().unwrap_or(0)),
        Value::Signal(signal) => Value::Signal(
            signal.map_memo(move |selected| indexes.get(&selected).copied().unwrap_or(0)),
        ),
    }
}

fn index_active_value(selected: &Value<usize>, index: usize) -> Value<bool> {
    match selected {
        Value::Static(selected) => Value::Static(*selected == index),
        Value::Signal(signal) => Value::Signal(signal.map_memo(move |selected| selected == index)),
    }
}

fn tab_active_value(selected: &Value<String>, key: &str) -> Value<bool> {
    match selected {
        Value::Static(selected) => Value::Static(selected == key),
        Value::Signal(signal) => {
            let key = key.to_string();
            Value::Signal(signal.map(move |selected| selected == key))
        }
    }
}

fn any_tab_active_value(selected: &Value<String>, keys: Vec<String>) -> Value<bool> {
    match selected {
        Value::Static(selected) => Value::Static(keys.iter().any(|key| key == selected)),
        Value::Signal(signal) => {
            Value::Signal(signal.map(move |selected| keys.iter().any(|key| key == &selected)))
        }
    }
}

fn active_opacity_value(active: Value<bool>) -> Value<f32> {
    match active {
        Value::Static(active) => Value::Static(if active { 1.0 } else { 0.0 }),
        Value::Signal(signal) => {
            Value::Signal(signal.map_memo(|active| if active { 1.0 } else { 0.0 }))
        }
    }
}

fn tab_panel_offset_value(selected: &Value<usize>, index: usize) -> Value<Point> {
    match selected {
        Value::Static(selected) => Value::Static(tab_panel_offset(*selected, index)),
        Value::Signal(signal) => {
            Value::Signal(signal.map_memo(move |selected| tab_panel_offset(selected, index)))
        }
    }
}

fn tab_panel_offset(selected: usize, index: usize) -> Point {
    if selected == index {
        Point::ZERO
    } else {
        let direction = if index < selected { -1.0 } else { 1.0 };
        Point::new(
            crate::ui::unit::dp(TAB_PANEL_SHIFT_DP * direction),
            crate::ui::unit::dp(0.0),
        )
    }
}

fn resolve_tabs_style_with_sheet(
    style: Option<&StyleResolver<TabsStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> TabsStyle {
    let mut base = TabsStyle::default_for_density(context.theme, context.density);
    context.theme.components.tabs.apply(&mut base, context);
    style_sheet.apply_tabs(&mut base, context, visual);
    style_sheet.apply_tabs_state(&mut base, context, visual, state);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

fn tab_button_style(
    style: TabsStyle,
    context: &StyleContext<'_>,
    active: Value<bool>,
) -> ButtonStyle {
    let mut button = ButtonStyle::default_for_theme(
        context.theme,
        crate::ui::widget::common::ButtonVariantKind::Ghost,
    );
    button.background = active_tab_state_value(
        &active,
        style.active_tab_background.clone(),
        style.tab_background.clone(),
    );
    button.foreground = active_tab_state_value(
        &active,
        style.active_tab_foreground.clone(),
        style.tab_foreground.clone(),
    );
    button.border = crate::ui::theme::StateValue::interactive(
        active_value(
            &active,
            style.indicator_color.clone(),
            crate::foundation::color::Color::TRANSPARENT.into(),
        ),
        active_value(&active, style.indicator_color.clone(), style.border.clone()),
        active_value(&active, style.indicator_color.clone(), style.border.clone()),
        active_value(
            &active,
            style.border.clone(),
            crate::foundation::color::Color::TRANSPARENT.into(),
        ),
    );
    button.border_width = style.indicator_thickness.into();
    button.radius = style.radius;
    button.padding_x = style.tab_padding.left;
    button.padding_y = style.tab_padding.top;
    button.min_height = style.tab_min_height;
    button.text_style = style.text_style;
    button
}

fn active_tab_state_value<T>(
    active: &Value<bool>,
    active_style: Value<T>,
    inactive: StateValue<Value<T>>,
) -> StateValue<Value<T>>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    // Selection owns the active fill/label across pointer states. Hover and
    // pressed tokens remain interaction feedback for inactive triggers only.
    StateValue {
        normal: active_value(active, active_style.clone(), inactive.normal),
        hovered: active_value(active, active_style.clone(), inactive.hovered),
        pressed: active_value(active, active_style.clone(), inactive.pressed),
        disabled: active_value(active, active_style.clone(), inactive.disabled),
        focused: inactive
            .focused
            .map(|value| active_value(active, active_style.clone(), value)),
        focus_visible: inactive
            .focus_visible
            .map(|value| active_value(active, active_style.clone(), value)),
        selected: inactive
            .selected
            .map(|value| active_value(active, active_style.clone(), value)),
        checked: inactive
            .checked
            .map(|value| active_value(active, active_style.clone(), value)),
        open: inactive
            .open
            .map(|value| active_value(active, active_style.clone(), value)),
        invalid: inactive
            .invalid
            .map(|value| active_value(active, active_style, value)),
    }
}

fn active_value<T>(active: &Value<bool>, on: Value<T>, off: Value<T>) -> Value<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    match active {
        Value::Static(true) => on,
        Value::Static(false) => off,
        // This reader intentionally stays lazy instead of introducing a second
        // memo layer. It always observes the current selection when the retained
        // property slot is consumed, while still subscribing directly through
        // the source signal for scene invalidation.
        Value::Signal(signal) => Value::Signal(signal.map(move |active| {
            if active {
                on.resolve_untracked()
            } else {
                off.resolve_untracked()
            }
        })),
    }
}

fn tab_bar_container_style(style: TabsStyle, context: &StyleContext<'_>) -> ContainerStyle {
    let mut container = ContainerStyle::default_for_theme(context.theme);
    container.surface = style.surface;
    container.surface.background = Some(style.tab_bar_background);
    container.surface.border_color = Some(style.border);
    container.surface.border_width = Some(style.border_width);
    container.surface.border_radius = Some(style.radius);
    container
}

fn panel_container_style(style: TabsStyle, context: &StyleContext<'_>) -> ContainerStyle {
    let mut container = ContainerStyle::default_for_theme(context.theme);
    container.surface.background = Some(style.panel_background);
    container.surface.border_color = Some(style.border);
    container.surface.border_width = Some(style.border_width);
    container.surface.border_radius = Some(style.radius);
    container
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::color::Color;
    use crate::ui::theme::Theme;

    #[test]
    fn overflow_entry_key_does_not_collide_with_a_tab_named_more() {
        let specs = ["more", "two", "three", "four", "five"]
            .into_iter()
            .enumerate()
            .map(|(index, key)| TabTriggerSpec {
                index,
                key: key.to_string(),
                label: key.to_string(),
                disabled: Value::Static(false),
            })
            .collect::<Vec<_>>();
        let keys = tab_trigger_entries(&specs, "five", TabsOverflowMode::More)
            .iter()
            .map(TabTriggerEntry::stable_key)
            .collect::<Vec<_>>();

        assert_eq!(
            keys.len(),
            keys.iter().collect::<std::collections::HashSet<_>>().len()
        );
        assert!(keys.iter().any(|key| key == "__tgui_tab_trigger_tab_more"));
        assert!(keys.iter().any(|key| key == "__tgui_tab_trigger_more"));
    }

    #[test]
    fn active_tab_tokens_override_hover_and_pressed_while_inactive_tabs_remain_stateful() {
        let inactive_background = [
            Color::rgb(11, 21, 31),
            Color::rgb(12, 22, 32),
            Color::rgb(13, 23, 33),
            Color::rgb(14, 24, 34),
        ];
        let inactive_foreground = [
            Color::rgb(41, 51, 61),
            Color::rgb(42, 52, 62),
            Color::rgb(43, 53, 63),
            Color::rgb(44, 54, 64),
        ];
        let active_background = Color::rgb(211, 31, 71);
        let active_foreground = Color::rgb(29, 181, 223);
        let theme = Theme::light();
        let context = StyleContext::from_theme(&theme);
        let mut style = TabsStyle::default_for_theme(&theme);
        style.tab_background = StateValue::interactive(
            inactive_background[0].into(),
            inactive_background[1].into(),
            inactive_background[2].into(),
            inactive_background[3].into(),
        );
        style.tab_foreground = StateValue::interactive(
            inactive_foreground[0].into(),
            inactive_foreground[1].into(),
            inactive_foreground[2].into(),
            inactive_foreground[3].into(),
        );
        style.active_tab_background = active_background.into();
        style.active_tab_foreground = active_foreground.into();

        let interaction_states = [
            WidgetState::default(),
            WidgetState {
                hovered: true,
                ..WidgetState::default()
            },
            WidgetState {
                hovered: true,
                pressed: true,
                ..WidgetState::default()
            },
            WidgetState {
                disabled: true,
                ..WidgetState::default()
            },
        ];
        let inactive_style = tab_button_style(style.clone(), &context, Value::Static(false));
        for (index, state) in interaction_states.iter().copied().enumerate() {
            assert_eq!(
                inactive_style.background.resolve(state).resolve(),
                inactive_background[index]
            );
            assert_eq!(
                inactive_style.foreground.resolve(state).resolve(),
                inactive_foreground[index]
            );
        }

        let active_style = tab_button_style(style, &context, Value::Static(true));
        for state in interaction_states {
            assert_eq!(
                active_style.background.resolve(state).resolve(),
                active_background
            );
            assert_eq!(
                active_style.foreground.resolve(state).resolve(),
                active_foreground
            );
        }
    }
}
