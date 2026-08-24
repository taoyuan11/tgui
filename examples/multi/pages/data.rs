use tgui::widget::{BuildContext, Widget, WidgetNode};
use tgui::widgets::{Container, Text};

use crate::app::GalleryModel;
use crate::layout::{column, fixed};

use super::usage;

pub fn build(context: &mut BuildContext, model: &GalleryModel) -> tgui::Result<Vec<WidgetNode>> {
    let rows = context.read_state(&model.virtual_rows)?;
    let example = Container::new()
        .with_key("virtual-list-example")
        .with_children(rows)
        .build(context)?
        .with_layout_style(column(640.0, 116.0, 2.0, 4.0))
        .with_semantics(model.list_semantics.clone());
    let metrics = fixed(
        Text::new(format!(
            "Materialized {} of {} rows",
            model.list_metrics.materialized_items, model.list_metrics.total_items
        ))
        .build(context)?,
        640.0,
        30.0,
    );
    let preview = Container::new()
        .with_children([metrics, example])
        .build(context)?
        .with_layout_style(column(640.0, 148.0, 2.0, 0.0));

    Ok(vec![usage(
        context,
        "VirtualList",
        "VirtualList retains stable item identity while materializing only the viewport plus overscan. Keep its controller on the UI thread.",
        "let mut list = VirtualList::new(source, 28.0)?; list.set_viewport(112.0, 28_000.0)?;",
        preview,
    )?])
}
