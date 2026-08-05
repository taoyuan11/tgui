use std::sync::{Arc, Mutex};

use crate::foundation::binding::{InvalidationSignal, State};
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Insets, LayoutStyle, Length, Overflow, Value};
use crate::ui::unit::{dp, Dp};

use super::common::{AccessibilityRole, VisualStyle};
use super::core::Element;
use super::icon::SvgIconId;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{
    CollapseStyle, ContainerStyle, IconStyle, StyleResolver, StyleSheet, TextWidgetStyle,
};
use super::{CursorStyle, Flex, FocusScopeOptions, Icon, Stack, Text, ViewSwitch, WidgetKey};

const COLLAPSE_ICON_SIZE: Dp = dp(20.0);

pub struct Collapse<VM> {
    title: Value<String>,
    content: Element<VM>,
    expanded: Value<bool>,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, bool>>,
    style: Option<StyleResolver<CollapseStyle>>,
    layout: LayoutStyle,
    visual: VisualStyle,
    key: Option<WidgetKey>,
}

impl<VM> Collapse<VM> {
    pub fn new(title: impl Into<Value<String>>, content: impl Into<Element<VM>>) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            expanded: Value::Static(false),
            disabled: Value::Static(false),
            on_change: None,
            style: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            key: None,
        }
    }

    pub fn expanded(mut self, expanded: impl Into<Value<bool>>) -> Self {
        self.expanded = expanded.into();
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_change = Some(command);
        self
    }

    /// Disables disclosure activation while preserving the current panel state.
    pub fn disable(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
        self
    }

    /// Alias for [`Collapse::disable`].
    pub fn disabled(self, disabled: impl Into<Value<bool>>) -> Self {
        self.disable(disabled)
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut CollapseStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| CollapseStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> CollapseStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<Collapse<VM>> for Element<VM> {
    fn from(collapse: Collapse<VM>) -> Self {
        // A static value is the uncontrolled initial value. Signals remain caller-owned,
        // controlled bindings. This keeps the simple `Collapse::new(...)` form usable while
        // preserving the existing MVVM path for `expanded(signal).on_change(...)`.
        let (expanded, internal_expanded) = interactive_value(collapse.expanded.clone());
        let expanded_for_click = expanded.clone();
        let progress = collapse_progress_value(expanded.clone());
        let panel_max_height = collapse_expanded_max_height(expanded.clone());
        let on_change = disclosure_change_command(internal_expanded, collapse.on_change.clone());
        let header_style = collapse.style.clone();
        let header_identity = collapse.visual.clone();
        let icon_index = collapse_icon_index_value(&expanded);
        let header_icon = ViewSwitch::new(icon_index)
            .case(collapse_header_icon(
                SvgIconId::ChevronDown,
                header_style.clone(),
            ))
            .case(collapse_header_icon(
                SvgIconId::ChevronUp,
                header_style.clone(),
            ));
        let enabled = inverted_bool_value(&collapse.disabled);
        let header_opacity = enabled_opacity_value(&enabled);
        let header = Flex::horizontal()
            .width(Length::Percent(1.0))
            .align(crate::ui::layout::Align::Center)
            .runtime_layout({
                let header_style = collapse.style.clone();
                move |layout, container, context, style_sheet, visual| {
                    let resolved = resolve_collapse_style_with_sheet(
                        header_style.as_ref(),
                        context,
                        style_sheet,
                        visual,
                        WidgetState::default(),
                    );
                    layout.min_height.get_or_insert_with(|| {
                        Value::Static(Length::Px(resolved.header_min_height))
                    });
                    container
                        .padding
                        .get_or_insert_with(|| Value::Static(resolved.padding));
                    if matches!(
                        container.gap,
                        Value::Static(Length::Px(value)) if value == Dp::ZERO
                    ) {
                        container.gap = Value::Static(Length::Px(resolved.header_gap));
                    }
                }
            })
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_collapse_style_with_sheet(
                    header_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                );
                let mut container = ContainerStyle::default_for_theme(context.theme);
                container.surface.background = Some(resolved.header_background.resolve(state));
                container
            })
            .opacity(header_opacity)
            .focus_scope(
                FocusScopeOptions::new()
                    .active(enabled)
                    .suppress_interactions_when_inactive(),
            )
            .child(
                Text::new(collapse.title.clone())
                    .style_full_with_style_sheet({
                        let header_style = collapse.style.clone();
                        move |context, style_sheet, visual, state| {
                            let resolved = resolve_collapse_style_with_sheet(
                                header_style.as_ref(),
                                context,
                                style_sheet,
                                visual,
                                state,
                            );
                            let mut text = TextWidgetStyle::default_for_theme(context.theme);
                            text.color = resolved.header_foreground.clone();
                            text.typography = resolved.text_style.clone();
                            text
                        }
                    })
                    .grow(1.0),
            )
            .child(header_icon);
        let header_interactive = on_change.is_some();
        let header = if let Some(on_change) = on_change {
            header
                .cursor(CursorStyle::Pointer)
                .focusable(true)
                .on_click(Command::new_with_context(move |vm, context| {
                    on_change.execute_with_context(
                        vm,
                        !expanded_for_click.resolve_untracked(),
                        context,
                    );
                }))
        } else {
            header
        };
        let mut header: Element<VM> = header.into();
        header.visual.accessibility_role = Some(AccessibilityRole::Button);
        header.visual.accessibility_label = Some(collapse.title.clone());
        header.visual.accessibility_expanded = Some(expanded.clone());
        header.visual.accessibility_disabled = Some(if header_interactive {
            collapse.disabled.clone()
        } else {
            Value::Static(true)
        });
        let mut children: Vec<Element<VM>> = vec![with_visual_identity(header, &header_identity)];
        let style = collapse.style.clone();
        let panel_identity = collapse.visual.clone();
        let panel_padding_cache = Arc::new(Mutex::new(
            None::<(Insets, Option<crate::animation::Transition>, Value<Insets>)>,
        ));
        children.push(with_visual_identity(
            Stack::new()
                .overflow(Overflow::Hidden)
                .focus_scope(
                    FocusScopeOptions::new()
                        .active(expanded.clone())
                        .suppress_interactions_when_inactive()
                        .hide_from_accessibility_when_inactive(),
                )
                .runtime_layout({
                    let style = collapse.style.clone();
                    let progress = progress.clone();
                    let max_height = panel_max_height.clone();
                    let padding_cache = panel_padding_cache.clone();
                    move |layout, container, context, style_sheet, visual| {
                        let resolved = resolve_collapse_style_with_sheet(
                            style.as_ref(),
                            context,
                            style_sheet,
                            visual,
                            WidgetState::default(),
                        );
                        let transition = context.motion_normal_transition();
                        layout.max_height =
                            Some(max_height.clone().with_default_transition(transition));
                        visual.opacity = progress.clone().with_default_transition(transition);
                        if container.padding.is_none() {
                            let mut cache = padding_cache
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            let padding = match cache.as_ref() {
                                Some((cached_padding, cached_transition, value))
                                    if *cached_padding == resolved.padding
                                        && *cached_transition == transition =>
                                {
                                    value.clone()
                                }
                                _ => {
                                    let value = collapse_progress_padding(
                                        progress.clone().with_default_transition(transition),
                                        resolved.padding,
                                    );
                                    *cache = Some((resolved.padding, transition, value.clone()));
                                    value
                                }
                            };
                            container.padding = Some(padding);
                        }
                    }
                })
                .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                    let resolved = resolve_collapse_style_with_sheet(
                        style.as_ref(),
                        context,
                        style_sheet,
                        visual,
                        state,
                    );
                    let mut container = ContainerStyle::default_for_theme(context.theme);
                    container.surface.background = Some(resolved.panel_background);
                    container
                })
                .child(collapse.content)
                .into(),
            &panel_identity,
        ));
        let style = collapse.style.clone();
        let mut root: Element<VM> = Flex::vertical()
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_collapse_style_with_sheet(
                    style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                );
                let mut container = ContainerStyle::default_for_theme(context.theme);
                container.surface.border_color = Some(resolved.border);
                container.surface.border_width = Some(Value::Static(resolved.border_width));
                container.surface.border_radius = Some(Value::Static(resolved.radius));
                container
            })
            .child(children)
            .into();
        root.key = collapse.key;
        root = with_visual_identity(root, &collapse.visual);
        root.layout = merge_layout(root.layout, collapse.layout);
        root
    }
}

