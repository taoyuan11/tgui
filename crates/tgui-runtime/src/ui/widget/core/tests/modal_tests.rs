pub(super) use super::*;

use std::time::Duration;

use crate::animation::{AnimationKey, Transition, WidgetProperty};
use crate::foundation::view_model::ValueCommand;
use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::ui::widget::{ComputedScene, Modal, ModalAction, ModalStyle};

fn modal_scene_at(
    tree: &WidgetTree<()>,
    theme: &Theme,
    font_manager: &FontManager,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    reduced_motion: bool,
    viewport: Rect,
    now: Instant,
) -> ComputedScene<()> {
    tree.compute_scene_with_units_and_widget_state_at(
        font_manager,
        theme,
        media,
        UnitContext::default(),
        animations,
        reduced_motion,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
        now,
    )
}

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
    let theme = Theme::light();
    let style = ModalStyle::default_for_theme(&theme);
    assert!((style.enter_scale - 0.96).abs() < f32::EPSILON);
    assert_eq!(style.title_text_style, theme.typography.title);
    assert!(style
        .title_text_style
        .line_height
        .is_some_and(|line_height| line_height > style.title_text_style.size));
}

#[test]
fn modal_title_primitive_preserves_theme_title_typography_across_modes_and_density() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();

    for mut theme in [Theme::light(), Theme::dark()] {
        for density in [Density::Compact, Density::Comfortable, Density::Spacious] {
            theme.density = density;
            let tree: WidgetTree<()> =
                WidgetTree::new(Modal::new(true).title("Agjpq dialog title"));
            let rendered = tree.render_output(
                &font_manager,
                &theme,
                &media,
                &mut AnimationEngine::default(),
                None,
                None,
                &HashMap::new(),
                Rect::new(0.0, 0.0, 720.0, 480.0),
                None,
                None,
                None,
                None,
                false,
            );
            let title = rendered
                .primitives
                .texts
                .iter()
                .find(|text| text.content.as_ref() == "Agjpq dialog title")
                .expect("Modal title primitive");
            assert_eq!(title.font_size, theme.typography.title.size.get());
            assert_eq!(title.font_weight, theme.typography.title.weight);
            assert!(
                title.line_height
                    >= theme
                        .typography
                        .title
                        .line_height
                        .expect("default title line height")
                        .get()
            );
            assert!(title.line_height > title.font_size);
            assert!(title.frame.height.get() >= title.line_height);
        }
    }
}

#[test]
fn modal_density_geometry_reaches_real_scene_primitives() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 900.0, 700.0);

    for (density, expected_width) in [
        (Density::Compact, dp(272.0)),
        (Density::Spacious, dp(360.0)),
    ] {
        let mut theme = Theme::light();
        theme.density = density;
        let modal_style = ModalStyle::default_for_density(&theme, density);
        let mut animations = AnimationEngine::default();
        let tree: WidgetTree<()> = WidgetTree::new(
            Modal::new(true)
                .title("Density-aware dialog")
                .style_full(move |_| modal_style.clone()),
        );
        let computed = tree.compute_scene(
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

        let card = computed
            .scene
            .shapes
            .iter()
            .find(|shape| {
                shape.color == theme.colors.outline_muted
                    && (shape.rect.width - expected_width).abs() <= dp(0.1)
            })
            .unwrap_or_else(|| {
                panic!(
                    "modal card should use density width {expected_width:?}; shapes={:?}",
                    computed
                        .scene
                        .shapes
                        .iter()
                        .map(|shape| (shape.rect, shape.color, shape.corner_radius))
                        .collect::<Vec<_>>()
                )
            });
        assert_eq!(card.corner_radius, theme.radius.xl.get());
        assert!(
            card.rect.x >= theme.spacing.md,
            "modal card should retain a viewport margin: {:?}",
            card.rect
        );
    }
}

