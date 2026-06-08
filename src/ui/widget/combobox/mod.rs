use std::sync::Arc;

use crate::foundation::binding::{TextChangeSet, TextController};
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::StyleContext;
use crate::ui::layout::{Insets, LayoutStyle, Value};
use crate::ui::theme::Theme;
use crate::ui::unit::{dp, Dp};

use super::core::Element;
use super::p3_support::{impl_p3_layout_api, merge_layout};
use super::style::{ComboboxStyle, ContainerStyle, StyleResolver, TextWidgetStyle};
use super::{Button, Input, ItemLayout, Popover, Stack, Text, VirtualList, WidgetKey};

#[derive(Clone, Debug, PartialEq)]
pub struct ComboboxOption {
    pub key: String,
    pub label: String,
    pub disabled: bool,
}

impl ComboboxOption {
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComboboxChange {
    pub text: String,
    pub selected_key: Option<String>,
}

type ComboboxFilter = Arc<dyn Fn(&ComboboxOption, &str) -> bool + Send + Sync>;

pub struct Combobox<VM> {
    controller: TextController,
    options: Value<Vec<ComboboxOption>>,
    open: Value<bool>,
    selected_key: Value<Option<String>>,
    placeholder: Value<String>,
    allow_custom: bool,
    filter: Option<ComboboxFilter>,
    on_change: Option<ValueCommand<VM, ComboboxChange>>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    style: Option<StyleResolver<ComboboxStyle>>,
    layout: LayoutStyle,
    key: Option<WidgetKey>,
}

pub type AutoComplete<VM> = Combobox<VM>;

impl<VM> Combobox<VM> {
    pub fn new(
        controller: impl Into<TextController>,
        options: impl Into<Value<Vec<ComboboxOption>>>,
    ) -> Self {
        Self {
            controller: controller.into(),
            options: options.into(),
            open: Value::Static(false),
            selected_key: Value::Static(None),
            placeholder: Value::Static("Search".to_string()),
            allow_custom: true,
            filter: None,
            on_change: None,
            on_open_change: None,
            style: None,
            layout: LayoutStyle::default(),
            key: None,
        }
    }

    pub fn selected_key(mut self, selected_key: impl Into<Value<Option<String>>>) -> Self {
        self.selected_key = selected_key.into();
        self
    }