#[derive(Clone)]
pub struct AccordionItem<VM> {
    pub key: String,
    pub title: Value<String>,
    pub content: Element<VM>,
}

impl<VM> AccordionItem<VM> {
    pub fn new(
        key: impl Into<String>,
        title: impl Into<Value<String>>,
        content: impl Into<Element<VM>>,
    ) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            content: content.into(),
        }
    }
}

pub struct Accordion<VM> {
    items: Vec<AccordionItem<VM>>,
    expanded_key: Value<Option<String>>,
    disabled: Value<bool>,
    on_change: Option<ValueCommand<VM, Option<String>>>,
    style: Option<StyleResolver<CollapseStyle>>,
    layout: LayoutStyle,
    visual: VisualStyle,
    key: Option<WidgetKey>,
}

impl<VM> Accordion<VM> {
    pub fn new(
        items: Vec<AccordionItem<VM>>,
        expanded_key: impl Into<Value<Option<String>>>,
    ) -> Self {
        Self {
            items,
            expanded_key: expanded_key.into(),
            disabled: Value::Static(false),
            on_change: None,
            style: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            key: None,
        }
    }

    pub fn on_change(mut self, command: ValueCommand<VM, Option<String>>) -> Self {
        self.on_change = Some(command);
        self
    }

    /// Disables activation of every accordion header.
    pub fn disable(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
        self
    }

