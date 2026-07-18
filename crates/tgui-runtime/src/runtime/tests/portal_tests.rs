use super::*;

use crate::runtime::portal::PortalRegistry;
use crate::ui::widget::{
    Button, ComputedScene, FocusScopeOptions, OverlayAlignment, OverlayLayer, OverlayPlacement,
    Portal, PortalAnchor, PortalTarget,
};
use accesskit::{Action, ActionRequest, Role, TreeId};
use std::sync::atomic::AtomicBool;

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

fn portal_accessibility_update(
    handler: &mut BoundRuntimeHandler<PortalVm>,
) -> accesskit::TreeUpdate {
    handler.accessibility_tree_update_for_test()
}

fn portal_action_request(node_id: accesskit::NodeId, action: Action) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: node_id,
        data: None,
    }
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
    assert_eq!(
        requests[0].source_window_instance_id,
        Some(source.window_instance_id)
    );

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
    assert!(target_computed
        .accessibility_fragments
        .iter()
        .all(|fragment| { fragment.source_window_instance_id == Some(source.window_instance_id) }));
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
        WidgetTree::new_legacy(Stack::<PortalVm>::new()),
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

#[test]
fn repeated_external_portal_collection_preserves_order_focus_and_hit_metadata() {
    let invalidation = InvalidationSignal::new();
    let action: Element<PortalVm> = Button::new("first remote")
        .size(dp(100.0), dp(30.0))
        .on_click(Command::new(|_: &mut PortalVm| {}))
        .into();
    let action_id = action.id;
    let portals: [Element<PortalVm>; 2] = [
        Portal::new(action)
            .target_window("target")
            .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
            .layer(OverlayLayer::Menu)
            .close_on_escape(true)
            .on_open_change(ValueCommand::new(|_: &mut PortalVm, _: bool| {}))
            .focus_scope(FocusScopeOptions::new().trap(true))
            .into(),
        Portal::new(Text::new("second remote"))
            .target_window("target")
            .anchor(Rect::new(20.0, 70.0, 1.0, 1.0))
            .layer(OverlayLayer::Menu)
            .into(),
    ];
    let source_tree = WidgetTree::new(Stack::<PortalVm>::new().child(portals));
    let target_tree = WidgetTree::new(Stack::<PortalVm>::new());
    let mut source = portal_handler("source", source_tree, invalidation.clone(), (240.0, 160.0));
    let mut target = portal_handler("target", target_tree, invalidation, (320.0, 220.0));

    target.set_external_portal_requests(source.external_portal_requests_from_computed(), 1);
    let fingerprints = target
        .external_portal_requests
        .iter()
        .map(|request| request.fingerprint())
        .collect::<Vec<_>>();

    for _ in 0..2 {
        target.invalidate_computed_scene();
        let computed = target.computed_scene().clone();
        let labels = overlay_labels(&computed);
        let first = labels
            .iter()
            .position(|label| label == "first remote")
            .expect("first external portal should render");
        let second = labels
            .iter()
            .position(|label| label == "second remote")
            .expect("second external portal should render");
        assert!(
            first < second,
            "external portal emit order must stay stable"
        );

        let hit = computed
            .overlay_hit_regions
            .iter()
            .find(|region| {
                matches!(
                    &region.interaction,
                    HitInteraction::Widget { id, .. } if *id == action_id
                )
            })
            .expect("external portal button should preserve hit metadata");
        let scope = computed
            .focus_scopes
            .iter()
            .find(|scope| scope.active && scope.options.is_trap())
            .expect("external portal should preserve its focus scope");
        assert_eq!(hit.scope_path, scope.path);
        assert_eq!(
            hit.focus
                .as_ref()
                .expect("button focus metadata should be retained")
                .scope_path,
            scope.path
        );
        assert!(computed
            .overlay_close_handlers
            .iter()
            .any(|handler| handler.layer == OverlayLayer::Menu && handler.close_on_escape));

        assert_eq!(target.external_portal_requests.len(), 2);
        assert_eq!(
            target
                .external_portal_requests
                .iter()
                .map(|request| request.fingerprint())
                .collect::<Vec<_>>(),
            fingerprints,
            "move-out collection must restore the original requests unchanged"
        );
    }
}

