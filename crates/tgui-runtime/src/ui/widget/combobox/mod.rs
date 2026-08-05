use std::sync::Arc;

use crate::foundation::binding::{TextChangeSet, TextController};
use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StateValue, StyleContext, WidgetState};
use crate::ui::layout::{pct, Insets, LayoutStyle, Length, Value};
use crate::ui::unit::{dp, Dp};

use super::common::{AccessibilityRole, VisualStyle};
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::popover::{PopoverOpenHandle, PopoverVirtualListNavigation};
use super::style::{
    ButtonStyle, ComboboxStyle, ContainerStyle, InputStyle, StyleResolver, StyleSheet,
    TextWidgetStyle,
};
use super::{Button, For, Input, ItemLayout, Popover, Stack, Text, VirtualList, WidgetKey};

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
    open: Option<Value<bool>>,
    selected_key: Value<Option<String>>,
    placeholder: Value<String>,
    label: Option<Value<String>>,
    disabled: Value<bool>,
    allow_custom: bool,
    filter: Option<ComboboxFilter>,
    on_change: Option<ValueCommand<VM, ComboboxChange>>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    style: Option<StyleResolver<ComboboxStyle>>,
    layout: LayoutStyle,
    visual: VisualStyle,
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
            open: None,
            selected_key: Value::Static(None),
            placeholder: Value::Static("Search".to_string()),
            label: None,
            disabled: Value::Static(false),
            allow_custom: true,
            filter: None,
            on_change: None,
            on_open_change: None,
            style: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            key: None,
        }
    }

    pub fn selected_key(mut self, selected_key: impl Into<Value<Option<String>>>) -> Self {
        self.selected_key = selected_key.into();
        self
    }

    pub fn open(mut self, open: impl Into<Value<bool>>) -> Self {
        self.open = Some(open.into());
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<Value<String>>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn label(mut self, label: impl Into<Value<String>>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn disable(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
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
        let menu_width_override = combo.layout.width.clone();
        let internal_open = combo.open.is_none().then(|| PopoverOpenHandle::new(false));
        let normalize_on_blur = (!combo.allow_custom).then(|| {
            let controller = combo.controller.clone();
            let options = combo.options.clone();
            let selected_key = combo.selected_key.clone();
            let internal_open = internal_open.clone();
            let open = combo.open.clone();
            Command::new_with_context(move |_vm, _context| {
                if internal_open
                    .as_ref()
                    .is_some_and(PopoverOpenHandle::is_open)
                    || open.as_ref().is_some_and(Value::resolve)
                {
                    return;
                }
                normalize_non_custom_text(&controller, &options, &selected_key);
            })
        });
        let on_text_change = combo
            .allow_custom
            .then(|| combo.on_change.clone())
            .flatten()
            .map({
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
        let input_style = combo.style.clone();
        let input_layout_style = combo.style.clone();
        let mut input = Input::new(combo.controller.clone())
            .placeholder(combo.placeholder.clone())
            .disable(combo.disabled.clone())
            .runtime_layout(move |layout, context, style_sheet, visual| {
                let resolved = resolve_combobox_style_with_sheet(
                    input_layout_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                if layout.width.is_none() {
                    layout.width = Some(Value::Static(Length::Px(resolved.width)));
                }
            })
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_combobox_style_with_sheet(
                    input_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                );
                let mut input = InputStyle::default_for_theme(context.theme);
                input.min_height = resolved.option_height;
                input
            });
        if let Some(label) = combo.label.clone() {
            input = input.label(label);
        }
        if let Some(command) = normalize_on_blur {
            input = input.on_blur(command);
        }
        if let Some(command) = on_text_change {
            input = input.on_change_set(command);
        }
        let mut trigger: Element<VM> = with_visual_identity(input.into(), &combo.visual);
        trigger.visual.accessibility_role = Some(AccessibilityRole::ComboBox);
        let source_options = combo.options.clone();
        let source_controller = combo.controller.clone();
        let source_filter = combo.filter.clone();
        let source_selected_key = combo.selected_key.clone();
        let navigation_options = combo.options.clone();
        let navigation_controller = combo.controller.clone();
        let navigation_filter = combo.filter.clone();
        let virtual_list_id = super::WidgetId::next();
        let render_controller = combo.controller.clone();
        let render_on_change = combo.on_change.clone();
        let render_on_open_change = combo.on_open_change.clone();
        let render_internal_open = internal_open.clone();
        let render_style = combo.style.clone();
        let render_visual = combo.visual.clone();
        let render_width = menu_width_override.clone();
        let content: Element<VM> = Stack::new()
            .child(For::new_with_resolver(
                move || {
                    let query = source_controller.text();
                    let options =
                        filtered_options(source_options.resolve(), &query, source_filter.as_ref());
                    let selected_key = source_selected_key.resolve();
                    vec![if options.is_empty() {
                        ComboboxContent::Empty
                    } else {
                        ComboboxContent::Options {
                            options,
                            selected_key,
                        }
                    }]
                },
                ComboboxContent::stable_key,
                move |_index, content| match content {
                    ComboboxContent::Empty => build_empty_combobox_content(
                        render_style.clone(),
                        render_visual.clone(),
                        render_width.clone(),
                    ),
                    ComboboxContent::Options {
                        options,
                        selected_key,
                    } => build_combobox_options_content(
                        options.clone(),
                        selected_key.clone(),
                        render_controller.clone(),
                        render_on_change.clone(),
                        render_on_open_change.clone(),
                        render_internal_open.clone(),
                        render_style.clone(),
                        render_visual.clone(),
                        render_width.clone(),
                        virtual_list_id,
                    ),
                },
            ))
            .into();
        let mut popover = Popover::new(trigger)
            .content(content)
            .style(|style, _| {
                style.background = Value::Static(crate::foundation::color::Color::TRANSPARENT);
                style.border = Value::Static(crate::foundation::color::Color::TRANSPARENT);
                style.border_width = Value::Static(Dp::ZERO);
                style.radius = Value::Static(Dp::ZERO);
                style.shadow = Default::default();
                style.padding = Insets::all(Dp::ZERO);
                style.min_width = Dp::ZERO;
            })
            .match_anchor_width(true)
            .disable(combo.disabled.clone())
            .list_keyboard_navigation(true);
        if let Some(open) = combo.open.clone() {
            popover = popover.open(open);
        }
        if let Some(internal_open) = internal_open {
            popover = popover.open_handle(internal_open);
        }
        let mut root: Element<VM> = if let Some(command) = combo.on_open_change {
            popover.on_open_change(command).into()
        } else {
            popover.into()
        };
        root.key = combo.key;
        root = with_visual_identity(root, &combo.visual);
        root.layout = merge_layout(root.layout, combo.layout);
        root.popover
            .as_mut()
            .expect("Combobox root should retain its popover descriptor")
            .virtual_list_navigation = Some(PopoverVirtualListNavigation::new(
            virtual_list_id,
            move || {
                let query = navigation_controller.text();
                filtered_options(
                    navigation_options.resolve(),
                    &query,
                    navigation_filter.as_ref(),
                )
                .into_iter()
                .map(|option| option.disabled)
                .collect()
            },
        ));
        root
    }
}

#[derive(Clone)]
enum ComboboxContent {
    Empty,
    Options {
        options: Vec<ComboboxOption>,
        selected_key: Option<String>,
    },
}

impl ComboboxContent {
    fn stable_key(&self) -> &'static str {
        match self {
            Self::Empty => "__tgui_combobox_empty",
            Self::Options { .. } => "__tgui_combobox_options",
        }
    }
}

fn build_empty_combobox_content<VM: 'static>(
    style: Option<StyleResolver<ComboboxStyle>>,
    visual: VisualStyle,
    menu_width_override: Option<Value<Length>>,
) -> Element<VM> {
    let text_style = style.clone();
    let surface_style = style.clone();
    let layout_style = style;
    let mut content = with_visual_identity(
        Stack::new()
            .runtime_layout(move |layout, container, context, style_sheet, visual| {
                let resolved = resolve_combobox_style_with_sheet(
                    layout_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                if layout.width.is_none() {
                    layout.width = menu_width_override
                        .clone()
                        .or_else(|| Some(Value::Static(Length::Px(resolved.menu_width))));
                }
                container.padding = Some(Value::Static(Insets::all(dp(resolved
                    .option_height
                    .get()
                    * 0.25))));
            })
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                combobox_menu_surface_style(
                    surface_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                )
            })
            .child(with_visual_identity(
                Text::new("No results")
                    .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                        let resolved = resolve_combobox_style_with_sheet(
                            text_style.as_ref(),
                            context,
                            style_sheet,
                            visual,
                            state,
                        );
                        TextWidgetStyle {
                            surface: Default::default(),
                            color: resolved.empty_foreground,
                            typography: context.theme.typography.body_small.clone(),
                        }
                    })
                    .into(),
                &visual,
            ))
            .into(),
        &visual,
    );
    content.visual.accessibility_role = Some(AccessibilityRole::ListBox);
    content
}

