use super::*;
use crate::foundation::binding::State;
use crate::ui::widget::ComputedScene;

#[test]
fn radio_group_renders_selected_option_and_dispatches_key_value() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<ScopeChildVm> = WidgetTree::new(
        RadioGroup::new(
            vec![
                ("email".to_string(), "Email".to_string()),
                ("sms".to_string(), "SMS".to_string()),
            ],
            "email".to_string(),
        )
        .on_change(ValueCommand::new(
            |vm: &mut ScopeChildVm, (key, value): (String, String)| {
                vm.selected_key = key;
                vm.selected_value = value;
            },
        )),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 80.0),
        None,
        None,
        None,
        None,
        false,
    );
    let indicator =
        default_radio_style(&theme, crate::ui::theme::WidgetState::default(), true).indicator;
    assert_eq!(
        rendered
            .primitives
            .overlay_shapes
            .iter()
            .filter(|shape| shape.color == indicator)
            .count(),
        1
    );

    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 80.0),
        Some(Point::new(4.0, 30.0)),
        None,
    );
    let mut vm = ScopeChildVm::default();
    match hit {
        Some(super::HitInteraction::Radio {
            on_change: Some(command),
            current,
            ..
        }) => {
            assert!(!current);
            command.execute(&mut vm, true);
        }
        _ => panic!("second radio should be hit"),
    }

    assert_eq!(vm.selected_key, "sms");
    assert_eq!(vm.selected_value, "SMS");
}

#[test]
fn radio_group_ignores_false_child_change_and_maps_direction() {
    let group: Element<ScopeChildVm> = RadioGroup::new(
        vec![
            ("email".to_string(), "Email".to_string()),
            ("sms".to_string(), "SMS".to_string()),
        ],
        "email".to_string(),
    )
    .horizontal()
    .on_change(ValueCommand::new(
        |vm: &mut ScopeChildVm, (key, value): (String, String)| {
            vm.selected_key = key;
            vm.selected_value = value;
        },
    ))
    .into();

    match &group.kind {
        WidgetKind::Container { layout, .. } => match &layout.kind {
            ContainerKind::Flex { direction, .. } => {
                assert_eq!(*direction, Axis::Horizontal);
            }
            _ => panic!("radio group should render as flex"),
        },
        _ => panic!("radio group should render as container"),
    }

    let tree = WidgetTree::new(group);
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 40.0),
        Some(Point::new(4.0, 4.0)),
        None,
    );
    let mut vm = ScopeChildVm::default();
    match hit {
        Some(super::HitInteraction::Radio {
            on_change: Some(command),
            current,
            ..
        }) => {
            assert!(current);
            command.execute(&mut vm, false);
        }
        _ => panic!("first radio should be hit"),
    }

    assert!(vm.selected_key.is_empty());
    assert!(vm.selected_value.is_empty());
}

#[test]
fn radio_group_disabled_option_exposes_disabled_hit_for_cursor_only() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<ScopeChildVm> = WidgetTree::new(
        RadioGroup::new(
            vec![
                RadioOption::new("email".to_string(), "Email".to_string()),
                RadioOption::new("sms".to_string(), "SMS".to_string()).disable(true),
            ],
            "email".to_string(),
        )
        .on_change(ValueCommand::new(
            |vm: &mut ScopeChildVm, (key, value): (String, String)| {
                vm.selected_key = key;
                vm.selected_value = value;
            },
        )),
    );

    let disabled_hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 80.0),
        Some(Point::new(4.0, 30.0)),
        None,
    );
    assert!(matches!(
        disabled_hit,
        Some(super::HitInteraction::Disabled { .. })
    ));

    let enabled_hit = tree.hit_test(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 180.0, 80.0),
        Some(Point::new(4.0, 4.0)),
        None,
    );
    assert!(matches!(
        enabled_hit,
        Some(super::HitInteraction::Radio { .. })
    ));
}

#[test]
fn radio_group_roving_tab_stop_tracks_selection_and_skips_disabled_options() {
    let invalidation = InvalidationSignal::new();
    let selected = State::new("email".to_string(), invalidation);
    let tree: WidgetTree<ScopeChildVm> = WidgetTree::new(
        RadioGroup::new(
            vec![
                RadioOption::new("email".to_string(), "Email".to_string()),
                RadioOption::new("sms".to_string(), "SMS".to_string()).disable(true),
                RadioOption::new("push".to_string(), "Push".to_string()),
            ],
            selected.signal(),
        )
        .horizontal(),
    );
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 260.0, 60.0);

    let radio_stops = |scene: &ComputedScene<ScopeChildVm>| {
        scene
            .hit_regions
            .iter()
            .filter_map(|hit| match &hit.interaction {
                HitInteraction::Radio { interactions, .. } => {
                    let group = interactions
                        .radio_group
                        .expect("RadioGroup child should carry group metadata");
                    Some((
                        group.index,
                        group.direction,
                        hit.focus
                            .as_ref()
                            .expect("enabled radio should be focusable")
                            .tab_index,
                    ))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    let initial = tree.compute_scene(
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
    assert_eq!(
        radio_stops(&initial),
        vec![
            (0, Axis::Horizontal, Some(0)),
            (2, Axis::Horizontal, Some(-1))
        ]
    );

    selected.set("push".to_string());
    let selected_push = tree.compute_scene(
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
    assert_eq!(
        radio_stops(&selected_push),
        vec![
            (0, Axis::Horizontal, Some(-1)),
            (2, Axis::Horizontal, Some(0))
        ]
    );

    selected.set("sms".to_string());
    let disabled_selection = tree.compute_scene(
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
    assert_eq!(
        radio_stops(&disabled_selection),
        vec![
            (0, Axis::Horizontal, Some(0)),
            (2, Axis::Horizontal, Some(-1))
        ]
    );
}
