use crate::foundation::view_model::Command;
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{LayoutStyle, Length, Value};

use super::common::VisualStyle;
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{CardStyle, ContainerStyle, StyleResolver, StyleSheet};
use super::{CursorStyle, Flex, WidgetKey};

pub struct Card<VM> {
    header: Option<Element<VM>>,
    body: Option<Element<VM>>,
    footer: Option<Element<VM>>,
    children: Vec<Element<VM>>,
    on_click: Option<Command<VM>>,
    style: Option<StyleResolver<CardStyle>>,
    layout: LayoutStyle,
    visual: VisualStyle,
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
            visual: VisualStyle::default(),
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
        let layout_style = card.style.clone();
        let mut root: Element<VM> = Flex::vertical()
            .runtime_layout(move |_layout, container, context, style_sheet, visual| {
                let resolved = resolve_card_style_with_sheet(
                    layout_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                container.gap = Value::Static(Length::Px(resolved.gap));
                container.padding = Some(Value::Static(resolved.padding));
            })
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_card_style_with_sheet(
                    style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                );
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
        root = with_visual_identity(root, &card.visual);
        root.layout = merge_layout(root.layout, card.layout);
        root
    }
}

fn resolve_card_style_with_sheet(
    style: Option<&StyleResolver<CardStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> CardStyle {
    resolve_component_style_with_sheet(
        style,
        context,
        style_sheet,
        visual,
        state,
        CardStyle::default_for_theme(context.theme),
        |base, context| context.theme.components.card.apply(base, context),
        |sheet, base, context, visual| sheet.apply_card(base, context, visual),
        |sheet, base, context, visual, state| sheet.apply_card_state(base, context, visual, state),
    )
}
