use super::*;

#[test]
fn switch_renders_custom_track_and_thumb_colors() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let active_background = Color::hexa(0x10B981FF);
    let inactive_background = Color::hexa(0x475569FF);
    let active_thumb = Color::hexa(0xECFDF5FF);
    let tree: WidgetTree<()> = WidgetTree::new(Switch::new(true).size(dp(52.0), dp(30.0)).style(
        move |mode| {
            switch_style(
                mode,
                active_background,
                inactive_background,
                Some(active_thumb),
                None,
            )
        },
    ));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == active_background));
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.color == active_thumb));

    let inactive_tree: WidgetTree<()> = WidgetTree::new(
        Switch::new(false)
            .size(dp(52.0), dp(30.0))
            .style(move |mode| {
                switch_style(
                    mode,
                    active_background,
                    inactive_background,
                    None,
                    Some(Color::WHITE),
                )
            }),
    );
    let inactive_render = inactive_tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(inactive_render
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == inactive_background));
}

#[test]
fn focused_switch_keeps_pressed_colors_and_renders_focus_ring() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let switch: Element<()> = Switch::new(true).into();
    let switch_id = switch.id;
    let tree: WidgetTree<()> = WidgetTree::new(switch);
    let mut state = WidgetStateMap::default();
    state.set(
        switch_id,
        crate::ui::theme::WidgetState {
            hovered: true,
            pressed: true,
            focused: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut AnimationEngine::default(),
        false,
        None,
        None,
        &state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 80.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let switch_style = default_switch_style(&theme);
    let base_state = crate::ui::theme::WidgetState {
        hovered: true,
        pressed: true,
        ..Default::default()
    };
    let focused_state = crate::ui::theme::WidgetState {
        hovered: true,
        pressed: true,
        focused: true,
        ..Default::default()
    };

    assert!(rendered.primitives.shapes.iter().any(|shape| shape.color
        == super::resolve_stateful_widget_color(&switch_style.track_checked, base_state)));
    assert_eq!(
        super::resolve_stateful_widget_color(&switch_style.track_checked, focused_state),
        super::resolve_stateful_widget_color(&switch_style.track_checked, base_state)
    );
    assert_eq!(
        super::resolve_stateful_widget_color(&switch_style.border_checked, focused_state),
        super::resolve_stateful_widget_color(&switch_style.border_checked, base_state)
    );
    assert!(rendered
        .primitives
        .overlay_shapes
        .iter()
        .any(|shape| shape.stroke_width == theme.focus_ring.width.get()
            && shape.color == theme.focus_ring.color
            && shape.rect.width > dp(42.0)));
}

#[test]
fn button_focus_ring_override_changes_overlay_without_affecting_layout() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let button: Element<()> = crate::ui::widget::Button::new("focus")
        .style(|mode| {
            let mut style = ButtonStyle::default_for(
                mode,
                crate::ui::widget::common::ButtonVariantKind::Primary,
            );
            style.focus_ring = Some(crate::ui::widget::FocusRingOverride {
                color: Some(Color::hexa(0x22C55EFF)),
                width: Some(dp(3.0)),
                gap: Some(dp(4.0)),
                enabled: Some(true),
            });
            style
        })
        .size(dp(120.0), dp(40.0))
        .into();
    let button_id = button.id;
    let tree: WidgetTree<()> = WidgetTree::new(button);
    let mut state = WidgetStateMap::default();
    state.set(
        button_id,
        crate::ui::theme::WidgetState {
            focused: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
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
        .any(|shape| shape.stroke_width == 3.0
            && shape.color == Color::hexa(0x22C55EFF)
            && shape.rect.width > dp(120.0)
            && shape.rect.height > dp(40.0)));
}

#[test]
fn focus_ring_overlay_is_not_clipped() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let button: Element<()> = crate::ui::widget::Button::new("focus")
        .size(dp(120.0), dp(40.0))
        .into();
    let button_id = button.id;
    let tree: WidgetTree<()> = WidgetTree::new(button);
    let mut state = WidgetStateMap::default();
    state.set(
        button_id,
        crate::ui::theme::WidgetState {
            focused: true,
            ..Default::default()
        },
    );

    let rendered = tree.render_output_with_widget_state(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &state,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );

    let ring = rendered
        .primitives
        .overlay_shapes
        .iter()
        .find(|shape| shape.stroke_width == theme.focus_ring.width.get())
        .expect("focused button should render focus ring overlay");
    assert_eq!(ring.clip_rect, None);
    assert_eq!(ring.clip_mask, None);
}

#[test]
fn neutral_components_remain_transparent_by_default() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let tree: WidgetTree<()> = WidgetTree::new(
        Stack::new()
            .size(dp(120.0), dp(80.0))
            .child(Image::from_bytes(ONE_BY_ONE_GIF).size(dp(40.0), dp(40.0)))
            .child(Canvas::new(CanvasRecorder::build(|_| {})).size(dp(40.0), dp(20.0))),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 80.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered
        .primitives
        .shapes
        .iter()
        .all(|shape| shape.color.a == 0));
}

#[test]
fn switch_thumb_animates_between_positions() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let checked = context.state(false);
    let tree: WidgetTree<()> = WidgetTree::new(Switch::new(checked.signal().animated(
        crate::animation::Transition::ease_in_out(std::time::Duration::from_millis(180)),
    )));

    let mut animations = AnimationEngine::default();
    let initial = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 60.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let start_x = initial.primitives.overlay_shapes[0].rect.x;

    checked.set(true);
    let toggled = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 60.0, 40.0),
        None,
        None,
        None,
        None,
        false,
    );
    let immediate_x = toggled.primitives.overlay_shapes[0].rect.x;

    let mut sampled_transition = None;
    let mut end_x = immediate_x;

    for _ in 0..18 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 60.0, 40.0),
            None,
            None,
            None,
            None,
            false,
        );
        let current_x = rendered.primitives.overlay_shapes[0].rect.x;
        if current_x > start_x && current_x > immediate_x {
            sampled_transition = Some(current_x);
        }
        end_x = current_x;
        if current_x > start_x && (current_x - start_x).get() >= 20.0 {
            break;
        }
    }

    assert_eq!(immediate_x, start_x);
    assert!(end_x > start_x);
    if let Some(mid_x) = sampled_transition {
        assert!(mid_x > start_x);
        assert!(mid_x <= end_x);
    }
}
