use super::*;
use crate::theme::WidgetState;
use crate::ui::layout::Value;

#[test]
fn default_select_keeps_active_border_for_focus_and_open_state() {
    for theme in [Theme::light(), Theme::dark()] {
        let focused = default_select_style(
            &theme,
            WidgetState {
                hovered: true,
                focused: true,
                ..Default::default()
            },
        );
        assert_eq!(focused.border, theme.colors.primary);
        assert!(focused.focus_ring.is_none());

        let keyboard_focused = default_select_style(
            &theme,
            WidgetState {
                focused: true,
                focus_visible: true,
                ..Default::default()
            },
        );
        assert_eq!(keyboard_focused.border, theme.colors.primary);
        assert_eq!(keyboard_focused.focus_ring, Some(theme.focus_ring.clone()));

        let open = default_select_style(
            &theme,
            WidgetState {
                open: true,
                hovered: true,
                ..Default::default()
            },
        );
        assert_eq!(open.border, theme.colors.primary);
        assert!(open.focus_ring.is_none());
    }
}

#[test]
fn default_select_arrow_stays_neutral_across_interactive_states() {
    for theme in [Theme::light(), Theme::dark()] {
        let style = crate::ui::widget::SelectStyle::default_for_theme(&theme);
        let normal = style.arrow.resolve(WidgetState::default()).resolve();
        assert_eq!(normal, theme.colors.on_surface_muted);
        for state in [
            WidgetState {
                hovered: true,
                ..Default::default()
            },
            WidgetState {
                pressed: true,
                ..Default::default()
            },
            WidgetState {
                focused: true,
                ..Default::default()
            },
            WidgetState {
                open: true,
                ..Default::default()
            },
        ] {
            assert_eq!(style.arrow.resolve(state).resolve(), normal);
        }
        assert_eq!(
            style
                .arrow
                .resolve(WidgetState {
                    disabled: true,
                    ..Default::default()
                })
                .resolve(),
            theme.colors.on_disabled
        );
    }
}

fn select_arrow_texture_id(
    tree: &WidgetTree<()>,
    theme: &Theme,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    state: WidgetState,
    reduced_motion: bool,
) -> u64 {
    let select_id = tree.root.id;
    let mut states = WidgetStateMap::default();
    states.set(select_id, state);
    tree.compute_scene_with_widget_state(
        &FontManager::new(&FontCatalog::default()),
        theme,
        media,
        animations,
        reduced_motion,
        None,
        None,
        &states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    )
    .scene
    .textures
    .first()
    .expect("select arrow texture")
    .texture
    .id()
}

#[test]
fn default_select_arrow_reuses_texture_for_pressed_full_recollect() {
    let theme = Theme::light();
    let media = test_media();
    let tree: WidgetTree<()> = WidgetTree::new(
        Select::<(), String, String>::new(
            vec![SelectOption::new("email".into(), "Email".into())],
            None::<String>,
        )
        .size(dp(180.0), dp(40.0)),
    );
    let normal = select_arrow_texture_id(
        &tree,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        WidgetState::default(),
        true,
    );
    let pressed = select_arrow_texture_id(
        &tree,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        WidgetState {
            pressed: true,
            ..Default::default()
        },
        true,
    );
    assert_eq!(normal, pressed);
}

#[test]
fn custom_select_pressed_arrow_color_remains_supported() {
    let theme = Theme::light();
    let media = test_media();
    let tree: WidgetTree<()> = WidgetTree::new(
        Select::<(), String, String>::new(
            vec![SelectOption::new("email".into(), "Email".into())],
            None::<String>,
        )
        .style(|style, context| {
            style.arrow.pressed = Value::Static(context.theme.colors.error);
        })
        .size(dp(180.0), dp(40.0)),
    );
    let normal = select_arrow_texture_id(
        &tree,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        WidgetState::default(),
        true,
    );
    let pressed = select_arrow_texture_id(
        &tree,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        WidgetState {
            pressed: true,
            ..Default::default()
        },
        true,
    );
    assert_ne!(normal, pressed);
}

#[test]
fn select_renders_placeholder_and_arrow_when_unselected() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Select::<(), String, String>::new(
            vec![SelectOption::new("email".to_string(), "Email".to_string())],
            None::<String>,
        )
        .placeholder("Choose one")
        .size(dp(180.0), dp(40.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .texts
        .iter()
        .any(|text| text.content.as_ref() == "Choose one"));
    assert!(rendered
        .primitives
        .textures
        .iter()
        .any(|texture| texture.frame.x > dp(140.0) && texture.opacity > 0.0));
    assert!(rendered
        .primitives
        .texts
        .iter()
        .all(|text| text.content.as_ref() != "keyboard_arrow_down"));
}

