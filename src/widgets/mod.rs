//! Built-in widget namespace.
//!
//! Standard controls must flow through Widget → Element → Layout → Render and
//! may never be backed by [`crate::native`] hosts.

use crate::accessibility::{ActionKind, Role, Semantics};
use crate::core::{ImageHandle, PropertyId, Result, Size, WidgetKey};
use crate::event::EventHandler;
use crate::layout::{
    AvailableDimension, LayoutStyle, MeasureHandle, MeasureInput, MeasureOutput, MeasureSpec,
};
use crate::state::UiCommand;
use crate::widget::{
    BuildContext, LAYOUT_HEIGHT, LAYOUT_WIDTH, OPACITY, PropertyImpact, Widget, WidgetNode,
    WidgetType,
};
use std::sync::Arc;

pub const TEXT_CONTENT: PropertyId = PropertyId::new(1);
pub const BUTTON_ENABLED: PropertyId = PropertyId::new(1);
pub const IMAGE_RESOURCE_SLOT: PropertyId = PropertyId::new(2);
pub const IMAGE_RESOURCE_GENERATION: PropertyId = PropertyId::new(3);
pub const IMAGE_ALT_TEXT: PropertyId = PropertyId::new(4);

pub fn container_type() -> WidgetType {
    WidgetType::of::<Container>()
}

pub fn text_type() -> WidgetType {
    WidgetType::of::<Text>()
}

pub fn button_type() -> WidgetType {
    WidgetType::of::<Button>()
}

pub fn image_type() -> WidgetType {
    WidgetType::of::<Image>()
}

/// Layout-neutral grouping declaration used by every backend.
#[derive(Clone, Debug, Default)]
pub struct Container {
    key: Option<WidgetKey>,
    children: Vec<WidgetNode>,
}

impl Container {
    pub const fn new() -> Self {
        Self {
            key: None,
            children: Vec::new(),
        }
    }

    pub fn with_key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn with_child(mut self, child: WidgetNode) -> Self {
        self.children.push(child);
        self
    }

    pub fn with_children(mut self, children: impl IntoIterator<Item = WidgetNode>) -> Self {
        self.children.extend(children);
        self
    }
}

impl Widget for Container {
    fn build(&self, _context: &mut BuildContext) -> Result<WidgetNode> {
        Ok(
            with_layout_animation_properties(WidgetNode::from_type(container_type()))
                .with_optional_key(self.key.clone())
                .with_semantics(Semantics::new(Role::Group))
                .with_children(self.children.clone()),
        )
    }
}

/// Backend-neutral text declaration shaped by [`crate::text::TextSystem`].
#[derive(Clone, Debug)]
pub struct Text {
    key: Option<WidgetKey>,
    content: Arc<str>,
}

impl Text {
    pub fn new(content: impl Into<Arc<str>>) -> Self {
        Self {
            key: None,
            content: content.into(),
        }
    }

    pub fn with_key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

impl Widget for Text {
    fn build(&self, _context: &mut BuildContext) -> Result<WidgetNode> {
        let content = Arc::clone(&self.content);
        let content_generation = content
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
            });
        let measure_content = Arc::clone(&content);
        let measure = MeasureHandle::text(move |input: MeasureInput| {
            let style = crate::text::TextStyle::default();
            #[cfg(feature = "text")]
            {
                let mut text_system = crate::text::TextSystem::new();
                let mut request = crate::text::TextRequest::new(measure_content.clone(), style)
                    .with_dpi(input.scale)
                    .with_wrap(crate::text::WrapStrategy::WordOrGlyph);
                if let AvailableDimension::Definite(width) = input.available_space.width {
                    request = request.with_width(width.max(0.0));
                }
                let layout = text_system.layout(&request)?;
                let metrics = layout.measure();
                Ok(MeasureOutput::new(metrics.size)
                    .with_baseline(metrics.first_baseline.unwrap_or(0.0)))
            }
            #[cfg(not(feature = "text"))]
            {
                let advance = style.font_size * 0.6;
                let intrinsic_width = measure_content.chars().count() as f32 * advance;
                let width = match input.available_space.width {
                    AvailableDimension::Definite(value) if value.is_finite() => {
                        intrinsic_width.min(value.max(0.0))
                    }
                    _ => intrinsic_width,
                };
                let lines = if width > 0.0 {
                    (intrinsic_width / width.max(advance)).ceil().max(1.0)
                } else {
                    1.0
                };
                Ok(
                    MeasureOutput::new(Size::new(width.max(0.0), style.line_height * lines))
                        .with_baseline(style.font_size),
                )
            }
        });
        Ok(
            with_presentation_properties(WidgetNode::from_type(text_type()))
                .with_optional_key(self.key.clone())
                .with_property(TEXT_CONTENT, self.content.clone())
                .with_property_impact(TEXT_CONTENT, PropertyImpact::LAYOUT)
                .with_measure(
                    MeasureSpec::new(measure)
                        .with_content_generation(content_generation)
                        .with_font_generation(0),
                )
                // Accessibility reads logical text, never glyph-atlas output.
                .with_semantics(Semantics::text(self.content.clone())),
        )
    }
}

