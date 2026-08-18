use std::cell::{Cell, RefCell};
use std::rc::Rc;

use tgui::core::{LayoutRevision, Point, RevisionSet};
use tgui::event::{
    CommittedHitTarget, EventHandler, EventPhase, PointerEvent, PointerId, PointerKind, UiEvent,
};
use tgui::widget::{BuildContext, View, Widget, WidgetNode};
use tgui::widgets::{Button, Container};
use tgui::{Application, CpuSnapshot, State, WindowSpec};

const BUTTON_KEY: &str = "counter";

#[derive(Clone)]
struct CounterView {
    count: State<u32>,
    trace: Rc<RefCell<Vec<String>>>,
    builds: Rc<Cell<usize>>,
}

impl View for CounterView {
    fn build_view(&self, context: &mut BuildContext) -> tgui::Result<WidgetNode> {
        self.builds.set(self.builds.get() + 1);
        let count = context.read_state(&self.count)?;

        let button_trace = self.trace.clone();
        let button_count = self.count.clone();
        let button = Button::new(format!("count = {count}"))
            .with_key(BUTTON_KEY)
            .with_event_handler(EventHandler::new(1, move |event, context| {
                if matches!(event, UiEvent::PointerDown(_)) {
                    button_trace
                        .borrow_mut()
                        .push(format!("button:{:?}", context.phase()));
                    if context.phase() == EventPhase::Target {
                        button_count.update(context.transaction(), |value| *value += 1)?;
                    }
                }
                Ok(())
            }))
            .build(context)?;

        let container_trace = self.trace.clone();
        Ok(Container::new()
            .with_key("root")
            .with_child(button)
            .build(context)?
            .with_event_handler(EventHandler::new(1, move |event, context| {
                if matches!(event, UiEvent::PointerDown(_)) {
                    container_trace
                        .borrow_mut()
                        .push(format!("container:{:?}", context.phase()));
                }
                Ok(())
            })))
    }
}

fn main() -> tgui::Result<()> {
    let count = State::new(0_u32);
    let trace = Rc::new(RefCell::new(Vec::new()));
    let builds = Rc::new(Cell::new(0));

    let mut application = Application::new();
    let window = application.create_window(WindowSpec::new("P1 headless"))?;
    application.set_view(
        window,
        CounterView {
            count: count.clone(),
            trace: trace.clone(),
            builds: builds.clone(),
        },
    )?;
    application.commit_snapshot(window, CpuSnapshot::empty(RevisionSet::ZERO))?;

    let button = application
        .element_diagnostics(window)
        .expect("window exists")
        .into_iter()
        .find(|node| {
            node.key
                .as_ref()
                .is_some_and(|key| key.as_str() == Some(BUTTON_KEY))
        })
        .expect("counter button is mounted")
        .id;

    assert_eq!(application.take_frame_requests()?, [window]);
    assert!(application.take_frame_requests()?.is_empty());

    let event = UiEvent::PointerDown(PointerEvent::new(
        PointerId::MOUSE,
        PointerKind::Mouse,
        Point::new(10.0, 10.0),
    ));
    let receipt = application.dispatch_event(
        window,
        CommittedHitTarget::for_window(window, LayoutRevision::ZERO, Some(button)),
        &event,
    )?;

    assert_eq!(
        trace.borrow().as_slice(),
        ["container:Capture", "button:Target", "container:Bubble"]
    );
    assert_eq!(count.get()?, 1);
    assert_eq!(builds.get(), 2);
    assert!(receipt.reconciliation.is_some());
    assert_eq!(application.focused_element(window), Some(button));
    assert_eq!(application.take_frame_requests()?, [window]);
    assert!(application.take_frame_requests()?.is_empty());

    println!(
        "trace={:?} count={} builds={} rebuilt={} focused={} idle=true",
        trace.borrow(),
        count.get()?,
        builds.get(),
        receipt.reconciliation.is_some(),
        application.focused_element(window) == Some(button),
    );
    Ok(())
}
