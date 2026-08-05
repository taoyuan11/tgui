use std::time::Duration;

use crate::foundation::binding::{InvalidationSignal, State};
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Align, LayoutStyle, Length, Value};

use super::common::{
    AccessibilityCurrent, AccessibilityRole, CarouselAutoPlayState, Point, VisualStyle,
};
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{CarouselStyle, ContainerStyle, StyleResolver, StyleSheet};
use super::{Button, CursorStyle, Flex, FocusScopeOptions, Stack, WidgetKey};

const CAROUSEL_PANEL_SHIFT_DP: f32 = 10.0;

pub struct Carousel<VM> {
    items: Vec<Element<VM>>,
    selected: Value<usize>,
    on_change: Option<ValueCommand<VM, usize>>,
    auto_play: Option<Duration>,
    disabled: Value<bool>,
    style: Option<StyleResolver<CarouselStyle>>,
    layout: LayoutStyle,
    visual: VisualStyle,
    key: Option<WidgetKey>,
}

impl<VM> Carousel<VM> {
    pub fn new(items: Vec<Element<VM>>, selected: impl Into<Value<usize>>) -> Self {
        Self {
            items,
            selected: selected.into(),
            on_change: None,
            auto_play: None,
            disabled: Value::Static(false),
            style: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            key: None,
        }
    }

    pub fn on_change(mut self, command: ValueCommand<VM, usize>) -> Self {
        self.on_change = Some(command);
        self
    }

    pub fn auto_play(mut self, interval: Duration) -> Self {
        self.auto_play = Some(interval);
        self
    }

    /// Disables manual navigation and pauses auto-play without disabling slide content.
    pub fn disable(mut self, disabled: impl Into<Value<bool>>) -> Self {
        self.disabled = disabled.into();
        self
    }