/// Backend-neutral image declaration. The handle is generation stamped and is
/// resolved by the media/resource pipeline when the scene is submitted.
#[derive(Clone, Debug)]
pub struct Image {
    key: Option<WidgetKey>,
    handle: ImageHandle,
    alt_text: Arc<str>,
    size: Size,
}

impl Image {
    pub fn new(handle: ImageHandle) -> Self {
        Self {
            key: None,
            handle,
            alt_text: Arc::from("Image"),
            size: Size::new(64.0, 64.0),
        }
    }

    pub fn with_key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn with_alt_text(mut self, alt_text: impl Into<Arc<str>>) -> Self {
        self.alt_text = alt_text.into();
        self
    }

    pub const fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl Widget for Image {
    fn build(&self, _context: &mut BuildContext) -> Result<WidgetNode> {
        let mut node = with_presentation_properties(WidgetNode::from_type(image_type()))
            .with_optional_key(self.key.clone())
            .with_layout_style(LayoutStyle::default().with_size(
                crate::layout::Dimension::Length(self.size.width),
                crate::layout::Dimension::Length(self.size.height),
            ))
            .with_property(IMAGE_RESOURCE_SLOT, u64::from(self.handle.slot()))
            .with_property(
                IMAGE_RESOURCE_GENERATION,
                u64::from(self.handle.generation()),
            )
            .with_property(IMAGE_ALT_TEXT, self.alt_text.clone())
            .with_property_impact(
                IMAGE_RESOURCE_SLOT,
                PropertyImpact::RESOURCE.union(PropertyImpact::PAINT),
            )
            .with_property_impact(
                IMAGE_RESOURCE_GENERATION,
                PropertyImpact::RESOURCE.union(PropertyImpact::PAINT),
            )
            .with_property_impact(IMAGE_ALT_TEXT, PropertyImpact::SEMANTICS)
            .with_semantics(
                Semantics::new(Role::Image)
                    .with_name(self.alt_text.clone())
                    .with_enabled(true),
            );
        if self.handle.generation() == 0 {
            node = node.with_enabled(false);
        }
        Ok(node)
    }
}

/// Minimal standard button declaration.
///
/// It is represented by the same immutable node pipeline as every other
/// widget; there is no native-control shortcut.
#[derive(Clone, Debug)]
pub struct Button {
    key: Option<WidgetKey>,
    label: Arc<str>,
    enabled: bool,
    handler: Option<EventHandler<UiCommand>>,
}

impl Button {
    pub fn new(label: impl Into<Arc<str>>) -> Self {
        Self {
            key: None,
            label: label.into(),
            enabled: true,
            handler: None,
        }
    }

    pub fn with_key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_event_handler(mut self, handler: EventHandler<UiCommand>) -> Self {
        self.handler = Some(handler);
        self
    }
}

impl Widget for Button {
    fn build(&self, context: &mut BuildContext) -> Result<WidgetNode> {
        let label = Text::new(self.label.clone()).build(context)?;
        let mut node = with_presentation_properties(WidgetNode::from_type(button_type()))
            .with_optional_key(self.key.clone())
            .with_property(BUTTON_ENABLED, self.enabled)
            .with_property_impact(
                BUTTON_ENABLED,
                PropertyImpact::PAINT
                    .union(PropertyImpact::HIT_TEST)
                    .union(PropertyImpact::SEMANTICS),
            )
            .with_focusable(true)
            .with_enabled(self.enabled)
            .with_semantics(
                Semantics::new(Role::Button)
                    .with_name(self.label.clone())
                    .with_enabled(self.enabled)
                    .with_focusable(true)
                    .with_actions([ActionKind::Activate, ActionKind::Focus]),
            )
            .with_child(label);
        if let Some(handler) = self.handler.clone() {
            node = node.with_event_handler(handler);
        }
        Ok(node)
    }
}

fn with_presentation_properties(node: WidgetNode) -> WidgetNode {
    with_layout_animation_properties(node)
        .with_property(OPACITY, 1.0_f32)
        .with_property_impact(OPACITY, PropertyImpact::PAINT)
}

fn with_layout_animation_properties(node: WidgetNode) -> WidgetNode {
    node.with_property_impact(LAYOUT_WIDTH, PropertyImpact::LAYOUT)
        .with_property_impact(LAYOUT_HEIGHT, PropertyImpact::LAYOUT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_share_the_widget_node_pipeline() {
        let mut context = BuildContext::new();
        let text = Text::new("hello").build(&mut context).unwrap();
        let button = Button::new("press").build(&mut context).unwrap();
        let image = Image::new(ImageHandle::from_parts(1, 1))
            .with_alt_text("preview")
            .build(&mut context)
            .unwrap();
        let container = Container::new()
            .with_child(text.clone())
            .with_child(button.clone())
            .build(&mut context)
            .unwrap();

        assert_eq!(text.widget_type(), &text_type());
        assert_eq!(button.widget_type(), &button_type());
        assert_eq!(button.children()[0].widget_type(), &text_type());
        assert_eq!(image.widget_type(), &image_type());
        assert_eq!(image.semantics().name(), Some("preview"));
        assert_eq!(container.children(), [text, button]);
    }
}
