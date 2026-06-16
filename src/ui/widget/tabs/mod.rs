use crate::foundation::view_model::{Command, CommandContext, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Align, Axis, Insets, LayoutStyle, Overflow, Value};
use crate::ui::widget::button::Button;
use crate::ui::widget::common::{Point, TabPlacement, TabTriggerState, VisualStyle, WidgetId};
use crate::ui::widget::container::{set_layout_inset, set_layout_length, set_layout_lengths};
use crate::ui::widget::container::{Flex, IntoLengthValue};
use crate::ui::widget::core::Element;
use crate::ui::widget::menu::{Menu, MenuItem};
use crate::ui::widget::scroll_view::ScrollView;
use crate::ui::widget::style::{ButtonStyle, ContainerStyle, StyleResolver, StyleSheet, TabsStyle};
use crate::ui::widget::Stack;

const TABS_MORE_VISIBLE_BUDGET: usize = 4;
const HIDDEN_PANEL_OFFSET_DP: f32 = 100_000.0;

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
    let axis = if placement.is_horizontal() {
        Axis::Horizontal
    } else {
        Axis::Vertical
    };
    let layout_style = resolve_tabs_style_for_layout(style.as_ref());
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

    let list = Flex::new(axis)
        .gap(layout_style.tab_gap)
        .align(Align::Start);
    let list = list.child(build_triggers(
        group_id,
        &specs,
        &selected,
        placement,
        overflow_mode,
        reorderable.clone(),
        style.clone(),
        on_change,
        on_reorder,
    ));
    match overflow_mode {
        TabsOverflowMode::Scroll => ScrollView::new()
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
            })
            .child(list)
            .into(),
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

fn build_triggers<VM: 'static>(
    group_id: WidgetId,
    specs: &[TabTriggerSpec],
    selected: &Value<String>,
    placement: TabPlacement,
    overflow_mode: TabsOverflowMode,
    reorderable: Value<bool>,
    style: Option<StyleResolver<TabsStyle>>,
    on_change: Option<ValueCommand<VM, (String, String)>>,
    on_reorder: Option<ValueCommand<VM, TabsReorderEvent>>,
) -> Vec<Element<VM>> {
    let layout_style = resolve_tabs_style_for_layout(style.as_ref());
    let initial_selected = selected.resolve_untracked();
    let (visible_specs, overflow_specs) =
        split_tabs_for_overflow(specs, &initial_selected, overflow_mode);
    let mut triggers =
        Vec::with_capacity(visible_specs.len() + usize::from(!overflow_specs.is_empty()));
    for item in visible_specs {
        let active = tab_active_value(selected, &item.key);
        let mut button = Button::new(item.label.clone())
            .ghost()
            .disable(item.disabled.clone())
            .min_width(layout_style.tab_min_width)
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
        element.focus.tab_index = Some(0);
        element = element.with_tab_trigger_state(TabTriggerState {
            group_id,
            index: item.index,
            placement,
            key: item.key.clone(),
            label: item.label.clone(),
            active,
            on_change: on_change.clone(),
            reorderable: reorderable.clone(),
            on_reorder: on_reorder.clone(),
        });
        triggers.push(element);
    }
    if !overflow_specs.is_empty() {
        triggers.push(build_more_trigger(
            overflow_specs,
            selected,
            placement,
            style,
            on_change,
        ));
    }
    triggers
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
    overflow_specs: Vec<&TabTriggerSpec>,
    selected: &Value<String>,
    placement: TabPlacement,
    style: Option<StyleResolver<TabsStyle>>,
    on_change: Option<ValueCommand<VM, (String, String)>>,
) -> Element<VM> {
    let layout_style = resolve_tabs_style_for_layout(style.as_ref());
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

    let trigger = Button::new("More")
        .ghost()
        .min_width(layout_style.tab_min_width)
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
    let layout_style = resolve_tabs_style_for_layout(style.as_ref());
    let panel_keys: Vec<String> = items.iter().map(|item| item.key.clone()).collect();
    let initial_selected = selected.resolve_untracked();
    let mut panel = Flex::vertical()
        .grow(1.0)
        .padding(layout_style.panel_padding)
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

    for item in items {
        let active = Value::Static(tab_panel_is_active(
            &initial_selected,
            &item.key,
            panel_keys.first().map(String::as_str),
            &panel_keys,
        ));
        let slot = Stack::new()
            .position_absolute()
            .inset(crate::ui::unit::dp(0.0))
            .width(crate::ui::layout::Length::Percent(1.0))
            .height(crate::ui::layout::Length::Percent(1.0))
            .opacity(active_opacity_value(active.clone()))
            .offset(active_panel_offset_value(active))
            .child(item.panel);
        panel = panel.child(slot);
    }
    panel.into()
}

