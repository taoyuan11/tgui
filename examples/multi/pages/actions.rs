use tgui::event::{EventHandler, EventPhase, UiEvent};
use tgui::widget::{BuildContext, Widget, WidgetNode};
use tgui::widgets::{Button, Container};

use crate::app::GalleryModel;
use crate::layout::{fixed, row};

use super::usage;

pub fn build(context: &mut BuildContext, model: &GalleryModel) -> tgui::Result<Vec<WidgetNode>> {
    let clicks = context.read_state(&model.clicks)?;
    let clicks_state = model.clicks.clone();
    let enabled = Button::new(format!("Clicked {clicks} time(s)"))
        .with_key("action-button")
        .with_event_handler(EventHandler::new(1, move |event, event_context| {
            if matches!(event, UiEvent::PointerDown(_))
                && matches!(
                    event_context.phase(),
                    EventPhase::Target | EventPhase::Bubble
                )
            {
                clicks_state.update(event_context.transaction(), |value| *value += 1)?;
            }
            Ok(())
        }))
        .build(context)?;
    let disabled = Button::new("Disabled")
        .with_key("disabled-action-button")
        .with_enabled(false)
        .build(context)?;
    let example = Container::new()
        .with_children([fixed(enabled, 230.0, 44.0), fixed(disabled, 180.0, 44.0)])
        .build(context)?
        .with_layout_style(row(640.0, 56.0, 12.0, 6.0));

    Ok(vec![usage(
        context,
        "Button",
        "Button is focusable and semantic by default. Handle Target-phase events and publish state through the shared UpdateTxn.",
        "Button::new(\"Save\").with_event_handler(handler).build(context)?",
        example,
    )?])
}
