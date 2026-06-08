use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::foundation::view_model::{Command, ValueCommand};
use crate::media::MediaSource;
use crate::text::font::FontWeight;
use crate::theme::StyleContext;
use crate::ui::layout::{Align, Insets, LayoutStyle, Value, Wrap};
use crate::ui::theme::Theme;
use crate::ui::unit::{dp, sp};

use super::core::Element;
use super::p3_support::{impl_p3_layout_api, merge_layout};
use super::style::{ContainerStyle, ImageStyle, RichTextStyle, StyleResolver, TextWidgetStyle};
use super::{CursorStyle, Flex, Image, Stack, Text, WidgetKey};

#[derive(Clone, Debug, PartialEq)]
pub struct RichTextLinkClick {
    pub href: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RichTextImage {
    pub source: MediaSource,
    pub alt: String,
}

pub struct RichText<VM> {
    markdown: Value<String>,
    on_link_click: Option<ValueCommand<VM, RichTextLinkClick>>,
    style: Option<StyleResolver<RichTextStyle>>,
    layout: LayoutStyle,
    key: Option<WidgetKey>,
}

impl<VM> RichText<VM> {
    pub fn markdown(markdown: impl Into<Value<String>>) -> Self {
        Self {
            markdown: markdown.into(),
            on_link_click: None,
            style: None,
            layout: LayoutStyle::default(),
            key: None,
        }
    }

