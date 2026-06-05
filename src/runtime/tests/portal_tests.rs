use super::*;

use crate::runtime::portal::PortalRegistry;
use crate::ui::widget::{
    Button, ComputedScene, OverlayAlignment, OverlayLayer, OverlayPlacement, Portal, PortalAnchor,
    PortalTarget,
};

#[derive(Default)]
struct PortalVm;

impl crate::foundation::view_model::ViewModel for PortalVm {
    fn new(_context: &ViewModelContext) -> Self {
        Self
    }

    fn view(&self) -> Element<Self>
    where
        Self: Sized,
    {
        Stack::new().into()
    }
}

fn portal_handler(
    key: &str,
    tree: WidgetTree<PortalVm>,
    invalidation: InvalidationSignal,
    size: (f64, f64),
) -> BoundRuntimeHandler<PortalVm> {
    let mut handler = test_handler_with_config(
        PortalVm,
        Some(tree),
        invalidation,
        test_config_with_size(size.0, size.1),
    );
    handler.window_key = key.to_string();
    handler
}

fn overlay_labels(computed: &ComputedScene<PortalVm>) -> Vec<String> {
    computed
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.to_string())
        .collect()
}

#[test]
fn cross_window_portal_declared_by_source_renders_in_target_layer() {
    let invalidation = InvalidationSignal::new();
    let source_tree = WidgetTree::new(
        Portal::<PortalVm>::new(Text::new("remote portal"))
            .target_window("target")
            .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
            .layer(OverlayLayer::Toast)
            .close_on_escape(true)
            .on_open_change(ValueCommand::new(|_: &mut PortalVm, _: bool| {})),
    );
    let target_tree = WidgetTree::new(Stack::<PortalVm>::new().child(Text::new("target base")));
    let mut source = portal_handler("source", source_tree, invalidation.clone(), (240.0, 160.0));
    let mut target = portal_handler("target", target_tree, invalidation, (320.0, 220.0));

    let source_computed = source.computed_scene().clone();
    assert!(
        source_computed.scene.overlay_texts.is_empty(),
        "external portal should not render in the source window"
    );
    let requests = source.external_portal_requests_from_computed();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].target_window_key, "target");

    target.set_external_portal_requests(requests, 1);
    let target_computed = target.computed_scene().clone();
    assert!(
        overlay_labels(&target_computed).contains(&"remote portal".to_string()),
        "target window should merge source portal content"
    );
    assert!(
        target_computed
            .overlay_close_handlers
            .iter()
            .any(|handler| handler.layer == OverlayLayer::Toast),
        "close metadata should be registered in the selected portal layer"
    );
}

#[test]
fn cross_window_portal_anchor_uses_target_window_coordinates() {
    let invalidation = InvalidationSignal::new();
    let action: Element<PortalVm> = Button::new("target action")
        .size(dp(80.0), dp(30.0))
        .on_click(Command::new(|_: &mut PortalVm| {}))
        .into();
    let action_id = action.id;
    let source_tree = WidgetTree::new(
        Portal::new(action)
            .target(PortalTarget::WindowKey("target".to_string()))
            .anchor(PortalAnchor::Rect(Rect::new(30.0, 40.0, 10.0, 10.0)))
            .placement(OverlayPlacement::bottom().align(OverlayAlignment::Start))
            .offset(dp(0.0))
            .viewport_padding(dp(0.0)),
    );
    let target_tree = WidgetTree::new(Stack::<PortalVm>::new());
    let mut source = portal_handler("source", source_tree, invalidation.clone(), (120.0, 80.0));
    let mut target = portal_handler("target", target_tree, invalidation, (320.0, 220.0));

    target.set_external_portal_requests(source.external_portal_requests_from_computed(), 1);
    let computed = target.computed_scene().clone();
    let hit = computed
        .overlay_hit_regions
        .iter()
        .find(|region| {
            matches!(
                &region.interaction,
                HitInteraction::Widget { id, .. } if *id == action_id
            )
        })
        .expect("external portal button should be hittable in target window");

    assert!(
        (hit.rect.x - dp(30.0)).abs() <= dp(0.1) && (hit.rect.y - dp(50.0)).abs() <= dp(0.1),
        "external portal should interpret Rect anchor in target viewport coordinates: {:?}",
        hit.rect
    );
}

