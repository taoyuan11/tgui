use tgui::widget::{BuildContext, Widget, WidgetNode};
use tgui::widgets::{Container, Text};

use crate::layout::{fixed, row};

use super::usage;

pub fn build(context: &mut BuildContext) -> tgui::Result<Vec<WidgetNode>> {
    let text_example = fixed(
        Text::new("Hello tgui / English / 中文 / مرحبا")
            .with_key("text-example")
            .build(context)?,
        640.0,
        32.0,
    );
    let container_example = Container::new()
        .with_key("container-example")
        .with_children([
            fixed(Text::new("Left child").build(context)?, 150.0, 32.0),
            fixed(Text::new("Right child").build(context)?, 150.0, 32.0),
        ])
        .build(context)?
        .with_layout_style(row(640.0, 48.0, 12.0, 8.0));

    Ok(vec![
        usage(
            context,
            "Text",
            "Text stores logical content, participates in intrinsic layout, and exposes that content to accessibility.",
            "Text::new(\"Hello tgui\").with_key(\"greeting\").build(context)?",
            text_example,
        )?,
        usage(
            context,
            "Container",
            "Container groups declarations. Apply LayoutStyle to choose row, column, grid, spacing, and overflow behavior.",
            "Container::new().with_children([left, right]).build(context)?",
            container_example,
        )?,
    ])
}
