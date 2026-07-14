use std::time::Duration;

use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Align, LayoutStyle, Length, Value};

use super::common::CarouselAutoPlayState;
use super::common::VisualStyle;
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{CarouselStyle, ContainerStyle, StyleResolver, StyleSheet};
use super::{Button, CursorStyle, Flex, Stack, WidgetKey};

pub struct Carousel<VM> {
    items: Vec<Element<VM>>,
    selected: Value<usize>,
    on_change: Option<ValueCommand<VM, usize>>,
    auto_play: Option<Duration>,
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
        let count = carousel.items.len().max(1);
        let selected = carousel.selected.resolve().min(count - 1);
        let item = carousel
            .items
            .into_iter()
            .nth(selected)
            .unwrap_or_else(|| Stack::new().into());
        let prev = carousel_change_button("Prev", selected, count, -1, carousel.on_change.clone());
        let next = carousel_change_button("Next", selected, count, 1, carousel.on_change.clone());
        let indicators = (0..count)
            .map(|index| {
                let on_change = carousel.on_change.clone();
                let style = carousel.style.clone();
                with_visual_identity(
                    Stack::new()
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
                            container.surface.background = Some(if index == selected {
                                resolved.active_indicator
                            } else {
                                resolved.indicator
                            });
                            container.surface.border_radius =
                                Some(Value::Static(context.theme.radius.full));
                            container
                        })
                        .on_click(Command::new_with_context(move |vm, context| {
                            if let Some(command) = on_change.as_ref() {
                                command.execute_with_context(vm, index, context);
                            }
                        }))
                        .focusable(true)
                        .tab_index(0)
                        .cursor(CursorStyle::Pointer)
                        .into(),
                    &carousel.visual,
                )
            })
            .collect::<Vec<Element<VM>>>();
        let root_style = carousel.style.clone();
        let row_style = carousel.style.clone();
        let indicator_row_style = carousel.style.clone();
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
                    .child(Stack::new().grow(1.0).child(item))
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
        if let Some(interval) = carousel.auto_play {
            root.carousel_auto_play = Some(CarouselAutoPlayState {
                id: root.id,
                frame: crate::ui::widget::Rect::new(
                    crate::ui::unit::Dp::ZERO,
                    crate::ui::unit::Dp::ZERO,
                    crate::ui::unit::Dp::ZERO,
                    crate::ui::unit::Dp::ZERO,
                ),
                selected,
                count,
                interval,
                on_change: carousel.on_change.clone(),
            });
        }
        root.key = carousel.key;
        root = with_visual_identity(root, &carousel.visual);
        root.layout = merge_layout(root.layout, carousel.layout);
        root
    }
}

fn carousel_change_button<VM: 'static>(
    label: &str,
    selected: usize,
    count: usize,
    step: i32,
    command: Option<ValueCommand<VM, usize>>,
) -> Element<VM> {
    let target = if step < 0 {
        if selected == 0 {
            count - 1
        } else {
            selected - 1
        }
    } else {
        (selected + 1) % count
    };
    Button::new(label)
        .secondary()
        .on_click(Command::new_with_context(move |vm, context| {
            if let Some(command) = command.as_ref() {
                command.execute_with_context(vm, target, context);
            }
        }))
        .into()
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
