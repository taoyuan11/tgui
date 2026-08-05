use super::*;

use crate::foundation::view_model::ValueCommand;
use crate::ui::layout::Value;
use crate::ui::theme::Density;
use crate::ui::widget::{
    AccessibilityRole, DefaultActivation, Pane, ResizablePanels, ResolvedWidgetKind, RichText,
    RichTextLinkClick, RichTextStyle, SplitterAxis, SplitterStyle,
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
fn rich_text_markdown_signal_updates_blocks_on_the_same_tree() {
    let context = test_context();
    let markdown = context.state("before".to_string());
    let tree: WidgetTree<()> = WidgetTree::new(RichText::markdown(markdown.signal()));
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let render = |animations: &mut AnimationEngine| {
        tree.render_output(
            &font_manager,
            &Theme::default(),
            &media,
            animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 240.0),
            None,
            None,
            None,
            None,
            false,
        )
    };

    let initial = render(&mut animations);
    assert!(initial
        .primitives
        .texts
        .iter()
        .any(|text| text.content.as_ref() == "before"));

    markdown.set("# After\n\nsecond block".to_string());
    let updated = render(&mut animations);
    let text = updated
        .primitives
        .texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();
    assert!(text.contains(&"After"), "{text:?}");
    assert!(text.contains(&"second block"), "{text:?}");
    assert!(!text.contains(&"before"), "{text:?}");
}

#[test]
fn rich_text_links_are_only_interactive_with_a_handler_and_use_link_activation() {
    let render = |element: Element<()>| {
        let tree = WidgetTree::new(element);
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        tree.compute_scene(
            &font_manager,
            &Theme::default(),
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 120.0),
            None,
            None,
            None,
            None,
            false,
        )
    };

    let static_scene = render(RichText::markdown("[Docs](https://example.com)").into());
    assert!(static_scene.hit_regions.iter().all(|region| {
        !region
            .interaction
            .interactions()
            .is_some_and(|interactions| interactions.on_click.is_some())
    }));

    let interactive_scene = render(
        RichText::markdown("[Docs](https://example.com)")
            .on_link_click(ValueCommand::new(|_: &mut (), _: RichTextLinkClick| {}))
            .into(),
    );
    let link = interactive_scene
        .hit_regions
        .iter()
        .find(|region| {
            region
                .interaction
                .interactions()
                .is_some_and(|interactions| interactions.on_click.is_some())
        })
        .expect("handled rich-text link should publish an interactive hit");
    assert_eq!(
        link.interaction.keyboard_activation(),
        Some((link.interaction.widget_id(), true, false))
    );
    assert!(matches!(
        link.interaction,
        HitInteraction::Widget {
            default_activation: DefaultActivation::Enter,
            ..
        }
    ));

    let tree: WidgetTree<()> = WidgetTree::new(
        RichText::markdown("[Docs](https://example.com)")
            .on_link_click(ValueCommand::new(|_: &mut (), _: RichTextLinkClick| {})),
    );
    let layout = tree.build_scene_layout(
        &FontManager::new(&FontCatalog::default()),
        &Theme::default(),
        &test_media(),
        &mut AnimationEngine::default(),
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 320.0, 120.0),
    );
    let link_role = layout
        .all_widget_ids()
        .filter_map(|id| layout.resolved_widget(id))
        .find_map(|element| element.visual.accessibility_role);
    assert_eq!(link_role, Some(AccessibilityRole::Link));
}

#[test]
fn rich_text_image_preserves_markdown_alt_for_accessibility() {
    let image_label = |markdown: &'static str| {
        let tree: WidgetTree<()> = WidgetTree::new(RichText::markdown(markdown));
        let layout = tree.build_scene_layout(
            &FontManager::new(&FontCatalog::default()),
            &Theme::default(),
            &test_media(),
            &mut AnimationEngine::default(),
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 320.0, 200.0),
        );
        let label = layout
            .all_widget_ids()
            .filter_map(|id| layout.resolved_widget(id))
            .find(|resolved| matches!(resolved.kind, ResolvedWidgetKind::Image { .. }))
            .expect("rich-text image should resolve")
            .visual
            .accessibility_label
            .as_ref()
            .map(Value::resolve);
        label
    };

    assert_eq!(
        image_label("![Architecture diagram](https://example.com/diagram.png)"),
        Some("Architecture diagram".to_string())
    );
    assert_eq!(image_label("![](https://example.com/decorative.png)"), None);
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
