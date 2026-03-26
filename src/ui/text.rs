use crate::graphics::TextStyle;
use crate::ui::node::{Element, IntoTextSource, Node, TextNode};

pub fn text(source: impl IntoTextSource) -> Element {
    Element::from_node(Node::Text(TextNode {
        source: source.into_text_source(),
        style: TextStyle::default(),
        width: None,
        height: None,
    }))
}
