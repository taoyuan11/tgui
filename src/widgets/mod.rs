//! Built-in widget namespace.
//!
//! Standard controls must flow through Widget → Element → Layout → Render and
//! may never be backed by [`crate::native`] hosts.

use crate::core::{PropertyId, Result, WidgetKey};
use crate::event::EventHandler;
use crate::state::UiCommand;
use crate::widget::{BuildContext, Widget, WidgetNode, WidgetType};
use std::sync::Arc;

pub const TEXT_CONTENT: PropertyId = PropertyId::new(1);
pub const BUTTON_ENABLED: PropertyId = PropertyId::new(1);

pub fn container_type() -> WidgetType {
    WidgetType::of::<Container>()
}

pub fn text_type() -> WidgetType {
    WidgetType::of::<Text>()
}

pub fn button_type() -> WidgetType {
    WidgetType::of::<Button>()
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
        Ok(WidgetNode::from_type(container_type())
            .with_optional_key(self.key.clone())
            .with_children(self.children.clone()))
    }
}

/// Placeholder text declaration. Shaping and glyph resources arrive in P4.
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
        Ok(WidgetNode::from_type(text_type())
            .with_optional_key(self.key.clone())
            .with_property(TEXT_CONTENT, self.content.clone()))
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
        let mut node = WidgetNode::from_type(button_type())
            .with_optional_key(self.key.clone())
            .with_property(BUTTON_ENABLED, self.enabled)
            .with_focusable(true)
            .with_enabled(self.enabled)
            .with_child(label);
        if let Some(handler) = self.handler.clone() {
            node = node.with_event_handler(handler);
        }
        Ok(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_share_the_widget_node_pipeline() {
        let mut context = BuildContext::new();
        let text = Text::new("hello").build(&mut context).unwrap();
        let button = Button::new("press").build(&mut context).unwrap();
        let container = Container::new()
            .with_child(text.clone())
            .with_child(button.clone())
            .build(&mut context)
            .unwrap();

        assert_eq!(text.widget_type(), &text_type());
        assert_eq!(button.widget_type(), &button_type());
        assert_eq!(button.children()[0].widget_type(), &text_type());
        assert_eq!(container.children(), [text, button]);
    }
}
