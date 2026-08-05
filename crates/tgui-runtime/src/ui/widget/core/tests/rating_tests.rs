use super::*;

use crate::foundation::view_model::ValueCommand;
use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::ui::widget::{
    HitInteraction, Rating, RatingStyle, ResolvedSceneLayout, ResolvedWidgetKind, SliderStyle,
    StyleSheet,
};

fn resolved_rating_icon_styles(
    layout: &ResolvedSceneLayout<()>,
    theme: &Theme,
    style_sheet: &StyleSheet,
) -> Vec<(Color, Dp)> {
    let context = StyleContext::from_theme(theme);
    layout
        .all_widget_ids()
        .filter_map(|id| {
            let resolved = layout.resolved_widget(id)?;
            let ResolvedWidgetKind::Icon { icon } = &resolved.kind else {
                return None;
            };
            let style = crate::ui::widget::icon::resolve_icon_style_with_sheet(
                icon.style.as_ref(),
                &context,
                style_sheet,
                &resolved.visual,
                crate::ui::theme::WidgetState::default(),
            );
            Some((style.color.resolve(), style.size))
        })
        .collect()
}

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

#[test]
fn rating_class_style_reaches_star_size_and_color() {
    let active = Color::hexa(0xE11D48FF);
    let inactive = Color::hexa(0x2563EBFF);
    let expected_size = dp(31.0);
    let style_sheet = StyleSheet::default().rating_class("featured", move |style, _| {
        style.active = Value::Static(active);
        style.inactive = Value::Static(inactive);
        style.size = expected_size;
    });
    let theme = Theme::light();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 320.0, 96.0);
    let tree: WidgetTree<()> = WidgetTree::new(Rating::new(2.0).class("featured"));
    let mut animations = AnimationEngine::default();
    let now = Instant::now();
    let layout = tree.build_scene_layout_at_with_previous_and_style_sheet(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        now,
        None,
        &style_sheet,
    );
    let styles = resolved_rating_icon_styles(&layout, &theme, &style_sheet);
    assert_eq!(styles.len(), 5);
    assert!(styles.iter().all(|(_, size)| *size == expected_size));
    assert_eq!(
        styles.iter().filter(|(color, _)| *color == active).count(),
        2,
    );
    assert_eq!(
        styles
            .iter()
            .filter(|(color, _)| *color == inactive)
            .count(),
        3,
    );
}

#[test]
fn rating_value_signal_updates_stars_on_the_same_tree() {
    let context = test_context();
    let value = context.state(1.0_f32);
    let theme = Theme::light();
    let active = RatingStyle::default_for_theme(&theme).active.resolve();
    let tree: WidgetTree<()> = WidgetTree::new(Rating::new(value.signal()).half());
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let render_active_count = || {
        let layout = tree.build_scene_layout(
            &font_manager,
            &theme,
            &media,
            &mut AnimationEngine::default(),
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 96.0),
        );
        resolved_rating_icon_styles(&layout, &theme, &StyleSheet::default())
            .iter()
            .filter(|(color, _)| *color == active)
            .count()
    };

    assert_eq!(render_active_count(), 1);
    value.set(3.5);
    assert_eq!(render_active_count(), 4);
}

#[test]
fn rating_read_only_signal_removes_and_restores_slider_hit_target() {
    let context = test_context();
    let read_only = context.state(false);
    let tree: WidgetTree<()> = WidgetTree::new(
        Rating::new(3.0)
            .read_only(read_only.signal())
            .on_change(ValueCommand::new(|_: &mut (), _| {})),
    );
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let has_slider_hit = || {
        tree.compute_scene(
            &font_manager,
            &Theme::light(),
            &media,
            &mut AnimationEngine::default(),
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 96.0),
            None,
            None,
            None,
            None,
            false,
        )
        .hit_regions
        .iter()
        .any(|hit| matches!(hit.interaction, HitInteraction::Slider { .. }))
    };

    assert!(has_slider_hit());
    read_only.set(true);
    assert!(!has_slider_hit());
    read_only.set(false);
    assert!(has_slider_hit());
}
