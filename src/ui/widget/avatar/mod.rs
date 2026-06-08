use crate::foundation::view_model::Command;
use crate::media::{ContentFit, MediaSource};
use crate::theme::StyleContext;
use crate::ui::layout::{Axis, LayoutStyle, Value};
use crate::ui::theme::Theme;

use super::badge::Badge;
use super::core::Element;
use super::p3_support::{impl_p3_layout_api, merge_layout};
use super::style::{
    AvatarShape, AvatarStyle, ContainerStyle, ImageStyle, StyleResolver, TextWidgetStyle,
};
use super::{CursorStyle, Flex, Image, Stack, Text, WidgetKey};

#[derive(Clone)]
pub enum AvatarSource {
    Image(Value<MediaSource>),
    Initials(Value<String>),
    Name(Value<String>),
}

#[derive(Clone)]
pub struct Avatar<VM> {
    source: AvatarSource,
    shape: AvatarShape,
    badge: Option<Badge<VM>>,
    on_click: Option<Command<VM>>,
    style: Option<StyleResolver<AvatarStyle>>,
    layout: LayoutStyle,
    key: Option<WidgetKey>,
}

impl<VM> Avatar<VM> {
    pub fn image(source: impl Into<Value<MediaSource>>) -> Self {
        Self::new(AvatarSource::Image(source.into()))
    }

    pub fn initials(initials: impl Into<Value<String>>) -> Self {
        Self::new(AvatarSource::Initials(initials.into()))
    }

    pub fn name(name: impl Into<Value<String>>) -> Self {
        Self::new(AvatarSource::Name(name.into()))
    }

    fn new(source: AvatarSource) -> Self {
        Self {
            source,
            shape: AvatarShape::Circle,
            badge: None,
            on_click: None,
            style: None,
            layout: LayoutStyle::default(),
            key: None,
        }
    }

    pub fn shape(mut self, shape: AvatarShape) -> Self {
        self.shape = shape;
        self
    }

    pub fn badge(mut self, badge: Badge<VM>) -> Self {
        self.badge = Some(badge);
        self
    }

    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.on_click = Some(command);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut AvatarStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| AvatarStyle::default_for_theme(context.theme, AvatarShape::Circle),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> AvatarStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<Avatar<VM>> for Element<VM> {
    fn from(avatar: Avatar<VM>) -> Self {
        let layout_style = resolve_avatar_style_for_layout(avatar.style.as_ref(), avatar.shape);
        let content: Element<VM> = match avatar.source {
            AvatarSource::Image(source) => {
                let style = avatar.style.clone();
                let shape = avatar.shape;
                Image::new(source)
                    .size(layout_style.size, layout_style.size)
                    .style_full(move |context| {
                        let resolved = resolve_avatar_style(style.as_ref(), context, shape);
                        let mut image = ImageStyle::default_for_theme(context.theme);
                        image.fit = ContentFit::Cover;
                        image.surface.border_radius = Some(Value::Static(resolved.radius));
                        image
                    })
                    .into()
            }
            AvatarSource::Initials(initials) => {
                avatar_initials(initials, avatar.style.clone(), avatar.shape)
            }
            AvatarSource::Name(name) => {
                let initials = match name {
                    Value::Static(name) => Value::Static(initials_from_name(&name)),
                    Value::Signal(signal) => signal.map(|name| initials_from_name(&name)).into(),
                };
                avatar_initials(initials, avatar.style.clone(), avatar.shape)
            }
        };
        let mut root = if let Some(badge) = avatar.badge {
            badge.attach(content).into()
        } else {
            content
        };
        if let Some(command) = avatar.on_click {
            root = root.on_click(command).focusable(true).tab_index(0);
            root.interactions.cursor_style = Some(Value::Static(CursorStyle::Pointer));
        }
        root.key = avatar.key;
        root.layout = merge_layout(root.layout, avatar.layout);
        root
    }
}

fn avatar_initials<VM: 'static>(
    initials: Value<String>,
    style: Option<StyleResolver<AvatarStyle>>,
    shape: AvatarShape,
) -> Element<VM> {
    let layout_style = resolve_avatar_style_for_layout(style.as_ref(), shape);
    let container_style = style.clone();
    let text_style = style;
    Stack::new()
        .size(layout_style.size, layout_style.size)
        .center()
        .style_full(move |context| {
            let resolved = resolve_avatar_style(container_style.as_ref(), context, shape);
            let mut container = ContainerStyle::default_for_theme(context.theme);
            container.surface.background = Some(resolved.background);
            container.surface.border_radius = Some(Value::Static(resolved.radius));
            container
        })
        .child(Text::new(initials).style_full(move |context| {
            let resolved = resolve_avatar_style(text_style.as_ref(), context, shape);
            TextWidgetStyle {
                surface: Default::default(),
                color: resolved.foreground,
                typography: resolved.text_style,
            }
        }))
        .into()
}

fn initials_from_name(name: &str) -> String {
    let mut initials = String::new();
    for part in name
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .take(2)
    {
        if let Some(ch) = part.chars().next() {
            initials.extend(ch.to_uppercase());
        }
    }
    if initials.is_empty() {
        "?".to_string()
    } else {
        initials
    }
}

pub struct AvatarGroup<VM> {
    avatars: Vec<Avatar<VM>>,
    max_visible: usize,
    style: Option<StyleResolver<AvatarStyle>>,
    layout: LayoutStyle,
    key: Option<WidgetKey>,
}

impl<VM> AvatarGroup<VM> {
    pub fn new(avatars: Vec<Avatar<VM>>) -> Self {
        Self {
            avatars,
            max_visible: usize::MAX,
            style: None,
            layout: LayoutStyle::default(),
            key: None,
        }
    }

    pub fn max_visible(mut self, max_visible: usize) -> Self {
        self.max_visible = max_visible.max(1);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut AvatarStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| AvatarStyle::default_for_theme(context.theme, AvatarShape::Circle),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> AvatarStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<AvatarGroup<VM>> for Element<VM> {
    fn from(group: AvatarGroup<VM>) -> Self {
        let layout_style =
            resolve_avatar_style_for_layout(group.style.as_ref(), AvatarShape::Circle);
        let total = group.avatars.len();
        let visible = group.max_visible.min(total);
        let mut children = group
            .avatars
            .into_iter()
            .take(visible)
            .map(Element::from)
            .collect::<Vec<_>>();
        if total > visible {
            children.push(
                Avatar::initials(format!("+{}", total - visible))
                    .style_full({
                        let style = group.style.clone();
                        move |context| {
                            resolve_avatar_style(style.as_ref(), context, AvatarShape::Circle)
                        }
                    })
                    .into(),
            );
        }
        let mut root: Element<VM> = Flex::new(Axis::Horizontal)
            .gap(-layout_style.group_overlap.get())
            .child(children)
            .into();
        root.key = group.key;
        root.layout = merge_layout(root.layout, group.layout);
        root
    }
}

fn resolve_avatar_style(
    style: Option<&StyleResolver<AvatarStyle>>,
    context: &StyleContext<'_>,
    shape: AvatarShape,
) -> AvatarStyle {
    let mut base = AvatarStyle::default_for_theme(context.theme, shape);
    context.theme.components.avatar.apply(&mut base, context);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

fn resolve_avatar_style_for_layout(
    style: Option<&StyleResolver<AvatarStyle>>,
    shape: AvatarShape,
) -> AvatarStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_avatar_style(style, &context, shape)
}