fn tab_active_value(selected: &Value<String>, key: &str) -> Value<bool> {
    match selected {
        Value::Static(selected) => Value::Static(selected == key),
        Value::Signal(signal) => {
            let key = key.to_string();
            Value::Signal(signal.map_memo(move |selected| selected == key))
        }
    }
}

fn any_tab_active_value(selected: &Value<String>, keys: Vec<String>) -> Value<bool> {
    match selected {
        Value::Static(selected) => Value::Static(keys.iter().any(|key| key == selected)),
        Value::Signal(signal) => {
            Value::Signal(signal.map_memo(move |selected| keys.iter().any(|key| key == &selected)))
        }
    }
}

fn tab_panel_is_active(
    selected: &str,
    key: &str,
    fallback_key: Option<&str>,
    known_keys: &[String],
) -> bool {
    selected == key
        || (!known_keys.iter().any(|known| known == selected) && fallback_key == Some(key))
}

fn active_opacity_value(active: Value<bool>) -> Value<f32> {
    match active {
        Value::Static(active) => Value::Static(if active { 1.0 } else { 0.0 }),
        Value::Signal(signal) => {
            Value::Signal(signal.map_memo(|active| if active { 1.0 } else { 0.0 }))
        }
    }
}

fn active_panel_offset_value(active: Value<bool>) -> Value<Point> {
    match active {
        Value::Static(active) => Value::Static(panel_offset_for_active(active)),
        Value::Signal(signal) => Value::Signal(signal.map_memo(panel_offset_for_active)),
    }
}

fn panel_offset_for_active(active: bool) -> Point {
    if active {
        Point::ZERO
    } else {
        Point::new(
            crate::ui::unit::dp(HIDDEN_PANEL_OFFSET_DP),
            crate::ui::unit::dp(0.0),
        )
    }
}

fn resolve_tabs_style(
    style: Option<&StyleResolver<TabsStyle>>,
    context: &StyleContext<'_>,
) -> TabsStyle {
    let style_sheet = StyleSheet::default();
    let visual = VisualStyle::default();
    resolve_tabs_style_with_sheet(
        style,
        context,
        &style_sheet,
        &visual,
        WidgetState::default(),
    )
}

fn resolve_tabs_style_with_sheet(
    style: Option<&StyleResolver<TabsStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> TabsStyle {
    let mut base = TabsStyle::default_for_theme(context.theme);
    context.theme.components.tabs.apply(&mut base, context);
    style_sheet.apply_tabs(&mut base, context, visual);
    style_sheet.apply_tabs_state(&mut base, context, visual, state);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

fn resolve_tabs_style_for_layout(style: Option<&StyleResolver<TabsStyle>>) -> TabsStyle {
    let theme = crate::ui::theme::Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_tabs_style(style, &context)
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
    button.background = style.tab_background.clone();
    button.foreground = style.tab_foreground.clone();
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

fn active_value<T>(active: &Value<bool>, on: Value<T>, off: Value<T>) -> Value<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    match active {
        Value::Static(true) => on,
        Value::Static(false) => off,
        Value::Signal(signal) => Value::Signal(signal.map_memo(move |active| {
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
