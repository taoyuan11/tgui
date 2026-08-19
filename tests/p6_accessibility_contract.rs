use std::cell::Cell;
use std::rc::Rc;

use tgui::accessibility::{
    ActionKind, NativeSemanticsBridge, NodeId, Role, Semantics, compare_accessibility_snapshots,
};
use tgui::core::{ItemKey, SemanticRevision, Size};
use tgui::event::{AccessibilityAction, EventHandler, EventPhase, UiEvent};
use tgui::virtualization::{CollectionSemantics, ItemSemantics};
use tgui::widget::WidgetNode;
use tgui::{Application, WindowSpec};

struct Root;
struct Item;

fn item(key: &'static str, name: &'static str, activations: Rc<Cell<usize>>) -> WidgetNode {
    WidgetNode::new::<Item>()
        .with_key(key)
        .with_focusable(true)
        .with_semantics(
            Semantics::new(Role::Button)
                .with_name(name)
                .with_focusable(true)
                .with_actions([ActionKind::Activate, ActionKind::Focus]),
        )
        .with_event_handler(EventHandler::new(1, move |event, context| {
            if context.phase() == EventPhase::Target
                && matches!(
                    event,
                    UiEvent::AccessibilityAction(action)
                        if matches!(action.action, AccessibilityAction::Activate)
                )
            {
                activations.set(activations.get() + 1);
            }
            Ok(())
        }))
}

fn root(children: impl IntoIterator<Item = WidgetNode>) -> WidgetNode {
    WidgetNode::new::<Root>()
        .with_semantics(Semantics::new(Role::List))
        .with_children(children)
}

#[test]
fn application_commits_semantics_and_routes_actions_to_stable_elements() {
    let activations = Rc::new(Cell::new(0));
    let first = item("first", "First", activations.clone());
    let second = item("second", "Second", activations.clone());
    let mut application = Application::new();
    let window = application
        .create_window(WindowSpec::new("accessibility").with_inner_size(Size::new(300.0, 200.0)))
        .unwrap();
    application
        .mount_widget(window, root([first.clone(), second.clone()]))
        .unwrap();

    let initial = application.layout_window(window).unwrap();
    assert!(initial.semantics.performed);
    assert_eq!(
        initial.semantics.snapshot.revision(),
        SemanticRevision::new(1)
    );
    assert_eq!(initial.semantics.snapshot.node_count(), 3);
    let first_node = initial
        .semantics
        .snapshot
        .nodes()
        .iter()
        .find(|node| node.semantics().name() == Some("First"))
        .unwrap();
    let stable_id = first_node.id();
    let stable_element = first_node.element();

    application
        .dispatch_accessibility_action(window, stable_id, AccessibilityAction::Activate)
        .unwrap()
        .unwrap();
    assert_eq!(activations.get(), 1);

    let idle = application.layout_window(window).unwrap();
    assert!(!idle.semantics.performed);
    assert_eq!(idle.semantics.snapshot.revision(), SemanticRevision::new(1));

    application
        .reconcile_widget(window, root([second, first]))
        .unwrap();
    let reordered = application.layout_window(window).unwrap();
    let retained = reordered
        .semantics
        .snapshot
        .node_for_element(stable_element)
        .unwrap();
    assert_eq!(retained.id(), stable_id);
    assert_eq!(retained.semantics().name(), Some("First"));

    application
        .dispatch_accessibility_action(window, stable_id, AccessibilityAction::Focus)
        .unwrap()
        .unwrap();
    let focused = application.layout_window(window).unwrap();
    assert_eq!(focused.semantics.snapshot.focus(), Some(stable_id));
    assert!(
        focused
            .semantics
            .snapshot
            .node(stable_id)
            .unwrap()
            .is_focused()
    );

    let rebuilt = focused.semantics.snapshot.clone();
    compare_accessibility_snapshots(&focused.semantics.snapshot, &rebuilt).unwrap();
}

