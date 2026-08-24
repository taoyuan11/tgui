//! Small layout helpers shared by gallery modules.

use tgui::layout::{
    Dimension, FlexDirection, LayoutSize, LayoutStyle, LengthPercentage, OverflowAxes, Sides,
};
use tgui::widget::WidgetNode;

pub fn fixed(node: WidgetNode, width: f32, height: f32) -> WidgetNode {
    node.with_layout_style(sized(width, height))
}

pub fn sized(width: f32, height: f32) -> LayoutStyle {
    LayoutStyle::default().with_size(Dimension::Length(width), Dimension::Length(height))
}

pub fn row(width: f32, height: f32, gap: f32, padding: f32) -> LayoutStyle {
    stack(width, height, gap, padding, FlexDirection::Row)
}

pub fn column(width: f32, height: f32, gap: f32, padding: f32) -> LayoutStyle {
    stack(width, height, gap, padding, FlexDirection::Column)
}

pub fn scroll_column(width: f32, height: f32, gap: f32, padding: f32) -> LayoutStyle {
    let mut style = column(width, height, gap, padding);
    style.overflow = OverflowAxes::SCROLL;
    style
}

fn stack(width: f32, height: f32, gap: f32, padding: f32, direction: FlexDirection) -> LayoutStyle {
    let mut style = sized(width, height);
    style.flex_direction = direction;
    style.gap = match direction {
        FlexDirection::Row | FlexDirection::RowReverse => {
            LayoutSize::new(LengthPercentage::Length(gap), LengthPercentage::ZERO)
        }
        FlexDirection::Column | FlexDirection::ColumnReverse => {
            LayoutSize::new(LengthPercentage::ZERO, LengthPercentage::Length(gap))
        }
    };
    style.padding = Sides::all(LengthPercentage::Length(padding));
    style
}