#[test]
fn cross_window_accessibility_routes_use_real_source_instances_and_retire_stale_actions() {
    let invalidation = InvalidationSignal::new();
    let clicks = Arc::new(AtomicUsize::new(0));
    let clicks_for_command = Arc::clone(&clicks);
    let action: Element<PortalVm> = Button::new("remote accessible action")
        .size(dp(150.0), dp(30.0))
        .on_click(Command::new_with_context(move |_: &mut PortalVm, ctx| {
            clicks_for_command.fetch_add(1, Ordering::SeqCst);
            ctx.window().minimize();
        }))
        .into();
    let portal: Element<PortalVm> = Portal::new(action)
        .target_window("target")
        .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
        .into();
    let mut source = portal_handler(
        "source",
        WidgetTree::new(portal.clone()),
        invalidation.clone(),
        (240.0, 160.0),
    );
    source.window_instance_id = 101;
    let source_update = portal_accessibility_update(&mut source);
    assert!(!source_update
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("remote accessible action")));
    let mut target = portal_handler(
        "target",
        WidgetTree::new(Stack::<PortalVm>::new()),
        invalidation.clone(),
        (320.0, 220.0),
    );
    target.window_instance_id = 201;
    target.widget_tree = None;

    target.set_external_portal_requests(source.external_portal_requests_from_computed(), 1);
    let first = portal_accessibility_update(&mut target);
    let first_node_id = first
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (node.role() == Role::Button && node.label() == Some("remote accessible action"))
                .then_some(*node_id)
        })
        .expect("external Portal should publish without a target base layout");
    assert!(target
        .cached_scene
        .as_ref()
        .expect("target cache")
        .computed
        .accessibility_fragments
        .iter()
        .all(|fragment| fragment.source_window_instance_id == Some(101)));
    source
        .accessibility_action_sender
        .send(portal_action_request(first_node_id, Action::Click))
        .unwrap();
    assert!(!source.drain_accessibility_actions());
    assert_eq!(source.focused_widget_id(), None);
    assert_eq!(clicks.load(Ordering::SeqCst), 0);
    target
        .accessibility_action_sender
        .send(portal_action_request(first_node_id, Action::Focus))
        .unwrap();
    assert!(target.drain_accessibility_actions());
    assert!(target.focused_widget_id().is_some());
    assert_eq!(source.focused_widget_id(), None);
    target
        .accessibility_action_sender
        .send(portal_action_request(first_node_id, Action::Click))
        .unwrap();
    assert!(target.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 1);
    assert!(source.window_requests.drain().is_empty());
    assert_eq!(
        target.window_requests.drain(),
        vec![crate::foundation::window_control::WindowRequest::Minimize]
    );

    let mut recreated_source = portal_handler(
        "source",
        WidgetTree::new(portal),
        invalidation,
        (240.0, 160.0),
    );
    recreated_source.window_instance_id = 102;
    target
        .set_external_portal_requests(recreated_source.external_portal_requests_from_computed(), 2);
    let recreated = portal_accessibility_update(&mut target);
    let recreated_node_id = recreated
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (node.role() == Role::Button && node.label() == Some("remote accessible action"))
                .then_some(*node_id)
        })
        .expect("recreated source Portal should remain accessible");
    assert_ne!(recreated_node_id, first_node_id);

    target
        .accessibility_action_sender
        .send(portal_action_request(first_node_id, Action::Click))
        .unwrap();
    assert!(!target.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 1);
    target
        .accessibility_action_sender
        .send(portal_action_request(recreated_node_id, Action::Click))
        .unwrap();
    assert!(target.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 2);

    target.set_external_portal_requests(Vec::new(), 3);
    let removed = portal_accessibility_update(&mut target);
    assert!(!removed
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("remote accessible action")));
    target
        .accessibility_action_sender
        .send(portal_action_request(recreated_node_id, Action::Click))
        .unwrap();
    assert!(!target.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 2);
}

