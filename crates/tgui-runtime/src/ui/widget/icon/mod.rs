use std::sync::Arc;

use crate::media::{MediaBytes, MediaSource};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Align, Insets, LayoutStyle, Length, PositionType, Value};
use crate::ui::unit::sp;

mod svg;

use super::common::VisualStyle;
use super::common::WidgetKind;
use super::container::{set_layout_inset, set_layout_length, IntoLengthValue};
use super::core::Element;
use super::p3_support::{merge_layout, resolve_component_style_with_sheet, with_visual_identity};
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
    Home,
    Settings,
    Bell,
    Mail,
    Lock,
    Unlock,
    Eye,
    EyeOff,
    Edit,
    Copy,
    Download,
    Upload,
    File,
    Folder,
    Trash,
    RefreshCw,
    ExternalLink,
    Menu,
    Filter,
    SortAsc,
    SortDesc,
    Play,
    Pause,
    VolumeUp,
    VolumeDown,
    VolumeOff,
    Palette,
    MapPin,
    Link,
    Heart,
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
            Self::Home => "home",
            Self::Settings => "settings",
            Self::Bell => "bell",
            Self::Mail => "mail",
            Self::Lock => "lock",
            Self::Unlock => "unlock",
            Self::Eye => "eye",
            Self::EyeOff => "eye_off",
            Self::Edit => "edit",
            Self::Copy => "copy",
            Self::Download => "download",
            Self::Upload => "upload",
            Self::File => "file",
            Self::Folder => "folder",
            Self::Trash => "trash",
            Self::RefreshCw => "refresh_cw",
            Self::ExternalLink => "external_link",
            Self::Menu => "menu",
            Self::Filter => "filter",
            Self::SortAsc => "sort_asc",
            Self::SortDesc => "sort_desc",
            Self::Play => "play",
            Self::Pause => "pause",
            Self::VolumeUp => "volume_up",
            Self::VolumeDown => "volume_down",
            Self::VolumeOff => "volume_off",
            Self::Palette => "palette",
            Self::MapPin => "map_pin",
            Self::Link => "link",
            Self::Heart => "heart",
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

    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.visual.classes.push(class.into());
        self
    }

    pub fn classes<I, S>(mut self, classes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.visual
            .classes
            .extend(classes.into_iter().map(Into::into));
        self
    }

    pub fn style_id(mut self, style_id: impl Into<String>) -> Self {
        self.visual.style_id = Some(style_id.into());
        self
    }

    pub fn size(mut self, size: impl IntoLengthValue) -> Self {
        let size = size.into_length_value();
        self.layout.width = Some(size.clone());
        self.layout.height = Some(size);
        self
    }

    pub fn width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.width, width);
        self
    }

    pub fn height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.height, height);
        self
    }

    pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.min_width, width);
        self
    }

    pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.min_height, height);
        self
    }

    pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.max_width, width);
        self
    }

    pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.max_height, height);
        self
    }

    pub fn aspect_ratio(mut self, aspect_ratio: impl Into<Value<f32>>) -> Self {
        self.layout.aspect_ratio = Some(aspect_ratio.into());
        self
    }

    pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
        self.layout.margin = insets.into();
        self
    }

    pub fn padding(mut self, insets: impl Into<Value<Insets>>) -> Self {
        self.layout.padding = Some(insets.into());
        self
    }

    pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
        self.layout.grow = grow.into();
        self
    }

    pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
        self.layout.shrink = shrink.into();
        self
    }

    pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
        self.layout.basis = Some(basis.into_length_value());
        self
    }

    pub fn align_self(mut self, align: Align) -> Self {
        self.layout.align_self = Some(align);
        self
    }

    pub fn justify_self(mut self, align: Align) -> Self {
        self.layout.justify_self = Some(align);
        self
    }

    pub fn position_absolute(mut self) -> Self {
        self.layout.position_type = PositionType::Absolute;
        self
    }

    pub fn left(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.layout.left, value);
        self
    }

    pub fn top(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.layout.top, value);
        self
    }

    pub fn right(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.layout.right, value);
        self
    }

    pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.layout.bottom, value);
        self
    }

    pub fn inset(mut self, value: impl IntoLengthValue) -> Self {
        let value = value.into_length_value();
        self.layout.left = Some(value.clone());
        self.layout.top = Some(value.clone());
        self.layout.right = Some(value.clone());
        self.layout.bottom = Some(value);
        self
    }
}

impl<VM: 'static> From<Icon<VM>> for Element<VM> {
    fn from(icon: Icon<VM>) -> Self {
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
                        .runtime_layout({
                            let style = style.clone();
                            move |layout, context, style_sheet, visual| {
                                let resolved = resolve_icon_style_with_sheet(
                                    style.as_ref(),
                                    context,
                                    style_sheet,
                                    visual,
                                    WidgetState::default(),
                                );
                                if layout.width.is_none() {
                                    layout.width = Some(Value::Static(Length::Px(resolved.size)));
                                }
                                if layout.height.is_none() {
                                    layout.height = Some(Value::Static(Length::Px(resolved.size)));
                                }
                            }
                        })
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
    Element {
        id: WidgetId::next(),
        key: None,
        // Icon geometry is resolved from the active theme during element
        // resolution. Keeping this empty here avoids freezing the default
        // theme into a retained tree; explicit `.size/.width/.height` values
        // are merged by the caller and therefore retain precedence.
        layout: LayoutStyle::default(),
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
