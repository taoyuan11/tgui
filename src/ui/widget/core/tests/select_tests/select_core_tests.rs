use super::*;

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
        .any(|text| text.content == "Choose one"));
    assert!(rendered
        .primitives
        .meshes
        .iter()
        .any(|mesh| mesh.vertices.len() == 3));
    assert!(rendered
        .primitives
        .texts
        .iter()
        .all(|text| text.font_family.as_deref() != Some("tgui-icons")));
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
    let disabled_point = computed
        .overlay_hit_regions
        .iter()
        .find_map(|hit| match &hit.interaction {
            super::HitInteraction::Disabled { .. } => Some(overlay_hit_point(hit)),
            _ => None,
        })
        .expect("disabled option hit region should be present");

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
    assert!(matches!(
        disabled_hit,
        Some(super::HitInteraction::Disabled { .. })
    ));
}