#[test]
fn cross_window_accessibility_distinguishes_simultaneous_cloned_sources() {
    let invalidation = InvalidationSignal::new();
    let clicks = Arc::new(AtomicUsize::new(0));
    let clicks_for_command = Arc::clone(&clicks);
    let action: Element<PortalVm> = Button::new("shared remote occurrence")
        .size(dp(160.0), dp(30.0))
        .on_click(Command::new(move |_: &mut PortalVm| {
            clicks_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .into();
    let portal: Element<PortalVm> = Portal::new(action)
        .target_window("target")
        .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
        .into();
    let mut source_a = portal_handler(
        "source-a",
        WidgetTree::new(portal.clone()),
        invalidation.clone(),
        (240.0, 160.0),
    );
    source_a.window_instance_id = 301;
    let mut source_b = portal_handler(
        "source-b",
        WidgetTree::new(portal),
        invalidation.clone(),
        (240.0, 160.0),
    );
    source_b.window_instance_id = 302;
    let mut target = portal_handler(
        "target",
        WidgetTree::new(Stack::<PortalVm>::new()),
        invalidation,
        (320.0, 220.0),
    );
    target.window_instance_id = 401;
    let mut registry = PortalRegistry::default();
    registry.publish_source(
        "source-a",
        source_a.external_portal_requests_from_computed(),
    );
    registry.publish_source(
        "source-b",
        source_b.external_portal_requests_from_computed(),
    );
    target.set_external_portal_requests(
        registry.requests_for_target("target"),
        registry.target_revision("target"),
    );

    let opened = portal_accessibility_update(&mut target);
    let by_source = opened
        .nodes
        .iter()
        .filter_map(|(node_id, node)| {
            (node.role() == Role::Button && node.label() == Some("shared remote occurrence"))
                .then(|| {
                    target
                        .accessibility_node_registry
                        .live_route(*node_id)
                        .map(|route| (route.source_window_instance_id, *node_id))
                })
                .flatten()
        })
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(by_source.len(), 2);
    let source_a_node = by_source[&301];
    let source_b_node = by_source[&302];
    assert_ne!(source_a_node, source_b_node);

    for node_id in [source_a_node, source_b_node] {
        target
            .accessibility_action_sender
            .send(portal_action_request(node_id, Action::Click))
            .unwrap();
        assert!(target.drain_accessibility_actions());
    }
    assert_eq!(clicks.load(Ordering::SeqCst), 2);

    registry.remove_source("source-a");
    target.set_external_portal_requests(
        registry.requests_for_target("target"),
        registry.target_revision("target"),
    );
    let remaining = portal_accessibility_update(&mut target);
    assert!(remaining
        .nodes
        .iter()
        .any(|(node_id, _)| *node_id == source_b_node));
    assert!(!remaining
        .nodes
        .iter()
        .any(|(node_id, _)| *node_id == source_a_node));
    target
        .accessibility_action_sender
        .send(portal_action_request(source_a_node, Action::Click))
        .unwrap();
    assert!(!target.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 2);
}

#[test]
fn cross_window_accessibility_keyboard_activation_uses_the_focused_occurrence() {
    let invalidation = InvalidationSignal::new();
    let source_a_clicks = Arc::new(AtomicUsize::new(0));
    let source_b_clicks = Arc::new(AtomicUsize::new(0));

    let source_a_clicks_for_command = Arc::clone(&source_a_clicks);
    let source_a_action: Element<PortalVm> = Button::new("shared keyboard occurrence")
        .size(dp(180.0), dp(30.0))
        .on_click(Command::new(move |_: &mut PortalVm| {
            source_a_clicks_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .into();
    let shared_action_id = source_a_action.id;
    let source_a_portal: Element<PortalVm> = Portal::new(source_a_action)
        .target_window("target")
        .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
        .into();
    let shared_portal_id = source_a_portal.id;

    let source_b_clicks_for_command = Arc::clone(&source_b_clicks);
    let mut source_b_action: Element<PortalVm> = Button::new("shared keyboard occurrence")
        .size(dp(180.0), dp(30.0))
        .on_click(Command::new(move |_: &mut PortalVm| {
            source_b_clicks_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .into();
    source_b_action.id = shared_action_id;
    let mut source_b_portal: Element<PortalVm> = Portal::new(source_b_action)
        .target_window("target")
        .anchor(Rect::new(20.0, 70.0, 1.0, 1.0))
        .into();
    source_b_portal.id = shared_portal_id;

    let mut source_a = portal_handler(
        "source-a",
        WidgetTree::new(source_a_portal),
        invalidation.clone(),
        (240.0, 160.0),
    );
    source_a.window_instance_id = 311;
    let mut source_b = portal_handler(
        "source-b",
        WidgetTree::new(source_b_portal),
        invalidation.clone(),
        (240.0, 160.0),
    );
    source_b.window_instance_id = 312;
    let mut target = portal_handler(
        "target",
        WidgetTree::new(Stack::<PortalVm>::new()),
        invalidation,
        (320.0, 220.0),
    );
    target.window_instance_id = 411;

    let mut registry = PortalRegistry::default();
    registry.publish_source(
        "source-a",
        source_a.external_portal_requests_from_computed(),
    );
    registry.publish_source(
        "source-b",
        source_b.external_portal_requests_from_computed(),
    );
    target.set_external_portal_requests(
        registry.requests_for_target("target"),
        registry.target_revision("target"),
    );
    let update = portal_accessibility_update(&mut target);
    let by_source = update
        .nodes
        .iter()
        .filter_map(|(node_id, node)| {
            (node.role() == Role::Button && node.label() == Some("shared keyboard occurrence"))
                .then(|| {
                    target
                        .accessibility_node_registry
                        .live_route(*node_id)
                        .map(|route| (route.source_window_instance_id, *node_id))
                })
                .flatten()
        })
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(by_source.len(), 2);

    target
        .accessibility_action_sender
        .send(portal_action_request(by_source[&312], Action::Focus))
        .unwrap();
    assert!(target.drain_accessibility_actions());
    assert!(target.activate_focused_widget(true, false));
    assert_eq!(source_a_clicks.load(Ordering::SeqCst), 0);
    assert_eq!(source_b_clicks.load(Ordering::SeqCst), 1);

    target
        .accessibility_action_sender
        .send(portal_action_request(by_source[&311], Action::Focus))
        .unwrap();
    assert!(target.drain_accessibility_actions());
    assert!(target.activate_focused_widget(false, true));
    assert_eq!(source_a_clicks.load(Ordering::SeqCst), 1);
    assert_eq!(source_b_clicks.load(Ordering::SeqCst), 1);
}

#[test]
fn cross_window_accessibility_rechecks_source_open_between_queued_actions() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let open = context.state(true);
    let clicks = Arc::new(AtomicUsize::new(0));
    let clicks_for_command = Arc::clone(&clicks);
    let open_for_command = open.clone();
    let action: Element<PortalVm> = Button::new("close on first accessible click")
        .size(dp(190.0), dp(30.0))
        .on_click(Command::new(move |_: &mut PortalVm| {
            clicks_for_command.fetch_add(1, Ordering::SeqCst);
            open_for_command.set(false);
        }))
        .into();
    let source_tree = WidgetTree::new(
        Portal::new(action)
            .open(open.signal())
            .target_window("target")
            .anchor(Rect::new(20.0, 20.0, 1.0, 1.0)),
    );
    let mut source = portal_handler("source", source_tree, invalidation.clone(), (240.0, 160.0));
    source.window_instance_id = 321;
    let mut target = portal_handler(
        "target",
        WidgetTree::new(Stack::<PortalVm>::new()),
        invalidation,
        (320.0, 220.0),
    );
    target.window_instance_id = 421;
    target.set_external_portal_requests(source.external_portal_requests_from_computed(), 1);

    let opened = portal_accessibility_update(&mut target);
    let node_id = opened
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (node.role() == Role::Button && node.label() == Some("close on first accessible click"))
                .then_some(*node_id)
        })
        .expect("open external Portal should publish its button");
    for _ in 0..2 {
        target
            .accessibility_action_sender
            .send(portal_action_request(node_id, Action::Click))
            .unwrap();
    }
    assert!(target.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 1);
    assert!(!open.get());

    let closed = portal_accessibility_update(&mut target);
    assert!(!closed
        .nodes
        .iter()
        .any(|(candidate, _)| *candidate == node_id));
    target
        .accessibility_action_sender
        .send(portal_action_request(node_id, Action::Click))
        .unwrap();
    assert!(!target.drain_accessibility_actions());
    assert_eq!(clicks.load(Ordering::SeqCst), 1);
}

#[test]
fn cross_window_portal_publication_generation_retires_replaced_actions() {
    let invalidation = InvalidationSignal::new();
    let old_clicks = Arc::new(AtomicUsize::new(0));
    let new_clicks = Arc::new(AtomicUsize::new(0));
    let old_clicks_for_command = Arc::clone(&old_clicks);
    let old_action: Element<PortalVm> = Button::new("old remote action")
        .size(dp(160.0), dp(30.0))
        .on_click(Command::new(move |_: &mut PortalVm| {
            old_clicks_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .into();
    let shared_action_id = old_action.id;
    let old_portal: Element<PortalVm> = Portal::new(old_action)
        .target_window("target")
        .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
        .into();
    let shared_portal_id = old_portal.id;

    let new_clicks_for_command = Arc::clone(&new_clicks);
    let mut new_action: Element<PortalVm> = Button::new("new remote action")
        .size(dp(160.0), dp(30.0))
        .on_click(Command::new(move |_: &mut PortalVm| {
            new_clicks_for_command.fetch_add(1, Ordering::SeqCst);
        }))
        .into();
    new_action.id = shared_action_id;
    let mut new_portal: Element<PortalVm> = Portal::new(new_action)
        .target_window("target")
        .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
        .into();
    new_portal.id = shared_portal_id;

    let mut source = portal_handler(
        "source",
        WidgetTree::new(old_portal),
        invalidation.clone(),
        (240.0, 160.0),
    );
    source.window_instance_id = 331;
    source.root_view = Some(Arc::new(move |_: &PortalVm| new_portal.clone()));
    let mut target = portal_handler(
        "target",
        WidgetTree::new(Stack::<PortalVm>::new()),
        invalidation,
        (320.0, 220.0),
    );
    target.window_instance_id = 431;
    let mut registry = PortalRegistry::default();

    assert_eq!(
        registry.publish_source("source", source.external_portal_requests_from_computed()),
        vec!["target".to_string()]
    );
    let old_revision = registry.target_revision("target");
    target.set_external_portal_requests(registry.requests_for_target("target"), old_revision);
    let old_update = portal_accessibility_update(&mut target);
    let old_node_id = old_update
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (node.role() == Role::Button && node.label() == Some("old remote action"))
                .then_some(*node_id)
        })
        .expect("old Portal publication should be accessible");

    assert!(source.rebuild_widget_tree_from_root_view());
    assert_eq!(
        registry.publish_source("source", source.external_portal_requests_from_computed()),
        vec!["target".to_string()]
    );
    let new_revision = registry.target_revision("target");
    assert_ne!(new_revision, old_revision);
    target.set_external_portal_requests(registry.requests_for_target("target"), new_revision);

    target
        .accessibility_action_sender
        .send(portal_action_request(old_node_id, Action::Click))
        .unwrap();
    assert!(!target.drain_accessibility_actions());
    assert_eq!(old_clicks.load(Ordering::SeqCst), 0);
    assert_eq!(new_clicks.load(Ordering::SeqCst), 0);

    let new_update = portal_accessibility_update(&mut target);
    assert!(!new_update
        .nodes
        .iter()
        .any(|(node_id, _)| *node_id == old_node_id));
    let new_node_id = new_update
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (node.role() == Role::Button && node.label() == Some("new remote action"))
                .then_some(*node_id)
        })
        .expect("replacement Portal publication should be accessible");
    assert_ne!(new_node_id, old_node_id);
    target
        .accessibility_action_sender
        .send(portal_action_request(new_node_id, Action::Click))
        .unwrap();
    assert!(target.drain_accessibility_actions());
    assert_eq!(old_clicks.load(Ordering::SeqCst), 0);
    assert_eq!(new_clicks.load(Ordering::SeqCst), 1);
}

#[test]
fn external_portal_request_rebuild_updates_every_window_and_keeps_target_services() {
    let invalidation = InvalidationSignal::new();
    let switched = Arc::new(AtomicBool::new(false));
    let source_root: crate::application::RootViewFactory<PortalVm> = Arc::new({
        let switched = Arc::clone(&switched);
        move |_: &PortalVm| {
            let label = if switched.load(Ordering::SeqCst) {
                "rebuilt remote action"
            } else {
                "initial remote action"
            };
            let switched_for_command = Arc::clone(&switched);
            Portal::new(Button::new(label).size(dp(170.0), dp(30.0)).on_click(
                Command::new_with_context(move |_: &mut PortalVm, context| {
                    switched_for_command.store(true, Ordering::SeqCst);
                    context.request_rebuild();
                    context.window().minimize();
                }),
            ))
            .target_window("target")
            .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
            .into()
        }
    });
    let target_root: crate::application::RootViewFactory<PortalVm> = Arc::new({
        let switched = Arc::clone(&switched);
        move |_: &PortalVm| {
            Text::new(if switched.load(Ordering::SeqCst) {
                "rebuilt target root"
            } else {
                "initial target root"
            })
            .into()
        }
    });

    let mut source = portal_handler(
        "source",
        WidgetTree::new(source_root(&PortalVm)),
        invalidation.clone(),
        (240.0, 160.0),
    );
    source.root_view = Some(Arc::clone(&source_root));
    source.window_instance_id = 351;
    let mut target = portal_handler(
        "target",
        WidgetTree::new(target_root(&PortalVm)),
        invalidation,
        (320.0, 220.0),
    );
    target.root_view = Some(target_root);
    target.window_instance_id = 451;
    let source_generation = source.portal_publication_generation;
    let mut registry = PortalRegistry::default();
    registry.publish_source("source", source.external_portal_requests_from_computed());
    target.set_external_portal_requests(
        registry.requests_for_target("target"),
        registry.target_revision("target"),
    );

    let initial = portal_accessibility_update(&mut target);
    let initial_node = initial
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (node.role() == Role::Button && node.label() == Some("initial remote action"))
                .then_some(*node_id)
        })
        .expect("initial external action should be accessible");
    target
        .accessibility_action_sender
        .send(portal_action_request(initial_node, Action::Click))
        .unwrap();
    assert!(target.drain_accessibility_actions());
    assert_eq!(
        target.window_requests.drain(),
        vec![crate::foundation::window_control::WindowRequest::Minimize]
    );
    assert!(source.window_requests.drain().is_empty());
    assert!(target
        .computed_scene()
        .scene
        .texts
        .iter()
        .any(|text| text.content.as_ref() == "rebuilt target root"));

    let _ = source.computed_scene();
    assert_eq!(
        source.portal_publication_generation,
        source_generation.wrapping_add(1)
    );
    assert_eq!(
        registry.publish_source("source", source.external_portal_requests_from_computed()),
        vec!["target".to_string()]
    );
    target.set_external_portal_requests(
        registry.requests_for_target("target"),
        registry.target_revision("target"),
    );
    let rebuilt = portal_accessibility_update(&mut target);
    assert!(rebuilt.nodes.iter().any(|(_, node)| {
        node.role() == Role::Button && node.label() == Some("rebuilt remote action")
    }));
    assert!(!rebuilt
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("initial remote action")));
}

#[test]
fn portal_registry_self_target_and_cycle_reach_a_stable_fixed_point() {
    let invalidation = InvalidationSignal::new();
    let self_tree = WidgetTree::new(
        Portal::<PortalVm>::new(Text::new("self portal"))
            .target_window("self")
            .anchor(Rect::new(10.0, 10.0, 1.0, 1.0)),
    );
    let mut self_handler = portal_handler("self", self_tree, invalidation.clone(), (240.0, 160.0));
    self_handler.window_instance_id = 341;
    let mut self_registry = PortalRegistry::default();
    assert_eq!(
        self_registry.publish_source(
            "self",
            self_handler.external_portal_requests_from_computed(),
        ),
        vec!["self".to_string()]
    );
    let self_revision = self_registry.target_revision("self");
    self_handler
        .set_external_portal_requests(self_registry.requests_for_target("self"), self_revision);
    for _ in 0..3 {
        self_handler.invalidate_computed_scene();
        assert!(self_registry
            .publish_source(
                "self",
                self_handler.external_portal_requests_from_computed(),
            )
            .is_empty());
        assert_eq!(self_registry.target_revision("self"), self_revision);
        self_handler
            .set_external_portal_requests(self_registry.requests_for_target("self"), self_revision);
    }

    let a_tree = WidgetTree::new(
        Portal::<PortalVm>::new(Text::new("a to b"))
            .target_window("b")
            .anchor(Rect::new(10.0, 10.0, 1.0, 1.0)),
    );
    let b_tree = WidgetTree::new(
        Portal::<PortalVm>::new(Text::new("b to a"))
            .target_window("a")
            .anchor(Rect::new(10.0, 10.0, 1.0, 1.0)),
    );
    let mut a = portal_handler("a", a_tree, invalidation.clone(), (240.0, 160.0));
    a.window_instance_id = 342;
    let mut b = portal_handler("b", b_tree, invalidation, (240.0, 160.0));
    b.window_instance_id = 343;
    let mut cycle_registry = PortalRegistry::default();
    cycle_registry.publish_source("a", a.external_portal_requests_from_computed());
    cycle_registry.publish_source("b", b.external_portal_requests_from_computed());
    let a_revision = cycle_registry.target_revision("a");
    let b_revision = cycle_registry.target_revision("b");
    a.set_external_portal_requests(cycle_registry.requests_for_target("a"), a_revision);
    b.set_external_portal_requests(cycle_registry.requests_for_target("b"), b_revision);
    for _ in 0..3 {
        a.invalidate_computed_scene();
        b.invalidate_computed_scene();
        assert!(cycle_registry
            .publish_source("a", a.external_portal_requests_from_computed())
            .is_empty());
        assert!(cycle_registry
            .publish_source("b", b.external_portal_requests_from_computed())
            .is_empty());
        assert_eq!(cycle_registry.target_revision("a"), a_revision);
        assert_eq!(cycle_registry.target_revision("b"), b_revision);
        a.set_external_portal_requests(cycle_registry.requests_for_target("a"), a_revision);
        b.set_external_portal_requests(cycle_registry.requests_for_target("b"), b_revision);
    }
}

#[test]
fn cross_window_portal_focus_trap_hides_target_base_accessibility_tree() {
    let invalidation = InvalidationSignal::new();
    let remote: Element<PortalVm> = Button::new("trapped remote")
        .size(dp(120.0), dp(30.0))
        .into();
    let source_tree = WidgetTree::new(
        Portal::new(remote)
            .target_window("target")
            .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
            .focus_scope(FocusScopeOptions::new().trap(true)),
    );
    let base: Element<PortalVm> = Button::new("target base action")
        .size(dp(120.0), dp(30.0))
        .into();
    let base_id = base.id;
    let mut source = portal_handler("source", source_tree, invalidation.clone(), (240.0, 160.0));
    source.window_instance_id = 501;
    let mut target = portal_handler(
        "target",
        WidgetTree::new(base),
        invalidation,
        (320.0, 220.0),
    );
    target.window_instance_id = 502;
    target.set_external_portal_requests(source.external_portal_requests_from_computed(), 1);

    let update = portal_accessibility_update(&mut target);
    assert!(update
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("trapped remote")));
    assert!(!update
        .nodes
        .iter()
        .any(|(node_id, _)| { *node_id == crate::accessibility::node_id_from_widget(base_id) }));
}

#[test]
fn cross_window_focus_scopes_are_source_qualified() {
    let invalidation = InvalidationSignal::new();
    let action: Element<PortalVm> = Button::new("shared trapped occurrence")
        .size(dp(170.0), dp(30.0))
        .into();
    let portal: Element<PortalVm> = Portal::new(action)
        .target_window("target")
        .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
        .focus_scope(FocusScopeOptions::new().trap(true))
        .into();
    let mut source_a = portal_handler(
        "source-a",
        WidgetTree::new(portal.clone()),
        invalidation.clone(),
        (240.0, 160.0),
    );
    source_a.window_instance_id = 361;
    let mut source_b = portal_handler(
        "source-b",
        WidgetTree::new(portal),
        invalidation.clone(),
        (240.0, 160.0),
    );
    source_b.window_instance_id = 362;
    let source_a_requests = source_a.external_portal_requests_from_computed();
    let source_b_requests = source_b.external_portal_requests_from_computed();
    let source_a_scope = source_a_requests[0]
        .focus_scope_instance_id
        .expect("source A trap scope identity");
    let source_b_scope = source_b_requests[0]
        .focus_scope_instance_id
        .expect("source B trap scope identity");
    assert_ne!(source_a_scope, source_b_scope);
    assert_ne!(source_a_scope, source_a_requests[0].source_widget_id);
    assert_ne!(source_b_scope, source_b_requests[0].source_widget_id);

    let mut registry = PortalRegistry::default();
    registry.publish_source("source-a", source_a_requests);
    registry.publish_source("source-b", source_b_requests);
    let mut target = portal_handler(
        "target",
        WidgetTree::new(Stack::<PortalVm>::new()),
        invalidation,
        (320.0, 220.0),
    );
    target.window_instance_id = 461;
    target.set_external_portal_requests(
        registry.requests_for_target("target"),
        registry.target_revision("target"),
    );
    let computed = target.computed_scene().clone();
    let scope_by_source = computed
        .accessibility_fragments
        .iter()
        .filter_map(|fragment| {
            Some((
                fragment.source_window_instance_id?,
                *fragment.scope_path.first()?,
            ))
        })
        .collect::<std::collections::HashMap<_, _>>();
    assert_eq!(scope_by_source[&361], source_a_scope);
    assert_eq!(scope_by_source[&362], source_b_scope);

    let trapped = portal_accessibility_update(&mut target);
    let visible_sources = trapped
        .nodes
        .iter()
        .filter_map(|(node_id, node)| {
            (node.role() == Role::Button && node.label() == Some("shared trapped occurrence"))
                .then(|| {
                    target
                        .accessibility_node_registry
                        .live_route(*node_id)
                        .map(|route| route.source_window_instance_id)
                })
                .flatten()
        })
        .collect::<Vec<_>>();
    assert_eq!(visible_sources, vec![362]);

    registry.remove_source("source-b");
    target.set_external_portal_requests(
        registry.requests_for_target("target"),
        registry.target_revision("target"),
    );
    let remaining = portal_accessibility_update(&mut target);
    assert!(remaining.nodes.iter().any(|(node_id, node)| {
        node.role() == Role::Button
            && node.label() == Some("shared trapped occurrence")
            && target
                .accessibility_node_registry
                .live_route(*node_id)
                .is_some_and(|route| route.source_window_instance_id == 361)
    }));
}

#[test]
fn cross_window_focus_scope_identity_is_retired_when_portal_closes() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let open = context.state(true);
    let source_tree = WidgetTree::new(
        Portal::<PortalVm>::new(Button::new("reopen trapped occurrence"))
            .open(open.signal())
            .target_window("target")
            .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
            .focus_scope(FocusScopeOptions::new().trap(true)),
    );
    let mut source = portal_handler("source", source_tree, invalidation, (240.0, 160.0));
    source.window_instance_id = 371;
    let first_scope = source.external_portal_requests_from_computed()[0]
        .focus_scope_instance_id
        .expect("first focus scope identity");

    open.set(false);
    source.invalidate_computed_scene();
    assert!(source.external_portal_requests_from_computed().is_empty());
    assert!(source.external_portal_focus_scopes.is_empty());

    open.set(true);
    source.invalidate_computed_scene();
    let reopened_scope = source.external_portal_requests_from_computed()[0]
        .focus_scope_instance_id
        .expect("reopened focus scope identity");
    assert_ne!(reopened_scope, first_scope);
}

