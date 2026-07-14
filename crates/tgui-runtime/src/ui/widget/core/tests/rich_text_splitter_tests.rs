use super::*;

use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::ui::widget::{
    Pane, ResizablePanels, ResolvedWidgetKind, RichText, RichTextStyle, SplitterAxis, SplitterStyle,
};

#[test]
fn rich_text_block_gap_tracks_density_on_the_same_tree() {
    let tree: WidgetTree<()> = WidgetTree::new(RichText::markdown("# Title\n\nBody"));
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
        let ResolvedWidgetKind::Container {
            layout: root,
            children,
            ..
        } = &layout.resolved_root.kind
        else {
            panic!("rich text should resolve to a vertical container");
        };
        let expected = RichTextStyle::default_for_theme(&theme);
        assert_eq!(
            root.gap,
            Value::Static(crate::ui::layout::Length::Px(expected.gap))
        );
        assert_eq!(children.len(), 2);
    }
}

#[test]
fn splitter_handles_follow_runtime_style_and_keep_flat_hit_geometry() {
    let panes = vec![Pane::new(Text::new("left")), Pane::new(Text::new("right"))];
    let tree: WidgetTree<()> = WidgetTree::new(ResizablePanels::new(panes, vec![0.4, 0.6]));
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    for density in [Density::Compact, Density::Comfortable, Density::Spacious] {
        let mut theme = Theme::dark();
        theme.density = density;
        let expected_extent = SplitterStyle::default_for_theme(&theme).hit_extent;
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
        let ResolvedWidgetKind::Container { children, .. } = &layout.resolved_root.kind else {
            panic!("splitter should resolve to a flex container");
        };
        assert_eq!(children.len(), 3);
        assert_eq!(
            children[1].layout.width,
            Some(Value::Static(crate::ui::layout::Length::Px(
                expected_extent
            )))
        );
    }
}

#[test]
fn rich_text_and_splitter_explicit_layout_and_custom_style_are_preserved() {
    let mut theme = Theme::dark();
    theme.density = Density::Spacious;
    let rich: WidgetTree<()> = WidgetTree::new(
        RichText::markdown("**styled**")
            .style_full(|_| RichTextStyle {
                gap: dp(17.0),
                ..RichTextStyle::default_for_theme(&Theme::light())
            })
            .size(dp(201.0), dp(99.0)),
    );
    let split: WidgetTree<()> = WidgetTree::new(
        ResizablePanels::new(
            vec![Pane::new(Text::new("a")), Pane::new(Text::new("b"))],
            vec![0.5, 0.5],
        )
        .axis(SplitterAxis::Vertical)
        .style_full(|_| SplitterStyle {
            handle_color: crate::ui::theme::StateValue::new(Value::Static(Color::hexa(0xFF00FFFF))),
            handle_thickness: dp(5.0),
            hit_extent: dp(19.0),
            gap: dp(3.0),
        })
        .size(dp(180.0), dp(140.0)),
    );
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let rich_layout = rich.build_scene_layout(
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
        rich_layout.resolved_root.layout.width,
        Some(Value::Static(crate::ui::layout::Length::Px(dp(201.0))))
    );
    let split_layout = split.build_scene_layout(
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
        split_layout.resolved_root.layout.height,
        Some(Value::Static(crate::ui::layout::Length::Px(dp(140.0))))
    );
}
