use super::*;

use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::ui::widget::{ResolvedWidgetKind, Skeleton, SkeletonStyle};

fn layout_for<VM: 'static>(
    element: impl Into<Element<VM>>,
    theme: &Theme,
) -> super::super::ResolvedSceneLayout<VM> {
    let tree = WidgetTree::new(element);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    tree.build_scene_layout(
        &font_manager,
        theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 320.0, 240.0),
    )
}

#[test]
fn skeleton_line_and_multiline_geometry_follow_density_on_the_same_tree() {
    for density in [Density::Compact, Density::Comfortable, Density::Spacious] {
        let mut theme = Theme::light();
        theme.density = density;
        let line = layout_for(Skeleton::<()>::line(), &theme);
        let expected = SkeletonStyle::default_for_theme(&theme);
        assert_eq!(
            line.resolved_root.layout.height,
            Some(crate::ui::layout::Value::Static(
                crate::ui::layout::Length::Px(expected.line_height)
            ))
        );
        let lines = layout_for(Skeleton::<()>::lines(3), &theme);
        let ResolvedWidgetKind::Container {
            layout, children, ..
        } = &lines.resolved_root.kind
        else {
            panic!("multiline skeleton should resolve to a flex container");
        };
        assert_eq!(children.len(), 3);
        assert_eq!(
            layout.gap,
            crate::ui::layout::Value::Static(crate::ui::layout::Length::Px(expected.gap))
        );
    }
}

#[test]
fn skeleton_style_resolver_updates_real_scene_and_explicit_size_wins() {
    let mut theme = Theme::dark();
    theme.density = Density::Spacious;
    let element = Skeleton::<()>::line()
        .style_full(|_| SkeletonStyle {
            base: Value::Static(Color::hexa(0x112233FF)),
            highlight: Value::Static(Color::hexa(0xAABBCCFF)),
            radius: dp(13.0),
            line_height: dp(27.0),
            gap: dp(9.0),
        })
        .size(dp(177.0), dp(39.0));
    let layout = layout_for(element, &theme);
    assert_eq!(
        layout.resolved_root.layout.width,
        Some(Value::Static(crate::ui::layout::Length::Px(dp(177.0))))
    );
    assert_eq!(
        layout.resolved_root.layout.height,
        Some(Value::Static(crate::ui::layout::Length::Px(dp(39.0))))
    );
    assert!(matches!(
        layout.resolved_root.kind,
        ResolvedWidgetKind::Container { .. }
    ));
}