#[test]
fn external_portal_return_focus_is_fail_closed_against_target_id_collisions() {
    let invalidation = InvalidationSignal::new();
    let source_return_target: Element<PortalVm> = Button::new("source return target").into();
    let source_return_id = source_return_target.id;
    let source_tree = WidgetTree::new(
        Stack::<PortalVm>::new().child([
            source_return_target,
            Portal::new(Button::new("remote close action"))
                .target_window("target")
                .anchor(Rect::new(20.0, 20.0, 1.0, 1.0))
                .on_open_change(ValueCommand::new(|_: &mut PortalVm, _: bool| {}))
                .return_focus_to(source_return_id)
                .into(),
        ]),
    );
    let mut colliding_target: Element<PortalVm> = Button::new("target id collision").into();
    colliding_target.id = source_return_id;
    let mut source = portal_handler("source", source_tree, invalidation.clone(), (240.0, 160.0));
    let mut target = portal_handler(
        "target",
        WidgetTree::new(colliding_target),
        invalidation,
        (320.0, 220.0),
    );
    target.set_external_portal_requests(source.external_portal_requests_from_computed(), 1);
    let computed = target.computed_scene();
    assert!(computed
        .overlay_close_handlers
        .iter()
        .any(|handler| handler.return_focus_to.is_none()));
    assert!(!computed
        .overlay_close_handlers
        .iter()
        .any(|handler| handler.return_focus_to == Some(source_return_id)));
}

