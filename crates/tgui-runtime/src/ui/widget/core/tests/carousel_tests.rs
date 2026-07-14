use super::*;

use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::ui::widget::{Carousel, CarouselStyle, ResolvedWidgetKind};

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
