use super::*;

use crate::foundation::view_model::ValueCommand;
use crate::ui::theme::Density;
use crate::ui::widget::{HitInteraction, Rating, RatingStyle, ResolvedWidgetKind, SliderStyle};

#[test]
fn rating_spacing_tracks_density_on_the_same_tree() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> = WidgetTree::new(Rating::new(3.0));

    for mut theme in [Theme::light(), Theme::dark()] {
        for (density, expected_size, expected_gap) in [
            (Density::Compact, dp(16.0), dp(2.0)),
            (Density::Comfortable, dp(20.0), dp(4.0)),
            (Density::Spacious, dp(24.0), dp(6.0)),
        ] {
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
                Rect::new(0.0, 0.0, 320.0, 96.0),
            );
            let ResolvedWidgetKind::Container {
                children,
                layout: row,
                ..
            } = &layout.resolved_root.kind
            else {
                panic!("rating root should be a container");
            };
            assert_eq!(children.len(), 5, "rating row should remain flat");
            assert_eq!(
                row.gap,
                crate::ui::layout::Value::Static(crate::ui::layout::Length::Px(expected_gap))
            );
            let style = RatingStyle::default_for_theme(&theme);
            assert_eq!(style.size, expected_size);
            assert_eq!(style.gap, expected_gap);
            assert_eq!(style.active.resolve(), theme.colors.warning);
            assert_eq!(style.inactive.resolve(), theme.colors.outline_muted);

            let rendered = tree.render_output(
                &font_manager,
                &theme,
                &media,
                &mut animations,
                None,
                None,
                &HashMap::new(),
                Rect::new(0.0, 0.0, 320.0, 96.0),
                None,
                None,
                None,
                None,
                false,
            );
            assert_eq!(rendered.primitives.textures.len(), 5);
            assert!(rendered
                .primitives
                .textures
                .iter()
                .all(|texture| texture.frame.width == expected_size
                    && texture.frame.height == expected_size));
        }
    }
}

#[test]
fn interactive_rating_keeps_density_appropriate_hit_height() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> =
        WidgetTree::new(Rating::new(3.0).on_change(ValueCommand::new(|_: &mut (), _| {})));

    for density in [Density::Compact, Density::Comfortable, Density::Spacious] {
        let mut theme = Theme::light();
        theme.density = density;
        let expected_height = SliderStyle::default_for_theme(&theme).min_height;
        let mut animations = AnimationEngine::default();
        let scene = tree.compute_scene(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 96.0),
            None,
            None,
            None,
            None,
            false,
        );
        let hit = scene
            .hit_regions
            .iter()
            .find(|hit| matches!(hit.interaction, HitInteraction::Slider { .. }))
            .expect("interactive Rating should expose its transparent Slider hit target");
        assert!(hit.rect.height >= expected_height);
    }
}

#[test]
fn rating_custom_size_and_gap_override_density_defaults() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> = WidgetTree::new(Rating::new(3.0).style(|style, _| {
        style.size = dp(30.0);
        style.gap = dp(7.0);
    }));

    for density in [Density::Compact, Density::Comfortable, Density::Spacious] {
        let mut theme = Theme::dark();
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
            Rect::new(0.0, 0.0, 320.0, 96.0),
        );
        let ResolvedWidgetKind::Container { layout: row, .. } = &layout.resolved_root.kind else {
            panic!("rating root should be a container");
        };
        assert_eq!(
            row.gap,
            crate::ui::layout::Value::Static(crate::ui::layout::Length::Px(dp(7.0)))
        );
        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 96.0),
            None,
            None,
            None,
            None,
            false,
        );
        assert_eq!(rendered.primitives.textures.len(), 5);
        assert!(rendered
            .primitives
            .textures
            .iter()
            .all(|texture| texture.frame.width == dp(30.0) && texture.frame.height == dp(30.0)));
    }
}