#[test]
fn nested_external_portal_requests_are_one_hop_fail_closed() {
    let invalidation = InvalidationSignal::new();
    let outer_action: Element<PortalVm> = Button::new("outer remote action")
        .size(dp(150.0), dp(30.0))
        .into();
    let outer_action_id = outer_action.id;
    let outer_content = Stack::<PortalVm>::new().child([
        outer_action,
        Portal::new(Text::new("nested remote content"))
            .target_window("nested-target")
            .anchor(Rect::new(40.0, 40.0, 1.0, 1.0))
            .into(),
    ]);
    let source_tree = WidgetTree::new(
        Portal::new(outer_content)
            .target_window("target")
            .anchor(Rect::new(20.0, 20.0, 1.0, 1.0)),
    );
    let mut source = portal_handler("source", source_tree, invalidation.clone(), (240.0, 160.0));
    let mut target = portal_handler(
        "target",
        WidgetTree::new(Stack::<PortalVm>::new()),
        invalidation,
        (320.0, 220.0),
    );
    target.set_external_portal_requests(source.external_portal_requests_from_computed(), 1);
    let computed = target.computed_scene().clone();
    assert!(overlay_labels(&computed).contains(&"outer remote action".to_string()));
    assert!(computed.overlay_hit_regions.iter().any(|hit| {
        matches!(hit.interaction, HitInteraction::Widget { id, .. } if id == outer_action_id)
    }));
    assert!(computed.external_portal_requests.is_empty());
    assert!(!overlay_labels(&computed).contains(&"nested remote content".to_string()));
}

