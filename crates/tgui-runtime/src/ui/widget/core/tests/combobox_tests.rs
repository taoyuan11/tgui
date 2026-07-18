use super::*;

use crate::foundation::binding::TextController;
use crate::ui::layout::{Length, Value};
use crate::ui::theme::Density;
use crate::ui::widget::{
    Combobox, ComboboxOption, ComboboxStyle, ComputedScene, RenderPrimitive, ResolvedSceneLayout,
};

fn options(count: usize) -> Vec<ComboboxOption> {
    (0..count)
        .map(|index| ComboboxOption::new(format!("item-{index}"), format!("Option {index}")))
        .collect()
}

fn combo_tree(count: usize) -> WidgetTree<()> {
    WidgetTree::new(Combobox::new(TextController::from(""), options(count)).open(true))
}

fn build_layout(
    tree: &WidgetTree<()>,
    theme: &Theme,
    font_manager: &FontManager,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    viewport: Rect,
) -> ResolvedSceneLayout<()> {
    tree.build_scene_layout(
        font_manager,
        theme,
        media,
        animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    )
}

fn collect_scene(
    tree: &WidgetTree<()>,
    layout: &ResolvedSceneLayout<()>,
    theme: &Theme,
    font_manager: &FontManager,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    viewport: Rect,
) -> ComputedScene<()> {
    tree.collect_scene_from_layout(
        font_manager,
        layout,
        theme,
        media,
        animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
    )
}

fn shape_fingerprints(shapes: &[RenderPrimitive]) -> Vec<(Rect, Color, u32, u32, Option<Rect>)> {
    shapes
        .iter()
        .map(|shape| {
            (
                shape.rect,
                shape.color,
                shape.corner_radius.to_bits(),
                shape.stroke_width.to_bits(),
                shape.clip_rect,
            )
        })
        .collect()
}

fn scroll_fingerprints(scene: &ComputedScene<()>) -> Vec<(Rect, Rect, Rect, Point)> {
    scene
        .scroll_regions
        .iter()
        .map(|region| {
            (
                region.content_viewport,
                region.visible_frame,
                region.content_bounds,
                region.scroll_offset,
            )
        })
        .collect()
}

#[test]
fn combobox_trigger_and_virtual_viewport_follow_runtime_density_on_the_same_tree() {
    let tree = combo_tree(8);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 640.0, 560.0);

    for (density, width, option_height) in [
        (Density::Compact, dp(240.0), dp(32.0)),
        (Density::Comfortable, dp(260.0), dp(40.0)),
        (Density::Spacious, dp(288.0), dp(48.0)),
    ] {
        let mut theme = Theme::light();
        theme.density = density;
        let mut animations = AnimationEngine::default();
        let layout = build_layout(
            &tree,
            &theme,
            &font_manager,
            &media,
            &mut animations,
            viewport,
        );

        assert_eq!(
            layout.resolved_root.layout.width,
            Some(Value::Static(Length::Px(width)))
        );
        let ResolvedWidgetKind::TextEditor { style, .. } = &layout.resolved_root.kind else {
            panic!("combobox trigger should remain the TextEditor root");
        };
        assert_eq!(style.min_height, option_height);

        let mut animations = AnimationEngine::default();
        let scene = collect_scene(
            &tree,
            &layout,
            &theme,
            &font_manager,
            &media,
            &mut animations,
            viewport,
        );
        let update = scene
            .virtual_state_updates
            .first()
            .expect("open combobox should emit a virtual viewport update");
        let content_width = width - theme.border.thin - theme.border.thin;
        let content_height = option_height * 6.0 - theme.border.thin - theme.border.thin;
        assert_eq!(update.viewport_hint.width, content_width);
        assert_eq!(update.viewport_hint.height, content_height);

        let menu_scroll = scene
            .scroll_regions
            .iter()
            .find(|region| region.can_scroll_y())
            .expect("eight options in a six-row viewport should scroll");
        assert_eq!(menu_scroll.visible_frame.width, width);
        assert_eq!(menu_scroll.visible_frame.height, option_height * 6.0);
        assert_eq!(menu_scroll.content_viewport.width, content_width);
        assert_eq!(menu_scroll.content_viewport.height, content_height);

        assert!(
            scene.scene.overlay_shapes.iter().any(|shape| {
                shape.color == theme.colors.surface_overlay
                    && shape.rect.width == content_width
                    && shape.rect.height == content_height
            }),
            "density={density:?}, shapes={:?}",
            shape_fingerprints(&scene.scene.overlay_shapes)
        );
        assert!(scene.scene.overlay_shapes.iter().any(|shape| {
            shape.color == theme.colors.outline_muted
                && shape.rect.width == width
                && shape.rect.height == option_height * 6.0
        }));
    }
}

