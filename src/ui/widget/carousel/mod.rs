use std::time::Duration;

use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::StyleContext;
use crate::ui::layout::{Align, LayoutStyle, Value};
use crate::ui::theme::Theme;

use super::common::CarouselAutoPlayState;
use super::core::Element;
use super::p3_support::{impl_p3_layout_api, merge_layout};
use super::style::{CarouselStyle, ContainerStyle, StyleResolver};
use super::{Button, CursorStyle, Flex, Stack, WidgetKey};

pub struct Carousel<VM> {
    items: Vec<Element<VM>>,
    selected: Value<usize>,
    on_change: Option<ValueCommand<VM, usize>>,
    auto_play: Option<Duration>,
    style: Option<StyleResolver<CarouselStyle>>,
    layout: LayoutStyle,
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
        let layout_style = resolve_carousel_style_for_layout(carousel.style.as_ref());
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
                Stack::new()
                    .size(layout_style.indicator_size, layout_style.indicator_size)
                    .style_full(move |context| {
                        let resolved = resolve_carousel_style(style.as_ref(), context);
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
                    .into()
            })
            .collect::<Vec<Element<VM>>>();
        let mut root: Element<VM> = Flex::vertical()
            .gap(layout_style.gap)
            .child(
                Flex::horizontal()
                    .align(Align::Center)
                    .gap(layout_style.gap)
                    .child(prev)
                    .child(Stack::new().grow(1.0).child(item))
                    .child(next),
            )
            .child(
                Flex::horizontal()
                    .center()
                    .gap(layout_style.indicator_gap)
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

fn resolve_carousel_style(
    style: Option<&StyleResolver<CarouselStyle>>,
    context: &StyleContext<'_>,
) -> CarouselStyle {
    let mut base = CarouselStyle::default_for_theme(context.theme);
    context.theme.components.carousel.apply(&mut base, context);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

fn resolve_carousel_style_for_layout(
    style: Option<&StyleResolver<CarouselStyle>>,
) -> CarouselStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_carousel_style(style, &context)
}
