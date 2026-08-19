use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use tgui::animation::{Animated, AnimationImpact, AnimationKey, AnimationSpec};
use tgui::core::{DpiScale, ImageHandle, ItemKey, Point, Rect, ResourceId};
use tgui::event::{
    AccessibilityAction, AccessibilityActionEvent, EventHandler, EventPhase, PointerEvent,
    PointerId, PointerKind, UiEvent,
};
use tgui::media::{
    DecodedImage, ImageDecodeResult, ImagePresentation, ImageRegistry, ImageRequestKey, ImageSize,
    ImageSource,
};
use tgui::native::{
    MockNativeHostFactory, NativeHostComposition, NativeHostCreateContext, NativeHostLayout,
    NativeHostManager, NativeHostScheduler, NativeHostWidget,
};
use tgui::test_support::FakeClock;
use tgui::virtualization::{VirtualList, VirtualListDataSource};
use tgui::widget::{BuildContext, OPACITY, View, Widget, WidgetNode};
use tgui::widgets::{Button, Container, Image, Text};
use tgui::{Application, ResourceCompletion, State, WindowSpec};

const BUTTON_KEY: &str = "integrated-button";
const IMAGE_KEY: &str = "integrated-image";
const HOST_KEY: &str = "integrated-host";

#[derive(Clone)]
struct IntegratedView {
    count: State<u32>,
    events: Rc<RefCell<Vec<EventPhase>>>,
    host_composition: State<Option<NativeHostComposition>>,
    image_handle: State<Option<ImageHandle>>,
    virtual_items: State<Vec<WidgetNode>>,
}

impl View for IntegratedView {
    fn build_view(&self, context: &mut BuildContext) -> tgui::Result<WidgetNode> {
        let count = context.read_state(&self.count)?;
        let composition = context.read_state(&self.host_composition)?;
        let image_handle = context.read_state(&self.image_handle)?;
        let virtual_items = context.read_state(&self.virtual_items)?;
        let count_state = self.count.clone();
        let events = self.events.clone();
        let button = Button::new(format!("count = {count}"))
            .with_key(BUTTON_KEY)
            .with_event_handler(EventHandler::new(1, move |event, context| {
                if matches!(
                    event,
                    UiEvent::PointerDown(_)
                        | UiEvent::AccessibilityAction(AccessibilityActionEvent {
                            action: AccessibilityAction::Activate,
                            ..
                        })
                ) {
                    events.borrow_mut().push(context.phase());
                    if context.phase() == EventPhase::Target {
                        count_state.update(context.transaction(), |value| *value += 1)?;
                    }
                }
                Ok(())
            }))
            .build(context)?;
        let title = Text::new("tgui P7: English / 中文 / مرحبا")
            .with_key("title")
            .build(context)?;
        let host = composition.map_or_else(
            || NativeHostWidget::new().with_key(HOST_KEY),
            |composition| {
                NativeHostWidget::new()
                    .with_key(HOST_KEY)
                    .with_composition(composition)
            },
        );
        let image = image_handle
            .map_or_else(|| Image::new(ImageHandle::from_parts(0, 0)), Image::new)
            .with_key(IMAGE_KEY)
            .with_alt_text("Decoded preview")
            .with_size(tgui::Size::new(96.0, 64.0))
            .build(context)?;
        let virtual_list = Container::new()
            .with_key("virtual-list")
            .with_children(virtual_items)
            .build(context)?;
        Container::new()
            .with_key("root")
            .with_children([title, button, image, host.build(context)?, virtual_list])
            .build(context)
    }
}

struct Rows(usize);
struct Row;

impl VirtualListDataSource for Rows {
    fn len(&self) -> usize {
        self.0
    }

    fn item_key(&self, index: usize) -> ItemKey {
        ItemKey::numeric(index as u64)
    }

    fn build_item(
        &self,
        index: usize,
        _key: &ItemKey,
        _context: &mut BuildContext,
    ) -> tgui::Result<WidgetNode> {
        Ok(WidgetNode::new::<Row>().with_key(index as u64))
    }
}

fn element_with_key(
    application: &Application,
    window: tgui::WindowId,
    key: &str,
) -> tgui::ElementId {
    application
        .element_diagnostics(window)
        .expect("window exists")
        .into_iter()
        .find(|node| node.key.as_ref().and_then(|key| key.as_str()) == Some(key))
        .unwrap_or_else(|| panic!("missing element key {key}"))
        .id
}

