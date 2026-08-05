use crate::foundation::binding::{InvalidationSignal, State};
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Align, LayoutStyle, Length, Value};
use crate::ui::theme::{Density, StateValue};

use super::common::{AccessibilityCurrent, VisualStyle};
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{ButtonStyle, PaginationStyle, StyleResolver, StyleSheet};
use super::{Button, Flex, For, WidgetKey};

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
            page_size_options: Vec::new(),
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
        self.page_size_options = normalize_page_size_options(options);
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
        let (page, internal_page) = interactive_pagination_value(pagination.page.clone());
        let (page_size, internal_page_size) =
            interactive_pagination_value(pagination.page_size.clone());
        let page_mutable = internal_page.is_some() || pagination.on_change.is_some();
        let page_size_mutable = internal_page_size.is_some() || pagination.on_change.is_some();
        let item_page = page;
        let item_page_count = pagination.page_count.clone();
        let item_page_size = page_size;
        let page_size_options = pagination.page_size_options.clone();
        let item_style = pagination.style.clone();
        let item_visual = pagination.visual.clone();
        let item_command = pagination_change_command(
            internal_page,
            internal_page_size,
            pagination.on_change.clone(),
        );
        let children = For::new_with_resolver(
            move || {
                pagination_entries(
                    item_page.resolve(),
                    item_page_count.resolve(),
                    item_page_size.resolve(),
                    &page_size_options,
                )
            },
            |item| item.key.clone(),
            move |_index, item| {
                let can_activate = item.role.can_activate(page_mutable, page_size_mutable);
                let mut button = Button::new(item.label.clone())
                    .ghost()
                    .disable(item.disabled || !can_activate);
                if can_activate {
                    let command = item_command
                        .clone()
                        .expect("mutable pagination entry must have a change command");
                    let page = item.page;
                    let page_size = item.page_size;
                    button = button.on_click(Command::new_with_context(move |vm, context| {
                        command.execute_with_context(
                            vm,
                            PaginationChange { page, page_size },
                            context,
                        );
                    }));
                }
                pagination_button(button, item_style.clone(), item_visual.clone(), item.role)
            },
        );
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

#[derive(Clone)]
struct PaginationEntry {
    key: String,
    label: String,
    page: usize,
    page_size: usize,
    disabled: bool,
    role: PaginationButtonRole,
}

#[derive(Clone, Copy)]
enum PaginationButtonRole {
    Navigation,
    Page { selected: bool },
    PageSize { selected: bool },
}

fn pagination_entries(
    page: usize,
    page_count: usize,
    page_size: usize,
    page_size_options: &[usize],
) -> Vec<PaginationEntry> {
    let page_count = page_count.max(1);
    let page = page.clamp(1, page_count);
    let page_size = page_size.max(1);
    let mut entries = Vec::new();
    entries.push(PaginationEntry {
        key: "__tgui_pagination_prev".to_string(),
        label: "Prev".to_string(),
        page: page.saturating_sub(1).max(1),
        page_size,
        disabled: page <= 1,
        role: PaginationButtonRole::Navigation,
    });
    for item in pagination_window(page, page_count) {
        match item {
            PageItem::Page(value) => entries.push(PaginationEntry {
                key: format!("__tgui_pagination_page_{value}"),
                label: value.to_string(),
                page: value,
                page_size,
                disabled: false,
                role: PaginationButtonRole::Page {
                    selected: value == page,
                },
            }),
            PageItem::Ellipsis { target } => entries.push(PaginationEntry {
                key: format!(
                    "__tgui_pagination_ellipsis_{}_{}",
                    if target < page { "before" } else { "after" },
                    target
                ),
                label: "...".to_string(),
                page: target.clamp(1, page_count),
                page_size,
                disabled: false,
                role: PaginationButtonRole::Page { selected: false },
            }),
        }
    }
    entries.push(PaginationEntry {
        key: "__tgui_pagination_next".to_string(),
        label: "Next".to_string(),
        page: page.saturating_add(1).min(page_count),
        page_size,
        disabled: page >= page_count,
        role: PaginationButtonRole::Navigation,
    });
    for option in page_size_options
        .iter()
        .copied()
        .filter(|option| *option > 0)
    {
        let selected = option == page_size;
        entries.push(PaginationEntry {
            key: format!("__tgui_pagination_page_size_{option}"),
            label: format!("{option}/page"),
            page,
            page_size: option,
            disabled: selected,
            role: PaginationButtonRole::PageSize { selected },
        });
    }
    entries
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

    fn can_activate(self, page_mutable: bool, page_size_mutable: bool) -> bool {
        match self {
            Self::Navigation | Self::Page { .. } => page_mutable,
            Self::PageSize { .. } => page_size_mutable,
        }
    }
}

