use crate::foundation::view_model::{Command, CommandContext, ValueCommand};
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Axis, Insets, LayoutStyle, Overflow, Value};
use crate::ui::widget::button::Button;
use crate::ui::widget::common::{TabPlacement, TabTriggerState, WidgetId};
use crate::ui::widget::container::{set_layout_inset, set_layout_length, set_layout_lengths};
use crate::ui::widget::container::{Flex, IntoLengthValue};
use crate::ui::widget::core::Element;
use crate::ui::widget::scroll_view::ScrollView;
use crate::ui::widget::style::{ButtonStyle, ContainerStyle, StyleResolver, TabsStyle};
use crate::ui::widget::Stack;

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
    style: Option<StyleResolver<TabsStyle>>,
    on_change: Option<ValueCommand<VM, (String, String)>>,
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
            style: None,
            on_change: None,
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

    pub fn style(
        mut self,
        resolver: impl Fn(ResolvedThemeMode) -> TabsStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::new(resolver));
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, (String, String)>) -> Self {
        self.on_change = Some(command);
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
            style,
            on_change,
            layout,
        } = tabs;
        let strip = build_tab_strip(
            id,
            &items,
            selected.clone(),
            placement,
            style.clone(),
            on_change,
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
    style: Option<StyleResolver<TabsStyle>>,
    on_change: Option<ValueCommand<VM, (String, String)>>,
) -> Element<VM> {
    let axis = if placement.is_horizontal() {
        Axis::Horizontal
    } else {
        Axis::Vertical
    };
    let layout_style = resolve_tabs_style_for_layout(style.as_ref());
    // 仅保留构建触发按钮所需的轻量信息，便于在选中信号变化时整体重建。
    let specs: Vec<TabTriggerSpec> = items
        .iter()
        .map(|item| TabTriggerSpec {
            key: item.key.clone(),
            label: item.label.clone(),
            disabled: item.disabled.clone(),
        })
        .collect();

    let list = Flex::new(axis)
        .gap(layout_style.tab_gap)
        .align(Align::Start);
    // 触发按钮的 active 样式依赖选中值，必须与 panel 一样走信号驱动的动态子节点，
    // 否则点击切换后 active 样式会停留在首帧的值上。
    let list = match selected {
        Value::Static(selected) => list.child(build_triggers(
            group_id,
            &specs,
            &selected,
            placement,
            style.clone(),
            on_change,
        )),
        Value::Signal(signal) => {
            let strip_style = style.clone();
            list.child(signal.map(move |selected| {
                build_triggers(
                    group_id,
                    &specs,
                    &selected,
                    placement,
                    strip_style.clone(),
                    on_change.clone(),
                )
            }))
        }
    };
    ScrollView::new()
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
        .style({
            let style = style.clone();
            move |mode| tab_bar_container_style(resolve_tabs_style(style.as_ref(), mode))
        })
        .child(list)
        .into()
}

/// 构建 tab 触发按钮所需的轻量信息（不含 panel 内容）。
#[derive(Clone)]
struct TabTriggerSpec {
    key: String,
    label: String,
    disabled: Value<bool>,
}

fn build_triggers<VM: 'static>(
    group_id: WidgetId,
    specs: &[TabTriggerSpec],
    selected: &str,
    placement: TabPlacement,
    style: Option<StyleResolver<TabsStyle>>,
    on_change: Option<ValueCommand<VM, (String, String)>>,
) -> Vec<Element<VM>> {
    let layout_style = resolve_tabs_style_for_layout(style.as_ref());
    let mut triggers = Vec::with_capacity(specs.len());
    for (index, item) in specs.iter().enumerate() {
        let active = item.key == selected;
        let disabled_now = item.disabled.resolve();
        let mut button = Button::new(item.label.clone())
            .ghost()
            .disable(item.disabled.clone())
            .min_width(layout_style.tab_min_width)
            .style({
                let style = style.clone();
                move |mode| tab_button_style(resolve_tabs_style(style.as_ref(), mode), mode, active)
            });
        if !disabled_now {
            if let Some(command) = on_change.clone() {
                let key = item.key.clone();
                let label = item.label.clone();
                button = button.on_click(Command::new_with_context(
                    move |vm: &mut VM, ctx: &CommandContext<VM>| {
                        command.execute_with_context(vm, (key.clone(), label.clone()), ctx);
                    },
                ));
            }
        }

        let mut element: Element<VM> = button.into();
        element.focus.focusable = Some(!disabled_now);
        element.focus.tab_index = Some(if active { 0 } else { -1 });
        element = element.with_tab_trigger_state(TabTriggerState {
            group_id,
            index,
            placement,
            key: item.key.clone(),
            label: item.label.clone(),
            on_change: on_change.clone(),
        });
        triggers.push(element);
    }
    triggers
}