    pub fn on_link_click(mut self, command: ValueCommand<VM, RichTextLinkClick>) -> Self {
        self.on_link_click = Some(command);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut RichTextStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| RichTextStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> RichTextStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<RichText<VM>> for Element<VM> {
    fn from(rich: RichText<VM>) -> Self {
        let layout_style = resolve_rich_text_style_for_layout(rich.style.as_ref());
        let blocks = markdown_blocks(&rich.markdown.resolve());
        let children = blocks
            .into_iter()
            .map(|block| rich_block_element(block, rich.style.clone(), rich.on_link_click.clone()))
            .collect::<Vec<_>>();
        let mut root: Element<VM> = Flex::vertical()
            .gap(layout_style.gap)
            .child(children)
            .into();
        root.key = rich.key;
        root.layout = merge_layout(root.layout, rich.layout);
        root
    }
}

#[derive(Clone, Debug, PartialEq)]
enum RichBlock {
    Paragraph(Vec<RichInline>),
    Heading(u8, Vec<RichInline>),
    Code(String),
    List {
        ordered: bool,
        items: Vec<Vec<RichInline>>,
    },
    Image(RichTextImage),
}

#[derive(Clone, Debug, PartialEq)]
enum RichInline {
    Text(String),
    Strong(String),
    Emphasis(String),
    Code(String),
    Link { href: String, label: String },
}

fn markdown_blocks(markdown: &str) -> Vec<RichBlock> {
    let parser = Parser::new_ext(markdown, Options::all());
    let mut blocks = Vec::new();
    let mut current: Option<RichBlock> = None;
    let mut strong = false;
    let mut emphasis = false;
    let mut link_href: Option<String> = None;
    let mut link_label = String::new();
    let mut image_href: Option<String> = None;
    let mut image_alt = String::new();
    let mut image_title = String::new();
    let mut list: Option<(bool, Vec<Vec<RichInline>>)> = None;

    for event in parser {
        match event {
            Event::Start(Tag::Paragraph) => {
                if current.is_none() {
                    current = Some(RichBlock::Paragraph(Vec::new()));
                }
            }
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some(RichBlock::Heading(heading_level(level), Vec::new()));
            }
            Event::Start(Tag::List(start)) => {
                list = Some((start.is_some(), Vec::new()));
            }
            Event::Start(Tag::Item) => {
                current = Some(RichBlock::Paragraph(Vec::new()));
            }
            Event::Start(Tag::Strong) => strong = true,
            Event::End(TagEnd::Strong) => strong = false,
            Event::Start(Tag::Emphasis) => emphasis = true,
            Event::End(TagEnd::Emphasis) => emphasis = false,
            Event::Start(Tag::Link { dest_url, .. }) => {
                link_href = Some(dest_url.to_string());
                link_label.clear();
            }
            Event::End(TagEnd::Link) => {
                if let Some(href) = link_href.take() {
                    let label = if link_label.is_empty() {
                        href.clone()
                    } else {
                        link_label.clone()
                    };
                    push_inline(&mut current, RichInline::Link { href, label });
                }
            }
            Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                image_href = Some(dest_url.to_string());
                image_title = title.to_string();
                image_alt.clear();
            }
            Event::End(TagEnd::Image) => {
                if let Some(href) = image_href.take() {
                    let alt = if image_alt.is_empty() {
                        image_title.clone()
                    } else {
                        image_alt.clone()
                    };
                    blocks.push(RichBlock::Image(RichTextImage {
                        source: MediaSource::url(href),
                        alt,
                    }));
                }
            }
            Event::Text(text) | Event::Html(text) => {
                if image_href.is_some() {
                    image_alt.push_str(&text);
                } else if link_href.is_some() {
                    link_label.push_str(&text);
                } else if strong {
                    push_inline(&mut current, RichInline::Strong(text.to_string()));
                } else if emphasis {
                    push_inline(&mut current, RichInline::Emphasis(text.to_string()));
                } else {
                    push_inline(&mut current, RichInline::Text(text.to_string()));
                }
            }
            Event::Code(code) => push_inline(&mut current, RichInline::Code(code.to_string())),
            Event::SoftBreak | Event::HardBreak => {
                push_inline(&mut current, RichInline::Text("\n".to_string()))
            }
            Event::End(TagEnd::Item) => {
                if let Some(block) = current.take() {
                    let inlines = match block {
                        RichBlock::Paragraph(items) | RichBlock::Heading(_, items) => items,
                        RichBlock::Code(text) => vec![RichInline::Code(text)],
                        _ => Vec::new(),
                    };
                    if let Some((_, items)) = &mut list {
                        items.push(inlines);
                    }
                }
            }
            Event::End(TagEnd::List(_)) => {
                if let Some((ordered, items)) = list.take() {
                    blocks.push(RichBlock::List { ordered, items });
                }
            }
            Event::End(TagEnd::Paragraph) | Event::End(TagEnd::Heading(_)) => {
                if list.is_none() {
                    if let Some(block) = current.take() {
                        blocks.push(block);
                    }
                }
            }
            Event::Start(Tag::CodeBlock(_)) => current = Some(RichBlock::Code(String::new())),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(block) = current.take() {
                    blocks.push(block);
                }
            }
            _ => {}
        }
    }
    if let Some(block) = current {
        blocks.push(block);
    }
    blocks
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn push_inline(current: &mut Option<RichBlock>, inline: RichInline) {
    if current.is_none() {
        *current = Some(RichBlock::Paragraph(Vec::new()));
    }
    match current {
        Some(RichBlock::Paragraph(items)) | Some(RichBlock::Heading(_, items)) => {
            items.push(inline)
        }
        Some(RichBlock::Code(text)) => match inline {
            RichInline::Text(value) | RichInline::Code(value) => text.push_str(&value),
            RichInline::Strong(value) | RichInline::Emphasis(value) => text.push_str(&value),
            RichInline::Link { label, .. } => text.push_str(&label),
        },
        _ => {}
    }
}

fn rich_block_element<VM: 'static>(
    block: RichBlock,
    style: Option<StyleResolver<RichTextStyle>>,
    on_link_click: Option<ValueCommand<VM, RichTextLinkClick>>,
) -> Element<VM> {
    match block {
        RichBlock::Paragraph(inlines) => rich_inline_row(inlines, style, on_link_click),
        RichBlock::Heading(level, inlines) => {
            rich_inline_row_with_heading(inlines, style, on_link_click, level)
        }
        RichBlock::Code(code) => {
            let container_style = style.clone();
            let text_style = style;
            Stack::new()
                .padding(Insets::all(dp(10.0)))
                .style_full(move |context| {
                    let resolved = resolve_rich_text_style(container_style.as_ref(), context);
                    let mut container = ContainerStyle::default_for_theme(context.theme);
                    container.surface.background = Some(resolved.code_background);
                    container.surface.border_radius = Some(Value::Static(dp(6.0)));
                    container
                })
                .child(
                    Text::new(code)
                        .user_select(true)
                        .style_full(move |context| {
                            let resolved = resolve_rich_text_style(text_style.as_ref(), context);
                            TextWidgetStyle {
                                surface: Default::default(),
                                color: resolved.code_foreground,
                                typography: resolved.code_text_style,
                            }
                        }),
                )
                .into()
        }
        RichBlock::List { ordered, items } => {
            let rows = items
                .into_iter()
                .enumerate()
                .map(|(index, inlines)| {
                    let marker = if ordered {
                        format!("{}.", index + 1)
                    } else {
                        "-".to_string()
                    };
                    Flex::horizontal()
                        .align(Align::Start)
                        .gap(dp(6.0))
                        .child(Text::new(marker).style_full(rich_text_style(style.clone())))
                        .child(rich_inline_row(
                            inlines,
                            style.clone(),
                            on_link_click.clone(),
                        ))
                        .into()
                })
                .collect::<Vec<Element<VM>>>();
            Flex::vertical().gap(dp(4.0)).child(rows).into()
        }
        RichBlock::Image(image) => Image::new(image.source)
            .height(dp(160.0))
            .style_full(|context| ImageStyle::default_for_theme(context.theme))
            .into(),
    }
}

