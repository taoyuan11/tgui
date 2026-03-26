mod button;
mod box_layout;
mod column;
mod node;
mod row;
mod scene;
mod text;

pub use button::button;
pub use box_layout::box_layout;
pub use column::column;
pub use node::{ClickEvent, Element, View};
pub use row::row;
pub use text::text;

pub(crate) use node::Node;
pub(crate) use scene::{Scene, build_scene};

pub fn r#box<const N: usize>(children: [Element; N]) -> Element {
    box_layout(children)
}
