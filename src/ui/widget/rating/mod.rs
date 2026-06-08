use crate::foundation::color::Color;
use crate::foundation::view_model::ValueCommand;
use crate::theme::StyleContext;
use crate::ui::layout::{pct, Align, LayoutStyle, Value};
use crate::ui::theme::{StateValue, Theme};

use super::core::Element;
use super::icon::{BuiltinIcon, Icon};
use super::p3_support::{impl_p3_layout_api, merge_layout};
use super::style::{IconStyle, RatingStyle, SliderStyle, StyleResolver};
use super::{Flex, Slider, Stack, WidgetKey};

#[derive(Clone, Debug, PartialEq)]
pub struct RatingChange {
    pub value: f32,
}

pub struct Rating<VM> {
    value: Value<f32>,
    max: usize,
    step: f32,
    read_only: Value<bool>,
    on_change: Option<ValueCommand<VM, RatingChange>>,
    style: Option<StyleResolver<RatingStyle>>,
    layout: LayoutStyle,
    key: Option<WidgetKey>,
}

impl<VM> Rating<VM> {
    pub fn new(value: impl Into<Value<f32>>) -> Self {
        Self {
            value: value.into(),
            max: 5,
            step: 1.0,
            read_only: Value::Static(false),
            on_change: None,
            style: None,
            layout: LayoutStyle::default(),
            key: None,
        }
    }

    pub fn max(mut self, max: usize) -> Self {
        self.max = max.max(1);
        self
    }

    pub fn half(mut self) -> Self {
        self.step = 0.5;
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        self.step = step.clamp(0.1, 1.0);
        self
    }

    pub fn read_only(mut self, read_only: impl Into<Value<bool>>) -> Self {
        self.read_only = read_only.into();
        self
    }

    pub fn on_change(mut self, command: ValueCommand<VM, RatingChange>) -> Self {
        self.on_change = Some(command);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut RatingStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| RatingStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> RatingStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<Rating<VM>> for Element<VM> {
    fn from(rating: Rating<VM>) -> Self {
        let layout_style = resolve_rating_style_for_layout(rating.style.as_ref());
        let value = rating.value.resolve().clamp(0.0, rating.max as f32);
        let read_only = rating.read_only.resolve();
        let mut stars = Vec::new();
        for index in 0..rating.max {
            let threshold = index as f32 + 1.0;
            let icon = if value + f32::EPSILON >= threshold {
                BuiltinIcon::Star
            } else if value + f32::EPSILON >= threshold - 0.5 {
                BuiltinIcon::StarHalf
            } else {
                BuiltinIcon::Star
            };
            let active = value + f32::EPSILON >= threshold - 0.5;
            let style = rating.style.clone();
            let icon_element: Element<VM> = Icon::builtin(icon)
                .style_full(move |context| {
                    let resolved = resolve_rating_style(style.as_ref(), context);
                    IconStyle {
                        color: if active {
                            resolved.active
                        } else {
                            resolved.inactive
                        },
                        size: resolved.size,
                    }
                })
                .into();
            stars.push(icon_element);
        }
        let star_row: Element<VM> = Flex::horizontal()
            .align(Align::Center)
            .gap(layout_style.gap)
            .child(stars)
            .into();
        let mut root: Element<VM> = if !read_only {
            if let Some(command) = rating.on_change.clone() {
                let step = rating.step;
                let max = rating.max;
                Stack::new()
                    .child(star_row)
                    .child(
                        Slider::new(value, 0.0, max as f32)
                            .step(step)
                            .position_absolute()
                            .left(pct(0.0))
                            .top(pct(0.0))
                            .width(pct(100.0))
                            .height(pct(100.0))
                            .style_full(move |context| transparent_rating_slider_style(context))
                            .on_change(ValueCommand::new_with_context(
                                move |vm, next: f32, context| {
                                    command.execute_with_context(
                                        vm,
                                        RatingChange {
                                            value: next.clamp(0.0, max as f32),
                                        },
                                        context,
                                    );
                                },
                            )),
                    )
                    .into()
            } else {
                star_row
            }
        } else {
            star_row
        };
        root.key = rating.key;
        root.layout = merge_layout(root.layout, rating.layout);
        root
    }
}

fn transparent_rating_slider_style(context: &StyleContext<'_>) -> SliderStyle {
    let mut style = SliderStyle::default_for_theme(context.theme);
    let transparent = Value::Static(Color::TRANSPARENT);
    style.track = StateValue::new(transparent.clone());
    style.active_track = StateValue::new(transparent.clone());
    style.thumb = StateValue::new(transparent.clone());
    style.tick = StateValue::new(transparent.clone());
    style.label = StateValue::new(transparent);
    style.thumb_shadow = None;
    style.focus_ring = None;
    style.track_height = context.theme.spacing.lg;
    style.thumb_size = context.theme.spacing.lg;
    style.border_width = Value::Static(crate::ui::unit::Dp::ZERO);
    style.min_width = crate::ui::unit::Dp::ZERO;
    style.min_height = crate::ui::unit::Dp::ZERO;
    style
}

fn resolve_rating_style(
    style: Option<&StyleResolver<RatingStyle>>,
    context: &StyleContext<'_>,
) -> RatingStyle {
    let mut base = RatingStyle::default_for_theme(context.theme);
    context.theme.components.rating.apply(&mut base, context);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

fn resolve_rating_style_for_layout(style: Option<&StyleResolver<RatingStyle>>) -> RatingStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_rating_style(style, &context)
}
