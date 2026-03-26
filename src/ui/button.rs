use crate::ui::node::{ButtonNode, Element, Node, TextNode, View};

pub fn button(label: Element) -> Element {
    let label = match label.into_node() {
        Node::Text(text) => text,
        _ => TextNode {
            source: crate::ui::node::TextSource::Static("Button".to_string()),
            style: Default::default(),
        },
    };

    Element::from_node(Node::Button(ButtonNode {
        label,
        on_click: None,
        width: None,
        height: None,
    }))
}