#[allow(clippy::too_many_arguments)]
fn build_combobox_options_content<VM: 'static>(
    options: Vec<ComboboxOption>,
    selected_key: Option<String>,
    controller: TextController,
    on_change: Option<ValueCommand<VM, ComboboxChange>>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    internal_open: Option<PopoverOpenHandle>,
    style: Option<StyleResolver<ComboboxStyle>>,
    visual: VisualStyle,
    menu_width_override: Option<Value<Length>>,
    virtual_list_id: super::WidgetId,
) -> Element<VM> {
    let option_count = options.len();
    let option_visual = visual.clone();
    let option_style = style.clone();
    let menu_layout_style = style.clone();
    let mut menu: Element<VM> = VirtualList::new_with_style_context(
        options,
        move |_index, option: &ComboboxOption, context, style_sheet| {
            let controller = controller.clone();
            let on_change = on_change.clone();
            let on_open_change = on_open_change.clone();
            let internal_open = internal_open.clone();
            let key = option.key.clone();
            let label = option.label.clone();
            let selected = selected_key.as_deref() == Some(option.key.as_str());
            let button_style = option_style.clone();
            let resolved = resolve_combobox_style_with_sheet(
                button_style.as_ref(),
                &context,
                style_sheet,
                &option_visual,
                WidgetState::default(),
            );
            let button = Button::new(label.clone())
                .ghost()
                .disable(option.disabled)
                .width(pct(100.0))
                .height(resolved.option_height);
            let variant = button.variant();
            let mut option_element = with_visual_identity(
                button
                    .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                        let resolved = resolve_combobox_style_with_sheet(
                            button_style.as_ref(),
                            context,
                            style_sheet,
                            visual,
                            state,
                        );
                        let mut button = ButtonStyle::default_for_theme(context.theme, variant);
                        button.background =
                            combobox_option_background(resolved.highlight.clone(), selected);
                        button
                    })
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
                        if let Some(internal_open) = internal_open.as_ref() {
                            internal_open.set(false);
                        }
                        if let Some(command) = on_open_change.as_ref() {
                            command.execute_with_context(vm, false, context);
                        }
                    }))
                    .into(),
                &option_visual,
            );
            option_element.visual.accessibility_role =
                Some(AccessibilityRole::ListBoxOption { selected });
            option_element
        },
    )
    .widget_id(virtual_list_id)
    .item_layout(ItemLayout::Fixed {
        item_extent: dp(40.0),
        spacing: Dp::ZERO,
        overscan: 3,
    })
    .runtime_layout(move |layout, item_layout, context, style_sheet, visual| {
        let resolved = resolve_combobox_style_with_sheet(
            menu_layout_style.as_ref(),
            context,
            style_sheet,
            visual,
            WidgetState::default(),
        );
        if layout.width.is_none() {
            layout.width = menu_width_override
                .clone()
                .or_else(|| Some(Value::Static(Length::Px(resolved.menu_width))));
        }
        if layout.height.is_none() {
            let visible = resolved.max_visible_options.min(option_count).max(1);
            layout.height = Some(Value::Static(Length::Px(dp(
                resolved.option_height.get() * visible as f32
            ))));
        }
        *item_layout = item_layout.with_estimate(resolved.option_height);
    })
    .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
        combobox_menu_surface_style(style.as_ref(), context, style_sheet, visual, state)
    })
    .into();
    menu.visual.accessibility_role = Some(AccessibilityRole::ListBox);
    with_visual_identity(menu, &visual)
}

