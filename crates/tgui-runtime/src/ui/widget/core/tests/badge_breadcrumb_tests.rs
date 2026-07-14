use super::*;

use crate::ui::layout::{Length, Value};
use crate::ui::theme::Density;
use crate::ui::widget::{
    Badge, BadgeStyle, BadgeTone, Breadcrumb, BreadcrumbItem, BreadcrumbStyle, ResolvedElement,
    ResolvedSceneLayout,
};

fn resolved_children<VM>(element: &ResolvedElement<VM>) -> &[ResolvedElement<VM>] {
    match &element.kind {
        ResolvedWidgetKind::Container { children, .. }
        | ResolvedWidgetKind::Virtual { children, .. } => children.as_slice(),
        _ => &[],
    }
}

fn build_layout(tree: &WidgetTree<()>, theme: &Theme) -> ResolvedSceneLayout<()> {
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
        Rect::new(0.0, 0.0, 320.0, 120.0),
    )
}

#[test]
fn badge_runtime_layout_tracks_density_on_the_same_tree() {
    let dot: WidgetTree<()> = WidgetTree::new(Badge::dot());
    let pill: WidgetTree<()> = WidgetTree::new(Badge::text("Stable"));

    for (density, dot_size, min_height, padding_x) in [
        (Density::Compact, dp(6.0), dp(16.0), dp(4.0)),
        (Density::Comfortable, dp(8.0), dp(20.0), dp(6.0)),
        (Density::Spacious, dp(10.0), dp(24.0), dp(8.0)),
    ] {
        let mut theme = Theme::light();
        theme.density = density;

        let dot_layout = build_layout(&dot, &theme);
        assert_eq!(
            dot_layout.resolved_root.layout.width,
            Some(Value::Static(Length::Px(dot_size)))
        );
        assert_eq!(
            dot_layout.resolved_root.layout.height,
            Some(Value::Static(Length::Px(dot_size)))
        );

        let pill_layout = build_layout(&pill, &theme);
        assert_eq!(
            pill_layout.resolved_root.layout.min_height,
            Some(Value::Static(Length::Px(min_height)))
        );
        let ResolvedWidgetKind::Container {
            layout: container_layout,
            children,
            ..
        } = &pill_layout.resolved_root.kind
        else {
            panic!("badge pill should remain a single container");
        };
        assert_eq!(children.len(), 1, "badge structure must remain flat");
        assert_eq!(
            container_layout.padding,
            Some(Value::Static(Insets::symmetric(padding_x, dp(1.0))))
        );
    }

    let theme = Theme::light();
    let explicit_dot: WidgetTree<()> = WidgetTree::new(Badge::dot().size(dp(14.0), dp(15.0)));
    let explicit_dot_layout = build_layout(&explicit_dot, &theme);
    assert_eq!(
        explicit_dot_layout.resolved_root.layout.width,
        Some(Value::Static(Length::Px(dp(14.0))))
    );
    assert_eq!(
        explicit_dot_layout.resolved_root.layout.height,
        Some(Value::Static(Length::Px(dp(15.0))))
    );

    let explicit_pill: WidgetTree<()> =
        WidgetTree::new(Badge::text("Override").min_height(dp(31.0)));
    let explicit_pill_layout = build_layout(&explicit_pill, &theme);
    assert_eq!(
        explicit_pill_layout.resolved_root.layout.min_height,
        Some(Value::Static(Length::Px(dp(31.0))))
    );
}

#[test]
fn badge_component_theme_geometry_is_resolved_at_runtime() {
    let tree: WidgetTree<()> = WidgetTree::new(Badge::text("Custom"));
    let mut theme = Theme::dark();
    theme.components = crate::ui::theme::ComponentThemes::default().badge(|style, _| {
        style.min_height = dp(29.0);
        style.padding_x = dp(11.0);
    });

    let layout = build_layout(&tree, &theme);
    assert_eq!(
        layout.resolved_root.layout.min_height,
        Some(Value::Static(Length::Px(dp(29.0))))
    );
    let ResolvedWidgetKind::Container {
        layout: container_layout,
        ..
    } = &layout.resolved_root.kind
    else {
        panic!("badge pill should resolve to a container");
    };
    assert_eq!(
        container_layout.padding,
        Some(Value::Static(Insets::symmetric(dp(11.0), dp(1.0))))
    );
}

