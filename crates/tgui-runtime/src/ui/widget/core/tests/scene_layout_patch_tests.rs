use super::*;

use crate::ui::theme::Density;
use crate::ui::widget::common::{HitRegion, HitTargetId, LayoutNode, RenderCommand, ScrollRegion};
use crate::ui::widget::core::{
    reset_layout_patch_stats, take_layout_patch_stats, LayoutPatchStats,
};
use crate::ui::widget::{
    Card, ComputedScene, DataGrid, DataGridCellContext, DataGridColumn, DataGridRow, Drawer,
    ItemLayout, MenuBar, MenuItem, MeshPrimitive, MeshVertex, Modal, ModalAction, RenderPrimitive,
    ResolvedElement, ResolvedSceneLayout, StyleSheet, TabItem, Tabs, Text, TexturePrimitive,
    WidgetId,
};

fn build_layout_with_sheet(
    tree: &WidgetTree<()>,
    theme: &Theme,
    style_sheet: &StyleSheet,
    font_manager: &FontManager,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    viewport: Rect,
    now: Instant,
) -> ResolvedSceneLayout<()> {
    tree.build_scene_layout_at_with_previous_and_style_sheet(
        font_manager,
        theme,
        media,
        animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        now,
        None,
        style_sheet,
    )
}

fn rebuild_layout_with_sheet_and_previous(
    tree: &WidgetTree<()>,
    theme: &Theme,
    style_sheet: &StyleSheet,
    font_manager: &FontManager,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    viewport: Rect,
    now: Instant,
    previous: &ResolvedSceneLayout<()>,
) -> ResolvedSceneLayout<()> {
    tree.build_scene_layout_at_with_previous_and_style_sheet(
        font_manager,
        theme,
        media,
        animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        now,
        Some(previous),
        style_sheet,
    )
}

fn collect_scene_with_sheet(
    tree: &WidgetTree<()>,
    layout: &ResolvedSceneLayout<()>,
    theme: &Theme,
    style_sheet: &StyleSheet,
    font_manager: &FontManager,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    viewport: Rect,
    now: Instant,
) -> ComputedScene<()> {
    tree.collect_scene_cache_from_layout_with_focus_value_at(
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
        None,
        None,
        None,
        None,
        false,
        now,
        &HashMap::new(),
        None,
        None,
        style_sheet,
    )
    .computed
}

fn assert_layout_tree_equivalent_with_identity(
    patched: &ResolvedSceneLayout<()>,
    full: &ResolvedSceneLayout<()>,
    compare_identity: bool,
) {
    fn compare_nodes(
        patched_layout: &ResolvedSceneLayout<()>,
        patched: &ResolvedElement<()>,
        patched_node: &LayoutNode,
        full_layout: &ResolvedSceneLayout<()>,
        full: &ResolvedElement<()>,
        full_node: &LayoutNode,
        compare_identity: bool,
    ) {
        if compare_identity {
            assert_eq!(patched.id, full.id);
        }
        assert!(
            patched.layout == full.layout,
            "resolved layout properties differ for widget {:?}",
            patched.id
        );
        assert_eq!(
            patched.visual.scale.resolve(),
            full.visual.scale.resolve(),
            "resolved runtime scale differs for widget {:?}",
            patched.id
        );
        assert_eq!(
            patched_layout
                .taffy
                .layout(patched_node.node)
                .expect("patched Taffy layout should exist"),
            full_layout
                .taffy
                .layout(full_node.node)
                .expect("full Taffy layout should exist"),
            "computed Taffy layout differs for widget {:?}",
            patched.id
        );

        let patched_children = match &patched.kind {
            ResolvedWidgetKind::Container { children, .. }
            | ResolvedWidgetKind::Virtual { children, .. } => children.as_slice(),
            _ => &[],
        };
        let full_children = match &full.kind {
            ResolvedWidgetKind::Container { children, .. }
            | ResolvedWidgetKind::Virtual { children, .. } => children.as_slice(),
            _ => &[],
        };
        assert_eq!(patched_children.len(), full_children.len());
        assert_eq!(patched_node.children.len(), full_node.children.len());
        for index in 0..patched_children.len() {
            compare_nodes(
                patched_layout,
                &patched_children[index],
                &patched_node.children[index],
                full_layout,
                &full_children[index],
                &full_node.children[index],
                compare_identity,
            );
        }
    }

    compare_nodes(
        patched,
        &patched.resolved_root,
        &patched.layout_root,
        full,
        &full.resolved_root,
        &full.layout_root,
        compare_identity,
    );
}

