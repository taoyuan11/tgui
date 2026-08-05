use super::*;

use crate::ui::layout::{Length, Value};
use crate::ui::theme::Density;
use crate::ui::widget::{
    AccessibilityRole, Card, CardStyle, DefaultActivation, HitInteraction, ResolvedWidgetKind,
};

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

#[test]
fn card_defaults_stay_on_the_regular_surface_plane() {
    for theme in [Theme::light(), Theme::dark()] {
        let style = CardStyle::default_for_theme(&theme);
        assert_eq!(style.background.resolve(), theme.colors.surface);
        assert_eq!(style.radius, theme.radius.lg);
        assert_eq!(style.shadow, theme.elevation.none);
        assert!(
            style.radius <= theme.radius.lg,
            "regular card corners should not use overlay-scale radii"
        );
    }
}

#[test]
fn card_shadow_is_texture_free_by_default_and_remains_explicitly_available() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let render = |tree: &WidgetTree<()>| {
        tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut AnimationEngine::default(),
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 160.0),
            None,
            None,
            None,
            None,
            false,
        )
    };

    let default_card = WidgetTree::new(
        Card::new()
            .body(Text::new("Border-only card"))
            .size(dp(280.0), dp(120.0)),
    );
    assert!(render(&default_card).primitives.textures.is_empty());

    let elevated_card = WidgetTree::new(
        Card::new()
            .body(Text::new("Elevated card"))
            .style(|style, context| style.shadow = context.theme.elevation.sm.clone())
            .size(dp(280.0), dp(120.0)),
    );
    assert_eq!(render(&elevated_card).primitives.textures.len(), 1);
}

#[test]
fn clickable_card_does_not_add_focus_ring_for_keyboard_focus_by_default() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let card: Element<()> = Card::new()
        .body(Text::new("Keyboard action"))
        .on_click(Command::new(|_: &mut ()| {}))
        .size(dp(240.0), dp(96.0))
        .into();
    let card_id = card.id;
    let tree = WidgetTree::new(card);
    let mut states = WidgetStateMap::default();
    states.set(
        card_id,
        crate::ui::theme::WidgetState {
            focused: true,
            focus_visible: true,
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
        &states,
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 96.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(!rendered.primitives.overlay_shapes.iter().any(|shape| {
        shape.color == theme.focus_ring.color
            && shape.stroke_width == theme.focus_ring.width.get()
            && shape.rect.width > dp(240.0)
            && shape.rect.height > dp(96.0)
    }));
}

#[test]
fn clickable_card_is_a_keyboard_activatable_button_but_static_card_is_not() {
    let static_card: Element<()> = Card::new().body(Text::new("Preview")).into();
    assert_eq!(static_card.visual.accessibility_role, None);
    assert!(static_card.interactions.on_click.is_none());

    let clickable: Element<()> = Card::new()
        .body(Text::new("Open details"))
        .on_click(Command::new(|_: &mut ()| {}))
        .size(dp(180.0), dp(72.0))
        .into();
    let clickable_id = clickable.id;
    assert_eq!(
        clickable.visual.accessibility_role,
        Some(AccessibilityRole::Button)
    );
    assert_eq!(clickable.focus.focusable, Some(true));
    assert_eq!(clickable.focus.tab_index, Some(0));

    let scene = WidgetTree::new(clickable).compute_scene(
        &FontManager::new(&FontCatalog::default()),
        &Theme::default(),
        &test_media(),
        &mut AnimationEngine::default(),
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 100.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(scene.hit_regions.iter().any(|hit| matches!(
        hit.interaction,
        HitInteraction::Widget {
            id,
            default_activation: DefaultActivation::EnterAndSpace,
            ..
        } if id == clickable_id
    )));
}
