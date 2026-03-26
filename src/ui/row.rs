use crate::ui::node::{Element, Node, RowNode, View};

pub fn row<const N: usize>(children: [Element; N]) -> Element {
    let nodes = children.into_iter().map(|item| item.into_node()).collect();
    Element::from_node(Node::Row(RowNode {
        children: nodes,
        spacing: 0.0,
        width: None,
        height: None,
    }))
}