fn assert_layout_tree_equivalent(
    patched: &ResolvedSceneLayout<()>,
    full: &ResolvedSceneLayout<()>,
) {
    assert_layout_tree_equivalent_with_identity(patched, full, true);
}

fn assert_layout_tree_equivalent_ignoring_identity(
    patched: &ResolvedSceneLayout<()>,
    full: &ResolvedSceneLayout<()>,
) {
    assert_layout_tree_equivalent_with_identity(patched, full, false);
}

fn shape_fingerprint(
    shapes: &[RenderPrimitive],
) -> Vec<(Rect, Color, u32, u32, Option<Rect>, Option<ClipMask>)> {
    shapes
        .iter()
        .map(|shape| {
            (
                shape.rect,
                shape.color,
                shape.corner_radius.to_bits(),
                shape.stroke_width.to_bits(),
                shape.clip_rect,
                shape.clip_mask,
            )
        })
        .collect()
}

fn texture_fingerprint(
    textures: &[TexturePrimitive],
) -> Vec<(
    Rect,
    Option<[Point; 4]>,
    Option<Rect>,
    u32,
    u32,
    Option<Rect>,
)> {
    textures
        .iter()
        .map(|texture| {
            (
                texture.frame,
                texture.quad,
                texture.uv_rect,
                texture.corner_radius.to_bits(),
                texture.opacity.to_bits(),
                texture.clip_rect,
            )
        })
        .collect()
}

fn mesh_fingerprint(meshes: &[MeshPrimitive]) -> Vec<(Vec<MeshVertex>, Option<Rect>)> {
    meshes
        .iter()
        .map(|mesh| (mesh.vertices.to_vec(), mesh.clip_rect))
        .collect()
}

fn command_fingerprint(commands: &[RenderCommand]) -> Vec<&'static str> {
    commands
        .iter()
        .map(|command| match command {
            RenderCommand::BackdropBlur(_) => "backdrop-blur",
            RenderCommand::Brush(_) => "brush",
            RenderCommand::CanvasComposite(_) => "canvas-composite",
            RenderCommand::Shape(_) => "shape",
            RenderCommand::Texture(_) => "texture",
            #[cfg(feature = "video")]
            RenderCommand::VideoTexture(_) => "video-texture",
            RenderCommand::Text(_) => "text",
            RenderCommand::TextDecoration(_) => "text-decoration",
            RenderCommand::Mesh(_) => "mesh",
        })
        .collect()
}

fn hit_fingerprint(
    hits: &[HitRegion<()>],
) -> Vec<(
    HitTargetId,
    Rect,
    Option<Rect>,
    Vec<WidgetId>,
    Option<WidgetId>,
)> {
    hits.iter()
        .map(|hit| {
            (
                hit.interaction.target_id(),
                hit.rect,
                hit.clip_rect,
                hit.scope_path.clone(),
                hit.gpu_scroll_container,
            )
        })
        .collect()
}

fn scroll_fingerprint(
    regions: &[ScrollRegion],
) -> Vec<(
    WidgetId,
    Rect,
    Rect,
    Rect,
    Point,
    Point,
    Overflow,
    Overflow,
    Option<Rect>,
    Option<Rect>,
    Option<Rect>,
    Option<Rect>,
)> {
    regions
        .iter()
        .map(|region| {
            (
                region.id,
                region.content_viewport,
                region.visible_frame,
                region.content_bounds,
                region.gpu_base_scroll_offset,
                region.scroll_offset,
                region.overflow_x,
                region.overflow_y,
                region.horizontal_track,
                region.horizontal_thumb,
                region.vertical_track,
                region.vertical_thumb,
            )
        })
        .collect()
}

