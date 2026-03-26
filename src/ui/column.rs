use crate::ui::node::{ColumnNode, Element, Node, View};

pub fn column<const N: usize>(children: [Element; N]) -> Element {
    let nodes = children.into_iter().map(|item| item.into_node()).collect();
    Element::from_node(Node::Column(ColumnNode {
        children: nodes,
        spacing: 0.0,
        centered: false,
        width: None,
        height: None,
    }))
}
