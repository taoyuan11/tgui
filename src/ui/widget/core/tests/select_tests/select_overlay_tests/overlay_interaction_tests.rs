use super::*;

#[test]
fn select_dropdown_highlights_pressed_option() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let select: Element<ScopeChildVm> = Select::new(
        vec![
            SelectOption::new("email".to_string(), "Email".to_string()),
            SelectOption::new("sms".to_string(), "SMS".to_string()),
        ],
        None::<String>,
    )
    .open(true)
    .size(dp(180.0), dp(32.0))
    .into();
    let select_id = select.id;
    let tree = WidgetTree::new(Stack::new().child(select));
    let mut widget_states = WidgetStateMap::default();
    widget_states.set_select_option(
        select_id,
        1,
        crate::ui::theme::WidgetState {
            pressed: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 140.0),
        None,
        None,
        None,
        None,
        false,
    );
    let hovered_options = rendered
        .primitives
        .overlay_shapes
        .iter()
        .filter(|shape| {
            shape.rect.y > dp(60.0)
                && shape.rect.height
                    == UnitContext::default().resolve_dp(
                        default_select_style(&theme, crate::ui::theme::WidgetState::default())
                            .option_height,
                    )
                && shape.color.a > 0
        })
        .collect::<Vec<_>>();

    assert_eq!(hovered_options.len(), 1);
}

#[test]
fn select_dropdown_pressed_highlight_preserves_menu_corner_clip() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let select: Element<ScopeChildVm> = Select::new(
        vec![
            SelectOption::new("email".to_string(), "Email".to_string()),
            SelectOption::new("sms".to_string(), "SMS".to_string()),
        ],
        None::<String>,
    )
    .open(true)
    .size(dp(180.0), dp(32.0))
    .into();
    let select_id = select.id;
    let tree = WidgetTree::new(Stack::new().child(select));
    let mut widget_states = WidgetStateMap::default();
    widget_states.set_select_option(
        select_id,
        0,
        crate::ui::theme::WidgetState {
            pressed: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &widget_states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 140.0),
        None,
        None,
        None,
        None,
        false,
    );
    let select_style = default_select_style(&theme, crate::ui::theme::WidgetState::default());
    let option_height = UnitContext::default().resolve_dp(select_style.option_height);
    let menu_radius = select_style.radius.get();
    let highlight = rendered
        .primitives
        .overlay_shapes
        .iter()
        .find(|shape| shape.rect.y > dp(20.0) && shape.rect.height == option_height)
        .expect("pressed option highlight should render");

    assert_eq!(
        highlight.clip_mask,
        Some(ClipMask {
            rect: Rect::new(
                highlight.rect.x,
                highlight.rect.y,
                highlight.rect.width,
                option_height * 2.0,
            ),
            corner_radius: menu_radius,
        })
    );
}

#[test]
fn select_dropdown_animates_open_and_close() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let select: Element<()> = Select::<(), String, String>::new(
        vec![
            SelectOption::new("email".to_string(), "Email".to_string()),
            SelectOption::new("sms".to_string(), "SMS".to_string()),
        ],
        None::<String>,
    )
    .size(dp(180.0), dp(32.0))
    .into();
    let select_id = select.id;
    let tree = WidgetTree::new(Stack::new().child(select));
    let viewport = Rect::new(0.0, 0.0, 180.0, 140.0);
    let start = Instant::now();

    let closed = tree.compute_scene_with_units_and_widget_state_at(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
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
        start,
    );
    assert!(closed.scene.overlay_shapes.is_empty());

    let mut open_states = HashMap::new();
    open_states.insert(select_id, true);

    let _opening_start = tree.compute_scene_with_units_and_widget_state_at(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
        None,
        None,
        &WidgetStateMap::default(),
        &open_states,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
        start,
    );

    let opening = tree.compute_scene_with_units_and_widget_state_at(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
        None,
        None,
        &WidgetStateMap::default(),
        &open_states,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
        start + Duration::from_millis(80),
    );
    let opening_menu = opening
        .scene
        .overlay_shapes
        .iter()
        .find(|shape| shape.rect.height > dp(32.0))
        .expect("opening select menu should render");
    assert!(opening_menu.rect.height > dp(0.0));
    assert!(opening_menu.rect.height < dp(80.0));
    assert!(opening_menu.color.a > 0);
    assert!(opening.overlay_hit_regions.is_empty());

    animations.refresh(start + Duration::from_millis(200));

    let open = tree.compute_scene_with_units_and_widget_state_at(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
        None,
        None,
        &WidgetStateMap::default(),
        &open_states,
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
        start + Duration::from_millis(200),
    );
    assert!(open
        .overlay_hit_regions
        .iter()
        .any(|hit| matches!(hit.interaction, super::HitInteraction::SelectOption { .. })));

    let _closing_start = tree.compute_scene_with_units_and_widget_state_at(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
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
        start + Duration::from_millis(200),
    );

    let closing = tree.compute_scene_with_units_and_widget_state_at(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
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
        start + Duration::from_millis(280),
    );
    let closing_menu = closing
        .scene
        .overlay_shapes
        .iter()
        .find(|shape| shape.rect.height > dp(0.0))
        .expect("closing select menu should still render during animation");
    assert!(
        closing_menu.rect.height
            < open
                .scene
                .overlay_shapes
                .iter()
                .find(|shape| shape.rect.height > dp(32.0))
                .expect("open menu background should render")
                .rect
                .height
    );
    assert!(closing.overlay_hit_regions.is_empty());

    animations.refresh(start + Duration::from_millis(500));

    let settled_closed = tree.compute_scene_with_units_and_widget_state_at(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
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
        start + Duration::from_millis(500),
    );
    assert!(settled_closed.scene.overlay_shapes.is_empty());
}