fn assert_scene_equivalent(patched: &ComputedScene<()>, full: &ComputedScene<()>) {
    assert_eq!(
        shape_fingerprint(&patched.scene.shapes),
        shape_fingerprint(&full.scene.shapes)
    );
    assert_eq!(patched.scene.brushes, full.scene.brushes);
    assert_eq!(patched.scene.backdrop_blurs, full.scene.backdrop_blurs);
    assert_eq!(patched.scene.texts, full.scene.texts);
    assert_eq!(patched.scene.text_decorations, full.scene.text_decorations);
    assert_eq!(
        texture_fingerprint(&patched.scene.textures),
        texture_fingerprint(&full.scene.textures)
    );
    assert_eq!(
        mesh_fingerprint(&patched.scene.meshes),
        mesh_fingerprint(&full.scene.meshes)
    );
    assert_eq!(
        shape_fingerprint(&patched.scene.overlay_shapes),
        shape_fingerprint(&full.scene.overlay_shapes)
    );
    assert_eq!(patched.scene.overlay_texts, full.scene.overlay_texts);
    assert_eq!(
        texture_fingerprint(&patched.scene.overlay_textures),
        texture_fingerprint(&full.scene.overlay_textures)
    );
    assert_eq!(
        command_fingerprint(&patched.scene.commands),
        command_fingerprint(&full.scene.commands)
    );
    assert_eq!(
        command_fingerprint(&patched.scene.overlay_commands),
        command_fingerprint(&full.scene.overlay_commands)
    );
    assert_eq!(
        hit_fingerprint(&patched.hit_regions),
        hit_fingerprint(&full.hit_regions)
    );
    assert_eq!(
        hit_fingerprint(&patched.overlay_hit_regions),
        hit_fingerprint(&full.overlay_hit_regions)
    );
    assert_eq!(
        scroll_fingerprint(&patched.scroll_regions),
        scroll_fingerprint(&full.scroll_regions)
    );
    assert_eq!(patched.ime_cursor_area, full.ime_cursor_area);
    assert_eq!(
        patched.virtual_state_updates.len(),
        full.virtual_state_updates.len()
    );
}