#[test]
fn combobox_explicit_trigger_size_wins_and_dropdown_matches_width() {
    let tree: WidgetTree<()> = WidgetTree::new(
        Combobox::new(TextController::from(""), options(8))
            .open(true)
            .size(dp(333.0), dp(54.0)),
    );
    let theme = Theme::light();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 720.0, 560.0);
    let mut animations = AnimationEngine::default();
    let layout = build_layout(
        &tree,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        viewport,
    );

    assert_eq!(
        layout.resolved_root.layout.width,
        Some(Value::Static(Length::Px(dp(333.0))))
    );
    assert_eq!(
        layout.resolved_root.layout.height,
        Some(Value::Static(Length::Px(dp(54.0))))
    );

    let mut animations = AnimationEngine::default();
    let scene = collect_scene(
        &tree,
        &layout,
        &theme,
        &font_manager,
        &media,
        &mut animations,
        viewport,
    );
    assert_eq!(
        scene.virtual_state_updates[0].viewport_hint.width,
        dp(333.0) - theme.border.thin - theme.border.thin
    );
}

#[test]
fn virtual_runtime_geometry_updates_window_plan_without_wrappers() {
    let tree: WidgetTree<()> = WidgetTree::new(
        VirtualList::new((0..30usize).collect::<Vec<_>>(), |index, _| {
            Text::new(format!("row {index}")).into()
        })
        .runtime_layout(|layout, item_layout, context, _, _| {
            let (width, height, item_extent) = match context.density {
                Density::Compact => (dp(120.0), dp(64.0), dp(16.0)),
                Density::Comfortable => (dp(140.0), dp(96.0), dp(24.0)),
                Density::Spacious => (dp(180.0), dp(144.0), dp(36.0)),
            };
            if layout.width.is_none() {
                layout.width = Some(Value::Static(Length::Px(width)));
            }
            if layout.height.is_none() {
                layout.height = Some(Value::Static(Length::Px(height)));
            }
            *item_layout = item_layout.with_estimate(item_extent);
        }),
    );
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);

    for (density, width, height, item_extent) in [
        (Density::Compact, dp(120.0), dp(64.0), dp(16.0)),
        (Density::Comfortable, dp(140.0), dp(96.0), dp(24.0)),
        (Density::Spacious, dp(180.0), dp(144.0), dp(36.0)),
    ] {
        let mut theme = Theme::light();
        theme.density = density;
        let mut animations = AnimationEngine::default();
        let layout = build_layout(
            &tree,
            &theme,
            &font_manager,
            &media,
            &mut animations,
            viewport,
        );
        assert_eq!(
            layout.resolved_root.layout.width,
            Some(Value::Static(Length::Px(width)))
        );
        assert_eq!(
            layout.resolved_root.layout.height,
            Some(Value::Static(Length::Px(height)))
        );
        let ResolvedWidgetKind::Virtual {
            item_layout,
            window_plan,
            ..
        } = &layout.resolved_root.kind
        else {
            panic!("runtime-layout virtual list should remain a single Virtual node");
        };
        assert_eq!(item_layout.estimate(), item_extent);
        assert_eq!(window_plan.total_main_extent, item_extent * 30.0);
    }
}