fn main() -> tgui::Result<()> {
    let clock = Rc::new(FakeClock::new());
    let count = State::new(0_u32);
    let events = Rc::new(RefCell::new(Vec::new()));
    let host_composition = State::new(None);
    let mut images = ImageRegistry::new();
    let image_key = ImageRequestKey::new(ImageSource::bytes([1_u8, 2, 3, 4].as_slice()));
    let image_request = images.request(image_key.clone());
    let image_handle = State::new(Some(image_request.handle));
    let mut list = VirtualList::new(Rows(100_000), 20.0)?;
    list.set_viewport(320.0, 100.0)?;
    list.set_scroll_offset(40_000.0)?;
    let list_metrics = list.metrics();
    let virtual_items = State::new(
        list.materialized_items()
            .map(|item| item.node().clone())
            .collect::<Vec<_>>(),
    );
    let mut application = Application::with_frame_clock(clock.clone());
    let window = application.create_window(WindowSpec::new("P7 integrated headless"))?;
    application.set_view(
        window,
        IntegratedView {
            count: count.clone(),
            events: events.clone(),
            host_composition: host_composition.clone(),
            image_handle: image_handle.clone(),
            virtual_items,
        },
    )?;
    let initial = application.render_window(window)?;

    let button = element_with_key(&application, window, BUTTON_KEY);
    let hit = application.hit_test(window, Point::new(1.0, 1.0))?;
    let hit = tgui::event::CommittedHitTarget::for_window(window, hit.revision(), Some(button));
    application.dispatch_event(
        window,
        hit,
        &UiEvent::PointerDown(PointerEvent::new(
            PointerId::MOUSE,
            PointerKind::Mouse,
            Point::new(1.0, 1.0),
        )),
    )?;

    let opacity = Animated::new(1.0_f32);
    application.animate(
        window,
        AnimationKey::new(button, OPACITY),
        &opacity,
        0.4,
        AnimationSpec::new(Duration::from_millis(120), AnimationImpact::Paint),
    )?;
    clock.advance(Duration::from_millis(60))?;
    application.tick_animations(window)?;

    let image_element = element_with_key(&application, window, IMAGE_KEY);
    assert_eq!(
        images.presentation(image_request.handle),
        ImagePresentation::Placeholder
    );
    let decoded = DecodedImage::new(ImageSize::new(1, 1)?, [32, 128, 224, 255])?;
    let decoded = ImageDecodeResult {
        handle: image_request.handle,
        key: image_key,
        decoded: Ok(decoded),
    };
    images.complete(image_request.handle.stamp(), &decoded);
    let image_ticket =
        application.begin_resource_request(window, image_element, image_request.handle.stamp())?;
    application.complete_resource_request(ResourceCompletion::new(
        image_ticket,
        [ResourceId::from_parts(
            image_request.handle.slot(),
            image_request.handle.generation(),
        )],
        0x1a2b_3c4d,
    ))?;

    let host_factory = MockNativeHostFactory::default();
    let mut hosts = NativeHostManager::new();
    let host = hosts.create(
        &host_factory,
        NativeHostCreateContext::new(window, image_element),
    )?;
    hosts.mount(host)?;
    let host_layout = NativeHostLayout::new(Rect::from_xywh(0.0, 0.0, 160.0, 90.0), DpiScale::ONE);
    hosts.update_layout(host, host_layout)?;
    let host_schedule = NativeHostScheduler::schedule(
        hosts.capabilities(host).expect("live host capabilities"),
        hosts.cost(host).expect("live host cost"),
        host_layout,
    )?;
    let composition = hosts.compose(host, host_schedule.strategy)?;
    let mut native_transaction = tgui::UpdateTxn::new();
    // Publish the composition through the retained View so its surface is
    // compiled in this window's scene instead of remaining a side object.
    host_composition.set(&mut native_transaction, Some(composition))?;
    application.apply_transaction(native_transaction)?;
    application.record_virtualization_metrics(window, list_metrics.into())?;

    let action_revision = application
        .committed_snapshot(window)
        .expect("resource completion keeps a committed snapshot")
        .layout()
        .revision();
    application.dispatch_event(
        window,
        tgui::event::CommittedHitTarget::for_window(window, action_revision, Some(button)),
        &UiEvent::AccessibilityAction(AccessibilityActionEvent::for_window(
            window,
            button,
            AccessibilityAction::Activate,
        )),
    )?;

    let final_frame = application.render_window(window)?;
    let committed = application
        .committed_snapshot(window)
        .expect("render commits a CPU snapshot");
    let metrics = application
        .frame_metrics(window)
        .expect("render records frame metrics");
    println!(
        "count={} event_phases={:?} text_commands={} image={:?} opacity={:.2} list={}/{} semantics={} native_host={} revisions={:?} batches={} initial_scene={} final_scene={}",
        count.get()?,
        events.borrow(),
        final_frame.scene.command_count(),
        images.presentation(image_request.handle),
        opacity.value(),
        list_metrics.materialized_items,
        list_metrics.total_items,
        committed.semantics().node_count(),
        host_composition.get()?.is_some(),
        committed.revisions(),
        metrics.scene.batches,
        initial.scene.fingerprint(),
        final_frame.scene.fingerprint(),
    );
    hosts.destroy(host)?;
    Ok(())
}
