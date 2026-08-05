use super::*;

use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::ui::widget::{
    Button, Carousel, CarouselStyle, ComputedScene, ResolvedElement, ResolvedWidgetKind, WidgetId,
};

fn carousel_scene_at<VM: 'static>(
    tree: &WidgetTree<VM>,
    theme: &Theme,
    font_manager: &FontManager,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    reduced_motion: bool,
    now: Instant,
) -> ComputedScene<VM> {
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
        Rect::new(0.0, 0.0, 360.0, 220.0),
        None,
        None,
        None,
        None,
        false,
        now,
    )
}

fn text_alpha<VM>(scene: &ComputedScene<VM>, content: &str) -> u8 {
    scene
        .scene
        .texts
        .iter()
        .find(|text| text.content.as_ref() == content)
        .unwrap_or_else(|| panic!("missing text primitive {content:?}"))
        .color
        .a
}

fn scene_has_widget_hit<VM>(scene: &ComputedScene<VM>, id: WidgetId) -> bool {
    scene.hit_regions.iter().any(|hit| match hit.interaction {
        HitInteraction::Widget { id: candidate, .. } => candidate == id,
        _ => false,
    })
}

fn resolved_children<VM>(element: &ResolvedElement<VM>) -> &[ResolvedElement<VM>] {
    match &element.kind {
        ResolvedWidgetKind::Container { children, .. }
        | ResolvedWidgetKind::Virtual { children, .. } => children.as_slice(),
        _ => &[],
    }
}

fn resolved_button<'a, VM>(
    element: &'a ResolvedElement<VM>,
    expected_label: &str,
) -> Option<&'a ResolvedElement<VM>> {
    match &element.kind {
        ResolvedWidgetKind::Button { label, .. } if label.resolve() == expected_label => {
            Some(element)
        }
        _ => resolved_children(element)
            .iter()
            .find_map(|child| resolved_button(child, expected_label)),
    }
}

fn button_disabled<VM>(element: &ResolvedElement<VM>) -> bool {
    match &element.kind {
        ResolvedWidgetKind::Button { disabled, .. } => disabled.resolve(),
        _ => panic!("expected button"),
    }
}

fn keyboard_activation<VM>(scene: &ComputedScene<VM>, id: WidgetId) -> Option<(bool, bool)> {
    scene.hit_regions.iter().find_map(|region| {
        let (candidate, enter, space) = region.interaction.keyboard_activation()?;
        (candidate == id).then_some((enter, space))
    })
}

#[test]
fn carousel_gaps_and_indicator_geometry_follow_density_on_the_same_tree() {
    let tree: WidgetTree<()> = WidgetTree::new(Carousel::new(
        vec![Text::new("one").into(), Text::new("two").into()],
        0usize,
    ));
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    for density in [Density::Compact, Density::Comfortable, Density::Spacious] {
        let mut theme = Theme::light();
        theme.density = density;
        let mut animations = AnimationEngine::default();
        let layout = tree.build_scene_layout(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 240.0),
        );
        let expected = CarouselStyle::default_for_theme(&theme);
        let ResolvedWidgetKind::Container {
            layout: root,
            children,
            ..
        } = &layout.resolved_root.kind
        else {
            panic!("carousel should resolve to a vertical container");
        };
        assert_eq!(
            root.gap,
            Value::Static(crate::ui::layout::Length::Px(expected.gap))
        );
        assert_eq!(children.len(), 2);
        let ResolvedWidgetKind::Container { layout: row, .. } = &children[0].kind else {
            panic!("carousel content row should remain a container");
        };
        assert_eq!(
            row.gap,
            Value::Static(crate::ui::layout::Length::Px(expected.gap))
        );
        let ResolvedWidgetKind::Container {
            layout: indicators,
            children: dots,
            ..
        } = &children[1].kind
        else {
            panic!("carousel indicator row should remain a container");
        };
        assert_eq!(
            indicators.gap,
            Value::Static(crate::ui::layout::Length::Px(expected.indicator_gap))
        );
        assert_eq!(dots.len(), 2);
        assert_eq!(
            dots[0].layout.width,
            Some(Value::Static(crate::ui::layout::Length::Px(
                expected.indicator_size
            )))
        );
    }
}

