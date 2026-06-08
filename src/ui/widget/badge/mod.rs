use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Insets, LayoutStyle, Value};
use crate::ui::theme::Theme;
use crate::ui::unit::{dp, Dp};

use super::common::VisualStyle;
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{
    BadgeStyle, BadgeTone, ContainerStyle, StyleResolver, StyleSheet, TextWidgetStyle,
};
use super::{Stack, Text, WidgetKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgePlacement {
    TopStart,
    TopEnd,
    BottomStart,
    BottomEnd,
}

impl Default for BadgePlacement {
    fn default() -> Self {
        Self::TopEnd
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BadgeContent {
    Dot,
    Text(Value<String>),
    Count { value: Value<u32>, max: u32 },
}

#[derive(Clone)]
pub struct Badge<VM> {
    content: BadgeContent,
    tone: BadgeTone,
    placement: BadgePlacement,
    offset: (Dp, Dp),
    style: Option<StyleResolver<BadgeStyle>>,
    anchor: Option<Element<VM>>,
    layout: LayoutStyle,
    visual: VisualStyle,
    key: Option<WidgetKey>,
}

impl<VM> Badge<VM> {
    pub fn dot() -> Self {
        Self::new(BadgeContent::Dot)
    }

    pub fn text(text: impl Into<Value<String>>) -> Self {
        Self::new(BadgeContent::Text(text.into()))
    }

    pub fn count(value: impl Into<Value<u32>>) -> Self {
        Self::new(BadgeContent::Count {
            value: value.into(),
            max: 99,
        })
    }

    fn new(content: BadgeContent) -> Self {
        Self {
            content,
            tone: BadgeTone::Error,
            placement: BadgePlacement::TopEnd,
            offset: (Dp::ZERO, Dp::ZERO),
            style: None,
            anchor: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            key: None,
        }
    }

    pub fn tone(mut self, tone: BadgeTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn max(mut self, max: u32) -> Self {
        if let BadgeContent::Count { max: target, .. } = &mut self.content {
            *target = max.max(1);
        }
        self
    }

    pub fn placement(mut self, placement: BadgePlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn offset(mut self, x: Dp, y: Dp) -> Self {
        self.offset = (x, y);
        self
    }

    pub fn attach(mut self, anchor: impl Into<Element<VM>>) -> Self {
        self.anchor = Some(anchor.into());
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut BadgeStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| BadgeStyle::default_for_theme(context.theme, BadgeTone::Error),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> BadgeStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<Badge<VM>> for Element<VM> {
    fn from(badge: Badge<VM>) -> Self {
        let layout_style = resolve_badge_style_for_layout(badge.style.as_ref(), badge.tone);
        let badge_element = badge_content_element(
            &badge.content,
            badge.style.clone(),
            badge.tone,
            badge.visual.clone(),
        );
        let mut root = if let Some(anchor) = badge.anchor {
            let mut overlay = badge_element;
            overlay.layout.position_type = crate::ui::layout::PositionType::Absolute;
            let (x, y) = badge.offset;
            match badge.placement {
                BadgePlacement::TopStart => {
                    overlay.layout.left = Some(crate::ui::layout::Length::Px(x).into());
                    overlay.layout.top = Some(crate::ui::layout::Length::Px(y).into());
                }
                BadgePlacement::TopEnd => {
                    overlay.layout.right = Some(crate::ui::layout::Length::Px(x).into());
                    overlay.layout.top = Some(crate::ui::layout::Length::Px(y).into());
                }
                BadgePlacement::BottomStart => {
                    overlay.layout.left = Some(crate::ui::layout::Length::Px(x).into());
                    overlay.layout.bottom = Some(crate::ui::layout::Length::Px(y).into());
                }
                BadgePlacement::BottomEnd => {
                    overlay.layout.right = Some(crate::ui::layout::Length::Px(x).into());
                    overlay.layout.bottom = Some(crate::ui::layout::Length::Px(y).into());
                }
            }
            Stack::new().child(anchor).child(overlay).into()
        } else {
            badge_element
        };
        root.key = badge.key;
        root = with_visual_identity(root, &badge.visual);
        root.layout = merge_layout(root.layout, badge.layout);
        if matches!(badge.content, BadgeContent::Dot) && root.layout.width.is_none() {
            root.layout.width = Some(crate::ui::layout::Length::Px(layout_style.dot_size).into());
        }
        root
    }
}

fn badge_content_element<VM: 'static>(
    content: &BadgeContent,
    style: Option<StyleResolver<BadgeStyle>>,
    tone: BadgeTone,
    visual_identity: VisualStyle,
) -> Element<VM> {
    let layout_style = resolve_badge_style_for_layout(style.as_ref(), tone);
    match content {
        BadgeContent::Dot => {
            let style = style.clone();
            with_visual_identity(
                Stack::new()
                    .size(layout_style.dot_size, layout_style.dot_size)
                    .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                        let resolved = resolve_badge_style_with_sheet(
                            style.as_ref(),
                            context,
                            style_sheet,
                            visual,
                            state,
                            tone,
                        );
                        let mut container = ContainerStyle::default_for_theme(context.theme);
                        container.surface.background = Some(resolved.background);
                        container.surface.border_radius = Some(Value::Static(resolved.radius));
                        container
                    })
                    .into(),
                &visual_identity,
            )
        }
        BadgeContent::Text(text) => badge_pill(text.clone(), style, tone, visual_identity),
        BadgeContent::Count { value, max } => {
            let max = *max;
            let label = match value {
                Value::Static(value) => Value::Static(format_badge_count(*value, max)),
                Value::Signal(signal) => signal
                    .map(move |value| format_badge_count(value, max))
                    .into(),
            };
            badge_pill(label, style, tone, visual_identity)
        }
    }
}

fn badge_pill<VM: 'static>(
    label: Value<String>,
    style: Option<StyleResolver<BadgeStyle>>,
    tone: BadgeTone,
    visual_identity: VisualStyle,
) -> Element<VM> {
    let layout_style = resolve_badge_style_for_layout(style.as_ref(), tone);
    let container_style = style.clone();
    let text_style = style;
    let text_identity = visual_identity.clone();
    with_visual_identity(
        Stack::new()
            .min_height(layout_style.min_height)
            .padding(Insets::symmetric(layout_style.padding_x, dp(1.0)))
            .center()
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_badge_style_with_sheet(
                    container_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                    tone,
                );
                let mut container = ContainerStyle::default_for_theme(context.theme);
                container.surface.background = Some(resolved.background);
                container.surface.border_radius = Some(Value::Static(resolved.radius));
                container
            })
            .child(with_visual_identity(
                Text::new(label)
                    .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                        let resolved = resolve_badge_style_with_sheet(
                            text_style.as_ref(),
                            context,
                            style_sheet,
                            visual,
                            state,
                            tone,
                        );
                        TextWidgetStyle {
                            surface: Default::default(),
                            color: resolved.foreground,
                            typography: resolved.text_style,
                        }
                    })
                    .into(),
                &text_identity,
            ))
            .into(),
        &visual_identity,
    )
}

