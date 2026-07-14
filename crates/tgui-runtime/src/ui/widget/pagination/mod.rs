use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Align, LayoutStyle, Length, Value};
use crate::ui::theme::{Density, StateValue};

use super::common::VisualStyle;
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{ButtonStyle, PaginationStyle, StyleResolver, StyleSheet};
use super::{Button, Flex, Stack, WidgetKey};

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
                .ghost()
                .disable(page <= 1)
                .on_click(change(
                    page.saturating_sub(1),
                    page_size,
                    pagination.on_change.clone(),
                )),
            pagination.style.clone(),
            pagination.visual.clone(),
            PaginationButtonRole::Navigation,
        )];
        for item in pagination_window(page, page_count) {
            match item {
                PageItem::Page(value) => {
                    let selected = value == page;
                    let button = Button::new(value.to_string()).ghost().on_click(change(
                        value,
                        page_size,
                        pagination.on_change.clone(),
                    ));
                    children.push(pagination_button(
                        button,
                        pagination.style.clone(),
                        pagination.visual.clone(),
                        PaginationButtonRole::Page { selected },
                    ));
                }
                PageItem::Ellipsis { target } => children.push(pagination_button(
                    Button::new("...").ghost().on_click(change(
                        target,
                        page_size,
                        pagination.on_change.clone(),
                    )),
                    pagination.style.clone(),
                    pagination.visual.clone(),
                    PaginationButtonRole::Page { selected: false },
                )),
            }
        }
        children.push(pagination_button(
            Button::new("Next")
                .ghost()
                .disable(page >= page_count)
                .on_click(change(page + 1, page_size, pagination.on_change.clone())),
            pagination.style.clone(),
            pagination.visual.clone(),
            PaginationButtonRole::Navigation,
        ));
        for option in pagination.page_size_options {
            let selected = option == page_size;
            children.push(pagination_button(
                Button::new(format!("{option}/page"))
                    .ghost()
                    .disable(selected)
                    .on_click(change(page, option, pagination.on_change.clone())),
                pagination.style.clone(),
                pagination.visual.clone(),
                PaginationButtonRole::PageSize { selected },
            ));
        }
        let root_style = pagination.style.clone();
        let mut root: Element<VM> = Flex::horizontal()
            .align(Align::Center)
            .runtime_layout(move |_layout, container, context, style_sheet, visual| {
                let resolved = resolve_pagination_style_with_sheet(
                    root_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                container.gap = Value::Static(Length::Px(resolved.gap));
            })
            .child(children)
            .into();
        root.key = pagination.key;
        root = with_visual_identity(root, &pagination.visual);
        root.layout = merge_layout(root.layout, pagination.layout);
        root
    }
}

#[derive(Clone, Copy)]
enum PaginationButtonRole {
    Navigation,
    Page { selected: bool },
    PageSize { selected: bool },
}

impl PaginationButtonRole {
    fn selected(self) -> bool {
        match self {
            Self::Navigation => false,
            Self::Page { selected } | Self::PageSize { selected } => selected,
        }
    }

    fn fixed_page_width(self) -> bool {
        matches!(self, Self::Page { .. })
    }
}

fn pagination_button<VM: 'static>(
    button: Button<VM>,
    style: Option<StyleResolver<PaginationStyle>>,
    visual_identity: VisualStyle,
    role: PaginationButtonRole,
) -> Element<VM> {
    let button = if role.fixed_page_width() {
        button.width(Length::Percent(1.0))
    } else {
        button
    };
    let variant = button.variant();
    let button_style = style.clone();
    let button = with_visual_identity(
        button
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_pagination_style_with_sheet(
                    button_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                );
                let mut button = ButtonStyle::default_for_theme(context.theme, variant);
                button.text_style = resolved.text_style;
                button.padding_x = match context.theme.density {
                    Density::Compact => context.theme.spacing.xs,
                    Density::Comfortable => context.theme.spacing.sm - context.theme.spacing.xxs,
                    Density::Spacious => context.theme.spacing.sm,
                };
                // Button measurement already honors min_height; pagination keeps
                // the compact 32/40/48dp rhythm without adding a second vertical inset.
                button.padding_y = crate::ui::unit::Dp::ZERO;
                let colors = &context.theme.colors;
                let selected = role.selected();
                let normal_background = if selected {
                    colors.primary_container
                } else {
                    Color::TRANSPARENT
                };
                let normal_foreground = if selected {
                    colors.on_primary_container
                } else {
                    colors.on_surface
                };
                button.background = StateValue::interactive(
                    Value::Static(normal_background),
                    Value::Static(colors.primary_container.with_alpha_factor(if selected {
                        1.0
                    } else {
                        0.46
                    })),
                    Value::Static(colors.primary_container.with_alpha_factor(if selected {
                        0.82
                    } else {
                        0.68
                    })),
                    Value::Static(if selected {
                        colors.primary_container
                    } else {
                        Color::TRANSPARENT
                    }),
                );
                button.foreground = StateValue::interactive(
                    Value::Static(normal_foreground),
                    Value::Static(if selected {
                        colors.on_primary_container
                    } else {
                        colors.primary
                    }),
                    Value::Static(colors.on_primary_container),
                    Value::Static(if selected {
                        colors.on_primary_container
                    } else {
                        colors.on_disabled
                    }),
                );
                button.border = StateValue::new(Value::Static(Color::TRANSPARENT));
                button.border_width = Value::Static(crate::ui::unit::Dp::ZERO);
                button
            })
            .into(),
        &visual_identity,
    );

    if role.fixed_page_width() {
        let slot_style = style;
        Stack::new()
            .runtime_layout(move |layout, _container, context, style_sheet, visual| {
                let resolved = resolve_pagination_style_with_sheet(
                    slot_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                layout.width = Some(Value::Static(Length::Px(resolved.page_width)));
            })
            .child(button)
            .into()
    } else {
        button
    }
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
