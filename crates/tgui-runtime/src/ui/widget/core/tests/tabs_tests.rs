use super::*;
use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::ui::widget::ComputedScene;
use crate::widgets::{Button, TabItem, Tabs, TabsOverflowMode, TabsStyle};

fn tabs_tree(selected: &str) -> WidgetTree<()> {
    WidgetTree::new(
        Tabs::new(
            vec![
                TabItem::new("one", "One", Text::new("Panel one")),
                TabItem::new("two", "Two", Text::new("Panel two")),
                TabItem::new("disabled", "Disabled", Text::new("Hidden")).disabled(true),
            ],
            selected.to_string(),
        )
        .width(dp(260.0)),
    )
}

#[test]
fn tabs_render_strip_and_selected_panel() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree = tabs_tree("two");

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 320.0, 180.0),
        None,
        None,
        None,
        None,
        false,
    );
    let texts: Vec<_> = rendered.primitives.texts.iter().collect();

    assert!(texts.iter().any(|text| text.content.as_ref() == "One"));
    assert!(texts.iter().any(|text| text.content.as_ref() == "Two"));
    assert!(texts
        .iter()
        .any(|text| text.content.as_ref() == "Panel two" && text.color.a > 0));
    assert!(texts
        .iter()
        .any(|text| text.content.as_ref() == "Panel one" && text.color.a == 0));
}

#[test]
fn disabled_tabs_do_not_create_tab_trigger_hit_region() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree = tabs_tree("one");
    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 320.0, 180.0),
        None,
        None,
        None,
        None,
        false,
    );
    let tab_trigger_count = computed
        .hit_regions
        .iter()
        .filter(|hit| matches!(hit.interaction, HitInteraction::TabTrigger { .. }))
        .count();
    let disabled_count = computed
        .hit_regions
        .iter()
        .filter(|hit| matches!(hit.interaction, HitInteraction::Disabled { .. }))
        .count();

    assert_eq!(tab_trigger_count, 2);
    assert!(disabled_count >= 1);
}

#[test]
fn tabs_style_defaults_resolve_for_light_and_dark() {
    let light = TabsStyle::default_for_theme(&Theme::light());
    let dark = TabsStyle::default_for_theme(&Theme::dark());

    assert_ne!(
        light.tab_foreground.normal.resolve(),
        dark.tab_foreground.normal.resolve()
    );
    assert!(light.indicator_thickness > dp(0.0));
    assert!(dark.tab_min_height > dp(0.0));
}

#[test]
fn tabs_defaults_follow_density_and_keep_the_surface_borderless() {
    let expected = [
        (Density::Compact, dp(32.0), dp(64.0), dp(8.0)),
        (Density::Comfortable, dp(40.0), dp(72.0), dp(16.0)),
        (Density::Spacious, dp(48.0), dp(80.0), dp(24.0)),
    ];

    for (density, min_height, min_width, panel_padding) in expected {
        let mut theme = Theme::light();
        theme.density = density;
        let style = TabsStyle::default_for_theme(&theme);
        assert_eq!(style.tab_min_height, min_height);
        assert_eq!(style.tab_min_width, min_width);
        assert_eq!(style.panel_padding, Insets::all(panel_padding));
        assert_eq!(style.border_width.resolve(), theme.border.none);
        assert_eq!(style.indicator_thickness, theme.border.thin);
    }
}

#[test]
fn tabs_runtime_geometry_tracks_density_and_custom_style_on_the_same_tree() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 520.0, 240.0);
    let tree: WidgetTree<()> = WidgetTree::new(
        Tabs::new(
            vec![
                TabItem::new("one", "One", Text::new("Panel one")),
                TabItem::new("two", "Two", Text::new("Panel two")),
            ],
            "one".to_string(),
        )
        .style(|style, context| match context.density {
            Density::Compact => {
                style.tab_min_width = dp(76.0);
                style.tab_min_height = dp(42.0);
                style.tab_gap = dp(4.0);
                style.panel_padding = Insets::all(dp(6.0));
            }
            Density::Comfortable => {}
            Density::Spacious => {
                style.tab_min_width = dp(112.0);
                style.tab_min_height = dp(58.0);
                style.tab_gap = dp(12.0);
                style.panel_padding = Insets::all(dp(20.0));
            }
        })
        .width(dp(480.0))
        .height(dp(220.0)),
    );

    for (density, min_width, min_height, gap, panel_padding) in [
        (Density::Compact, dp(76.0), dp(42.0), dp(4.0), dp(6.0)),
        (Density::Spacious, dp(112.0), dp(58.0), dp(12.0), dp(20.0)),
    ] {
        let mut theme = Theme::light();
        theme.density = density;
        let mut animations = AnimationEngine::default();
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
        let mut tab_hits = computed
            .hit_regions
            .iter()
            .filter_map(|hit| match hit.interaction {
                HitInteraction::TabTrigger { .. } => Some(hit.rect),
                _ => None,
            })
            .collect::<Vec<_>>();
        tab_hits.sort_by(|left, right| left.x.partial_cmp(&right.x).unwrap());
        assert_eq!(tab_hits.len(), 2);
        assert!(
            tab_hits.iter().all(|rect| {
                (rect.width - min_width).abs() <= dp(0.1) && rect.height >= min_height
            }),
            "density={density:?}, expected=({min_width:?}, {min_height:?}), hits={tab_hits:?}"
        );
        assert!((tab_hits[1].x - tab_hits[0].right() - gap).abs() <= dp(0.1));

        let mut animations = AnimationEngine::default();
        let layout = tree.build_scene_layout(
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
            panic!("tabs root should remain a container");
        };
        let ResolvedWidgetKind::Container {
            layout: panel_layout,
            ..
        } = &children[1].kind
        else {
            panic!("tabs panel should remain a container");
        };
        assert_eq!(
            panel_layout.padding,
            Some(Value::Static(Insets::all(panel_padding)))
        );
    }
}

