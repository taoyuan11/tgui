use super::*;
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
    let texts: Vec<_> = rendered
        .primitives
        .texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect();

    assert!(texts.contains(&"One"));
    assert!(texts.contains(&"Two"));
    assert!(texts.contains(&"Panel two"));
    assert!(!texts.contains(&"Panel one"));
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
