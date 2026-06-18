pub(super) use super::*;

use std::time::Duration;

use crate::animation::Transition;
use crate::foundation::view_model::ValueCommand;
use crate::ui::widget::{Modal, ModalAction, ModalStyle};

#[test]
fn modal_builder_attaches_descriptor() {
    let element: Element<()> = Modal::new(true)
        .title("Hello")
        .action(ModalAction::primary("OK"))
        .into();
    assert!(
        element.modal.is_some(),
        "modal descriptor must be attached to outer Stack element"
    );
    let descriptor = element.modal.as_ref().unwrap();
    assert!(descriptor.open.resolve(), "open should resolve to true");
    assert!(descriptor.close_on_escape);
    assert!(descriptor.close_on_backdrop_click);
}

#[test]
fn modal_closed_renders_minimal() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Modal::new(false)
            .title("Closed Title")
            .content(Text::new("Hidden content"))
            .action(ModalAction::primary("OK")),
    );
    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 600.0, 400.0),
        None,
        None,
        None,
        None,
        false,
    );

    // 关闭状态下，modal 内的 title / content / action 文本不能可见。
    let labels: Vec<&str> = rendered
        .primitives
        .texts
        .iter()
        .filter(|t| t.color.a > 0)
        .map(|t| t.content.as_ref())
        .collect();
    assert!(
        !labels.iter().any(|t| *t == "Closed Title"),
        "closed modal title should not be visible, got {labels:?}"
    );
    assert!(
        !labels.iter().any(|t| *t == "Hidden content"),
        "closed modal content should not be visible, got {labels:?}"
    );
    assert!(
        !labels.iter().any(|t| *t == "OK"),
        "closed modal action should not be visible, got {labels:?}"
    );
    assert!(
        rendered.primitives.overlay_texts.is_empty(),
        "closed modal should not emit overlay texts, got {:?}",
        rendered
            .primitives
            .overlay_texts
            .iter()
            .map(|t| t.content.as_ref())
            .collect::<Vec<_>>()
    );
}

#[test]
fn modal_open_renders_title_and_action_labels() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Modal::new(true)
            .title("Confirm")
            .content(Text::new("Are you sure?"))
            .action(ModalAction::new("Cancel"))
            .action(ModalAction::primary("OK")),
    );
    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 600.0, 400.0),
        None,
        None,
        None,
        None,
        false,
    );

    // Modal 是 in-tree 渲染，所以 title / content / action 都进入主 scene texts。
    let labels: Vec<&str> = rendered
        .primitives
        .texts
        .iter()
        .map(|t| t.content.as_ref())
        .collect();
    assert!(
        labels.iter().any(|t| *t == "Confirm"),
        "title 'Confirm' should be rendered, got {labels:?}"
    );
    assert!(
        labels.iter().any(|t| *t == "Are you sure?"),
        "content 'Are you sure?' should be rendered, got {labels:?}"
    );
    assert!(
        labels.iter().any(|t| *t == "Cancel"),
        "action 'Cancel' should be rendered, got {labels:?}"
    );
    assert!(
        labels.iter().any(|t| *t == "OK"),
        "primary action 'OK' should be rendered, got {labels:?}"
    );
}

#[test]
fn modal_open_registers_focus_trap_on_outer_scope() {
    let modal_element: Element<()> = Modal::new(true)
        .title("X")
        .action(ModalAction::primary("OK"))
        .into();

    assert!(
        modal_element
            .focus
            .scope
            .as_ref()
            .map(|scope| scope.is_trap() && scope.is_auto_focus_first())
            .unwrap_or(false),
        "outer modal widget must have active trap/autofocus focus scope"
    );
}

#[test]
fn modal_with_on_open_change_keeps_descriptor_attached() {
    let element: Element<()> = Modal::new(true)
        .on_open_change(ValueCommand::new(|_: &mut (), _: bool| {}))
        .close_on_backdrop_click(false)
        .into();
    let descriptor = element.modal.as_ref().expect("descriptor exists");
    assert!(descriptor.on_open_change.is_some());
    assert!(!descriptor.close_on_backdrop_click);
}

#[test]
fn modal_style_defaults_include_enter_scale() {
    let style = ModalStyle::default_for_theme(&Theme::light());
    assert!((style.enter_scale - 0.96).abs() < f32::EPSILON);
}

#[test]
fn visual_scale_changes_hit_rect_about_center() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Stack::<()>::new()
            .size(dp(100.0), dp(50.0))
            .scale(0.5)
            .on_click(Command::new(|_: &mut ()| {})),
    );

    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );
    let rect = computed
        .hit_regions
        .iter()
        .find_map(|hit| match hit.interaction {
            HitInteraction::Widget { .. } => Some(hit.rect),
            _ => None,
        })
        .expect("scaled widget should remain hittable");

    assert_eq!(rect.x, dp(25.0));
    assert_eq!(rect.y, dp(12.5));
    assert_eq!(rect.width, dp(50.0));
    assert_eq!(rect.height, dp(25.0));
}

#[test]
fn visual_scale_reduced_motion_uses_target_without_transition() {
    let invalidation = InvalidationSignal::new();
    let scale = crate::foundation::binding::State::new(0.5_f32, invalidation.clone());
    let animated_scale = scale
        .signal()
        .animated(Transition::ease_in_out(Duration::from_millis(160)));
    let tree: WidgetTree<()> = WidgetTree::new(
        Stack::<()>::new()
            .size(dp(100.0), dp(50.0))
            .scale(animated_scale)
            .on_click(Command::new(|_: &mut ()| {})),
    );

    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 200.0, 120.0);

    let mut animations = AnimationEngine::default();
    let _seed = tree.compute_scene(
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
    );
    scale.set(1.0);

    let normal = tree.compute_scene(
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
    );
    let normal_rect = normal
        .hit_regions
        .iter()
        .find_map(|hit| match hit.interaction {
            HitInteraction::Widget { .. } => Some(hit.rect),
            _ => None,
        })
        .expect("scaled widget should remain hittable");
    assert_eq!(normal_rect.width, dp(50.0));

    let mut animations = AnimationEngine::default();
    let reduced = tree.compute_scene(
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
        true,
    );
    let reduced_rect = reduced
        .hit_regions
        .iter()
        .find_map(|hit| match hit.interaction {
            HitInteraction::Widget { .. } => Some(hit.rect),
            _ => None,
        })
        .expect("scaled widget should remain hittable");
    assert_eq!(reduced_rect.width, dp(100.0));
    assert_eq!(reduced_rect.height, dp(50.0));
}