#[test]
fn modal_runtime_geometry_and_enter_scale_follow_the_same_tree() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 900.0, 700.0);
    let modal = |open| {
        Modal::new(open)
            .title("Runtime modal")
            .content(Text::new("Body"))
            .action(ModalAction::primary("OK"))
            .style(|style, context| match context.density {
                Density::Compact => {
                    style.min_width = dp(250.0);
                    style.max_width = dp(290.0);
                    style.max_height = dp(360.0);
                    style.margin = Insets::all(dp(8.0));
                    style.padding = Insets::all(dp(3.0));
                    style.title_padding = Insets::all(dp(5.0));
                    style.content_padding = Insets::all(dp(7.0));
                    style.actions_padding = Insets::all(dp(9.0));
                    style.actions_gap = dp(4.0);
                    style.enter_scale = 0.88;
                }
                Density::Comfortable => {}
                Density::Spacious => {
                    style.min_width = dp(340.0);
                    style.max_width = dp(420.0);
                    style.max_height = dp(520.0);
                    style.margin = Insets::all(dp(20.0));
                    style.padding = Insets::all(dp(11.0));
                    style.title_padding = Insets::all(dp(13.0));
                    style.content_padding = Insets::all(dp(15.0));
                    style.actions_padding = Insets::all(dp(17.0));
                    style.actions_gap = dp(12.0);
                    style.enter_scale = 0.94;
                }
            })
    };
    let open_tree: WidgetTree<()> = WidgetTree::new(modal(true));
    let closed_tree: WidgetTree<()> = WidgetTree::new(modal(false));

    for (
        mut theme,
        min_width,
        max_width,
        max_height,
        margin,
        card_padding,
        title_padding,
        content_padding,
        actions_padding,
        actions_gap,
        enter_scale,
    ) in [
        (
            Theme::light(),
            dp(250.0),
            dp(290.0),
            dp(360.0),
            dp(8.0),
            dp(3.0),
            dp(5.0),
            dp(7.0),
            dp(9.0),
            dp(4.0),
            0.88,
        ),
        (
            Theme::dark(),
            dp(340.0),
            dp(420.0),
            dp(520.0),
            dp(20.0),
            dp(11.0),
            dp(13.0),
            dp(15.0),
            dp(17.0),
            dp(12.0),
            0.94,
        ),
    ] {
        theme.density = if matches!(theme.mode, crate::ui::theme::ResolvedThemeMode::Light) {
            Density::Compact
        } else {
            Density::Spacious
        };
        let mut animations = AnimationEngine::default();
        let layout = open_tree.build_scene_layout(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            viewport,
        );
        let ResolvedWidgetKind::Container { children, .. } = &layout.resolved_root.kind else {
            panic!("modal root should remain a container");
        };
        let card = &children[1];
        assert_eq!(
            card.layout.min_width,
            Some(Value::Static(crate::ui::layout::Length::Px(min_width)))
        );
        assert_eq!(
            card.layout.max_width,
            Some(Value::Static(crate::ui::layout::Length::Px(max_width)))
        );
        assert_eq!(
            card.layout.max_height,
            Some(Value::Static(crate::ui::layout::Length::Px(max_height)))
        );
        assert_eq!(card.layout.margin, Value::Static(Insets::all(margin)));
        let ResolvedWidgetKind::Container {
            layout: card_container,
            children: card_children,
            ..
        } = &card.kind
        else {
            panic!("modal card should remain a container");
        };
        assert_eq!(
            card_container.padding,
            Some(Value::Static(Insets::all(card_padding)))
        );
        for (child, expected_padding) in
            card_children
                .iter()
                .zip([title_padding, content_padding, actions_padding])
        {
            let ResolvedWidgetKind::Container { layout, .. } = &child.kind else {
                panic!("modal section should remain a container");
            };
            assert_eq!(
                layout.padding,
                Some(Value::Static(Insets::all(expected_padding)))
            );
        }
        let ResolvedWidgetKind::Container {
            layout: actions_layout,
            ..
        } = &card_children[2].kind
        else {
            panic!("modal actions should remain a container");
        };
        assert_eq!(
            actions_layout.gap,
            Value::Static(crate::ui::layout::Length::Px(actions_gap))
        );

        let mut animations = AnimationEngine::default();
        let closed = closed_tree.build_scene_layout(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            viewport,
        );
        let ResolvedWidgetKind::Container { children, .. } = &closed.resolved_root.kind else {
            panic!("modal root should remain a container");
        };
        assert!(
            (children[1].visual.scale.resolve() - enter_scale).abs() <= f32::EPSILON,
            "closed modal should use the active theme's enter scale"
        );
    }
}

#[test]
fn modal_motion_uses_live_theme_tokens_and_reduced_motion_settles_slots() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 640.0, 420.0);
    let context = test_context();
    let open = context.state(false);
    let inside: Element<()> = crate::ui::widget::Button::new("inside").into();
    let inside_id = inside.id;
    let modal: Element<()> = Modal::new(open.signal()).content(inside).into();
    let backdrop_id = Modal::_backdrop_id_of(&modal).expect("modal backdrop id");
    let card_id = Modal::_card_id_of(&modal).expect("modal card id");
    let tree: WidgetTree<()> = WidgetTree::new(modal);
    let mut theme = Theme::light();
    theme.motion.normal_ms = 200;
    let start = Instant::now();
    let mut animations = AnimationEngine::default();

    let _ = modal_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        start,
    );
    open.set(true);
    let animation_start = start + Duration::from_millis(1);
    let _ = modal_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        animation_start,
    );
    for key in [
        AnimationKey::Widget {
            id: backdrop_id.raw(),
            property: WidgetProperty::Opacity,
        },
        AnimationKey::Widget {
            id: card_id.raw(),
            property: WidgetProperty::Opacity,
        },
        AnimationKey::Widget {
            id: card_id.raw(),
            property: WidgetProperty::Scale,
        },
    ] {
        assert!(animations.contains_key(key));
    }

    let mid = animation_start + Duration::from_millis(100);
    let refresh = animations.refresh(mid);
    assert!(refresh.changed && !refresh.layout_changed);
    assert!(refresh.scene_widget_ids.contains(&backdrop_id.raw()));
    assert!(refresh.scene_widget_ids.contains(&card_id.raw()));
    let _ = modal_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        animation_start + Duration::from_millis(199),
    );
    assert!(animations.has_active_animations());
    let _ = modal_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        animation_start + Duration::from_millis(201),
    );
    assert!(!animations.has_active_animations());

    open.set(false);
    let close_start = animation_start + Duration::from_millis(202);
    let closing = modal_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        viewport,
        close_start,
    );
    assert!(closing.hit_regions.iter().all(|hit| {
        !matches!(hit.interaction, HitInteraction::Widget { id, .. } if id == inside_id)
    }));
    assert!(animations.has_active_animations());
    let _reduced = modal_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        true,
        viewport,
        close_start + Duration::from_millis(1),
    );
    assert!(!animations.has_active_animations());
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
