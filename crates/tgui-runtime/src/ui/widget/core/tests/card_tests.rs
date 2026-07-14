use super::*;

use crate::ui::layout::{Length, Value};
use crate::ui::theme::Density;
use crate::ui::widget::{Card, CardStyle, ResolvedWidgetKind};

#[test]
fn card_runtime_layout_tracks_real_theme_density_on_the_same_tree() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> = WidgetTree::new(
        Card::new()
            .header(Text::new("Header"))
            .body(Text::new("Body"))
            .footer(Text::new("Footer")),
    );

    for (density, expected_padding, expected_gap) in [
        (Density::Compact, Insets::all(dp(8.0)), dp(4.0)),
        (Density::Comfortable, Insets::all(dp(16.0)), dp(8.0)),
        (Density::Spacious, Insets::all(dp(24.0)), dp(16.0)),
    ] {
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
        let ResolvedWidgetKind::Container {
            layout: card_layout,
            children,
            ..
        } = &layout.resolved_root.kind
        else {
            panic!("card root should resolve to a container");
        };
        assert_eq!(children.len(), 3, "card structure must remain flat");
        assert_eq!(
            card_layout.padding,
            Some(Value::Static(expected_padding)),
            "card padding should use the active theme density"
        );
        assert_eq!(
            card_layout.gap,
            Value::Static(Length::Px(expected_gap)),
            "card gap should use the active theme density"
        );
    }
}

#[test]
fn card_component_theme_geometry_is_resolved_at_runtime() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> = WidgetTree::new(Card::new().body(Text::new("Body")));
    let mut theme = Theme::dark();
    theme.components = crate::ui::theme::ComponentThemes::default().card(|style, _| {
        style.padding = Insets::symmetric(dp(19.0), dp(11.0));
        style.gap = dp(13.0);
    });
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
    let ResolvedWidgetKind::Container {
        layout: card_layout,
        ..
    } = &layout.resolved_root.kind
    else {
        panic!("card root should resolve to a container");
    };
    assert_eq!(
        card_layout.padding,
        Some(Value::Static(Insets::symmetric(dp(19.0), dp(11.0))))
    );
    assert_eq!(card_layout.gap, Value::Static(Length::Px(dp(13.0))));
}

#[test]
fn card_light_and_dark_themes_reach_real_scene_surfaces_on_the_same_tree() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> = WidgetTree::new(Card::new().body(Text::new("Body")));

    for theme in [Theme::light(), Theme::dark()] {
        let mut animations = AnimationEngine::default();
        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 240.0),
            None,
            None,
            None,
            None,
            false,
        );
        let style = CardStyle::default_for_theme(&theme);
        assert!(
            rendered
                .primitives
                .shapes
                .iter()
                .any(|shape| shape.color == style.background.resolve()),
            "card background should use the active {} theme",
            theme.name
        );
        assert!(
            rendered.primitives.shapes.iter().any(|shape| {
                shape.color == style.border.resolve()
                    && (shape.stroke_width - style.border_width.get()).abs() < f32::EPSILON
            }),
            "card border should use the active {} theme",
            theme.name
        );
    }
}
