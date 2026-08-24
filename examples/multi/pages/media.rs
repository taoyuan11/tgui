use tgui::Size;
use tgui::widget::{BuildContext, Widget, WidgetNode};
use tgui::widgets::Image;

use crate::app::GalleryModel;
use crate::layout::fixed;

use super::usage;

pub fn build(context: &mut BuildContext, model: &GalleryModel) -> tgui::Result<Vec<WidgetNode>> {
    let image = Image::new(model.image)
        .with_key("gallery-image")
        .with_alt_text("A generation-stamped gallery preview")
        .with_size(Size::new(128.0, 96.0))
        .build(context)?;

    Ok(vec![usage(
        context,
        "Image",
        "Image references a generation-stamped ImageHandle. Give every meaningful image alt text and a stable logical size.",
        "Image::new(handle).with_alt_text(\"Preview\").with_size(Size::new(128.0, 96.0)).build(context)?",
        fixed(image, 128.0, 96.0),
    )?])
}