#[test]
fn tabs_scene_uses_a_single_restrained_indicator_without_container_strokes() {
    let theme = Theme::light();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree = tabs_tree("one");

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 320.0, 180.0),
        None,
        None,
        None,
        None,
        false,
    );

    let style = TabsStyle::default_for_theme(&theme);
    let indicator_strokes = rendered
        .primitives
        .shapes
        .iter()
        .filter(|shape| {
            shape.color == style.indicator_color.resolve()
                && (shape.stroke_width - style.indicator_thickness.get()).abs() < f32::EPSILON
        })
        .count();
    assert_eq!(indicator_strokes, 1, "only the selected tab is outlined");
    assert!(
        rendered
            .primitives
            .shapes
            .iter()
            .all(|shape| { shape.stroke_width == 0.0 || shape.color != style.border.resolve() }),
        "tab strip and panel should not emit nested container outlines"
    );
}

#[test]
fn tabs_more_overflow_keeps_selected_trigger_visible_and_uses_menu_for_rest() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let tree: WidgetTree<()> = WidgetTree::new(
        Tabs::new(
            vec![
                TabItem::new("one", "One", Text::new("Panel one")),
                TabItem::new("two", "Two", Text::new("Panel two")),
                TabItem::new("three", "Three", Text::new("Panel three")),
                TabItem::new("four", "Four", Text::new("Panel four")),
                TabItem::new("five", "Five", Text::new("Panel five")),
            ],
            "five".to_string(),
        )
        .overflow_mode(TabsOverflowMode::More)
        .width(dp(260.0)),
    );

    let computed = tree.compute_scene(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 320.0, 180.0),
        None,
        None,
        None,
        None,
        false,
    );

    let visible_keys: Vec<_> = computed
        .hit_regions
        .iter()
        .filter_map(|hit| match &hit.interaction {
            HitInteraction::TabTrigger { key, .. } => Some(key.as_str()),
            _ => None,
        })
        .collect();
    assert!(visible_keys.contains(&"five"));
    assert!(visible_keys.len() < 5);
    assert!(
        computed
            .hit_regions
            .iter()
            .any(|hit| matches!(hit.interaction, HitInteraction::Widget { .. })),
        "More trigger should render as a menu trigger widget"
    );
}

