use std::sync::Arc;

use crate::media::{MediaBytes, MediaSource};
use crate::text::font::ICON_FONT_FAMILY;
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{LayoutStyle, Value};
use crate::ui::theme::Theme;
use crate::ui::unit::sp;

use super::common::VisualStyle;
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{IconStyle, ImageStyle, StyleResolver, StyleSheet, TextWidgetStyle};
use super::{Image, Text, WidgetKey};

#[derive(Clone, Debug, PartialEq)]
pub enum IconSource {
    Builtin(BuiltinIcon),
    Named(Value<String>),
    Glyph(char),
    Svg(MediaBytes),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinIcon {
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronDown,
    Close,
    Check,
    MoreHorizontal,
    Search,
    Star,
    StarHalf,
    User,
    Image,
    Plus,
    Minus,
    Info,
    Success,
    Warning,
    Error,
    Calendar,
    Clock,
}

impl BuiltinIcon {
    pub fn name(self) -> &'static str {
        match self {
            Self::ChevronLeft => "chevron_left",
            Self::ChevronRight => "chevron_right",
            Self::ChevronUp => "keyboard_arrow_up",
            Self::ChevronDown => "keyboard_arrow_down",
            Self::Close => "close",
            Self::Check => "check",
            Self::MoreHorizontal => "more_horiz",
            Self::Search => "search",
            Self::Star => "star",
            Self::StarHalf => "star_half",
            Self::User => "person",
            Self::Image => "image",
            Self::Plus => "add",
            Self::Minus => "remove",
            Self::Info => "info",
            Self::Success => "check_circle",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Calendar => "calendar_today",
            Self::Clock => "schedule",
        }
    }
}

pub struct Icon<VM> {
    source: IconSource,
    style: Option<StyleResolver<IconStyle>>,
    layout: LayoutStyle,
    visual: VisualStyle,
    key: Option<WidgetKey>,
    _marker: std::marker::PhantomData<fn() -> VM>,
}

impl<VM> Icon<VM> {
    pub fn builtin(icon: BuiltinIcon) -> Self {
        Self::new(IconSource::Builtin(icon))
    }

    pub fn named(name: impl Into<Value<String>>) -> Self {
        Self::new(IconSource::Named(name.into()))
    }

    pub fn glyph(ch: char) -> Self {
        Self::new(IconSource::Glyph(ch))
    }

    pub fn svg(bytes: &'static [u8]) -> Self {
        Self::new(IconSource::Svg(MediaBytes::from_static(bytes)))
    }

    pub fn svg_owned(bytes: Vec<u8>) -> Self {
        Self::new(IconSource::Svg(MediaBytes::from_shared(Arc::from(
            bytes.into_boxed_slice(),
        ))))
    }

    fn new(source: IconSource) -> Self {
        Self {
            source,
            style: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            key: None,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut IconStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| IconStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> IconStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<Icon<VM>> for Element<VM> {
    fn from(icon: Icon<VM>) -> Self {
        let layout_style = resolve_icon_style_for_layout(icon.style.as_ref());
        let mut root: Element<VM> = match icon.source {
            IconSource::Builtin(icon_source) => icon_text(
                Value::Static(icon_source.name().to_string()),
                icon.style.clone(),
                icon.visual.clone(),
                true,
            ),
            IconSource::Named(name) => {
                icon_text(name, icon.style.clone(), icon.visual.clone(), true)
            }
            IconSource::Glyph(ch) => icon_text(
                Value::Static(ch.to_string()),
                icon.style.clone(),
                icon.visual.clone(),
                false,
            ),
            IconSource::Svg(bytes) => {
                let style = icon.style.clone();
                with_visual_identity(
                    Image::new(MediaSource::bytes(bytes))
                        .size(layout_style.size, layout_style.size)
                        .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                            let _ = resolve_icon_style_with_sheet(
                                style.as_ref(),
                                context,
                                style_sheet,
                                visual,
                                state,
                            );
                            ImageStyle::default_for_theme(context.theme)
                        })
                        .into(),
                    &icon.visual,
                )
            }
        };
        root.key = icon.key;
        root = with_visual_identity(root, &icon.visual);
        root.layout = merge_layout(root.layout, icon.layout);
        root
    }
}

fn icon_text<VM: 'static>(
    name: Value<String>,
    style: Option<StyleResolver<IconStyle>>,
    visual_identity: VisualStyle,
    icon_font: bool,
) -> Element<VM> {
    with_visual_identity(
        Text::new(name)
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_icon_style_with_sheet(
                    style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                );
                let mut text = TextWidgetStyle::default_for_theme(context.theme);
                text.color = resolved.color;
                text.typography.size = sp(resolved.size.get());
                text.typography.line_height = Some(sp(resolved.size.get()));
                if icon_font {
                    text.typography.font_family = Some(ICON_FONT_FAMILY.to_string());
                }
                text
            })
            .into(),
        &visual_identity,
    )
}

fn resolve_icon_style(
    style: Option<&StyleResolver<IconStyle>>,
    context: &StyleContext<'_>,
) -> IconStyle {
    let style_sheet = StyleSheet::default();
    resolve_icon_style_with_sheet(
        style,
        context,
        &style_sheet,
        &VisualStyle::default(),
        WidgetState::default(),
    )
}

fn resolve_icon_style_with_sheet(
    style: Option<&StyleResolver<IconStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> IconStyle {
    resolve_component_style_with_sheet(
        style,
        context,
        style_sheet,
        visual,
        state,
        IconStyle::default_for_theme(context.theme),
        |base, context| context.theme.components.icon.apply(base, context),
        |sheet, base, context, visual| sheet.apply_icon(base, context, visual),
        |sheet, base, context, visual, state| sheet.apply_icon_state(base, context, visual, state),
    )
}

fn resolve_icon_style_for_layout(style: Option<&StyleResolver<IconStyle>>) -> IconStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_icon_style(style, &context)
}
