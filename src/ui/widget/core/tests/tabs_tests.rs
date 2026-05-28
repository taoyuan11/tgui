use super::*;
use crate::theme::ResolvedThemeMode;
use crate::widgets::{TabItem, Tabs, TabsStyle};

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
        .map(|text| text.content.as_str())
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
    let light = TabsStyle::default_for(ResolvedThemeMode::Light);
    let dark = TabsStyle::default_for(ResolvedThemeMode::Dark);

    assert_ne!(
        light.active_tab_foreground.resolve(),
        dark.active_tab_foreground.resolve()
    );
    assert!(light.indicator_thickness > dp(0.0));
    assert!(dark.tab_min_height > dp(0.0));
}
