use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Align, LayoutStyle, Value};
use crate::ui::theme::Theme;

use super::common::VisualStyle;
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{ButtonStyle, PaginationStyle, StyleResolver, StyleSheet};
use super::{Button, Flex, WidgetKey};

#[derive(Clone, Debug, PartialEq)]
pub struct PaginationChange {
    pub page: usize,
    pub page_size: usize,
}

pub struct Pagination<VM> {
    page: Value<usize>,
    page_count: Value<usize>,
    page_size: Value<usize>,
    page_size_options: Vec<usize>,
    on_change: Option<ValueCommand<VM, PaginationChange>>,
    style: Option<StyleResolver<PaginationStyle>>,
    layout: LayoutStyle,
    visual: VisualStyle,
    key: Option<WidgetKey>,
}

impl<VM> Pagination<VM> {
    pub fn new(page: impl Into<Value<usize>>, page_count: impl Into<Value<usize>>) -> Self {
        Self {
            page: page.into(),
            page_count: page_count.into(),
            page_size: Value::Static(25),
            page_size_options: vec![10, 25, 50, 100],
            on_change: None,
            style: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            key: None,
        }
    }

    pub fn page_size(mut self, page_size: impl Into<Value<usize>>) -> Self {
        self.page_size = page_size.into();
        self
    }

    pub fn page_size_options(mut self, options: Vec<usize>) -> Self {
        self.page_size_options = options.into_iter().filter(|value| *value > 0).collect();
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, PaginationChange>) -> Self {
        self.on_change = Some(command);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut PaginationStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| PaginationStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> PaginationStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<Pagination<VM>> for Element<VM> {
    fn from(pagination: Pagination<VM>) -> Self {
        let layout_style = resolve_pagination_style_for_layout(pagination.style.as_ref());
        let page_count = pagination.page_count.resolve().max(1);
        let page = pagination.page.resolve().clamp(1, page_count);
        let page_size = pagination.page_size.resolve().max(1);
        let change = |target: usize,
                      page_size: usize,
                      command: Option<ValueCommand<VM, PaginationChange>>| {
            Command::new_with_context(move |vm, context| {
                if let Some(command) = command.as_ref() {
                    command.execute_with_context(
                        vm,
                        PaginationChange {
                            page: target.clamp(1, page_count),
                            page_size,
                        },
                        context,
                    );
                }
            })
        };
        let mut children: Vec<Element<VM>> = vec![pagination_button(
            Button::new("Prev")
                .secondary()
                .disable(page <= 1)
                .on_click(change(
                    page.saturating_sub(1),
                    page_size,
                    pagination.on_change.clone(),
                )),
            pagination.style.clone(),
            pagination.visual.clone(),
        )];
        for item in pagination_window(page, page_count) {
            match item {
                PageItem::Page(value) => {
                    let button = if value == page {
                        Button::new(value.to_string()).primary()
                    } else {
                        Button::new(value.to_string()).secondary()
                    }
                    .width(layout_style.page_width)
                    .on_click(change(
                        value,
                        page_size,
                        pagination.on_change.clone(),
                    ));
                    children.push(pagination_button(
                        button,
                        pagination.style.clone(),
                        pagination.visual.clone(),
                    ));
                }
                PageItem::Ellipsis { target } => children.push(pagination_button(
                    Button::new("...")
                        .ghost()
                        .width(layout_style.page_width)
                        .on_click(change(target, page_size, pagination.on_change.clone())),
                    pagination.style.clone(),
                    pagination.visual.clone(),
                )),
            }
        }
        children.push(pagination_button(
            Button::new("Next")
                .secondary()
                .disable(page >= page_count)
                .on_click(change(page + 1, page_size, pagination.on_change.clone())),
            pagination.style.clone(),
            pagination.visual.clone(),
        ));
        for option in pagination.page_size_options {
            children.push(pagination_button(
                Button::new(format!("{option}/page"))
                    .ghost()
                    .disable(option == page_size)
                    .on_click(change(page, option, pagination.on_change.clone())),
                pagination.style.clone(),
                pagination.visual.clone(),
            ));
        }
        let mut root: Element<VM> = Flex::horizontal()
            .align(Align::Center)
            .gap(layout_style.gap)
            .child(children)
            .into();
        root.key = pagination.key;
        root = with_visual_identity(root, &pagination.visual);
        root.layout = merge_layout(root.layout, pagination.layout);
        root
    }
}

fn pagination_button<VM: 'static>(
    button: Button<VM>,
    style: Option<StyleResolver<PaginationStyle>>,
    visual_identity: VisualStyle,
) -> Element<VM> {
    let variant = button.variant();
    with_visual_identity(
        button
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_pagination_style_with_sheet(
                    style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                );
                let mut button = ButtonStyle::default_for_theme(context.theme, variant);
                button.text_style = resolved.text_style;
                button
            })
            .into(),
        &visual_identity,
    )
}

enum PageItem {
    Page(usize),
    Ellipsis { target: usize },
}

fn pagination_window(page: usize, page_count: usize) -> Vec<PageItem> {
    if page_count <= 7 {
        return (1..=page_count).map(PageItem::Page).collect();
    }
    let mut items = vec![PageItem::Page(1)];
    if page > 4 {
        items.push(PageItem::Ellipsis {
            target: page.saturating_sub(5).max(1),
        });
    }
    let start = page.saturating_sub(2).max(2);
    let end = (page + 2).min(page_count - 1);
    for value in start..=end {
        items.push(PageItem::Page(value));
    }
    if page + 3 < page_count {
        items.push(PageItem::Ellipsis {
            target: (page + 5).min(page_count),
        });
    }
    items.push(PageItem::Page(page_count));
    items
}

fn resolve_pagination_style(
    style: Option<&StyleResolver<PaginationStyle>>,
    context: &StyleContext<'_>,
) -> PaginationStyle {
    let style_sheet = StyleSheet::default();
    resolve_pagination_style_with_sheet(
        style,
        context,
        &style_sheet,
        &VisualStyle::default(),
        WidgetState::default(),
    )
}

fn resolve_pagination_style_with_sheet(
    style: Option<&StyleResolver<PaginationStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> PaginationStyle {
    resolve_component_style_with_sheet(
        style,
        context,
        style_sheet,
        visual,
        state,
        PaginationStyle::default_for_theme(context.theme),
        |base, context| context.theme.components.pagination.apply(base, context),
        |sheet, base, context, visual| sheet.apply_pagination(base, context, visual),
        |sheet, base, context, visual, state| {
            sheet.apply_pagination_state(base, context, visual, state)
        },
    )
}

fn resolve_pagination_style_for_layout(
    style: Option<&StyleResolver<PaginationStyle>>,
) -> PaginationStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_pagination_style(style, &context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_window_ellipsis_items_carry_jump_targets() {
        let items = pagination_window(10, 20);
        let targets = items
            .into_iter()
            .filter_map(|item| match item {
                PageItem::Ellipsis { target } => Some(target),
                PageItem::Page(_) => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(targets, vec![5, 15]);
    }
}