fn interactive_pagination_value<T>(value: Value<T>) -> (Value<T>, Option<State<T>>)
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

fn pagination_change_command<VM: 'static>(
    internal_page: Option<State<usize>>,
    internal_page_size: Option<State<usize>>,
    callback: Option<ValueCommand<VM, PaginationChange>>,
) -> Option<ValueCommand<VM, PaginationChange>> {
    if internal_page.is_none() && internal_page_size.is_none() && callback.is_none() {
        return None;
    }
    Some(ValueCommand::new_with_context(
        move |vm, change: PaginationChange, context| {
            if let Some(page) = internal_page.as_ref() {
                page.set(change.page);
            }
            if let Some(page_size) = internal_page_size.as_ref() {
                page_size.set(change.page_size);
            }
            if let Some(callback) = callback.as_ref() {
                callback.execute_with_context(vm, change, context);
            }
        },
    ))
}

fn pagination_button<VM: 'static>(
    button: Button<VM>,
    style: Option<StyleResolver<PaginationStyle>>,
    visual_identity: VisualStyle,
    role: PaginationButtonRole,
) -> Element<VM> {
    let button = if role.fixed_page_width() {
        let layout_style = style.clone();
        button.runtime_layout(move |layout, context, style_sheet, visual| {
            let resolved = resolve_pagination_style_with_sheet(
                layout_style.as_ref(),
                context,
                style_sheet,
                visual,
                WidgetState::default(),
            );
            layout.width = Some(Value::Static(Length::Px(resolved.page_width)));
        })
    } else {
        button
    };
    let variant = button.variant();
    let button_style = style.clone();
    let mut element = with_visual_identity(
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
    if matches!(role, PaginationButtonRole::Page { selected: true }) {
        element.visual.accessibility_current =
            Some((Value::Static(true), AccessibilityCurrent::Page));
    }
    element
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
    let end = page.saturating_add(2).min(page_count - 1);
    for value in start..=end {
        items.push(PageItem::Page(value));
    }
    if page.saturating_add(3) < page_count {
        items.push(PageItem::Ellipsis {
            target: page.saturating_add(5).min(page_count),
        });
    }
    items.push(PageItem::Page(page_count));
    items
}

fn normalize_page_size_options(options: Vec<usize>) -> Vec<usize> {
    let mut normalized = Vec::with_capacity(options.len());
    for option in options {
        if option > 0 && !normalized.contains(&option) {
            normalized.push(option);
        }
    }
    normalized
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

    #[test]
    fn pagination_window_and_navigation_saturate_at_usize_max() {
        let items = pagination_window(usize::MAX, usize::MAX);
        let pages = items
            .into_iter()
            .filter_map(|item| match item {
                PageItem::Page(page) => Some(page),
                PageItem::Ellipsis { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(pages.last(), Some(&usize::MAX));

        let entries = pagination_entries(usize::MAX, usize::MAX, 25, &[]);
        let next = entries
            .iter()
            .find(|entry| entry.key == "__tgui_pagination_next")
            .expect("next navigation entry");
        assert_eq!(next.page, usize::MAX);
        assert!(next.disabled);
    }

    #[test]
    fn page_size_options_drop_zero_and_duplicate_entries_without_reordering() {
        assert_eq!(
            normalize_page_size_options(vec![0, 25, 10, 25, 50, 10]),
            vec![25, 10, 50]
        );
    }
}
