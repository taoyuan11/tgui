use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{LayoutStyle, Value};
use crate::ui::theme::Theme;

use super::common::VisualStyle;
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{ButtonStyle, CollapseStyle, ContainerStyle, StyleResolver, StyleSheet};
use super::{Button, Flex, Stack, WidgetKey};

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
        let layout_style = resolve_collapse_style_for_layout(collapse.style.as_ref());
        let expanded = collapse.expanded.resolve();
        let on_change = collapse.on_change.clone();
        let header_style = collapse.style.clone();
        let header_identity = collapse.visual.clone();
        let header = Button::new(collapse.title.clone())
            .secondary()
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_collapse_style_with_sheet(
                    header_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                );
                let mut button = ButtonStyle::default_for_theme(
                    context.theme,
                    crate::ui::widget::common::ButtonVariantKind::Secondary,
                );
                button.background = resolved.header_background;
                button.foreground =
                    crate::ui::theme::StateValue::new(resolved.header_foreground.clone());
                button.text_style = resolved.text_style;
                button
            })
            .on_click(Command::new_with_context(move |vm, context| {
                if let Some(command) = on_change.as_ref() {
                    command.execute_with_context(vm, !expanded, context);
                }
            }));
        let mut children: Vec<Element<VM>> =
            vec![with_visual_identity(header.into(), &header_identity)];
        if expanded {
            let style = collapse.style.clone();
            let panel_identity = collapse.visual.clone();
            children.push(with_visual_identity(
                Stack::new()
                    .padding(layout_style.padding)
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
        }
        let style = collapse.style.clone();
        let mut root: Element<VM> = Flex::vertical()
            .gap(layout_style.gap)
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
        let current = accordion.expanded_key.resolve();
        let layout_style = resolve_collapse_style_for_layout(accordion.style.as_ref());
        let items = accordion
            .items
            .into_iter()
            .map(|item| {
                let key = item.key.clone();
                let expanded = current.as_ref() == Some(&item.key);
                let on_change = accordion.on_change.clone();
                Collapse {
                    title: item.title,
                    content: item.content,
                    expanded: Value::Static(expanded),
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
        let mut root: Element<VM> = Flex::vertical().gap(layout_style.gap).child(items).into();
        root.key = accordion.key;
        root = with_visual_identity(root, &accordion.visual);
        root.layout = merge_layout(root.layout, accordion.layout);
        root
    }
}

fn resolve_collapse_style(
    style: Option<&StyleResolver<CollapseStyle>>,
    context: &StyleContext<'_>,
) -> CollapseStyle {
    let style_sheet = StyleSheet::default();
    resolve_collapse_style_with_sheet(
        style,
        context,
        &style_sheet,
        &VisualStyle::default(),
        WidgetState::default(),
    )
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

fn resolve_collapse_style_for_layout(
    style: Option<&StyleResolver<CollapseStyle>>,
) -> CollapseStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_collapse_style(style, &context)
}