    /// Alias for [`Carousel::disable`].
    pub fn disabled(self, disabled: impl Into<Value<bool>>) -> Self {
        self.disable(disabled)
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut CarouselStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| CarouselStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> CarouselStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<Carousel<VM>> for Element<VM> {
    fn from(carousel: Carousel<VM>) -> Self {
        let Carousel {
            items,
            selected,
            on_change,
            auto_play,
            disabled,
            style,
            layout,
            visual,
            key,
        } = carousel;
        let count = items.len();
        // Static indices are uncontrolled initial values. A Signal remains a controlled binding.
        let (selected, internal_selected) = interactive_carousel_selection(selected);
        let selected = normalized_carousel_selection(selected, count);
        let on_change = carousel_change_command(internal_selected, on_change, count);
        let navigation_disabled =
            carousel_navigation_disabled_value(&disabled, count < 2 || on_change.is_none());
        let panels = items
            .into_iter()
            .enumerate()
            .map(|(index, item)| {
                let active = carousel_index_active_value(&selected, index);
                let opacity = carousel_active_opacity_value(active.clone());
                let offset = carousel_panel_offset_value(&selected, index);
                Stack::new()
                    .runtime_layout(move |_, _, context, _, visual| {
                        visual.opacity = opacity
                            .clone()
                            .with_default_transition(context.motion_normal_transition());
                        visual.offset = offset
                            .clone()
                            .with_default_transition(context.motion_fast_transition());
                    })
                    .focus_scope(
                        FocusScopeOptions::new()
                            .active(active)
                            .suppress_interactions_when_inactive()
                            .hide_from_accessibility_when_inactive(),
                    )
                    .child(item)
            })
            .collect::<Vec<_>>();
        let prev = carousel_change_button(
            "Prev",
            selected.clone(),
            count,
            -1,
            navigation_disabled.clone(),
            on_change.clone(),
        );
        let next = carousel_change_button(
            "Next",
            selected.clone(),
            count,
            1,
            navigation_disabled.clone(),
            on_change.clone(),
        );
        let indicators = (0..count)
            .map(|index| {
                let on_change = on_change.clone();
                let style = style.clone();
                let active = carousel_index_active_value(&selected, index);
                let active_for_style = active.clone();
                let enabled = inverted_bool_value(&navigation_disabled);
                let visual_enabled = inverted_bool_value(&disabled);
                let opacity = enabled_opacity_value(&visual_enabled);
                let indicator = Stack::new()
                    .runtime_layout({
                        let style = style.clone();
                        move |layout, _container, context, style_sheet, visual| {
                            let resolved = resolve_carousel_style_with_sheet(
                                style.as_ref(),
                                context,
                                style_sheet,
                                visual,
                                WidgetState::default(),
                            );
                            if layout.width.is_none() {
                                layout.width =
                                    Some(Value::Static(Length::Px(resolved.indicator_size)));
                            }
                            if layout.height.is_none() {
                                layout.height =
                                    Some(Value::Static(Length::Px(resolved.indicator_size)));
                            }
                        }
                    })
                    .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                        let resolved = resolve_carousel_style_with_sheet(
                            style.as_ref(),
                            context,
                            style_sheet,
                            visual,
                            state,
                        );
                        let mut container = ContainerStyle::default_for_theme(context.theme);
                        container.surface.background = Some(if active_for_style.resolve() {
                            resolved.active_indicator
                        } else {
                            resolved.indicator
                        });
                        container.surface.border_radius =
                            Some(Value::Static(context.theme.radius.full));
                        container
                    })
                    .opacity(opacity)
                    .focus_scope(
                        FocusScopeOptions::new()
                            .active(enabled)
                            .suppress_interactions_when_inactive(),
                    );
                let indicator = if let Some(on_change) = on_change {
                    indicator
                        .on_click(Command::new_with_context(move |vm, context| {
                            on_change.execute_with_context(vm, index, context);
                        }))
                        .focusable(true)
                        .tab_index(0)
                        .cursor(CursorStyle::Pointer)
                } else {
                    indicator
                };
                let mut indicator = with_visual_identity(indicator.into(), &visual);
                indicator.visual.accessibility_role = Some(AccessibilityRole::Button);
                indicator.visual.accessibility_label = Some(Value::Static(format!(
                    "Go to slide {} of {}",
                    index + 1,
                    count
                )));
                indicator.visual.accessibility_selected = Some(active.clone());
                indicator.visual.accessibility_current = Some((active, AccessibilityCurrent::True));
                indicator.visual.accessibility_disabled = Some(navigation_disabled.clone());
                indicator.visual.accessibility_position_in_set = Some(index + 1);
                indicator.visual.accessibility_size_of_set = Some(count);
                indicator
            })
            .collect::<Vec<Element<VM>>>();
        let root_style = style.clone();
        let row_style = style.clone();
        let indicator_row_style = style.clone();
        let mut root: Element<VM> = Flex::vertical()
            .gap(crate::ui::unit::dp(0.0))
            .runtime_layout(move |_layout, container, context, style_sheet, visual| {
                let resolved = resolve_carousel_style_with_sheet(
                    root_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                container.gap = Value::Static(Length::Px(resolved.gap));
            })
            .child(
                Flex::horizontal()
                    .align(Align::Center)
                    .gap(crate::ui::unit::dp(0.0))
                    .runtime_layout(move |_layout, container, context, style_sheet, visual| {
                        let resolved = resolve_carousel_style_with_sheet(
                            row_style.as_ref(),
                            context,
                            style_sheet,
                            visual,
                            WidgetState::default(),
                        );
                        container.gap = Value::Static(Length::Px(resolved.gap));
                    })
                    .child(prev)
                    .child(Stack::new().grow(1.0).child(panels))
                    .child(next),
            )
            .child(
                Flex::horizontal()
                    .center()
                    .gap(crate::ui::unit::dp(0.0))
                    .runtime_layout(move |_layout, container, context, style_sheet, visual| {
                        let resolved = resolve_carousel_style_with_sheet(
                            indicator_row_style.as_ref(),
                            context,
                            style_sheet,
                            visual,
                            WidgetState::default(),
                        );
                        container.gap = Value::Static(Length::Px(resolved.indicator_gap));
                    })
                    .child(indicators),
            )
            .into();
        if let Some(interval) = auto_play {
            root.carousel_auto_play = Some(CarouselAutoPlayState {
                id: root.id,
                frame: crate::ui::widget::Rect::new(
                    crate::ui::unit::Dp::ZERO,
                    crate::ui::unit::Dp::ZERO,
                    crate::ui::unit::Dp::ZERO,
                    crate::ui::unit::Dp::ZERO,
                ),
                selected: selected.clone(),
                count,
                interval,
                disabled: disabled.clone(),
                on_change: on_change.clone(),
            });
        }
        root.key = key;
        root = with_visual_identity(root, &visual);
        root.layout = merge_layout(root.layout, layout);
        root
    }
}

fn normalized_carousel_selection(selected: Value<usize>, count: usize) -> Value<usize> {
    let max_index = count.saturating_sub(1);
    match selected {
        Value::Static(selected) => Value::Static(selected.min(max_index)),
        Value::Signal(signal) => {
            Value::Signal(signal.map_memo(move |selected| selected.min(max_index)))
        }
    }
}

fn interactive_carousel_selection(selected: Value<usize>) -> (Value<usize>, Option<State<usize>>) {
    match selected {
        Value::Static(initial) => {
            let state = State::new(initial, InvalidationSignal::new());
            (Value::Signal(state.signal()), Some(state))
        }
        Value::Signal(signal) => (Value::Signal(signal), None),
    }
}

fn carousel_change_command<VM: 'static>(
    internal: Option<State<usize>>,
    callback: Option<ValueCommand<VM, usize>>,
    count: usize,
) -> Option<ValueCommand<VM, usize>> {
    if internal.is_none() && callback.is_none() {
        return None;
    }
    Some(ValueCommand::new_with_context(
        move |vm, target: usize, context| {
            let target = target.min(count.saturating_sub(1));
            if let Some(internal) = internal.as_ref() {
                internal.set(target);
            }
            if let Some(callback) = callback.as_ref() {
                callback.execute_with_context(vm, target, context);
            }
        },
    ))
}

