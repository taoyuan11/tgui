use crate::foundation::view_model::Command;
use crate::media::{ContentFit, MediaSource};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Axis, LayoutStyle, Value};

use super::badge::Badge;
use super::common::VisualStyle;
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{
    AvatarShape, AvatarStyle, ContainerStyle, ImageStyle, StyleResolver, StyleSheet,
    TextWidgetStyle,
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
    visual: VisualStyle,
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
            visual: VisualStyle::default(),
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
        let visual_identity = avatar.visual.clone();
        let content: Element<VM> = match avatar.source {
            AvatarSource::Image(source) => {
                let style = avatar.style.clone();
                let shape = avatar.shape;
                with_visual_identity(
                    Image::new(source)
                        .runtime_layout({
                            let style = avatar.style.clone();
                            move |layout, context, style_sheet, visual| {
                                let resolved = resolve_avatar_style_with_sheet(
                                    style.as_ref(),
                                    context,
                                    style_sheet,
                                    visual,
                                    WidgetState::default(),
                                    shape,
                                );
                                if layout.width.is_none() {
                                    layout.width = Some(Value::Static(
                                        crate::ui::layout::Length::Px(resolved.size),
                                    ));
                                }
                                if layout.height.is_none() {
                                    layout.height = Some(Value::Static(
                                        crate::ui::layout::Length::Px(resolved.size),
                                    ));
                                }
                            }
                        })
                        .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                            let resolved = resolve_avatar_style_with_sheet(
                                style.as_ref(),
                                context,
                                style_sheet,
                                visual,
                                state,
                                shape,
                            );
                            let mut image = ImageStyle::default_for_theme(context.theme);
                            image.fit = ContentFit::Cover;
                            image.surface.border_radius = Some(Value::Static(resolved.radius));
                            image
                        })
                        .into(),
                    &visual_identity,
                )
            }
            AvatarSource::Initials(initials) => avatar_initials(
                initials,
                avatar.style.clone(),
                avatar.shape,
                avatar.visual.clone(),
            ),
            AvatarSource::Name(name) => {
                let initials = match name {
                    Value::Static(name) => Value::Static(initials_from_name(&name)),
                    Value::Signal(signal) => signal.map(|name| initials_from_name(&name)).into(),
                };
                avatar_initials(
                    initials,
                    avatar.style.clone(),
                    avatar.shape,
                    avatar.visual.clone(),
                )
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
        root = with_visual_identity(root, &avatar.visual);
        root.layout = merge_layout(root.layout, avatar.layout);
        root
    }
}

fn avatar_initials<VM: 'static>(
    initials: Value<String>,
    style: Option<StyleResolver<AvatarStyle>>,
    shape: AvatarShape,
    visual_identity: VisualStyle,
) -> Element<VM> {
    let container_style = style.clone();
    let layout_style = style.clone();
    let text_style = style;
    let text_identity = visual_identity.clone();
    with_visual_identity(
        Stack::new()
            .runtime_layout(move |layout, _container, context, style_sheet, visual| {
                let resolved = resolve_avatar_style_with_sheet(
                    layout_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                    shape,
                );
                if layout.width.is_none() {
                    layout.width =
                        Some(Value::Static(crate::ui::layout::Length::Px(resolved.size)));
                }
                if layout.height.is_none() {
                    layout.height =
                        Some(Value::Static(crate::ui::layout::Length::Px(resolved.size)));
                }
            })
            .center()
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_avatar_style_with_sheet(
                    container_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                    shape,
                );
                let mut container = ContainerStyle::default_for_theme(context.theme);
                container.surface.background = Some(resolved.background);
                container.surface.border_radius = Some(Value::Static(resolved.radius));
                container
            })
            .child(with_visual_identity(
                Text::new(initials)
                    .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                        let resolved = resolve_avatar_style_with_sheet(
                            text_style.as_ref(),
                            context,
                            style_sheet,
                            visual,
                            state,
                            shape,
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
    visual: VisualStyle,
    key: Option<WidgetKey>,
}

impl<VM> AvatarGroup<VM> {
    pub fn new(avatars: Vec<Avatar<VM>>) -> Self {
        Self {
            avatars,
            max_visible: usize::MAX,
            style: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
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
        let total = group.avatars.len();
        let visible = group.max_visible.min(total);
        let mut children = group
            .avatars
            .into_iter()
            .take(visible)
            .map(Element::from)
            .collect::<Vec<_>>();
        if total > visible {
            let mut overflow = Avatar::initials(format!("+{}", total - visible))
                .style_full({
                    let style = group.style.clone();
                    move |context| {
                        resolve_avatar_style(style.as_ref(), context, AvatarShape::Circle)
                    }
                })
                .classes(group.visual.classes.clone());
            if let Some(style_id) = group.visual.style_id.clone() {
                overflow = overflow.style_id(style_id);
            }
            children.push(overflow.into());
        }
        let runtime_style = group.style.clone();
        let mut root: Element<VM> = Flex::new(Axis::Horizontal)
            .gap(crate::ui::unit::dp(0.0))
            .runtime_layout(move |_layout, container, context, style_sheet, visual| {
                let resolved = resolve_avatar_style_with_sheet(
                    runtime_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                    AvatarShape::Circle,
                );
                container.gap =
                    Value::Static(crate::ui::layout::Length::Px(-resolved.group_overlap));
            })
            .child(children)
            .into();
        root.key = group.key;
        root = with_visual_identity(root, &group.visual);
        root.layout = merge_layout(root.layout, group.layout);
        root
    }
}

fn resolve_avatar_style(
    style: Option<&StyleResolver<AvatarStyle>>,
    context: &StyleContext<'_>,
    shape: AvatarShape,
) -> AvatarStyle {
    let style_sheet = StyleSheet::default();
    resolve_avatar_style_with_sheet(
        style,
        context,
        &style_sheet,
        &VisualStyle::default(),
        WidgetState::default(),
        shape,
    )
}

fn resolve_avatar_style_with_sheet(
    style: Option<&StyleResolver<AvatarStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
    shape: AvatarShape,
) -> AvatarStyle {
    resolve_component_style_with_sheet(
        style,
        context,
        style_sheet,
        visual,
        state,
        AvatarStyle::default_for_theme(context.theme, shape),
        |base, context| context.theme.components.avatar.apply(base, context),
        |sheet, base, context, visual| sheet.apply_avatar(base, context, visual),
        |sheet, base, context, visual, state| {
            sheet.apply_avatar_state(base, context, visual, state)
        },
    )
}
