use crate::foundation::view_model::Command;
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Align, LayoutStyle, Value, Wrap};
use crate::ui::theme::Theme;

use super::common::VisualStyle;
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{BreadcrumbStyle, StyleResolver, StyleSheet, TextWidgetStyle};
use super::{Button, CursorStyle, Flex, Menu, MenuItem, Text, WidgetKey};

pub struct BreadcrumbItem<VM> {
    label: Value<String>,
    on_click: Option<Command<VM>>,
}

impl<VM> Clone for BreadcrumbItem<VM> {
    fn clone(&self) -> Self {
        Self {
            label: self.label.clone(),
            on_click: self.on_click.clone(),
        }
    }
}

impl<VM> BreadcrumbItem<VM> {
    pub fn new(label: impl Into<Value<String>>) -> Self {
        Self {
            label: label.into(),
            on_click: None,
        }
    }

    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.on_click = Some(command);
        self
    }
}

pub struct Breadcrumb<VM> {
    items: Vec<BreadcrumbItem<VM>>,
    max_visible: usize,
    separator: Value<String>,
    style: Option<StyleResolver<BreadcrumbStyle>>,
    layout: LayoutStyle,
    visual: VisualStyle,
    key: Option<WidgetKey>,
}

impl<VM> Breadcrumb<VM> {
    pub fn new(items: Vec<BreadcrumbItem<VM>>) -> Self {
        Self {
            items,
            max_visible: 5,
            separator: Value::Static("/".to_string()),
            style: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            key: None,
        }
    }

    pub fn max_visible(mut self, max_visible: usize) -> Self {
        self.max_visible = max_visible.max(2);
        self
    }

    pub fn separator(mut self, separator: impl Into<Value<String>>) -> Self {
        self.separator = separator.into();
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut BreadcrumbStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| BreadcrumbStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> BreadcrumbStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<Breadcrumb<VM>> for Element<VM> {
    fn from(breadcrumb: Breadcrumb<VM>) -> Self {
        let layout_style = resolve_breadcrumb_style_for_layout(breadcrumb.style.as_ref());
        let total = breadcrumb.items.len();
        let visible = if total > breadcrumb.max_visible {
            let tail = breadcrumb.max_visible.saturating_sub(2).max(1);
            let hidden_end = total.saturating_sub(tail);
            let mut visible = Vec::new();
            if let Some(first) = breadcrumb.items.first().cloned() {
                visible.push(BreadcrumbRenderItem::Item(first));
            }
            let hidden = breadcrumb
                .items
                .iter()
                .skip(1)
                .take(hidden_end.saturating_sub(1))
                .cloned()
                .collect::<Vec<_>>();
            if !hidden.is_empty() {
                visible.push(BreadcrumbRenderItem::Overflow(hidden));
            }
            visible.extend(
                breadcrumb
                    .items
                    .into_iter()
                    .skip(hidden_end)
                    .map(BreadcrumbRenderItem::Item),
            );
            visible
        } else {
            breadcrumb
                .items
                .into_iter()
                .map(BreadcrumbRenderItem::Item)
                .collect()
        };

        let visible_len = visible.len();
        let mut children = Vec::new();
        for (index, item) in visible.into_iter().enumerate() {
            if index > 0 {
                children.push(with_visual_identity(
                    Text::new(breadcrumb.separator.clone())
                        .style_full_with_style_sheet(separator_text_style(breadcrumb.style.clone()))
                        .into(),
                    &breadcrumb.visual,
                ));
            }
            let current = index + 1 == visible_len;
            match item {
                BreadcrumbRenderItem::Item(item) => {
                    let text = Text::new(item.label.clone()).style_full_with_style_sheet(
                        breadcrumb_text_style(breadcrumb.style.clone(), current),
                    );
                    let text_element = if let Some(command) = item.on_click {
                        text.cursor(CursorStyle::Pointer)
                            .on_click(command)
                            .focusable(true)
                            .tab_index(0)
                    } else {
                        text.into()
                    };
                    children.push(with_visual_identity(text_element, &breadcrumb.visual));
                }
                BreadcrumbRenderItem::Overflow(items) => {
                    let menu_items = items
                        .into_iter()
                        .map(|item| {
                            let mut menu_item = MenuItem::new(item.label.clone());
                            if let Some(command) = item.on_click {
                                menu_item = menu_item.on_select(command);
                            } else {
                                menu_item = menu_item.disable(true);
                            }
                            menu_item
                        })
                        .collect::<Vec<_>>();
                    children.push(
                        Menu::new(Button::new("...").ghost())
                            .items(menu_items)
                            .into(),
                    );
                }
            }
        }
        let mut root: Element<VM> = Flex::horizontal()
            .wrap(Wrap::Wrap)
            .align(Align::Center)
            .gap(layout_style.gap)
            .child(children)
            .into();
        root.key = breadcrumb.key;
        root = with_visual_identity(root, &breadcrumb.visual);
        root.layout = merge_layout(root.layout, breadcrumb.layout);
        root
    }
}

enum BreadcrumbRenderItem<VM> {
    Item(BreadcrumbItem<VM>),
    Overflow(Vec<BreadcrumbItem<VM>>),
}

fn breadcrumb_text_style(
    style: Option<StyleResolver<BreadcrumbStyle>>,
    current: bool,
) -> impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> TextWidgetStyle
       + Send
       + Sync
       + 'static {
    move |context, style_sheet, visual, state| {
        let resolved = resolve_breadcrumb_style_with_sheet(
            style.as_ref(),
            context,
            style_sheet,
            visual,
            state,
        );
        TextWidgetStyle {
            surface: Default::default(),
            color: if current {
                resolved.current_foreground
            } else {
                resolved.foreground
            },
            typography: resolved.text_style,
        }
    }
}

fn separator_text_style(
    style: Option<StyleResolver<BreadcrumbStyle>>,
) -> impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> TextWidgetStyle
       + Send
       + Sync
       + 'static {
    move |context, style_sheet, visual, state| {
        let resolved = resolve_breadcrumb_style_with_sheet(
            style.as_ref(),
            context,
            style_sheet,
            visual,
            state,
        );
        TextWidgetStyle {
            surface: Default::default(),
            color: resolved.separator,
            typography: resolved.text_style,
        }
    }
}

fn resolve_breadcrumb_style(
    style: Option<&StyleResolver<BreadcrumbStyle>>,
    context: &StyleContext<'_>,
) -> BreadcrumbStyle {
    let style_sheet = StyleSheet::default();
    resolve_breadcrumb_style_with_sheet(
        style,
        context,
        &style_sheet,
        &VisualStyle::default(),
        WidgetState::default(),
    )
}

fn resolve_breadcrumb_style_with_sheet(
    style: Option<&StyleResolver<BreadcrumbStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> BreadcrumbStyle {
    resolve_component_style_with_sheet(
        style,
        context,
        style_sheet,
        visual,
        state,
        BreadcrumbStyle::default_for_theme(context.theme),
        |base, context| context.theme.components.breadcrumb.apply(base, context),
        |sheet, base, context, visual| sheet.apply_breadcrumb(base, context, visual),
        |sheet, base, context, visual, state| {
            sheet.apply_breadcrumb_state(base, context, visual, state)
        },
    )
}

fn resolve_breadcrumb_style_for_layout(
    style: Option<&StyleResolver<BreadcrumbStyle>>,
) -> BreadcrumbStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_breadcrumb_style(style, &context)
}
