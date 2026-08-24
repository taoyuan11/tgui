use tgui::native::NativeHostWidget;
use tgui::widget::{BuildContext, Widget, WidgetNode};

use crate::app::GalleryModel;
use crate::layout::sized;

use super::usage;

pub fn build(context: &mut BuildContext, model: &GalleryModel) -> tgui::Result<Vec<WidgetNode>> {
    let composition = context.read_state(&model.native_composition)?;
    let host = composition.map_or_else(
        || NativeHostWidget::new().with_key("gallery-native-host"),
        |composition| {
            NativeHostWidget::new()
                .with_key("gallery-native-host")
                .with_composition(composition)
        },
    );
    let host = host
        .with_z_order(1)
        .with_layout_style(sized(320.0, 110.0))
        .build(context)?;

    Ok(vec![usage(
        context,
        "NativeHostWidget",
        "Use NativeHostWidget only for external surfaces such as a WebView. Ordinary controls stay in the retained paint pipeline.",
        "NativeHostWidget::new().with_composition(composition).with_z_order(1).build(context)?",
        host,
    )?])
}
