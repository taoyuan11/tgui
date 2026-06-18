pub(super) use super::*;

use crate::ui::widget::{
    Button, ComputedScene, FocusScopeOptions, LayerStack, OverlayAlignment, OverlayAnchorKey,
    OverlayLayer, OverlayPlacement, Portal, PortalAnchor, PortalTarget,
};

fn compute_portal_scene(tree: &WidgetTree<()>, viewport: Rect) -> ComputedScene<()> {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    )
}

#[test]
fn portal_builder_defaults_and_public_reexports() {
    let portal = Portal::<()>::new(Text::new("floating"));
    assert!(portal.open.resolve());
    assert_eq!(portal.target, PortalTarget::CurrentWindow);
    assert_eq!(portal.anchor, None);
    assert_eq!(portal.layer, OverlayLayer::Popover);
    assert!(!portal.close_on_escape);
    assert!(!portal.close_on_outside_click);

    let stack = LayerStack::window("secondary", OverlayLayer::Toast);
    assert_eq!(
        stack.target(),
        &PortalTarget::WindowKey("secondary".to_string())
    );
    assert_eq!(stack.layer(), OverlayLayer::Toast);

    let element: Element<()> = Portal::new(Text::new("floating"))
        .stack(stack)
        .anchor(PortalAnchor::Key(OverlayAnchorKey::point()))
        .placement(OverlayPlacement::bottom().align(OverlayAlignment::Start))
        .into();
    let WidgetKind::Portal {
        target,
        anchor,
        layer,
        ..
    } = element.kind
    else {
        panic!("Portal should resolve to WidgetKind::Portal");
    };
    assert_eq!(target, PortalTarget::WindowKey("secondary".to_string()));
    assert_eq!(anchor, Some(PortalAnchor::Key(OverlayAnchorKey::point())));
    assert_eq!(layer, OverlayLayer::Toast);

    let _: crate::prelude::PortalTarget = crate::widgets::PortalTarget::CurrentWindow;
    let _: crate::prelude::PortalAnchor = crate::widgets::PortalAnchor::Viewport;
    let _: crate::prelude::LayerStack =
        crate::widgets::LayerStack::current(crate::prelude::OverlayLayer::Popover);
    let _: crate::prelude::OverlayAnchorKey = crate::widgets::OverlayAnchorKey::point();
}

#[test]
fn current_window_portal_escapes_parent_overflow_clip() {
    let action: Element<()> = Button::new("floating action")
        .size(dp(140.0), dp(32.0))
        .on_click(Command::new(|_: &mut ()| {}))
        .into();
    let action_id = action.id;
    let tree = WidgetTree::new(
        Stack::<()>::new()
            .size(dp(120.0), dp(1.0))
            .overflow(Overflow::Hidden)
            .child(Portal::new(action)),
    );

    let computed = compute_portal_scene(&tree, Rect::new(0.0, 0.0, 240.0, 120.0));
    let label = computed
        .scene
        .overlay_texts
        .iter()
        .find(|text| text.content.as_ref() == "floating action")
        .expect("portal content should render in the overlay scene");
    assert!(
        label.frame.y > dp(1.0),
        "portal text should be outside the clipped parent frame: {:?}",
        label.frame
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
        .expect("portal button should expose an overlay hit region");
    assert!(
        hit.rect.y > dp(1.0),
        "portal hit region should escape parent clip: {:?}",
        hit.rect
    );

    let point = Point::new(
        hit.rect.x + hit.rect.width * 0.5,
        hit.rect.y + hit.rect.height * 0.5,
    );
    let hit = WidgetTree::hit_path_from_computed(&computed, point).pop();
    assert!(
        matches!(hit, Some(HitInteraction::Widget { id, .. }) if id == action_id),
        "portal content outside the parent clip should remain hittable"
    );
}

#[test]
fn portal_layer_order_is_fixed_low_to_high() {
    let children: [Element<()>; 5] = [
        Portal::<()>::new(Text::new("toast"))
            .anchor(Rect::new(10.0, 10.0, 1.0, 1.0))
            .layer(OverlayLayer::Toast)
            .into(),
        Portal::<()>::new(Text::new("tooltip"))
            .anchor(Rect::new(10.0, 10.0, 1.0, 1.0))
            .layer(OverlayLayer::Tooltip)
            .into(),
        Portal::<()>::new(Text::new("modal"))
            .anchor(Rect::new(10.0, 10.0, 1.0, 1.0))
            .layer(OverlayLayer::Modal)
            .into(),
        Portal::<()>::new(Text::new("popover"))
            .anchor(Rect::new(10.0, 10.0, 1.0, 1.0))
            .layer(OverlayLayer::Popover)
            .into(),
        Portal::<()>::new(Text::new("menu"))
            .anchor(Rect::new(10.0, 10.0, 1.0, 1.0))
            .layer(OverlayLayer::Menu)
            .into(),
    ];
    let tree = WidgetTree::new(Stack::<()>::new().child(children));

    let computed = compute_portal_scene(&tree, Rect::new(0.0, 0.0, 320.0, 180.0));
    let labels: Vec<_> = computed
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref())
        .filter(|label| matches!(*label, "tooltip" | "popover" | "menu" | "modal" | "toast"))
        .collect();
    assert_eq!(labels, vec!["tooltip", "popover", "menu", "modal", "toast"]);
}

