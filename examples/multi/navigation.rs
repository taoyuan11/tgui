//! Sidebar sections and navigation button declarations.

use tgui::State;
use tgui::event::{EventHandler, EventPhase, UiEvent};
use tgui::widget::{BuildContext, Widget, WidgetNode};
use tgui::widgets::{Button, Container, Text};

use crate::layout::{column, fixed};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Basics,
    Actions,
    Media,
    Data,
    Native,
}

impl Page {
    pub const ALL: [Self; 5] = [
        Self::Basics,
        Self::Actions,
        Self::Media,
        Self::Data,
        Self::Native,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Basics => "basics",
            Self::Actions => "actions",
            Self::Media => "media",
            Self::Data => "data",
            Self::Native => "native",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Basics => "Basics",
            Self::Actions => "Actions",
            Self::Media => "Media",
            Self::Data => "Data & Lists",
            Self::Native => "Native Host",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Basics => "Container and Text declarations",
            Self::Actions => "Enabled, disabled, and stateful Button usage",
            Self::Media => "Generation-stamped Image resources",
            Self::Data => "Viewport-bounded VirtualList materialization",
            Self::Native => "Optional external-surface integration",
        }
    }
}

pub fn sidebar(
    context: &mut BuildContext,
    selected: Page,
    selected_state: &State<Page>,
) -> tgui::Result<WidgetNode> {
    let mut children = vec![fixed(
        Text::new("tgui gallery")
            .with_key("sidebar-title")
            .build(context)?,
        210.0,
        42.0,
    )];

    for page in Page::ALL {
        let next_page = page;
        let state = selected_state.clone();
        let prefix = if page == selected { "> " } else { "  " };
        let button = Button::new(format!("{prefix}{}", page.label()))
            .with_key(format!("nav-{}", page.key()))
            .with_event_handler(EventHandler::new(1, move |event, event_context| {
                if matches!(event, UiEvent::PointerDown(_))
                    && matches!(
                        event_context.phase(),
                        EventPhase::Target | EventPhase::Bubble
                    )
                {
                    state.set(event_context.transaction(), next_page)?;
                }
                Ok(())
            }))
            .build(context)?;
        children.push(fixed(button, 210.0, 42.0));
    }

    Container::new()
        .with_key("component-sidebar")
        .with_children(children)
        .build(context)
        .map(|node| node.with_layout_style(column(234.0, 720.0, 10.0, 12.0)))
}