#[test]
fn external_portal_without_source_identity_keeps_visuals_but_omits_accessibility() {
    let invalidation = InvalidationSignal::new();
    let source_tree = WidgetTree::new(
        Portal::<PortalVm>::new(Button::new("legacy remote").size(dp(120.0), dp(30.0)))
            .target_window("target")
            .anchor(Rect::new(20.0, 20.0, 1.0, 1.0)),
    );
    let mut source = portal_handler("source", source_tree, invalidation.clone(), (240.0, 160.0));
    let mut target = portal_handler(
        "target",
        WidgetTree::new(Stack::<PortalVm>::new()),
        invalidation,
        (320.0, 220.0),
    );
    let mut requests = source.external_portal_requests_from_computed();
    requests[0].source_window_instance_id = None;
    target.set_external_portal_requests(requests, 1);

    let computed = target.computed_scene().clone();
    assert!(overlay_labels(&computed).contains(&"legacy remote".to_string()));
    assert!(computed
        .overlay_hit_regions
        .iter()
        .any(|hit| matches!(hit.interaction, HitInteraction::Widget { .. })));
    let update = portal_accessibility_update(&mut target);
    assert!(!update
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("legacy remote")));
}

#[test]
fn portal_registry_remove_source_clears_empty_source_bookkeeping() {
    let invalidation = InvalidationSignal::new();
    let source_tree = WidgetTree::new(
        Portal::<PortalVm>::new(Text::new("temporary remote"))
            .target_window("target")
            .anchor(Rect::new(20.0, 20.0, 1.0, 1.0)),
    );
    let mut source = portal_handler("source", source_tree, invalidation, (240.0, 160.0));
    let mut registry = PortalRegistry::default();
    registry.publish_source("source", source.external_portal_requests_from_computed());
    registry.publish_source("source", Vec::new());
    assert!(registry.has_source_registration("source"));

    assert!(registry.remove_source("source").is_empty());
    assert!(!registry.has_source_registration("source"));
}
