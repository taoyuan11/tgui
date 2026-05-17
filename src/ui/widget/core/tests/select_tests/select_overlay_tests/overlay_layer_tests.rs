use super::*;

#[test]
fn select_dropdown_escapes_parent_overflow_clip() {
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
    .placeholder("Choose")
    .on_change(ValueCommand::new(
        |vm: &mut ScopeChildVm, (key, value): (String, String)| {
            vm.selected_key = key;
            vm.selected_value = value;
        },
    ))
    .open(true)
    .size(dp(180.0), dp(40.0))
    .into();
    let tree = WidgetTree::new(
        Stack::new()
            .size(dp(180.0), dp(45.0))
            .overflow(Overflow::Hidden)
            .child(select),
    );
    let widget_states = WidgetStateMap::default();

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
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.rect.y > dp(40.0) && shape.rect.bottom() > dp(45.0)));

    let hit = tree.hit_test_with_widget_state(
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
        Some(Point::new(8.0, 58.0)),
        None,
    );
    let mut vm = ScopeChildVm::default();
    match hit {
        Some(super::HitInteraction::SelectOption {
            on_select: Some(command),
            ..
        }) => command.execute(&mut vm),
        _ => panic!("select option outside parent clip should be hit"),
    }
    assert_eq!(vm.selected_key, "email");
}

#[test]
fn select_dropdown_stays_above_later_slider_decorations() {
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
    .size(dp(180.0), dp(40.0))
    .into();
    let slider: Element<ScopeChildVm> = Slider::new(50.0, 0.0, 100.0)
        .size(dp(180.0), dp(40.0))
        .show_ticks(true)
        .into();
    let tree = WidgetTree::new(
        crate::ui::widget::Flex::new(Axis::Vertical)
            .gap(dp(0.0))
            .child([select, slider]),
    );
    let widget_states = WidgetStateMap::default();

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

    assert!(
        !rendered.primitives.overlay_shapes.is_empty(),
        "open select should render menu in overlay layer"
    );
    assert!(
        rendered.primitives.overlay_textures.is_empty(),
        "later slider decorations should not leak into overlay layer"
    );
}

#[test]
fn select_dropdown_stays_above_later_media_placeholder() {
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
    .size(dp(180.0), dp(40.0))
    .into();
    let image_frame = Rect::new(0.0, 40.0, 180.0, 40.0);
    let tree = WidgetTree::new(
        crate::ui::widget::Flex::new(Axis::Vertical)
            .gap(dp(0.0))
            .child([
                select,
                Image::from_bytes(vec![0_u8; 4])
                    .size(dp(180.0), dp(40.0))
                    .into(),
            ]),
    );
    let widget_states = WidgetStateMap::default();

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

    assert!(
        rendered
            .primitives
            .overlay_shapes
            .iter()
            .all(|shape| shape.rect != image_frame),
        "media placeholders should not render in the overlay layer"
    );
    assert!(
        rendered
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.rect == image_frame),
        "media placeholder should still render in the normal scene"
    );
}