#[test]
fn tabs_active_color_tokens_follow_selection_without_reduced_motion_intermediates() {
    const INACTIVE_FILL: Color = Color::rgb(17, 37, 57);
    const INACTIVE_TEXT: Color = Color::rgb(67, 87, 107);
    const ACTIVE_FILL: Color = Color::rgb(197, 43, 83);
    const ACTIVE_TEXT: Color = Color::rgb(23, 173, 229);

    fn assert_selection_colors(
        scene: &ComputedScene<()>,
        selected_key: &str,
        selected_label: &str,
        inactive_key: &str,
        inactive_label: &str,
    ) {
        let trigger_rect = |key: &str| {
            scene
                .hit_regions
                .iter()
                .find_map(|hit| match &hit.interaction {
                    HitInteraction::TabTrigger { key: candidate, .. } if candidate == key => {
                        Some(hit.rect)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing tab trigger {key}"))
        };
        let active_fills = scene
            .scene
            .shapes
            .iter()
            .filter(|shape| shape.color == ACTIVE_FILL)
            .collect::<Vec<_>>();
        let inactive_fills = scene
            .scene
            .shapes
            .iter()
            .filter(|shape| shape.color == INACTIVE_FILL)
            .collect::<Vec<_>>();

        assert_eq!(
            active_fills.len(),
            1,
            "exactly one tab must use active fill"
        );
        assert!(
            trigger_rect(selected_key).contains(Point::new(
                active_fills[0].rect.x + active_fills[0].rect.width * 0.5,
                active_fills[0].rect.y + active_fills[0].rect.height * 0.5,
            )),
            "active fill must belong to the selected tab"
        );
        assert_eq!(
            inactive_fills.len(),
            1,
            "exactly one tab must use inactive fill"
        );
        assert!(trigger_rect(inactive_key).contains(Point::new(
            inactive_fills[0].rect.x + inactive_fills[0].rect.width * 0.5,
            inactive_fills[0].rect.y + inactive_fills[0].rect.height * 0.5,
        )));
        assert_eq!(
            scene
                .scene
                .texts
                .iter()
                .find(|text| text.content.as_ref() == selected_label)
                .expect("selected tab label")
                .color,
            ACTIVE_TEXT
        );
        assert_eq!(
            scene
                .scene
                .texts
                .iter()
                .find(|text| text.content.as_ref() == inactive_label)
                .expect("inactive tab label")
                .color,
            INACTIVE_TEXT
        );
    }

    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation, AnimationCoordinator::default());
    let selected = context.state("one".to_string());
    let tree: WidgetTree<()> = WidgetTree::new(
        Tabs::new(
            vec![
                TabItem::new("one", "One trigger", Text::new("First panel")),
                TabItem::new("two", "Two trigger", Text::new("Second panel")),
            ],
            selected.signal(),
        )
        .style(|style, _| {
            style.tab_background = StateValue::new(INACTIVE_FILL.into());
            style.tab_foreground = StateValue::new(INACTIVE_TEXT.into());
            style.active_tab_background = ACTIVE_FILL.into();
            style.active_tab_foreground = ACTIVE_TEXT.into();
        })
        .size(dp(320.0), dp(180.0)),
    );
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::light();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 340.0, 200.0);
    let mut collect_reduced_motion = || {
        tree.compute_scene_with_widget_state(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            true,
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
        )
    };

    let initial = collect_reduced_motion();
    assert_selection_colors(&initial, "one", "One trigger", "two", "Two trigger");

    selected.set("two".to_string());
    let switched = collect_reduced_motion();
    assert_selection_colors(&switched, "two", "Two trigger", "one", "One trigger");
    assert!(
        !animations.has_active_animations(),
        "reduced motion must land active tab colors directly on their targets"
    );
}

#[test]
fn tabs_selected_signal_switches_panel_hits_focus_and_visuals_on_the_same_tree() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation, AnimationCoordinator::default());
    let selected = context.state("one".to_string());
    let first: Element<()> = Button::new("First panel action")
        .size(dp(140.0), dp(40.0))
        .into();
    let first_id = first.id;
    let second: Element<()> = Button::new("Second panel action")
        .size(dp(140.0), dp(40.0))
        .into();
    let second_id = second.id;
    let tree = WidgetTree::new(
        Tabs::new(
            vec![
                TabItem::new("one", "One", first),
                TabItem::new("two", "Two", second),
            ],
            selected.signal(),
        )
        .size(dp(320.0), dp(180.0)),
    );
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut theme = Theme::light();
    theme.motion.fast_ms = 0;
    theme.motion.normal_ms = 0;
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 340.0, 200.0);
    let mut collect = || {
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
    };

    let initial = collect();
    assert!(initial.hit_regions.iter().any(|hit| {
        matches!(hit.interaction, HitInteraction::Widget { id, .. } if id == first_id)
    }));
    assert!(!initial.hit_regions.iter().any(|hit| {
        matches!(hit.interaction, HitInteraction::Widget { id, .. } if id == second_id)
    }));
    assert_eq!(
        initial
            .scene
            .texts
            .iter()
            .find(|text| text.content.as_ref() == "First panel action")
            .expect("first panel text")
            .color
            .a,
        255
    );

    selected.set("two".to_string());
    let switched = collect();
    assert!(!switched.hit_regions.iter().any(|hit| {
        matches!(hit.interaction, HitInteraction::Widget { id, .. } if id == first_id)
    }));
    assert!(switched.hit_regions.iter().any(|hit| {
        matches!(hit.interaction, HitInteraction::Widget { id, .. } if id == second_id)
    }));
    assert_eq!(
        switched
            .scene
            .texts
            .iter()
            .find(|text| text.content.as_ref() == "First panel action")
            .expect("retained outgoing text")
            .color
            .a,
        0
    );
    assert_eq!(
        switched
            .scene
            .texts
            .iter()
            .find(|text| text.content.as_ref() == "Second panel action")
            .expect("incoming text")
            .color
            .a,
        255
    );
    assert!(
        switched.focus_scopes.iter().any(|scope| !scope.active),
        "inactive panel keeps a conservative scene sentinel but no focus candidates"
    );
    assert!(
        switched
            .scene
            .texts
            .iter()
            .all(|text| text.frame.x < viewport.x + viewport.width + dp(16.0)),
        "inactive panels must use a short transition offset, not huge hidden geometry"
    );

    // Unknown selections preserve the documented first-panel fallback.
    selected.set("missing".to_string());
    let fallback = collect();
    assert!(fallback.hit_regions.iter().any(|hit| {
        matches!(hit.interaction, HitInteraction::Widget { id, .. } if id == first_id)
    }));
    assert!(!animations.has_active_animations());
}