#[test]
fn selected_option_label_updates_without_rebuilding_the_select() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation, AnimationCoordinator::default());
    let selected_key = context.state(Some("email".to_string()));
    let label = context.state("Email".to_string());
    let tree: WidgetTree<()> = WidgetTree::new(
        Select::<(), String, String>::new(
            vec![
                SelectOption::new("email".to_string(), "fallback".to_string())
                    .label(label.signal()),
            ],
            selected_key.signal(),
        )
        .size(dp(180.0), dp(40.0)),
    );
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let render_texts = |animations: &mut AnimationEngine| {
        tree.render_output(
            &font_manager,
            &theme,
            &media,
            animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 180.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        )
        .primitives
        .texts
        .into_iter()
        .map(|text| text.content.to_string())
        .collect::<Vec<_>>()
    };

    assert!(render_texts(&mut animations)
        .iter()
        .any(|text| text == "Email"));
    label.set("Electronic mail".to_string());
    assert!(
        render_texts(&mut animations)
            .iter()
            .any(|text| text == "Electronic mail"),
        "a selected option's reactive label must update in the retained WidgetTree"
    );
}

#[test]
fn disabled_select_exposes_disabled_hit_for_cursor_only() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Select::<(), String, String>::new(
            vec![SelectOption::new("email".to_string(), "Email".to_string())],
            None::<String>,
        )
        .disable(true)
        .size(dp(180.0), dp(40.0)),
    );

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 40.0),
        Some(Point::new(10.0, 10.0)),
        None,
    );
    assert!(matches!(hit, Some(super::HitInteraction::Disabled { .. })));
}

#[test]
fn focused_select_opens_upward_and_hits_enabled_and_disabled_options() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let select: Element<ScopeChildVm> = Select::new(
        vec![
            SelectOption::new("email".to_string(), "Email".to_string()),
            SelectOption::new("sms".to_string(), "SMS".to_string()).disable(true),
            SelectOption::new("phone".to_string(), "Phone".to_string()),
        ],
        Some("email".to_string()),
    )
    .on_change(ValueCommand::new(
        |vm: &mut ScopeChildVm, (key, value): (String, String)| {
            vm.selected_key = key;
            vm.selected_value = value;
        },
    ))
    .open(true)
    .size(dp(180.0), dp(40.0))
    .position_absolute()
    .top(dp(50.0))
    .into();
    let tree = WidgetTree::new(Stack::new().child(select));
    let widget_states = WidgetStateMap::default();
    let viewport = Rect::new(0.0, 0.0, 220.0, 90.0);

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.rect.y < dp(50.0) && shape.rect.height > dp(40.0)));
    let overlay_hit_point = |hit: &crate::ui::widget::HitRegion<ScopeChildVm>| {
        let visible = hit
            .clip_rect
            .and_then(|clip| hit.rect.intersect(clip))
            .unwrap_or(hit.rect);
        Point::new(visible.x + dp(8.0), visible.y + visible.height * 0.5)
    };
    let computed = tree.compute_scene_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    );
    let enabled_point = computed
        .overlay_hit_regions
        .iter()
        .find_map(|hit| match &hit.interaction {
            super::HitInteraction::SelectOption {
                option_index: 0, ..
            } => Some(overlay_hit_point(hit)),
            _ => None,
        })
        .expect("enabled option hit region should be present");
    let (disabled_point, disabled_rect, disabled_clip) = computed
        .overlay_hit_regions
        .iter()
        .find_map(|hit| match &hit.interaction {
            super::HitInteraction::Disabled { .. } => {
                Some((overlay_hit_point(hit), hit.rect, hit.clip_rect))
            }
            _ => None,
        })
        .expect("disabled option hit region should be present");
    let _ = (disabled_rect, disabled_clip);

    let enabled_hit = tree.hit_test_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        Some(enabled_point),
        None,
    );
    let mut vm = ScopeChildVm::default();
    match enabled_hit {
        Some(super::HitInteraction::SelectOption {
            on_select: Some(command),
            ..
        }) => command.execute(&mut vm),
        _ => panic!("enabled select option should be hit"),
    }
    assert_eq!(vm.selected_key, "email");
    assert_eq!(vm.selected_value, "Email");

    let disabled_hit = tree.hit_test_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        Some(disabled_point),
        None,
    );
    match disabled_hit {
        Some(super::HitInteraction::Disabled { .. }) => {}
        Some(super::HitInteraction::SelectOption { option_index, .. }) => {
            panic!("disabled option point hit enabled option {option_index}")
        }
        Some(_) => panic!("disabled option point hit a non-disabled interaction"),
        None => panic!("disabled option point should hit disabled interaction"),
    }
}