#[test]
fn attached_dot_badge_keeps_anchor_extent_and_runtime_dot_size() {
    let anchor_color = Color::hexa(0x123456FF);
    let tree: WidgetTree<()> = WidgetTree::new(
        Badge::dot().attach(
            Stack::new()
                .size(dp(40.0), dp(40.0))
                .style(move |style, _| {
                    style.surface.background = Some(anchor_color.into());
                }),
        ),
    );
    let mut theme = Theme::light();
    theme.density = Density::Spacious;
    let layout = build_layout(&tree, &theme);

    let children = resolved_children(&layout.resolved_root);
    assert_eq!(children.len(), 2);
    assert_eq!(
        children[1].layout.width,
        Some(Value::Static(Length::Px(dp(10.0))))
    );
    assert_eq!(
        children[1].layout.height,
        Some(Value::Static(Length::Px(dp(10.0))))
    );

    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 120.0, 80.0),
        None,
        None,
        None,
        None,
        false,
    );
    let anchor = rendered
        .primitives
        .shapes
        .iter()
        .find(|shape| shape.color == anchor_color)
        .expect("attached badge anchor should render");
    assert_eq!(anchor.rect.width, dp(40.0));
    assert_eq!(anchor.rect.height, dp(40.0));
}

#[test]
fn breadcrumb_runtime_gap_tracks_density_and_component_theme() {
    let tree: WidgetTree<()> = WidgetTree::new(Breadcrumb::new(vec![
        BreadcrumbItem::new("Workspace"),
        BreadcrumbItem::new("Library"),
        BreadcrumbItem::new("Current"),
    ]));

    for (density, expected_gap) in [
        (Density::Compact, dp(4.0)),
        (Density::Comfortable, dp(6.0)),
        (Density::Spacious, dp(8.0)),
    ] {
        let mut theme = Theme::dark();
        theme.density = density;
        let layout = build_layout(&tree, &theme);
        let ResolvedWidgetKind::Container {
            layout: container_layout,
            children,
            ..
        } = &layout.resolved_root.kind
        else {
            panic!("breadcrumb should resolve to a container");
        };
        assert_eq!(children.len(), 5, "breadcrumb structure must remain flat");
        assert_eq!(
            container_layout.gap,
            Value::Static(Length::Px(expected_gap))
        );
    }

    let mut theme = Theme::light();
    theme.components = crate::ui::theme::ComponentThemes::default().breadcrumb(|style, _| {
        style.gap = dp(13.0);
    });
    let layout = build_layout(&tree, &theme);
    let ResolvedWidgetKind::Container {
        layout: container_layout,
        ..
    } = &layout.resolved_root.kind
    else {
        panic!("breadcrumb should resolve to a container");
    };
    assert_eq!(container_layout.gap, Value::Static(Length::Px(dp(13.0))));
}

#[test]
fn badge_and_breadcrumb_use_restrained_light_and_dark_tokens() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let badge: WidgetTree<()> = WidgetTree::new(Badge::text("Ready").tone(BadgeTone::Primary));
    let breadcrumb: WidgetTree<()> = WidgetTree::new(Breadcrumb::new(vec![
        BreadcrumbItem::new("Library"),
        BreadcrumbItem::new("Current"),
    ]));

    for theme in [Theme::light(), Theme::dark()] {
        let mut animations = AnimationEngine::default();
        let badge_scene = badge.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 160.0, 48.0),
            None,
            None,
            None,
            None,
            false,
        );
        let badge_style = BadgeStyle::default_for_theme(&theme, BadgeTone::Primary);
        assert!(badge_scene
            .primitives
            .shapes
            .iter()
            .any(|shape| shape.color == badge_style.background.resolve()));
        assert!(badge_scene
            .primitives
            .texts
            .iter()
            .any(|text| text.content.as_ref() == "Ready"
                && text.color == badge_style.foreground.resolve()));

        let mut animations = AnimationEngine::default();
        let breadcrumb_scene = breadcrumb.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 240.0, 48.0),
            None,
            None,
            None,
            None,
            false,
        );
        let breadcrumb_style = BreadcrumbStyle::default_for_theme(&theme);
        let text_color = |content: &str| {
            breadcrumb_scene
                .primitives
                .texts
                .iter()
                .find(|text| text.content.as_ref() == content)
                .map(|text| text.color)
                .unwrap_or_else(|| panic!("missing breadcrumb text {content}"))
        };
        assert_eq!(text_color("Library"), breadcrumb_style.foreground.resolve());
        assert_eq!(text_color("/"), breadcrumb_style.separator.resolve());
        assert_eq!(
            text_color("Current"),
            breadcrumb_style.current_foreground.resolve()
        );
    }
}
