use std::sync::{Arc, Mutex};

use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Insets, LayoutStyle, Length, Overflow, Value};
use crate::ui::unit::{dp, Dp};

use super::common::VisualStyle;
use super::core::Element;
use super::icon::SvgIconId;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{
    CollapseStyle, ContainerStyle, IconStyle, StyleResolver, StyleSheet, TextWidgetStyle,
};
use super::{CursorStyle, Flex, Icon, Stack, Text, WidgetKey};

const COLLAPSE_ICON_SIZE: Dp = dp(20.0);
const COLLAPSE_PANEL_MAX_HEIGHT: Dp = dp(320.0);

pub struct Collapse<VM> {
    title: Value<String>,
    content: Element<VM>,
    expanded: Value<bool>,
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
        let expanded = collapse.expanded.resolve();
        let expanded_for_click = collapse.expanded.clone();
        let progress = collapse_progress_value(collapse.expanded.clone());
        let panel_max_height = collapse_progress_max_height(progress.clone());
        let on_change = collapse.on_change.clone();
        let header_style = collapse.style.clone();
        let header_identity = collapse.visual.clone();
        let icon_source = if expanded {
            SvgIconId::ChevronUp
        } else {
            SvgIconId::ChevronDown
        };
        let header_icon = Icon::internal(icon_source)
            .size(COLLAPSE_ICON_SIZE)
            .style_full_with_style_sheet({
                let header_style = header_style.clone();
                move |context, style_sheet, visual, state| {
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
                }
            });
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
            .cursor(CursorStyle::Pointer)
            .focusable(true)
            .on_click(Command::new_with_context(move |vm, context| {
                if let Some(command) = on_change.as_ref() {
                    command.execute_with_context(
                        vm,
                        !expanded_for_click.resolve_untracked(),
                        context,
                    );
                }
            }))
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
        let mut children: Vec<Element<VM>> =
            vec![with_visual_identity(header.into(), &header_identity)];
        let style = collapse.style.clone();
        let panel_identity = collapse.visual.clone();
        let panel_padding_cache = Arc::new(Mutex::new(
            None::<(Insets, Option<crate::animation::Transition>, Value<Insets>)>,
        ));
        children.push(with_visual_identity(
            Stack::new()
                .overflow(Overflow::Hidden)
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
        let items = accordion
            .items
            .into_iter()
            .map(|item| {
                let key = item.key.clone();
                let expanded = accordion_item_expanded_value(&accordion.expanded_key, &item.key);
                let on_change = accordion.on_change.clone();
                Collapse {
                    title: item.title,
                    content: item.content,
                    expanded,
                    on_change: Some(ValueCommand::new_with_context(move |vm, next, context| {
                        if let Some(command) = on_change.as_ref() {
                            command.execute_with_context(
                                vm,
                                if next { Some(key.clone()) } else { None },
                                context,
                            );
                        }
                    })),
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

fn accordion_item_expanded_value(expanded_key: &Value<Option<String>>, key: &str) -> Value<bool> {
    match expanded_key {
        Value::Static(current) => Value::Static(current.as_deref() == Some(key)),
        Value::Signal(signal) => {
            let key = key.to_string();
            Value::Signal(signal.map(move |current| current.as_ref() == Some(&key)))
        }
    }
}

fn collapse_progress_max_height(progress: Value<f32>) -> Value<Length> {
    match progress {
        Value::Static(value) => Value::Static(collapse_panel_max_height(value)),
        Value::Signal(signal) => Value::Signal(signal.map(collapse_panel_max_height)),
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

fn collapse_panel_max_height(progress: f32) -> Length {
    Length::Px(Dp::new(
        COLLAPSE_PANEL_MAX_HEIGHT.get() * progress.clamp(0.0, 1.0),
    ))
}