    pub fn open(mut self, open: impl Into<Value<bool>>) -> Self {
        self.open = open.into();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<Value<String>>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn allow_custom(mut self, allow_custom: bool) -> Self {
        self.allow_custom = allow_custom;
        self
    }

    pub fn filter(
        mut self,
        filter: impl Fn(&ComboboxOption, &str) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.filter = Some(Arc::new(filter));
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, ComboboxChange>) -> Self {
        self.on_change = Some(command);
        self
    }

    pub fn on_open_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_open_change = Some(command);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut ComboboxStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| ComboboxStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> ComboboxStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<Combobox<VM>> for Element<VM> {
    fn from(combo: Combobox<VM>) -> Self {
        let layout_style = resolve_combobox_style_for_layout(combo.style.as_ref());
        let query = combo.controller.text();
        let options = filtered_options(combo.options.resolve(), &query, combo.filter.as_ref());
        let on_text_change = combo.on_change.clone().map({
            let controller = combo.controller.clone();
            move |command| {
                ValueCommand::new_with_context(move |vm, _changes: TextChangeSet, context| {
                    command.execute_with_context(
                        vm,
                        ComboboxChange {
                            text: controller.text(),
                            selected_key: None,
                        },
                        context,
                    );
                })
            }
        });
        let mut input = Input::new(combo.controller.clone())
            .placeholder(combo.placeholder.clone())
            .width(layout_style.width);
        if let Some(command) = on_text_change {
            input = input.on_change_set(command);
        }
        if let Some(command) = combo.on_open_change.clone() {
            input = input.on_focus(Command::new_with_context(move |vm, context| {
                command.execute_with_context(vm, true, context);
            }));
        }
        let trigger: Element<VM> = input.into();
        let content: Element<VM> = if options.is_empty() {
            let style = combo.style.clone();
            Stack::new()
                .width(layout_style.menu_width)
                .padding(Insets::all(dp(10.0)))
                .child(Text::new("No results").style_full(move |context| {
                    let resolved = resolve_combobox_style(style.as_ref(), context);
                    TextWidgetStyle {
                        surface: Default::default(),
                        color: resolved.empty_foreground,
                        typography: context.theme.typography.body_small.clone(),
                    }
                }))
                .into()
        } else {
            let height = dp(layout_style.option_height.get()
                * layout_style.max_visible_options.min(options.len()).max(1) as f32);
            let controller = combo.controller.clone();
            let on_change = combo.on_change.clone();
            let on_open_change = combo.on_open_change.clone();
            let option_height = layout_style.option_height;
            let menu_width = layout_style.menu_width;
            VirtualList::new(options, move |_index, option: &ComboboxOption| {
                let controller = controller.clone();
                let on_change = on_change.clone();
                let on_open_change = on_open_change.clone();
                let key = option.key.clone();
                let label = option.label.clone();
                Button::new(label.clone())
                    .ghost()
                    .disable(option.disabled)
                    .width(menu_width)
                    .height(option_height)
                    .on_click(Command::new_with_context(move |vm, context| {
                        controller.set_text(label.clone());
                        if let Some(command) = on_change.as_ref() {
                            command.execute_with_context(
                                vm,
                                ComboboxChange {
                                    text: label.clone(),
                                    selected_key: Some(key.clone()),
                                },
                                context,
                            );
                        }
                        if let Some(command) = on_open_change.as_ref() {
                            command.execute_with_context(vm, false, context);
                        }
                    }))
                    .into()
            })
            .item_layout(ItemLayout::Fixed {
                item_extent: layout_style.option_height,
                spacing: Dp::ZERO,
                overscan: 3,
            })
            .width(layout_style.menu_width)
            .height(height)
            .style_full({
                let style = combo.style.clone();
                move |context| {
                    let resolved = resolve_combobox_style(style.as_ref(), context);
                    let mut container = ContainerStyle::default_for_theme(context.theme);
                    container.surface.background =
                        Some(Value::Static(context.theme.colors.surface_overlay));
                    container.surface.border_radius = Some(Value::Static(context.theme.radius.md));
                    container.surface.border_color =
                        Some(Value::Static(context.theme.colors.outline_muted));
                    container.surface.border_width = Some(Value::Static(context.theme.border.thin));
                    let _ = resolved.highlight;
                    container
                }
            })
            .into()
        };
        let popover = Popover::new(trigger)
            .content(content)
            .open(combo.open)
            .match_anchor_width(true);
        let mut root: Element<VM> = if let Some(command) = combo.on_open_change {
            popover.on_open_change(command).into()
        } else {
            popover.into()
        };
        root.key = combo.key;
        root.layout = merge_layout(root.layout, combo.layout);
        root
    }
}

fn filtered_options(
    options: Vec<ComboboxOption>,
    query: &str,
    filter: Option<&ComboboxFilter>,
) -> Vec<ComboboxOption> {
    let query_lower = query.to_lowercase();
    options
        .into_iter()
        .filter(|option| {
            if let Some(filter) = filter {
                filter(option, query)
            } else {
                query_lower.is_empty() || option.label.to_lowercase().contains(&query_lower)
            }
        })
        .collect()
}

fn resolve_combobox_style(
    style: Option<&StyleResolver<ComboboxStyle>>,
    context: &StyleContext<'_>,
) -> ComboboxStyle {
    let mut base = ComboboxStyle::default_for_theme(context.theme);
    context.theme.components.combobox.apply(&mut base, context);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

fn resolve_combobox_style_for_layout(
    style: Option<&StyleResolver<ComboboxStyle>>,
) -> ComboboxStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_combobox_style(style, &context)
}