#[test]
fn missing_target_window_is_skipped_without_target_overlay() {
    let invalidation = InvalidationSignal::new();
    let source_tree = WidgetTree::new(
        Portal::<PortalVm>::new(Text::new("missing target"))
            .target_window("missing")
            .anchor(Rect::new(0.0, 0.0, 1.0, 1.0)),
    );
    let mut source = portal_handler("source", source_tree, invalidation.clone(), (240.0, 160.0));
    let mut target = portal_handler(
        "target",
        WidgetTree::new(Stack::<PortalVm>::new()),
        invalidation,
        (320.0, 220.0),
    );
    let mut registry = PortalRegistry::default();

    registry.publish_source("source", source.external_portal_requests_from_computed());
    target.set_external_portal_requests(
        registry.requests_for_target("target"),
        registry.target_revision("target"),
    );
    let computed = target.computed_scene().clone();
    assert!(
        !overlay_labels(&computed).contains(&"missing target".to_string()),
        "unmatched target keys should not leak external portal content"
    );
}

#[test]
fn source_removal_clears_target_overlay_handlers() {
    let invalidation = InvalidationSignal::new();
    let source_tree = WidgetTree::new(
        Portal::<PortalVm>::new(
            Button::new("remote close")
                .size(dp(100.0), dp(30.0))
                .on_click(Command::new(|_: &mut PortalVm| {})),
        )
        .target_window("target")
        .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
        .close_on_escape(true)
        .on_open_change(ValueCommand::new(|_: &mut PortalVm, _: bool| {})),
    );
    let mut source = portal_handler("source", source_tree, invalidation.clone(), (240.0, 160.0));
    let mut target = portal_handler(
        "target",
        WidgetTree::new(Stack::<PortalVm>::new()),
        invalidation,
        (320.0, 220.0),
    );
    let mut registry = PortalRegistry::default();

    registry.publish_source("source", source.external_portal_requests_from_computed());
    target.set_external_portal_requests(
        registry.requests_for_target("target"),
        registry.target_revision("target"),
    );
    let shown = target.computed_scene().clone();
    assert!(overlay_labels(&shown).contains(&"remote close".to_string()));
    assert!(!shown.overlay_close_handlers.is_empty());

    registry.remove_source("source");
    target.set_external_portal_requests(
        registry.requests_for_target("target"),
        registry.target_revision("target"),
    );
    let hidden = target.computed_scene().clone();
    assert!(!overlay_labels(&hidden).contains(&"remote close".to_string()));
    assert!(hidden.overlay_close_handlers.is_empty());
}

#[test]
fn external_portal_signal_change_invalidates_target_computed_scene() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let label = context.state("first".to_string());
    let source_tree = WidgetTree::new(
        Portal::<PortalVm>::new(Text::new(label.signal()))
            .target_window("target")
            .anchor(Rect::new(20.0, 20.0, 1.0, 1.0)),
    );
    let mut source = portal_handler("source", source_tree, invalidation.clone(), (240.0, 160.0));
    let mut target = portal_handler(
        "target",
        WidgetTree::new(Stack::<PortalVm>::new()),
        invalidation.clone(),
        (320.0, 220.0),
    );

    target.set_external_portal_requests(source.external_portal_requests_from_computed(), 1);
    let initial = target.computed_scene().clone();
    assert!(overlay_labels(&initial).contains(&"first".to_string()));

    target.last_invalidation_revision = target.invalidation.revision();
    label.set("second".to_string());
    target.request_redraw_if_dirty(Instant::now());
    let updated = target.computed_scene().clone();
    assert!(
        overlay_labels(&updated).contains(&"second".to_string()),
        "target computed scene should recollect external portal signal dependencies"
    );
}

#[test]
fn cross_window_portal_close_handlers_run_in_target_handler() {
    let invalidation = InvalidationSignal::new();
    let close_calls = Arc::new(Mutex::new(Vec::new()));
    let close_calls_cmd = close_calls.clone();
    let source_tree = WidgetTree::new(
        Portal::<PortalVm>::new(Text::new("closable remote"))
            .target_window("target")
            .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
            .close_on_escape(true)
            .close_on_outside_click(true)
            .on_open_change(ValueCommand::new(move |_: &mut PortalVm, open| {
                close_calls_cmd.lock().unwrap().push(open);
            })),
    );
    let mut source = portal_handler("source", source_tree, invalidation.clone(), (240.0, 160.0));
    let mut target = portal_handler(
        "target",
        WidgetTree::new(Stack::<PortalVm>::new()),
        invalidation,
        (320.0, 220.0),
    );

    target.set_external_portal_requests(source.external_portal_requests_from_computed(), 1);
    let _ = target.computed_scene();

    assert!(target.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Escape))));
    let _ = target.consume_overlay_close_handlers_outside_click(Point::new(dp(300.0), dp(200.0)));
    assert_eq!(close_calls.lock().unwrap().as_slice(), &[false, false]);
}