#[test]
fn combobox_density_layout_patch_matches_fresh_full_recollect() {
    let tree = combo_tree(12);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 720.0, 640.0);
    let mut compact = Theme::dark();
    compact.density = Density::Compact;
    let mut spacious = Theme::dark();
    spacious.density = Density::Spacious;

    let mut animations = AnimationEngine::default();
    let mut patched_layout = build_layout(
        &tree,
        &compact,
        &font_manager,
        &media,
        &mut animations,
        viewport,
    );
    let root_id = patched_layout.root_id();
    patched_layout
        .patch_layout_roots(
            &[root_id],
            &font_manager,
            &spacious,
            &media,
            &mut animations,
            viewport,
            Instant::now(),
        )
        .expect("runtime density change should patch the combobox root layout");
    let mut animations = AnimationEngine::default();
    let patched_scene = collect_scene(
        &tree,
        &patched_layout,
        &spacious,
        &font_manager,
        &media,
        &mut animations,
        viewport,
    );

    let mut animations = AnimationEngine::default();
    let full_layout = build_layout(
        &tree,
        &spacious,
        &font_manager,
        &media,
        &mut animations,
        viewport,
    );
    let mut animations = AnimationEngine::default();
    let full_scene = collect_scene(
        &tree,
        &full_layout,
        &spacious,
        &font_manager,
        &media,
        &mut animations,
        viewport,
    );

    assert_eq!(
        shape_fingerprints(&patched_scene.scene.shapes),
        shape_fingerprints(&full_scene.scene.shapes)
    );
    assert_eq!(
        shape_fingerprints(&patched_scene.scene.overlay_shapes),
        shape_fingerprints(&full_scene.scene.overlay_shapes)
    );
    assert_eq!(patched_scene.scene.texts, full_scene.scene.texts);
    assert_eq!(
        patched_scene.scene.overlay_texts,
        full_scene.scene.overlay_texts
    );
    assert_eq!(
        scroll_fingerprints(&patched_scene),
        scroll_fingerprints(&full_scene)
    );
    assert_eq!(
        patched_scene.virtual_state_updates[0].viewport_hint.width,
        full_scene.virtual_state_updates[0].viewport_hint.width
    );
    assert_eq!(
        patched_scene.virtual_state_updates[0].viewport_hint.height,
        full_scene.virtual_state_updates[0].viewport_hint.height
    );
}

#[test]
fn combobox_menu_uses_real_light_and_dark_surface_tokens() {
    let tree = combo_tree(3);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 640.0, 420.0);

    for theme in [Theme::light(), Theme::dark()] {
        let mut animations = AnimationEngine::default();
        let layout = build_layout(
            &tree,
            &theme,
            &font_manager,
            &media,
            &mut animations,
            viewport,
        );
        let mut animations = AnimationEngine::default();
        let scene = collect_scene(
            &tree,
            &layout,
            &theme,
            &font_manager,
            &media,
            &mut animations,
            viewport,
        );
        let style = ComboboxStyle::default_for_theme(&theme);
        assert_eq!(style.option_height, dp(40.0));
        let highlight = style.highlight.resolve();
        assert!(
            scene
                .scene
                .overlay_shapes
                .iter()
                .all(|shape| shape.color != highlight),
            "resting combobox options must stay transparent until hover/press"
        );
        assert!(scene
            .scene
            .overlay_shapes
            .iter()
            .any(|shape| shape.color == theme.colors.surface_overlay));
        assert!(scene.scene.overlay_shapes.iter().any(|shape| {
            shape.color == theme.colors.outline_muted
                && shape.stroke_width == theme.border.thin.get()
                && shape.corner_radius == theme.radius.xl.get()
        }));
        assert!(scene
            .scene
            .overlay_texts
            .iter()
            .any(|text| text.content.as_ref() == "Option 0"));
    }
}