#[test]
fn carousel_explicit_root_size_and_custom_style_survive_runtime_resolution() {
    let mut theme = Theme::dark();
    theme.density = Density::Spacious;
    let tree: WidgetTree<()> = WidgetTree::new(
        Carousel::new(vec![Text::new("one").into()], 0usize)
            .style_full(|_| CarouselStyle {
                gap: dp(11.0),
                indicator_gap: dp(7.0),
                indicator_size: dp(13.0),
                indicator: Value::Static(Color::hexa(0x334455FF)),
                active_indicator: Value::Static(Color::hexa(0xAABBCCFF)),
            })
            .size(dp(222.0), dp(111.0)),
    );
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 320.0, 240.0),
    );
    assert_eq!(
        layout.resolved_root.layout.width,
        Some(Value::Static(crate::ui::layout::Length::Px(dp(222.0))))
    );
    assert_eq!(
        layout.resolved_root.layout.height,
        Some(Value::Static(crate::ui::layout::Length::Px(dp(111.0))))
    );
}

#[test]
fn carousel_selected_signal_crossfades_and_outgoing_panel_releases_input_immediately() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation, AnimationCoordinator::default());
    let selected = context.state(0usize);
    let first: Element<()> = Button::new("Panel alpha").size(dp(120.0), dp(40.0)).into();
    let first_id = first.id;
    let second: Element<()> = Button::new("Panel beta").size(dp(120.0), dp(40.0)).into();
    let second_id = second.id;
    let active_indicator = Color::hexa(0xE11D48FF);
    let idle_indicator = Color::hexa(0x0EA5E9FF);
    let tree = WidgetTree::new(
        Carousel::new(vec![first, second], selected.signal())
            .style(move |style, _| {
                style.active_indicator = Value::Static(active_indicator);
                style.indicator = Value::Static(idle_indicator);
            })
            .size(dp(340.0), dp(180.0)),
    );
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut theme = Theme::light();
    theme.motion.fast_ms = 80;
    theme.motion.normal_ms = 160;
    let mut animations = AnimationEngine::default();
    let start = Instant::now();

    let initial = carousel_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        start,
    );
    assert_eq!(text_alpha(&initial, "Panel alpha"), 255);
    assert_eq!(text_alpha(&initial, "Panel beta"), 0);
    assert!(scene_has_widget_hit(&initial, first_id));
    assert!(!scene_has_widget_hit(&initial, second_id));
    let initial_indicator_x = initial
        .scene
        .shapes
        .iter()
        .find(|shape| shape.color == active_indicator)
        .expect("initial active indicator")
        .rect
        .x;

    selected.set(1);
    let change_time = start + Duration::from_millis(1);
    let change = carousel_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        change_time,
    );
    assert!(
        text_alpha(&change, "Panel alpha") > 0,
        "outgoing panel should remain visual at transition start"
    );
    assert!(
        !scene_has_widget_hit(&change, first_id),
        "outgoing panel must release pointer/focus ownership immediately"
    );
    assert!(scene_has_widget_hit(&change, second_id));
    let changed_indicator_x = change
        .scene
        .shapes
        .iter()
        .find(|shape| shape.color == active_indicator)
        .expect("updated active indicator")
        .rect
        .x;
    assert_ne!(
        initial_indicator_x, changed_indicator_x,
        "indicator selection must follow the live signal"
    );

    let mid_time = change_time + Duration::from_millis(80);
    let mid = carousel_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        mid_time,
    );
    let first_mid = text_alpha(&mid, "Panel alpha");
    let second_mid = text_alpha(&mid, "Panel beta");
    assert!((1..255).contains(&first_mid), "first_mid={first_mid}");
    assert!((1..255).contains(&second_mid), "second_mid={second_mid}");

    // Reverse while both panels are in flight. The newly outgoing panel loses
    // input in this same scene; the animation engine preserves continuity.
    selected.set(0);
    let reversed = carousel_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        mid_time,
    );
    assert!(scene_has_widget_hit(&reversed, first_id));
    assert!(!scene_has_widget_hit(&reversed, second_id));

    let end = carousel_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        mid_time + Duration::from_millis(180),
    );
    assert_eq!(text_alpha(&end, "Panel alpha"), 255);
    assert_eq!(text_alpha(&end, "Panel beta"), 0);
    assert!(!animations.has_active_animations());
}