fn format_badge_count(value: u32, max: u32) -> String {
    if value > max {
        format!("{max}+")
    } else {
        value.to_string()
    }
}

fn resolve_badge_style(
    style: Option<&StyleResolver<BadgeStyle>>,
    context: &StyleContext<'_>,
    tone: BadgeTone,
) -> BadgeStyle {
    let style_sheet = StyleSheet::default();
    resolve_badge_style_with_sheet(
        style,
        context,
        &style_sheet,
        &VisualStyle::default(),
        WidgetState::default(),
        tone,
    )
}

fn resolve_badge_style_with_sheet(
    style: Option<&StyleResolver<BadgeStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
    tone: BadgeTone,
) -> BadgeStyle {
    resolve_component_style_with_sheet(
        style,
        context,
        style_sheet,
        visual,
        state,
        BadgeStyle::default_for_theme(context.theme, tone),
        |base, context| context.theme.components.badge.apply(base, context),
        |sheet, base, context, visual| sheet.apply_badge(base, context, visual),
        |sheet, base, context, visual, state| sheet.apply_badge_state(base, context, visual, state),
    )
}

fn resolve_badge_style_for_layout(
    style: Option<&StyleResolver<BadgeStyle>>,
    tone: BadgeTone,
) -> BadgeStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_badge_style(style, &context, tone)
}