fn assert_scene_content_equivalent_ignoring_identity(
    patched: &ComputedScene<()>,
    full: &ComputedScene<()>,
) {
    assert_eq!(
        shape_fingerprint(&patched.scene.shapes),
        shape_fingerprint(&full.scene.shapes)
    );
    assert_eq!(patched.scene.brushes, full.scene.brushes);
    assert_eq!(patched.scene.backdrop_blurs, full.scene.backdrop_blurs);
    assert_eq!(patched.scene.texts, full.scene.texts);
    assert_eq!(patched.scene.text_decorations, full.scene.text_decorations);
    assert_eq!(
        texture_fingerprint(&patched.scene.textures),
        texture_fingerprint(&full.scene.textures)
    );
    assert_eq!(
        mesh_fingerprint(&patched.scene.meshes),
        mesh_fingerprint(&full.scene.meshes)
    );
    assert_eq!(
        shape_fingerprint(&patched.scene.overlay_shapes),
        shape_fingerprint(&full.scene.overlay_shapes)
    );
    assert_eq!(patched.scene.overlay_texts, full.scene.overlay_texts);
    assert_eq!(
        texture_fingerprint(&patched.scene.overlay_textures),
        texture_fingerprint(&full.scene.overlay_textures)
    );
    assert_eq!(
        command_fingerprint(&patched.scene.commands),
        command_fingerprint(&full.scene.commands)
    );
    assert_eq!(
        command_fingerprint(&patched.scene.overlay_commands),
        command_fingerprint(&full.scene.overlay_commands)
    );
    assert_eq!(
        patched
            .hit_regions
            .iter()
            .map(|hit| (hit.rect, hit.clip_rect))
            .collect::<Vec<_>>(),
        full.hit_regions
            .iter()
            .map(|hit| (hit.rect, hit.clip_rect))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        patched
            .overlay_hit_regions
            .iter()
            .map(|hit| (hit.rect, hit.clip_rect))
            .collect::<Vec<_>>(),
        full.overlay_hit_regions
            .iter()
            .map(|hit| (hit.rect, hit.clip_rect))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        patched
            .scroll_regions
            .iter()
            .map(|region| {
                (
                    region.content_viewport,
                    region.visible_frame,
                    region.content_bounds,
                    region.gpu_base_scroll_offset,
                    region.scroll_offset,
                    region.overflow_x,
                    region.overflow_y,
                    region.horizontal_track,
                    region.horizontal_thumb,
                    region.vertical_track,
                    region.vertical_thumb,
                )
            })
            .collect::<Vec<_>>(),
        full.scroll_regions
            .iter()
            .map(|region| {
                (
                    region.content_viewport,
                    region.visible_frame,
                    region.content_bounds,
                    region.gpu_base_scroll_offset,
                    region.scroll_offset,
                    region.overflow_x,
                    region.overflow_y,
                    region.horizontal_track,
                    region.horizontal_thumb,
                    region.vertical_track,
                    region.vertical_thumb,
                )
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(patched.ime_cursor_area, full.ime_cursor_area);
    assert_eq!(
        patched.virtual_state_updates.len(),
        full.virtual_state_updates.len()
    );
    for (patched, full) in patched
        .virtual_state_updates
        .iter()
        .zip(&full.virtual_state_updates)
    {
        assert_eq!(patched.viewport_hint.width, full.viewport_hint.width);
        assert_eq!(patched.viewport_hint.height, full.viewport_hint.height);
        assert_eq!(patched.measured_extents, full.measured_extents);
        assert_eq!(patched.invalidate_layout, full.invalidate_layout);
    }
}

fn assert_patch_matches_full(
    tree: &WidgetTree<()>,
    patched_layout: &ResolvedSceneLayout<()>,
    full_layout: &ResolvedSceneLayout<()>,
    theme: &Theme,
    style_sheet: &StyleSheet,
    font_manager: &FontManager,
    media: &MediaManager,
    viewport: Rect,
    now: Instant,
) {
    assert_layout_tree_equivalent(patched_layout, full_layout);
    let patched_scene = collect_scene_with_sheet(
        tree,
        patched_layout,
        theme,
        style_sheet,
        font_manager,
        media,
        &mut AnimationEngine::default(),
        viewport,
        now,
    );
    let full_scene = collect_scene_with_sheet(
        tree,
        full_layout,
        theme,
        style_sheet,
        font_manager,
        media,
        &mut AnimationEngine::default(),
        viewport,
        now,
    );
    assert_scene_equivalent(&patched_scene, &full_scene);
}

fn assert_scene_content_from_layouts_ignoring_identity(
    tree: &WidgetTree<()>,
    patched_layout: &ResolvedSceneLayout<()>,
    fresh_layout: &ResolvedSceneLayout<()>,
    theme: &Theme,
    style_sheet: &StyleSheet,
    font_manager: &FontManager,
    media: &MediaManager,
    viewport: Rect,
    now: Instant,
) {
    let patched_scene = collect_scene_with_sheet(
        tree,
        patched_layout,
        theme,
        style_sheet,
        font_manager,
        media,
        &mut AnimationEngine::default(),
        viewport,
        now,
    );
    let fresh_scene = collect_scene_with_sheet(
        tree,
        fresh_layout,
        theme,
        style_sheet,
        font_manager,
        media,
        &mut AnimationEngine::default(),
        viewport,
        now,
    );
    assert_scene_content_equivalent_ignoring_identity(&patched_scene, &fresh_scene);
}

fn assert_patch_matches_fresh_content(
    tree: &WidgetTree<()>,
    patched_layout: &ResolvedSceneLayout<()>,
    theme: &Theme,
    style_sheet: &StyleSheet,
    font_manager: &FontManager,
    media: &MediaManager,
    viewport: Rect,
    now: Instant,
) {
    let fresh_layout = build_layout_with_sheet(
        tree,
        theme,
        style_sheet,
        font_manager,
        media,
        &mut AnimationEngine::default(),
        viewport,
        now,
    );
    assert_layout_tree_equivalent_ignoring_identity(patched_layout, &fresh_layout);
    assert_scene_content_from_layouts_ignoring_identity(
        tree,
        patched_layout,
        &fresh_layout,
        theme,
        style_sheet,
        font_manager,
        media,
        viewport,
        now,
    );
}

fn assert_patch_matches_identity_stable_full(
    tree: &WidgetTree<()>,
    patched_layout: &ResolvedSceneLayout<()>,
    theme: &Theme,
    style_sheet: &StyleSheet,
    font_manager: &FontManager,
    media: &MediaManager,
    viewport: Rect,
    now: Instant,
) {
    let identity_stable_full = rebuild_layout_with_sheet_and_previous(
        tree,
        theme,
        style_sheet,
        font_manager,
        media,
        &mut AnimationEngine::default(),
        viewport,
        now,
        patched_layout,
    );
    assert_patch_matches_full(
        tree,
        patched_layout,
        &identity_stable_full,
        theme,
        style_sheet,
        font_manager,
        media,
        viewport,
        now,
    );
}

fn first_virtual_layout_snapshot(
    node: &ResolvedElement<()>,
) -> Option<(ItemLayout, std::ops::Range<usize>, usize, Dp)> {
    match &node.kind {
        ResolvedWidgetKind::Virtual {
            item_layout,
            window_plan,
            children,
            ..
        } => Some((
            *item_layout,
            window_plan.visible_range.clone(),
            children.len(),
            window_plan.total_main_extent,
        )),
        ResolvedWidgetKind::Container { children, .. } => {
            children.iter().find_map(first_virtual_layout_snapshot)
        }
        _ => None,
    }
}

#[test]
fn data_grid_density_patch_uses_real_viewport_for_virtual_window() {
    let rows = (0..12)
        .map(|index| DataGridRow::keyed(index, format!("Row {index}")))
        .collect::<Vec<_>>();
    let columns = vec![DataGridColumn::new(
        "name",
        "Name".to_string(),
        |context: DataGridCellContext<String>| Text::new(context.row).into(),
    )
    .width(dp(160.0))];
    let tree = WidgetTree::new(
        DataGrid::new(rows, columns)
            .style(|style, context| {
                style.regular_row_height = match context.density {
                    Density::Compact => dp(24.0),
                    Density::Comfortable => dp(32.0),
                    Density::Spacious => dp(48.0),
                };
            })
            .size(dp(220.0), dp(160.0)),
    );
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let style_sheet = StyleSheet::default();
    let viewport = Rect::new(0.0, 0.0, 220.0, 160.0);
    let now = Instant::now();
    let mut initial = Theme::dark();
    initial.density = Density::Compact;
    let mut target = Theme::light();
    target.density = Density::Spacious;

    let mut animations = AnimationEngine::default();
    let mut patched = build_layout_with_sheet(
        &tree,
        &initial,
        &style_sheet,
        &font_manager,
        &media,
        &mut animations,
        viewport,
        now,
    );
    let root_id = patched.root_id();
    patched
        .patch_layout_roots(
            &[root_id],
            &font_manager,
            &target,
            &media,
            &mut animations,
            viewport,
            now,
        )
        .expect("DataGrid density patch should succeed");

    let full = build_layout_with_sheet(
        &tree,
        &target,
        &style_sheet,
        &font_manager,
        &media,
        &mut AnimationEngine::default(),
        viewport,
        now,
    );
    let patched_snapshot = first_virtual_layout_snapshot(&patched.resolved_root)
        .expect("patched DataGrid should contain a Virtual body");
    let full_snapshot = first_virtual_layout_snapshot(&full.resolved_root)
        .expect("full DataGrid should contain a Virtual body");

    assert_eq!(patched_snapshot, full_snapshot);
    assert_eq!(patched_snapshot.0.estimate(), dp(48.0));
    assert!(
        patched_snapshot.2 > 3,
        "a real 160dp viewport must materialize visible rows beyond the three-row zero-viewport overscan"
    );
}

#[test]
fn non_container_layout_patch_matches_full_across_theme_and_density_matrix() {
    let tree = WidgetTree::new(Input::new("theme-sensitive input"));
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let viewport = Rect::new(0.0, 0.0, 420.0, 180.0);
    let style_sheet = StyleSheet::default();

    let targets = [
        (Theme::light(), Density::Compact),
        (Theme::light(), Density::Comfortable),
        (Theme::light(), Density::Spacious),
        (Theme::dark(), Density::Compact),
        (Theme::dark(), Density::Comfortable),
        (Theme::dark(), Density::Spacious),
    ];
    for (mut target, density) in targets {
        target.density = density;
        let mut initial = if matches!(target.mode, crate::ui::theme::ResolvedThemeMode::Light) {
            Theme::dark()
        } else {
            Theme::light()
        };
        initial.density = Density::Comfortable;
        let now = Instant::now();
        let mut animations = AnimationEngine::default();
        let mut patched = build_layout_with_sheet(
            &tree,
            &initial,
            &style_sheet,
            &font_manager,
            &media,
            &mut animations,
            viewport,
            now,
        );
        let root_id = patched.root_id();

        reset_layout_patch_stats();
        patched
            .patch_layout_roots(
                &[root_id],
                &font_manager,
                &target,
                &media,
                &mut animations,
                viewport,
                now,
            )
            .expect("non-container root should patch");
        assert_eq!(
            take_layout_patch_stats(),
            LayoutPatchStats {
                visited_nodes: 1,
                reused_children: 0,
                rebuilt_children: 0,
                removed_subtrees: 0,
            }
        );

        let full = build_layout_with_sheet(
            &tree,
            &target,
            &style_sheet,
            &font_manager,
            &media,
            &mut AnimationEngine::default(),
            viewport,
            now,
        );
        assert_patch_matches_full(
            &tree,
            &patched,
            &full,
            &target,
            &style_sheet,
            &font_manager,
            &media,
            viewport,
            now,
        );
    }
}

#[test]
fn container_patch_reuses_stable_children_and_rebuilds_only_structural_delta() {
    let context = test_context();
    let expanded = context.state(false);
    let first: Element<()> = Text::new("first").into();
    let second: Element<()> = Text::new("second").into();
    let tree = WidgetTree::new_legacy(Stack::<()>::new().dynamic_child({
        let first = first.clone();
        let second = second.clone();
        expanded.signal().map_unchecked(move |expanded| {
            if expanded {
                vec![first.clone(), second.clone()]
            } else {
                vec![first.clone()]
            }
        })
    }));
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::light();
    let style_sheet = StyleSheet::default();
    let viewport = Rect::new(0.0, 0.0, 320.0, 180.0);
    let now = Instant::now();
    let mut animations = AnimationEngine::default();
    let mut patched = build_layout_with_sheet(
        &tree,
        &theme,
        &style_sheet,
        &font_manager,
        &media,
        &mut animations,
        viewport,
        now,
    );
    let root_id = patched.root_id();

    reset_layout_patch_stats();
    patched
        .patch_layout_roots(
            &[root_id],
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
            now,
        )
        .expect("stable container subtree should patch");
    assert_eq!(
        take_layout_patch_stats(),
        LayoutPatchStats {
            visited_nodes: 2,
            reused_children: 1,
            rebuilt_children: 0,
            removed_subtrees: 0,
        }
    );

    expanded.set(true);
    reset_layout_patch_stats();
    patched
        .patch_layout_roots(
            &[root_id],
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
            now,
        )
        .expect("adding one child should patch");
    assert_eq!(
        take_layout_patch_stats(),
        LayoutPatchStats {
            visited_nodes: 2,
            reused_children: 1,
            rebuilt_children: 1,
            removed_subtrees: 0,
        }
    );
    let full = build_layout_with_sheet(
        &tree,
        &theme,
        &style_sheet,
        &font_manager,
        &media,
        &mut AnimationEngine::default(),
        viewport,
        now,
    );
    assert_patch_matches_full(
        &tree,
        &patched,
        &full,
        &theme,
        &style_sheet,
        &font_manager,
        &media,
        viewport,
        now,
    );

    expanded.set(false);
    reset_layout_patch_stats();
    patched
        .patch_layout_roots(
            &[root_id],
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
            now,
        )
        .expect("removing one child should patch");
    assert_eq!(
        take_layout_patch_stats(),
        LayoutPatchStats {
            visited_nodes: 2,
            reused_children: 1,
            rebuilt_children: 0,
            removed_subtrees: 1,
        }
    );
    let full = build_layout_with_sheet(
        &tree,
        &theme,
        &style_sheet,
        &font_manager,
        &media,
        &mut AnimationEngine::default(),
        viewport,
        now,
    );
    assert_patch_matches_full(
        &tree,
        &patched,
        &full,
        &theme,
        &style_sheet,
        &font_manager,
        &media,
        viewport,
        now,
    );
}

#[test]
fn custom_style_sheet_runtime_patch_matches_full_and_missing_root_is_noop() {
    let tree = WidgetTree::new(
        Card::new()
            .body(Text::new("custom sheet"))
            .style_id("audit-card"),
    );
    let initial_sheet = StyleSheet::new().card_id("audit-card", |style, _| {
        style.padding = Insets::all(dp(4.0));
        style.background = Color::hexa(0x112233FF).into();
    });
    let target_sheet = StyleSheet::new().card_id("audit-card", |style, _| {
        style.padding = Insets::all(dp(28.0));
        style.gap = dp(17.0);
        style.background = Color::hexa(0x3A7AFEFF).into();
        style.radius = dp(19.0);
    });
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut theme = Theme::dark();
    theme.density = Density::Spacious;
    let viewport = Rect::new(0.0, 0.0, 420.0, 240.0);
    let now = Instant::now();
    let mut animations = AnimationEngine::default();
    let mut patched = build_layout_with_sheet(
        &tree,
        &theme,
        &initial_sheet,
        &font_manager,
        &media,
        &mut animations,
        viewport,
        now,
    );

    reset_layout_patch_stats();
    patched
        .patch_layout_roots(
            &[WidgetId::next()],
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
            now,
        )
        .expect("missing root should be a clean no-op");
    assert_eq!(take_layout_patch_stats(), LayoutPatchStats::default());

    let root_id = patched.root_id();
    reset_layout_patch_stats();
    patched
        .patch_layout_roots_with_runtime_state(
            &[root_id],
            &font_manager,
            &theme,
            &media,
            &mut animations,
            &HashMap::new(),
            &HashMap::new(),
            viewport,
            now,
            false,
            &target_sheet,
        )
        .expect("custom StyleSheet root should patch");
    assert_eq!(
        take_layout_patch_stats(),
        LayoutPatchStats {
            visited_nodes: 2,
            reused_children: 1,
            rebuilt_children: 0,
            removed_subtrees: 0,
        }
    );

    let full = build_layout_with_sheet(
        &tree,
        &theme,
        &target_sheet,
        &font_manager,
        &media,
        &mut AnimationEngine::default(),
        viewport,
        now,
    );
    assert_patch_matches_full(
        &tree,
        &patched,
        &full,
        &theme,
        &target_sheet,
        &font_manager,
        &media,
        viewport,
        now,
    );
}

fn modern_composite_menu() -> Element<()> {
    MenuBar::new(Some(0usize))
        .entry("File", vec![MenuItem::new("New")])
        .entry("Edit", vec![MenuItem::new("Undo")])
        .style(|style, context| {
            style.entry_gap = match context.density {
                Density::Compact => dp(2.0),
                Density::Comfortable => dp(6.0),
                Density::Spacious => dp(12.0),
            };
        })
        .into()
}

fn modern_composite_tabs() -> Element<()> {
    Tabs::new(
        vec![
            TabItem::new("one", "One", Text::new("Panel one")),
            TabItem::new("two", "Two", Text::new("Panel two")),
        ],
        "one".to_string(),
    )
    .style(|style, context| {
        style.tab_gap = match context.density {
            Density::Compact => dp(3.0),
            Density::Comfortable => dp(7.0),
            Density::Spacious => dp(13.0),
        };
    })
    .height(dp(120.0))
    .into()
}

fn modern_composite_data_grid() -> Element<()> {
    // Four rows are enough to distinguish the real viewport window from the
    // old three-row zero-viewport overscan while keeping this broad composite
    // regression comfortably within the default test-thread stack.
    let rows = (0..4)
        .map(|index| DataGridRow::keyed(index, format!("Row {index}")))
        .collect::<Vec<_>>();
    let columns: Vec<DataGridColumn<String, ()>> = vec![DataGridColumn::new(
        "name",
        "Name".to_string(),
        |context: DataGridCellContext<String>| Text::new(context.row).into(),
    )
    .width(dp(220.0))];
    DataGrid::new(rows, columns)
        .style(|style, context| match context.density {
            Density::Compact => {
                style.header_height = dp(32.0);
                style.regular_row_height = dp(28.0);
                style.cell_padding = Insets::all(dp(3.0));
            }
            Density::Comfortable => {}
            Density::Spacious => {
                style.header_height = dp(56.0);
                style.regular_row_height = dp(52.0);
                style.cell_padding = Insets::all(dp(15.0));
            }
        })
        .height(dp(260.0))
        .into()
}

fn modern_composite_modal() -> Element<()> {
    Modal::new(false)
        .title("Patch modal")
        .content(Text::new("Hidden"))
        .action(ModalAction::primary("OK"))
        .style(|style, context| {
            style.enter_scale = match context.density {
                Density::Compact => 0.86,
                Density::Comfortable => 0.91,
                Density::Spacious => 0.96,
            };
        })
        .into()
}

fn modern_composite_drawer() -> Element<()> {
    Drawer::new(true)
        .placement(crate::ui::widget::DrawerPlacement::Right)
        .content(Text::new("Drawer"))
        .style(|style, context| {
            style.width = match context.density {
                Density::Compact => dp(180.0),
                Density::Comfortable => dp(220.0),
                Density::Spacious => dp(280.0),
            };
        })
        .into()
}

fn modern_composite_tree() -> WidgetTree<()> {
    let content = Flex::<()>::vertical()
        .child(modern_composite_menu())
        .child(modern_composite_tabs())
        .child(modern_composite_data_grid());
    WidgetTree::new(
        Stack::<()>::new()
            .size(dp(720.0), dp(640.0))
            .child(content)
            .child(modern_composite_modal())
            .child(modern_composite_drawer()),
    )
}

#[test]
fn modern_composite_runtime_metrics_patch_matches_full_rebuild() {
    let tree = modern_composite_tree();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let style_sheet = StyleSheet::default();
    let viewport = Rect::new(0.0, 0.0, 720.0, 640.0);
    let now = Instant::now();
    let mut initial = Theme::dark();
    initial.density = Density::Compact;
    let mut target = Theme::light();
    target.density = Density::Spacious;
    let mut animations = AnimationEngine::default();
    let mut patched = build_layout_with_sheet(
        &tree,
        &initial,
        &style_sheet,
        &font_manager,
        &media,
        &mut animations,
        viewport,
        now,
    );
    let root_id = patched.root_id();

    reset_layout_patch_stats();
    patched
        .patch_layout_roots(
            &[root_id],
            &font_manager,
            &target,
            &media,
            &mut animations,
            viewport,
            now,
        )
        .expect("modern composite root should patch");
    let stats = take_layout_patch_stats();
    assert!(stats.visited_nodes > 10);

    assert_patch_matches_fresh_content(
        &tree,
        &patched,
        &target,
        &style_sheet,
        &font_manager,
        &media,
        viewport,
        now,
    );

    // A production full rebuild carries a previous resolved layout so dynamic
    // Virtual rows retain widget identities. This second control proves the
    // identity-bearing hit/scroll metadata as well as all visual content.
    assert_patch_matches_identity_stable_full(
        &tree,
        &patched,
        &target,
        &style_sheet,
        &font_manager,
        &media,
        viewport,
        now,
    );
}
