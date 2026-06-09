use std::sync::Arc;

use crate::media::{MediaBytes, MediaSource};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{LayoutStyle, Length, Value};
use crate::ui::theme::Theme;
use crate::ui::unit::sp;

mod svg;

use super::common::VisualStyle;
use super::common::WidgetKind;
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{IconStyle, ImageStyle, StyleResolver, StyleSheet, TextWidgetStyle};
use super::{Image, Text, WidgetId, WidgetKey};

pub(crate) use svg::{push_svg_icon_texture, SvgIconId};

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
            Self::ChevronUp => "chevron_up",
            Self::ChevronDown => "chevron_down",
            Self::Close => "close",
            Self::Check => "check",
            Self::MoreHorizontal => "more_horizontal",
            Self::Search => "search",
            Self::Star => "star",
            Self::StarHalf => "star_half",
            Self::User => "user",
            Self::Image => "image",
            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Calendar => "calendar",
            Self::Clock => "clock",
        }
    }
}

#[derive(Clone)]
enum IconSourceKind {
    Public(IconSource),
    Internal(SvgIconId),
}

#[derive(Clone)]
pub(crate) struct BuiltinSvgIcon {
    pub(crate) source: SvgIconId,
    pub(crate) style: Option<StyleResolver<IconStyle>>,
}

pub struct Icon<VM> {
    source: IconSourceKind,
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

    #[deprecated(note = "font-based named icons were removed; use Icon::builtin or Icon::svg")]
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
            source: IconSourceKind::Public(source),
            style: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            key: None,
            _marker: std::marker::PhantomData,
        }
    }

    pub(crate) fn internal(source: SvgIconId) -> Self {
        Self {
            source: IconSourceKind::Internal(source),
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

    pub(crate) fn style_full_with_style_sheet(
        mut self,
        resolver: impl Fn(&StyleContext<'_>, &StyleSheet, &VisualStyle, WidgetState) -> IconStyle
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full_with_style_sheet(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<Icon<VM>> for Element<VM> {
    fn from(icon: Icon<VM>) -> Self {
        let layout_style = resolve_icon_style_for_layout(icon.style.as_ref());
        let mut root: Element<VM> = match icon.source {
            IconSourceKind::Internal(source) => icon_svg(source, icon.style.clone()),
            IconSourceKind::Public(IconSource::Builtin(icon_source)) => {
                icon_svg(icon_source.into(), icon.style.clone())
            }
            IconSourceKind::Public(IconSource::Named(name)) => {
                icon_text(name, icon.style.clone(), icon.visual.clone())
            }
            IconSourceKind::Public(IconSource::Glyph(ch)) => icon_text(
                Value::Static(ch.to_string()),
                icon.style.clone(),
                icon.visual.clone(),
            ),
            IconSourceKind::Public(IconSource::Svg(bytes)) => {
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

fn icon_svg<VM: 'static>(
    source: SvgIconId,
    style: Option<StyleResolver<IconStyle>>,
) -> Element<VM> {
    let layout_style = resolve_icon_style_for_layout(style.as_ref());
    let mut layout = LayoutStyle::default();
    layout.width = Some(Value::Static(Length::Px(layout_style.size)));
    layout.height = Some(Value::Static(Length::Px(layout_style.size)));
    Element {
        id: WidgetId::next(),
        key: None,
        layout,
        focus: Default::default(),
        visual: VisualStyle::default(),
        interactions: Default::default(),
        lifecycle_events: Default::default(),
        media_events: Default::default(),
        background: None,
        tooltip: None,
        popover: None,
        menu: None,
        context_menu: None,
        modal: None,
        drawer: None,
        tab_trigger: None,
        list_item: None,
        tree_root: None,
        tree_node: None,
        data_grid_root: None,
        data_grid_cell: None,
        data_grid_header: None,
        data_grid_resize_handle: None,
        splitter_handle: None,
        carousel_auto_play: None,
        kind: WidgetKind::Icon {
            icon: BuiltinSvgIcon { source, style },
        },
    }
}

fn icon_text<VM: 'static>(
    name: Value<String>,
    style: Option<StyleResolver<IconStyle>>,
    visual_identity: VisualStyle,
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

pub(crate) fn resolve_icon_style_with_sheet(
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
