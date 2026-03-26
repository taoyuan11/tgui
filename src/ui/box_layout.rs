use crate::ui::node::{BoxNode, Element, Node, View};

pub fn box_layout<const N: usize>(children: [Element; N]) -> Element {
    let nodes = children.into_iter().map(|item| item.into_node()).collect();
    Element::from_node(Node::Box(BoxNode {
        children: nodes,
        width: None,
        height: None,
    }))
}