#[test]
fn carousel_reduced_and_zero_motion_land_on_selection_without_animation_slots() {
    for (reduced_motion, zero_duration) in [(true, false), (false, true)] {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation, AnimationCoordinator::default());
        let selected = context.state(0usize);
        let tree: WidgetTree<()> = WidgetTree::new(
            Carousel::new(
                vec![
                    Text::new("first panel").into(),
                    Text::new("second panel").into(),
                ],
                selected.signal(),
            )
            .size(dp(340.0), dp(180.0)),
        );
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut theme = Theme::light();
        if zero_duration {
            theme.motion.fast_ms = 0;
            theme.motion.normal_ms = 0;
        }
        let mut animations = AnimationEngine::default();
        let start = Instant::now();
        let _ = carousel_scene_at(
            &tree,
            &theme,
            &font_manager,
            &media,
            &mut animations,
            reduced_motion,
            start,
        );

        selected.set(1);
        let selected_scene = carousel_scene_at(
            &tree,
            &theme,
            &font_manager,
            &media,
            &mut animations,
            reduced_motion,
            start + Duration::from_millis(1),
        );
        assert_eq!(
            text_alpha(&selected_scene, "first panel"),
            0,
            "reduced_motion={reduced_motion} zero_duration={zero_duration}"
        );
        assert_eq!(
            text_alpha(&selected_scene, "second panel"),
            255,
            "reduced_motion={reduced_motion} zero_duration={zero_duration}"
        );
        assert!(!animations.has_active_animations());
    }
}

#[derive(Default)]
struct UncontrolledCarouselVm {
    changes: Vec<usize>,
}

#[test]
fn static_carousel_selection_is_uncontrolled_and_navigation_reads_latest_index() {
    let tree: WidgetTree<UncontrolledCarouselVm> = WidgetTree::new(
        Carousel::new(
            vec![
                Text::new("first panel").into(),
                Text::new("second panel").into(),
                Text::new("third panel").into(),
            ],
            0usize,
        )
        .on_change(ValueCommand::new(
            |vm: &mut UncontrolledCarouselVm, selected| vm.changes.push(selected),
        ))
        .size(dp(340.0), dp(180.0)),
    );
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut layout_animations = AnimationEngine::default();
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut layout_animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 360.0, 220.0),
    );
    let next = resolved_button(&layout.resolved_root, "Next")
        .and_then(|button| button.interactions.on_click.clone())
        .expect("uncontrolled carousel Next button should be active");
    let previous = resolved_button(&layout.resolved_root, "Prev")
        .and_then(|button| button.interactions.on_click.clone())
        .expect("uncontrolled carousel Prev button should be active");
    let mut vm = UncontrolledCarouselVm::default();

    next.execute(&mut vm);
    let mut animations = AnimationEngine::default();
    let selected_one = carousel_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        true,
        Instant::now(),
    );
    assert_eq!(text_alpha(&selected_one, "first panel"), 0);
    assert_eq!(text_alpha(&selected_one, "second panel"), 255);

    // Reusing the original command must read the internal selection live rather than a
    // construction-time snapshot.
    next.execute(&mut vm);
    let mut animations = AnimationEngine::default();
    let selected_two = carousel_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        true,
        Instant::now(),
    );
    assert_eq!(text_alpha(&selected_two, "second panel"), 0);
    assert_eq!(text_alpha(&selected_two, "third panel"), 255);

    previous.execute(&mut vm);
    let mut animations = AnimationEngine::default();
    let back_to_one = carousel_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        true,
        Instant::now(),
    );
    assert_eq!(text_alpha(&back_to_one, "second panel"), 255);
    assert_eq!(vm.changes, vec![1, 2, 1]);
}

