use crate::foundation::view_model::Command;
use crate::theme::StyleContext;
use crate::ui::layout::{LayoutStyle, Value};
use crate::ui::theme::Theme;

use super::core::Element;
use super::p3_support::{impl_p3_layout_api, merge_layout};
use super::style::{CardStyle, ContainerStyle, StyleResolver};
use super::{CursorStyle, Flex, WidgetKey};

pub struct Card<VM> {
    header: Option<Element<VM>>,
    body: Option<Element<VM>>,
    footer: Option<Element<VM>>,
    children: Vec<Element<VM>>,
    on_click: Option<Command<VM>>,
    style: Option<StyleResolver<CardStyle>>,
    layout: LayoutStyle,
    key: Option<WidgetKey>,
}

impl<VM> Card<VM> {
    pub fn new() -> Self {
        Self {
            header: None,
            body: None,
            footer: None,
            children: Vec::new(),
            on_click: None,
            style: None,
            layout: LayoutStyle::default(),
            key: None,
        }
    }

    pub fn header(mut self, header: impl Into<Element<VM>>) -> Self {
        self.header = Some(header.into());
        self
    }

    pub fn body(mut self, body: impl Into<Element<VM>>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn footer(mut self, footer: impl Into<Element<VM>>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    pub fn child(mut self, child: impl Into<Element<VM>>) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.on_click = Some(command);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut CardStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| CardStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> CardStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM> Default for Card<VM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<VM: 'static> From<Card<VM>> for Element<VM> {
    fn from(card: Card<VM>) -> Self {
        let layout_style = resolve_card_style_for_layout(card.style.as_ref());
        let mut children = Vec::new();
        if let Some(header) = card.header {
            children.push(header);
        }
        if let Some(body) = card.body {
            children.push(body);
        }
        children.extend(card.children);
        if let Some(footer) = card.footer {
            children.push(footer);
        }
        let style = card.style.clone();
        let mut root: Element<VM> = Flex::vertical()
            .gap(layout_style.gap)
            .padding(layout_style.padding)
            .style_full(move |context| {
                let resolved = resolve_card_style(style.as_ref(), context);
                let mut container = ContainerStyle::default_for_theme(context.theme);
                container.surface = resolved.surface.clone();
                container.surface.background = Some(resolved.background);
                container.surface.border_color = Some(resolved.border);
                container.surface.border_width = Some(Value::Static(resolved.border_width));
                container.surface.border_radius = Some(Value::Static(resolved.radius));
                container.surface.shadow = Some(Value::Static(resolved.shadow));
                container
            })
            .child(children)
            .into();
        if let Some(command) = card.on_click {
            root = root.on_click(command).focusable(true).tab_index(0);
            root.interactions.cursor_style = Some(Value::Static(CursorStyle::Pointer));
        }
        root.key = card.key;
        root.layout = merge_layout(root.layout, card.layout);
        root
    }
}

fn resolve_card_style(
    style: Option<&StyleResolver<CardStyle>>,
    context: &StyleContext<'_>,
) -> CardStyle {
    let mut base = CardStyle::default_for_theme(context.theme);
    context.theme.components.card.apply(&mut base, context);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

fn resolve_card_style_for_layout(style: Option<&StyleResolver<CardStyle>>) -> CardStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_card_style(style, &context)
}