fn carousel_navigation_disabled_value(disabled: &Value<bool>, unavailable: bool) -> Value<bool> {
    if unavailable {
        return Value::Static(true);
    }
    disabled.clone()
}

fn inverted_bool_value(value: &Value<bool>) -> Value<bool> {
    match value {
        Value::Static(value) => Value::Static(!*value),
        Value::Signal(signal) => Value::Signal(signal.map_memo(|value| !value)),
    }
}

fn enabled_opacity_value(enabled: &Value<bool>) -> Value<f32> {
    match enabled {
        Value::Static(enabled) => Value::Static(if *enabled { 1.0 } else { 0.38 }),
        Value::Signal(signal) => {
            Value::Signal(signal.map_memo(|enabled| if enabled { 1.0 } else { 0.38 }))
        }
    }
}

fn carousel_index_active_value(selected: &Value<usize>, index: usize) -> Value<bool> {
    match selected {
        Value::Static(selected) => Value::Static(*selected == index),
        Value::Signal(signal) => Value::Signal(signal.map_memo(move |selected| selected == index)),
    }
}

fn carousel_active_opacity_value(active: Value<bool>) -> Value<f32> {
    match active {
        Value::Static(active) => Value::Static(if active { 1.0 } else { 0.0 }),
        Value::Signal(signal) => {
            Value::Signal(signal.map_memo(|active| if active { 1.0 } else { 0.0 }))
        }
    }
}

fn carousel_panel_offset_value(selected: &Value<usize>, index: usize) -> Value<Point> {
    match selected {
        Value::Static(selected) => Value::Static(carousel_panel_offset(*selected, index)),
        Value::Signal(signal) => {
            Value::Signal(signal.map_memo(move |selected| carousel_panel_offset(selected, index)))
        }
    }
}

fn carousel_panel_offset(selected: usize, index: usize) -> Point {
    if selected == index {
        Point::ZERO
    } else {
        let direction = if index < selected { -1.0 } else { 1.0 };
        Point::new(
            crate::ui::unit::dp(CAROUSEL_PANEL_SHIFT_DP * direction),
            crate::ui::unit::dp(0.0),
        )
    }
}

fn carousel_change_button<VM: 'static>(
    label: &str,
    selected: Value<usize>,
    count: usize,
    step: i32,
    disabled: Value<bool>,
    command: Option<ValueCommand<VM, usize>>,
) -> Element<VM> {
    let button = Button::new(label).secondary().disable(disabled);
    let button = if let Some(command) = command {
        button.on_click(Command::new_with_context(move |vm, context| {
            if count >= 2 {
                let selected = selected.resolve_untracked().min(count.saturating_sub(1));
                let target = if step < 0 {
                    if selected == 0 {
                        count - 1
                    } else {
                        selected - 1
                    }
                } else {
                    (selected + 1) % count
                };
                command.execute_with_context(vm, target, context);
            }
        }))
    } else {
        button
    };
    button.into()
}

fn resolve_carousel_style_with_sheet(
    style: Option<&StyleResolver<CarouselStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> CarouselStyle {
    resolve_component_style_with_sheet(
        style,
        context,
        style_sheet,
        visual,
        state,
        CarouselStyle::default_for_theme(context.theme),
        |base, context| context.theme.components.carousel.apply(base, context),
        |sheet, base, context, visual| sheet.apply_carousel(base, context, visual),
        |sheet, base, context, visual, state| {
            sheet.apply_carousel_state(base, context, visual, state)
        },
    )
}