fn build_panel<VM: 'static>(
    items: Vec<TabItem<VM>>,
    selected: Value<String>,
    style: Option<StyleResolver<TabsStyle>>,
) -> Element<VM> {
    let layout_style = resolve_tabs_style_for_layout(style.as_ref());
    let panels: Vec<(String, Element<VM>)> = items
        .into_iter()
        .map(|item| (item.key, item.panel))
        .collect();
    let mut panel = Flex::vertical()
        .grow(1.0)
        .padding(layout_style.panel_padding)
        .style({
            let style = style.clone();
            move |mode| panel_container_style(resolve_tabs_style(style.as_ref(), mode))
        });

    match selected {
        Value::Static(selected) => {
            panel = panel.child(selected_panel(&panels, &selected));
        }
        Value::Signal(signal) => {
            panel = panel.child(signal.map(move |selected| selected_panel(&panels, &selected)));
        }
    }
    panel.into()
}

fn selected_panel<VM>(panels: &[(String, Element<VM>)], selected: &str) -> Element<VM> {
    panels
        .iter()
        .find(|(key, _)| key == selected)
        .or_else(|| panels.first())
        .map(|(_, panel)| panel.clone())
        .unwrap_or_else(|| Stack::new().into())
}

fn resolve_tabs_style(
    style: Option<&StyleResolver<TabsStyle>>,
    mode: ResolvedThemeMode,
) -> TabsStyle {
    style
        .map(|resolver| resolver.resolve(mode))
        .unwrap_or_else(|| TabsStyle::default_for(mode))
}

fn resolve_tabs_style_for_layout(style: Option<&StyleResolver<TabsStyle>>) -> TabsStyle {
    resolve_tabs_style(style, ResolvedThemeMode::Light)
}

fn tab_button_style(style: TabsStyle, mode: ResolvedThemeMode, active: bool) -> ButtonStyle {
    let mut button =
        ButtonStyle::default_for(mode, crate::ui::widget::common::ButtonVariantKind::Ghost);
    button.background = if active {
        crate::ui::theme::Stateful {
            normal: style.active_tab_background.clone(),
            hovered: style.active_tab_background.clone(),
            pressed: style.active_tab_background.clone(),
            disabled: style.tab_background.disabled.clone(),
        }
    } else {
        style.tab_background.clone()
    };
    button.foreground = if active {
        crate::ui::theme::Stateful {
            normal: style.active_tab_foreground.clone(),
            hovered: style.active_tab_foreground.clone(),
            pressed: style.active_tab_foreground.clone(),
            disabled: style.tab_foreground.disabled.clone(),
        }
    } else {
        style.tab_foreground.clone()
    };
    button.border = if active {
        crate::ui::theme::Stateful {
            normal: style.indicator_color.clone(),
            hovered: style.indicator_color.clone(),
            pressed: style.indicator_color.clone(),
            disabled: style.border.clone(),
        }
    } else {
        crate::ui::theme::Stateful {
            normal: crate::foundation::color::Color::TRANSPARENT.into(),
            hovered: style.border.clone(),
            pressed: style.border.clone(),
            disabled: crate::foundation::color::Color::TRANSPARENT.into(),
        }
    };
    button.border_width = if active {
        style.indicator_thickness.into()
    } else {
        crate::ui::unit::dp(0.0).into()
    };
    button.radius = style.radius;
    button.padding_x = style.tab_padding.left;
    button.padding_y = style.tab_padding.top;
    button.min_height = style.tab_min_height;
    button.text_style = style.text_style;
    button
}

fn tab_bar_container_style(style: TabsStyle) -> ContainerStyle {
    let mut container = ContainerStyle::default_for(ResolvedThemeMode::Light);
    container.surface = style.surface;
    container.surface.background = Some(style.tab_bar_background);
    container.surface.border_color = Some(style.border);
    container.surface.border_width = Some(style.border_width);
    container.surface.border_radius = Some(style.radius);
    container
}

fn panel_container_style(style: TabsStyle) -> ContainerStyle {
    let mut container = ContainerStyle::default_for(ResolvedThemeMode::Light);
    container.surface.background = Some(style.panel_background);
    container.surface.border_color = Some(style.border);
    container.surface.border_width = Some(style.border_width);
    container.surface.border_radius = Some(style.radius);
    container
}
