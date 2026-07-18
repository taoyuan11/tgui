use crate::foundation::color::Color;
use crate::foundation::view_model::ValueCommand;
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{pct, Align, LayoutStyle, Length, Value};
use crate::ui::theme::StateValue;

use super::common::VisualStyle;
use super::core::Element;
use super::icon::{BuiltinIcon, Icon};
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{IconStyle, RatingStyle, SliderStyle, StyleResolver, StyleSheet};
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
    visual: VisualStyle,
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
            visual: VisualStyle::default(),
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
        let value = rating.value.resolve().clamp(0.0, rating.max as f32);
        let read_only = rating.read_only.resolve();
        let visual_identity = rating.visual.clone();
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
            let icon_identity = visual_identity.clone();
            let icon_element: Element<VM> = with_visual_identity(
                Icon::builtin(icon)
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
                    .into(),
                &icon_identity,
            );
            stars.push(icon_element);
        }
        let runtime_style = rating.style.clone();
        let star_row: Element<VM> = Flex::horizontal()
            .align(Align::Center)
            .runtime_layout(move |layout, container, context, style_sheet, visual| {
                // Rating is composed from a Flex row, but its spacing is a
                // component token rather than user-authored container layout.
                // Resolve it at runtime so a retained tree follows density and
                // light/dark theme changes without rebuilding wrappers.
                let resolved = resolve_rating_style_with_sheet(
                    runtime_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                container.gap = Value::Static(Length::Px(resolved.gap));
                if layout.min_height.is_none() {
                    layout.min_height = Some(Value::Static(Length::Px(
                        SliderStyle::default_for_theme(context.theme).min_height,
                    )));
                }
            })
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
                            .style_full_with_style_sheet({
                                let style = rating.style.clone();
                                move |context, style_sheet, visual, state| {
                                    transparent_rating_slider_style(
                                        style.as_ref(),
                                        context,
                                        style_sheet,
                                        visual,
                                        state,
                                    )
                                }
                            })
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
        root = with_visual_identity(root, &rating.visual);
        root.layout = merge_layout(root.layout, rating.layout);
        root
    }
}

fn transparent_rating_slider_style(
    style: Option<&StyleResolver<RatingStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> SliderStyle {
    let _ = resolve_rating_style_with_sheet(style, context, style_sheet, visual, state);
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
    let style_sheet = StyleSheet::default();
    resolve_rating_style_with_sheet(
        style,
        context,
        &style_sheet,
        &VisualStyle::default(),
        WidgetState::default(),
    )
}

fn resolve_rating_style_with_sheet(
    style: Option<&StyleResolver<RatingStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> RatingStyle {
    resolve_component_style_with_sheet(
        style,
        context,
        style_sheet,
        visual,
        state,
        RatingStyle::default_for_theme(context.theme),
        |base, context| context.theme.components.rating.apply(base, context),
        |sheet, base, context, visual| sheet.apply_rating(base, context, visual),
        |sheet, base, context, visual, state| {
            sheet.apply_rating_state(base, context, visual, state)
        },
    )
}