fn rich_inline_row_with_heading<VM: 'static>(
    inlines: Vec<RichInline>,
    style: Option<StyleResolver<RichTextStyle>>,
    on_link_click: Option<ValueCommand<VM, RichTextLinkClick>>,
    level: u8,
) -> Element<VM> {
    let heading_style = style.clone();
    rich_inline_row_with_style(
        inlines,
        style,
        on_link_click,
        move |mut rich_style| {
            rich_style.text_style.size = match level {
                1 => sp(26.0),
                2 => sp(22.0),
                3 => sp(18.0),
                _ => rich_style.text_style.size,
            };
            rich_style.text_style.weight = FontWeight::SemiBold;
            rich_style
        },
        heading_style,
    )
}

fn rich_inline_row<VM: 'static>(
    inlines: Vec<RichInline>,
    style: Option<StyleResolver<RichTextStyle>>,
    on_link_click: Option<ValueCommand<VM, RichTextLinkClick>>,
) -> Element<VM> {
    rich_inline_row_with_style(inlines, style.clone(), on_link_click, |style| style, style)
}

fn rich_inline_row_with_style<VM: 'static>(
    inlines: Vec<RichInline>,
    style: Option<StyleResolver<RichTextStyle>>,
    on_link_click: Option<ValueCommand<VM, RichTextLinkClick>>,
    mapper: impl Fn(RichTextStyle) -> RichTextStyle + Clone + Send + Sync + 'static,
    link_style: Option<StyleResolver<RichTextStyle>>,
) -> Element<VM> {
    let children = inlines
        .into_iter()
        .map(|inline| match inline {
            RichInline::Text(text) => Text::new(text)
                .user_select(true)
                .style_full(rich_text_style_mapped(style.clone(), mapper.clone()))
                .into(),
            RichInline::Strong(text) => {
                let mapper = mapper.clone();
                let style = style.clone();
                Text::new(text)
                    .user_select(true)
                    .style_full(move |context| {
                        let mut resolved = mapper(resolve_rich_text_style(style.as_ref(), context));
                        resolved.text_style.weight = FontWeight::Bold;
                        TextWidgetStyle {
                            surface: Default::default(),
                            color: resolved.foreground,
                            typography: resolved.text_style,
                        }
                    })
                    .into()
            }
            RichInline::Emphasis(text) => Text::new(text)
                .user_select(true)
                .style_full(rich_text_style_mapped(style.clone(), mapper.clone()))
                .into(),
            RichInline::Code(text) => Text::new(text)
                .user_select(true)
                .style_full({
                    let style = style.clone();
                    move |context| {
                        let resolved = resolve_rich_text_style(style.as_ref(), context);
                        TextWidgetStyle {
                            surface: Default::default(),
                            color: resolved.code_foreground,
                            typography: resolved.code_text_style,
                        }
                    }
                })
                .into(),
            RichInline::Link { href, label } => {
                let command = on_link_click.clone();
                Text::new(label.clone())
                    .cursor(CursorStyle::Pointer)
                    .style_full({
                        let style = link_style.clone();
                        move |context| {
                            let resolved = resolve_rich_text_style(style.as_ref(), context);
                            TextWidgetStyle {
                                surface: Default::default(),
                                color: resolved.link,
                                typography: resolved.text_style,
                            }
                        }
                    })
                    .on_click(Command::new_with_context(move |vm, context| {
                        if let Some(command) = command.as_ref() {
                            command.execute_with_context(
                                vm,
                                RichTextLinkClick {
                                    href: href.clone(),
                                    label: label.clone(),
                                },
                                context,
                            );
                        }
                    }))
                    .focusable(true)
                    .tab_index(0)
            }
        })
        .collect::<Vec<Element<VM>>>();
    Flex::horizontal()
        .wrap(Wrap::Wrap)
        .gap(dp(2.0))
        .child(children)
        .into()
}

fn rich_text_style(
    style: Option<StyleResolver<RichTextStyle>>,
) -> impl Fn(&StyleContext<'_>) -> TextWidgetStyle + Send + Sync + 'static {
    rich_text_style_mapped(style, |style| style)
}

fn rich_text_style_mapped(
    style: Option<StyleResolver<RichTextStyle>>,
    mapper: impl Fn(RichTextStyle) -> RichTextStyle + Clone + Send + Sync + 'static,
) -> impl Fn(&StyleContext<'_>) -> TextWidgetStyle + Send + Sync + 'static {
    move |context| {
        let resolved = mapper(resolve_rich_text_style(style.as_ref(), context));
        TextWidgetStyle {
            surface: Default::default(),
            color: resolved.foreground,
            typography: resolved.text_style,
        }
    }
}

fn resolve_rich_text_style(
    style: Option<&StyleResolver<RichTextStyle>>,
    context: &StyleContext<'_>,
) -> RichTextStyle {
    let mut base = RichTextStyle::default_for_theme(context.theme);
    context.theme.components.rich_text.apply(&mut base, context);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

fn resolve_rich_text_style_for_layout(
    style: Option<&StyleResolver<RichTextStyle>>,
) -> RichTextStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_rich_text_style(style, &context)
}