#[test]
fn portal_registers_hit_close_and_focus_scope() {
    let close_calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let close_calls_cmd = close_calls.clone();
    let action: Element<()> = Button::new("inside portal")
        .size(dp(120.0), dp(32.0))
        .on_click(Command::new(|_: &mut ()| {}))
        .into();
    let action_id = action.id;
    let tree = WidgetTree::new(
        Portal::new(action)
            .anchor(Rect::new(24.0, 24.0, 1.0, 1.0))
            .layer(OverlayLayer::Menu)
            .close_on_escape(true)
            .close_on_outside_click(true)
            .on_open_change(ValueCommand::new(move |_: &mut (), open| {
                close_calls_cmd.lock().unwrap().push(open);
            }))
            .focus_scope(FocusScopeOptions::new().trap(true)),
    );

    let computed = compute_portal_scene(&tree, Rect::new(0.0, 0.0, 320.0, 180.0));
    let close = computed
        .overlay_close_handlers
        .iter()
        .find(|handler| handler.layer == OverlayLayer::Menu)
        .expect("portal should register a close handler");
    assert!(close.close_on_escape);
    assert!(close.close_on_outside_click);

    assert!(
        computed
            .focus_scopes
            .iter()
            .any(|scope| scope.active && scope.options.is_trap()),
        "portal focus_scope option should register an active trap"
    );
    assert!(
        computed.overlay_hit_regions.iter().any(|region| {
            matches!(
                &region.interaction,
                HitInteraction::Widget { id, .. } if *id == action_id
            )
        }),
        "interactive portal content should be in overlay hit regions"
    );
    assert!(
        close_calls.lock().unwrap().is_empty(),
        "collecting the portal should not eagerly close it"
    );
}

#[test]
fn portal_open_false_cleans_overlay_outputs() {
    let tree = WidgetTree::new(
        Portal::<()>::new(
            Button::new("closed portal")
                .size(dp(120.0), dp(32.0))
                .on_click(Command::new(|_: &mut ()| {})),
        )
        .open(false)
        .anchor(Rect::new(24.0, 24.0, 1.0, 1.0))
        .close_on_escape(true)
        .close_on_outside_click(true)
        .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {}))
        .focus_scope(FocusScopeOptions::new().trap(true)),
    );

    let computed = compute_portal_scene(&tree, Rect::new(0.0, 0.0, 320.0, 180.0));
    assert!(computed.scene.overlay_texts.is_empty());
    assert!(computed.overlay_hit_regions.is_empty());
    assert!(computed.overlay_close_handlers.is_empty());
    assert!(computed.focus_scopes.is_empty());
    assert!(computed.external_portal_requests.is_empty());
}

#[test]
fn cross_window_self_widget_anchor_emits_no_portal_request() {
    let tree = WidgetTree::new(
        Portal::<()>::new(Text::new("invalid remote anchor"))
            .target_window("target")
            .anchor(PortalAnchor::SelfWidget),
    );

    let computed = compute_portal_scene(&tree, Rect::new(0.0, 0.0, 320.0, 180.0));
    assert!(computed.scene.overlay_texts.is_empty());
    assert!(
        computed.external_portal_requests.is_empty(),
        "cross-window SelfWidget anchors cannot be resolved in target coordinates"
    );
}
