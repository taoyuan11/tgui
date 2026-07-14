use super::*;
use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::widgets::{TabItem, Tabs, TabsOverflowMode, TabsStyle};

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