    /// Alias for [`Accordion::disable`].
    pub fn disabled(self, disabled: impl Into<Value<bool>>) -> Self {
        self.disable(disabled)
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut CollapseStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| CollapseStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> CollapseStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<Accordion<VM>> for Element<VM> {
    fn from(accordion: Accordion<VM>) -> Self {
        let (expanded_key, internal_expanded_key) = interactive_value(accordion.expanded_key);
        let accordion_on_change =
            disclosure_change_command(internal_expanded_key, accordion.on_change.clone());
        let items = accordion
            .items
            .into_iter()
            .map(|item| {
                let key = item.key.clone();
                let expanded = accordion_item_expanded_value(&expanded_key, &item.key);
                let on_change = accordion_on_change.clone();
                Collapse {
                    title: item.title,
                    content: item.content,
                    expanded,
                    disabled: accordion.disabled.clone(),
                    on_change: on_change.map(|command| {
                        ValueCommand::new_with_context(move |vm, next, context| {
                            command.execute_with_context(
                                vm,
                                if next { Some(key.clone()) } else { None },
                                context,
                            );
                        })
                    }),
                    style: accordion.style.clone(),
                    layout: LayoutStyle::default(),
                    visual: accordion.visual.clone(),
                    key: None,
                }
                .into()
            })
            .collect::<Vec<Element<VM>>>();
        let mut root: Element<VM> = Flex::vertical()
            .runtime_layout({
                let style = accordion.style.clone();
                move |_layout, container, context, style_sheet, visual| {
                    let resolved = resolve_collapse_style_with_sheet(
                        style.as_ref(),
                        context,
                        style_sheet,
                        visual,
                        WidgetState::default(),
                    );
                    if matches!(
                        container.gap,
                        Value::Static(Length::Px(value)) if value == Dp::ZERO
                    ) {
                        container.gap = Value::Static(Length::Px(resolved.gap));
                    }
                }
            })
            .child(items)
            .into();
        root.key = accordion.key;
        root = with_visual_identity(root, &accordion.visual);
        root.layout = merge_layout(root.layout, accordion.layout);
        root
    }
}

fn resolve_collapse_style_with_sheet(
    style: Option<&StyleResolver<CollapseStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> CollapseStyle {
    resolve_component_style_with_sheet(
        style,
        context,
        style_sheet,
        visual,
        state,
        CollapseStyle::default_for_theme(context.theme),
        |base, context| context.theme.components.collapse.apply(base, context),
        |sheet, base, context, visual| sheet.apply_collapse(base, context, visual),
        |sheet, base, context, visual, state| {
            sheet.apply_collapse_state(base, context, visual, state)
        },
    )
}

fn collapse_progress_value(expanded: Value<bool>) -> Value<f32> {
    match expanded {
        Value::Static(expanded) => Value::Static(if expanded { 1.0 } else { 0.0 }),
        Value::Signal(signal) => {
            Value::Signal(signal.map(|expanded| if expanded { 1.0 } else { 0.0 }))
        }
    }
}

fn interactive_value<T>(value: Value<T>) -> (Value<T>, Option<State<T>>)
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    match value {
        Value::Static(initial) => {
            let state = State::new(initial, InvalidationSignal::new());
            (Value::Signal(state.signal()), Some(state))
        }
        Value::Signal(signal) => (Value::Signal(signal), None),
    }
}

fn disclosure_change_command<VM: 'static, T>(
    internal: Option<State<T>>,
    callback: Option<ValueCommand<VM, T>>,
) -> Option<ValueCommand<VM, T>>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    if internal.is_none() && callback.is_none() {
        return None;
    }
    Some(ValueCommand::new_with_context(
        move |vm, next: T, context| {
            if let Some(internal) = internal.as_ref() {
                internal.set(next.clone());
            }
            if let Some(callback) = callback.as_ref() {
                callback.execute_with_context(vm, next, context);
            }
        },
    ))
}