#[test]
fn stale_or_unsupported_semantic_actions_are_ignored() {
    let mut application = Application::new();
    let window = application.create_window(WindowSpec::new("stale")).unwrap();
    application
        .mount_widget(
            window,
            WidgetNode::new::<Root>().with_semantics(Semantics::new(Role::Group)),
        )
        .unwrap();
    application.layout_window(window).unwrap();

    let stale = NodeId::from_element(tgui::ElementId::from_parts(99, 1));
    assert!(
        application
            .dispatch_accessibility_action(window, stale, AccessibilityAction::Activate)
            .unwrap()
            .is_none()
    );
    let root = application
        .committed_snapshot(window)
        .unwrap()
        .semantics()
        .root()
        .unwrap();
    assert!(
        application
            .dispatch_accessibility_action(window, root, AccessibilityAction::Activate)
            .unwrap()
            .is_none()
    );
}

#[test]
fn virtual_list_semantics_preserve_collection_position_and_current_item() {
    let collection = CollectionSemantics {
        item_count: 100_000,
        current_item: Some(ItemKey::numeric(42)),
        selected_count: 1,
    }
    .accessibility();
    assert_eq!(collection.role(), Role::List);
    assert_eq!(collection.collection().unwrap().item_count, 100_000);

    let item = ItemSemantics {
        key: ItemKey::numeric(42),
        index: 42,
        position_in_set: 43,
        set_size: 100_000,
        selected: true,
        focused: true,
        current: true,
        materialized: true,
    }
    .accessibility();
    assert_eq!(item.role(), Role::ListItem);
    assert_eq!(item.collection_item().unwrap().position_in_set, 43);
    assert!(item.collection_item().unwrap().current);
    assert!(item.is_focused());
}

#[cfg(feature = "accessibility")]
#[test]
fn accesskit_update_contains_native_subtree_graft() {
    use tgui::accessibility::{AccessibilityTree, SemanticNodeInput, SemanticUpdateReasons};

    let element = tgui::ElementId::from_parts(1, 1);
    let uuid = [7_u8; 16];
    let mut tree = AccessibilityTree::new();
    let snapshot = tree
        .update(
            [SemanticNodeInput::new(
                element,
                Semantics::new(Role::NativeHost)
                    .with_native_bridge(NativeSemanticsBridge::Subtree(uuid)),
            )],
            None,
            SemanticUpdateReasons::SEMANTICS,
        )
        .unwrap()
        .snapshot;
    let update = snapshot.to_accesskit_update().unwrap();
    assert_eq!(update.nodes.len(), 1);
    assert_eq!(
        update.nodes[0].1.tree_id(),
        Some(accesskit::TreeId(accesskit::Uuid::from_bytes(uuid)))
    );
}

#[cfg(feature = "accessibility")]
#[test]
fn accesskit_action_resolves_the_committed_element() {
    use accesskit::{Action, ActionRequest, TreeId};
    use tgui::accessibility::{AccessibilityTree, SemanticNodeInput, SemanticUpdateReasons};

    let element = tgui::ElementId::from_parts(4, 2);
    let mut tree = AccessibilityTree::new();
    let snapshot = tree
        .update(
            [SemanticNodeInput::new(
                element,
                Semantics::new(Role::Button).with_action(ActionKind::Activate),
            )],
            None,
            SemanticUpdateReasons::SEMANTICS,
        )
        .unwrap()
        .snapshot;
    let request = ActionRequest {
        action: Action::Click,
        target_tree: TreeId::ROOT,
        target_node: accesskit::NodeId(NodeId::from_element(element).get()),
        data: None,
    };
    let event = tgui::accessibility::accesskit_action_event(
        tgui::WindowId::from_parts(1, 1),
        &snapshot,
        &request,
    )
    .unwrap()
    .unwrap();
    assert!(matches!(
        event,
        UiEvent::AccessibilityAction(action)
            if action.target == element && matches!(action.action, AccessibilityAction::Activate)
    ));
}

#[test]
fn native_opaque_bridge_remains_available_without_adapter_feature() {
    let semantics =
        Semantics::new(Role::NativeHost).with_native_bridge(NativeSemanticsBridge::Opaque);
    assert_eq!(
        semantics.native_bridge(),
        Some(&NativeSemanticsBridge::Opaque)
    );
}
