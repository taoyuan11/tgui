use super::*;

use crate::ui::theme::Density;
use crate::ui::widget::{Rating, RatingStyle, ResolvedWidgetKind};

#[test]
fn rating_spacing_tracks_density_on_the_same_tree() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let tree: WidgetTree<()> = WidgetTree::new(Rating::new(3.0));

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
        let expected = RatingStyle::default_for_theme(&theme).gap;
        assert_eq!(
            row.gap,
            crate::ui::layout::Value::Static(crate::ui::layout::Length::Px(expected))
        );
    }
}