fn combobox_option_background(highlight: Value<Color>, selected: bool) -> StateValue<Value<Color>> {
    let transparent = Value::Static(Color::TRANSPARENT);
    let normal = if selected {
        highlight.clone()
    } else {
        transparent.clone()
    };
    StateValue::interactive(normal, highlight.clone(), highlight, transparent)
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

fn normalize_non_custom_text(
    controller: &TextController,
    options: &Value<Vec<ComboboxOption>>,
    selected_key: &Value<Option<String>>,
) {
    let current = controller.text();
    let options = options.resolve();
    if options
        .iter()
        .any(|option| !option.disabled && option.label == current)
    {
        return;
    }

    let replacement = selected_key
        .resolve()
        .and_then(|selected_key| {
            options
                .iter()
                .find(|option| !option.disabled && option.key == selected_key)
                .map(|option| option.label.clone())
        })
        .unwrap_or_default();
    if replacement != current {
        controller.set_text(replacement);
    }
}

fn resolve_combobox_style_with_sheet(
    style: Option<&StyleResolver<ComboboxStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> ComboboxStyle {
    resolve_component_style_with_sheet(
        style,
        context,
        style_sheet,
        visual,
        state,
        ComboboxStyle::default_for_theme(context.theme),
        |base, context| context.theme.components.combobox.apply(base, context),
        |sheet, base, context, visual| sheet.apply_combobox(base, context, visual),
        |sheet, base, context, visual, state| {
            sheet.apply_combobox_state(base, context, visual, state)
        },
    )
}

fn combobox_menu_surface_style(
    style: Option<&StyleResolver<ComboboxStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> ContainerStyle {
    let _ = resolve_combobox_style_with_sheet(style, context, style_sheet, visual, state);
    let mut container = ContainerStyle::default_for_theme(context.theme);
    container.surface.background = Some(Value::Static(context.theme.colors.surface_overlay));
    container.surface.border_radius = Some(Value::Static(context.theme.radius.xl));
    container.surface.border_color = Some(Value::Static(context.theme.colors.outline_muted));
    container.surface.border_width = Some(Value::Static(context.theme.border.thin));
    container.surface.shadow = Some(Value::Static(context.theme.elevation.md.clone()));
    container
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_highlight_is_limited_to_hover_and_press() {
        let highlight = Color::rgba(32, 96, 224, 180);
        let background = combobox_option_background(Value::Static(highlight), false);

        for state in [
            WidgetState::default(),
            WidgetState {
                disabled: true,
                ..Default::default()
            },
        ] {
            assert_eq!(background.resolve(state).resolve(), Color::TRANSPARENT);
        }
        for state in [
            WidgetState {
                hovered: true,
                ..Default::default()
            },
            WidgetState {
                pressed: true,
                ..Default::default()
            },
        ] {
            assert_eq!(background.resolve(state).resolve(), highlight);
        }
    }

    #[test]
    fn selected_option_uses_highlight_at_rest() {
        let highlight = Color::rgba(32, 96, 224, 180);
        let background = combobox_option_background(Value::Static(highlight), true);

        assert_eq!(
            background.resolve(WidgetState::default()).resolve(),
            highlight
        );
    }

    #[test]
    fn non_custom_text_normalizes_to_selected_enabled_option() {
        let controller = TextController::new_legacy("free text");
        let options = Value::Static(vec![
            ComboboxOption::new("alpha", "Alpha"),
            ComboboxOption::new("disabled", "Disabled").disabled(true),
        ]);
        let selected = Value::Static(Some("alpha".to_string()));

        normalize_non_custom_text(&controller, &options, &selected);
        assert_eq!(controller.text(), "Alpha");

        controller.set_text("Disabled");
        normalize_non_custom_text(&controller, &options, &Value::Static(None));
        assert_eq!(controller.text(), "");
    }
}