fn collapse_header_icon<VM: 'static>(
    source: SvgIconId,
    header_style: Option<StyleResolver<CollapseStyle>>,
) -> Icon<VM> {
    Icon::internal(source)
        .size(COLLAPSE_ICON_SIZE)
        .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
            let resolved = resolve_collapse_style_with_sheet(
                header_style.as_ref(),
                context,
                style_sheet,
                visual,
                state,
            );
            let mut icon = IconStyle::default_for_theme(context.theme);
            icon.color = resolved.header_foreground.clone();
            icon.size = COLLAPSE_ICON_SIZE;
            icon
        })
}

fn collapse_icon_index_value(expanded: &Value<bool>) -> Value<usize> {
    match expanded {
        Value::Static(expanded) => Value::Static(usize::from(*expanded)),
        Value::Signal(signal) => Value::Signal(signal.map_memo(usize::from)),
    }
}

fn inverted_bool_value(value: &Value<bool>) -> Value<bool> {
    match value {
        Value::Static(value) => Value::Static(!*value),
        Value::Signal(signal) => Value::Signal(signal.map_memo(|value| !value)),
    }
}

fn enabled_opacity_value(enabled: &Value<bool>) -> Value<f32> {
    match enabled {
        Value::Static(enabled) => Value::Static(if *enabled { 1.0 } else { 0.5 }),
        Value::Signal(signal) => {
            Value::Signal(signal.map_memo(|enabled| if enabled { 1.0 } else { 0.5 }))
        }
    }
}

fn accordion_item_expanded_value(expanded_key: &Value<Option<String>>, key: &str) -> Value<bool> {
    match expanded_key {
        Value::Static(current) => Value::Static(current.as_deref() == Some(key)),
        Value::Signal(signal) => {
            let key = key.to_string();
            Value::Signal(signal.map(move |current| current.as_ref() == Some(&key)))
        }
    }
}

fn collapse_expanded_max_height(expanded: Value<bool>) -> Value<Length> {
    match expanded {
        Value::Static(expanded) => Value::Static(collapse_panel_max_height(expanded)),
        Value::Signal(signal) => Value::Signal(
            signal
                .map_memo(collapse_panel_max_height)
                .without_transition(),
        ),
    }
}

fn collapse_progress_padding(progress: Value<f32>, padding: Insets) -> Value<Insets> {
    let map = move |value: f32| {
        let clamped = value.clamp(0.0, 1.0);
        Insets {
            left: padding.left,
            top: Dp::new(padding.top.get() * clamped),
            right: padding.right,
            bottom: Dp::new(padding.bottom.get() * clamped),
        }
    };
    match progress {
        Value::Static(value) => Value::Static(map(value)),
        Value::Signal(signal) => Value::Signal(signal.map(map)),
    }
}

fn collapse_panel_max_height(expanded: bool) -> Length {
    if expanded {
        Length::Auto
    } else {
        Length::Px(Dp::ZERO)
    }
}