#[test]
fn carousel_with_no_callback_still_navigates_when_given_a_static_initial_index() {
    let tree: WidgetTree<()> = WidgetTree::new(
        Carousel::new(
            vec![Text::new("first").into(), Text::new("second").into()],
            0usize,
        )
        .size(dp(340.0), dp(180.0)),
    );
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut layout_animations = AnimationEngine::default();
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut layout_animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 360.0, 220.0),
    );
    let next = resolved_button(&layout.resolved_root, "Next")
        .and_then(|button| button.interactions.on_click.clone())
        .expect("default carousel navigation should own internal state");
    next.execute(&mut ());

    let mut animations = AnimationEngine::default();
    let scene = carousel_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        true,
        Instant::now(),
    );
    assert_eq!(text_alpha(&scene, "first"), 0);
    assert_eq!(text_alpha(&scene, "second"), 255);
}

#[test]
fn empty_and_single_item_carousels_disable_navigation_and_keep_indicator_count_exact() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();

    for (items, expected_indicators) in [
        (Vec::<Element<()>>::new(), 0usize),
        (vec![Text::new("only").into()], 1usize),
    ] {
        let tree: WidgetTree<()> = WidgetTree::new(Carousel::new(items, usize::MAX));
        let mut animations = AnimationEngine::default();
        let layout = tree.build_scene_layout(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 360.0, 220.0),
        );
        let previous = resolved_button(&layout.resolved_root, "Prev").expect("Prev button");
        let next = resolved_button(&layout.resolved_root, "Next").expect("Next button");
        assert!(button_disabled(previous));
        assert!(button_disabled(next));
        let root_children = resolved_children(&layout.resolved_root);
        assert_eq!(root_children.len(), 2);
        assert_eq!(
            resolved_children(&root_children[1]).len(),
            expected_indicators,
            "indicator count should match the real item count"
        );
    }
}

#[test]
fn carousel_navigation_supports_enter_and_space_and_tracks_disabled_signal() {
    let context = test_context();
    let disabled = context.state(false);
    let tree: WidgetTree<()> = WidgetTree::new(
        Carousel::new(
            vec![Text::new("first").into(), Text::new("second").into()],
            0usize,
        )
        .disabled(disabled.signal())
        .auto_play(Duration::from_millis(50))
        .size(dp(340.0), dp(180.0)),
    );
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut layout_animations = AnimationEngine::default();
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut layout_animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 360.0, 220.0),
    );
    let next_id = resolved_button(&layout.resolved_root, "Next")
        .expect("Next button")
        .id;
    let mut animations = AnimationEngine::default();
    let start = Instant::now();
    let enabled_scene = carousel_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        start,
    );
    assert_eq!(
        keyboard_activation(&enabled_scene, next_id),
        Some((true, true))
    );
    assert!(scene_has_widget_hit(&enabled_scene, next_id));
    assert_eq!(enabled_scene.carousel_auto_play.len(), 1);
    assert!(!enabled_scene.carousel_auto_play[0].disabled.resolve());

    disabled.set(true);
    let disabled_scene = carousel_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        start + Duration::from_millis(1),
    );
    assert_eq!(keyboard_activation(&disabled_scene, next_id), None);
    assert!(!scene_has_widget_hit(&disabled_scene, next_id));
    assert!(disabled_scene.carousel_auto_play[0].disabled.resolve());

    disabled.set(false);
    let reenabled_scene = carousel_scene_at(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        false,
        start + Duration::from_millis(2),
    );
    assert_eq!(
        keyboard_activation(&reenabled_scene, next_id),
        Some((true, true))
    );
    assert!(!reenabled_scene.carousel_auto_play[0].disabled.resolve());
}
