//! `emit_overlay` portal collect 阶段集成测试。
//!
//! 验证：
//! - collect 阶段只登记 portal entry，不直接写 overlay scene；
//! - finalize 后局部坐标会被平移到窗口坐标；
//! - finalize 后 clip_rect 会被强制覆盖为 solver 算出的 clip 范围；
//! - finalize 后 close handler 会注册到 `ComputedScene::overlay_close_handlers`；
//! - `FlipPolicy::Hide` 在两侧都放不下时 finalize 时跳过所有 push；
//! - 多浮层按 `OverlayLayer` 顺序进入平面列表（finalize 后 Tooltip < Popover < Menu < Modal）。

use crate::foundation::color::Color;
use crate::media::TextureFrame;
use crate::runtime::overlay::collect::emit_overlay;
use crate::runtime::overlay::{
    Anchor, FlipPolicy, Overlay, OverlayContent, OverlayId, OverlayLayer, OverlayPrimitive,
    Placement,
};
use crate::ui::unit::{dp, Dp};
use crate::ui::widget::{
    ComputedScene, FocusScopeOptions, FocusScopeState, FocusTargetMeta, HitGeometry,
    HitInteraction, HitRegion, Rect, RenderCommand, RenderPrimitive, TexturePrimitive, WidgetId,
};

fn viewport() -> Rect {
    Rect::new(dp(0.0), dp(0.0), dp(800.0), dp(600.0))
}

fn shape(rect_local: Rect, color: Color) -> OverlayPrimitive {
    OverlayPrimitive::Shape(RenderPrimitive {
        rect: rect_local,
        color,
        corner_radius: 0.0,
        stroke_width: 0.0,
        clip_rect: None,
        clip_mask: None,
    })
}

fn texture(frame_local: Rect) -> OverlayPrimitive {
    OverlayPrimitive::Texture(TexturePrimitive {
        texture: std::sync::Arc::new(TextureFrame::new(4, 4, vec![255; 4 * 4 * 4])),
        frame: frame_local,
        quad: None,
        uv_rect: None,
        corner_radius: 2.0,
        opacity: 0.75,
        clip_rect: None,
        clip_mask: None,
    })
}

#[test]
fn emit_overlay_writes_to_overlay_scene_not_main() {
    let mut scene = ComputedScene::<()>::default();
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(40.0), dp(40.0));
    let overlay = Overlay::<()>::new(OverlayId::new(1), Anchor::Rect(anchor))
        .placement(Placement::bottom())
        .offset(dp(8.0));

    let prims = vec![shape(
        Rect::new(dp(0.0), dp(0.0), dp(80.0), dp(40.0)),
        Color::rgba(255, 0, 0, 255),
    )];

    let solved = emit_overlay(
        &mut scene,
        viewport(),
        overlay,
        (dp(80.0), dp(40.0)),
        OverlayContent::Primitives(prims),
    );
    assert_eq!(scene.portal_entries.len(), 1);
    assert_eq!(scene.scene.overlay_shapes.len(), 0);
    scene.finalize_portals(viewport());

    assert!(!solved.was_hidden);
    assert_eq!(scene.scene.shapes.len(), 0);
    assert_eq!(scene.scene.overlay_shapes.len(), 1);
}

#[test]
fn emit_overlay_translates_primitive_to_window_coords() {
    let mut scene = ComputedScene::<()>::default();
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(40.0), dp(40.0));
    let overlay = Overlay::<()>::new(OverlayId::new(1), Anchor::Rect(anchor))
        .placement(Placement::bottom())
        .offset(dp(8.0));

    let prims = vec![shape(
        Rect::new(dp(10.0), dp(5.0), dp(60.0), dp(30.0)),
        Color::rgba(0, 255, 0, 255),
    )];

    let _ = emit_overlay(
        &mut scene,
        viewport(),
        overlay,
        (dp(80.0), dp(40.0)),
        OverlayContent::Primitives(prims),
    );
    scene.finalize_portals(viewport());

    let translated = &scene.scene.overlay_shapes[0];
    let close = |a: Dp, b: f32| (a.get() - b).abs() < 0.001;
    assert!(close(translated.rect.x, 90.0));
    assert!(close(translated.rect.y, 153.0));
    assert!(close(translated.rect.width, 60.0));
    assert!(close(translated.rect.height, 30.0));
}

#[test]
fn emit_overlay_sets_clip_rect_from_solver() {
    let mut scene = ComputedScene::<()>::default();
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(40.0), dp(40.0));
    let overlay = Overlay::<()>::new(OverlayId::new(1), Anchor::Rect(anchor));

    let prims = vec![shape(
        Rect::new(dp(0.0), dp(0.0), dp(80.0), dp(40.0)),
        Color::rgba(0, 0, 255, 255),
    )];

    let solved = emit_overlay(
        &mut scene,
        viewport(),
        overlay,
        (dp(80.0), dp(40.0)),
        OverlayContent::Primitives(prims),
    );
    scene.finalize_portals(viewport());

    let shape = &scene.scene.overlay_shapes[0];
    assert_eq!(shape.clip_rect, Some(solved.clip_rect));
}

#[test]
fn emit_overlay_texture_writes_to_overlay_texture_bucket() {
    let mut scene = ComputedScene::<()>::default();
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(40.0), dp(40.0));
    let overlay = Overlay::<()>::new(OverlayId::new(1), Anchor::Rect(anchor))
        .placement(Placement::bottom())
        .offset(dp(8.0));

    let solved = emit_overlay(
        &mut scene,
        viewport(),
        overlay,
        (dp(80.0), dp(40.0)),
        OverlayContent::Primitives(vec![texture(Rect::new(
            dp(10.0),
            dp(5.0),
            dp(16.0),
            dp(12.0),
        ))]),
    );
    scene.finalize_portals(viewport());

    assert_eq!(scene.scene.textures.len(), 0);
    assert_eq!(scene.scene.overlay_textures.len(), 1);
    assert!(
        scene
            .scene
            .overlay_commands
            .iter()
            .any(|command| matches!(command, RenderCommand::Texture(_))),
        "overlay texture must also be present in overlay command order"
    );

    let translated = &scene.scene.overlay_textures[0];
    let close = |a: Dp, b: f32| (a.get() - b).abs() < 0.001;
    assert!(close(translated.frame.x, 90.0));
    assert!(close(translated.frame.y, 153.0));
    assert!(close(translated.frame.width, 16.0));
    assert!(close(translated.frame.height, 12.0));
    assert_eq!(translated.clip_rect, Some(solved.clip_rect));
}

#[test]
fn was_hidden_skips_all_emit() {
    let mut scene = ComputedScene::<()>::default();
    let small_viewport = Rect::new(dp(0.0), dp(0.0), dp(100.0), dp(100.0));
    let anchor = Rect::new(dp(50.0), dp(50.0), dp(10.0), dp(10.0));
    let overlay = Overlay::<()>::new(OverlayId::new(1), Anchor::Rect(anchor))
        .placement(Placement::bottom())
        .flip_policy(FlipPolicy::Hide)
        .close_on_outside_click(true);

    let prims = vec![shape(
        Rect::new(dp(0.0), dp(0.0), dp(300.0), dp(300.0)),
        Color::rgba(0, 0, 0, 255),
    )];

    let solved = emit_overlay(
        &mut scene,
        small_viewport,
        overlay,
        (dp(300.0), dp(300.0)),
        OverlayContent::Primitives(prims),
    );
    scene.finalize_portals(small_viewport);

    assert!(solved.was_hidden);
    assert_eq!(scene.scene.overlay_shapes.len(), 0);
    assert_eq!(scene.overlay_close_handlers.len(), 0);
}

#[test]
fn close_handle_registered_when_any_close_hook_set() {
    let mut scene = ComputedScene::<()>::default();
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(40.0), dp(40.0));
    let overlay = Overlay::<()>::new(OverlayId::new(42), Anchor::Rect(anchor))
        .layer(OverlayLayer::Menu)
        .close_on_outside_click(true)
        .close_on_escape(true);

    let _ = emit_overlay(
        &mut scene,
        viewport(),
        overlay,
        (dp(80.0), dp(40.0)),
        OverlayContent::Primitives(vec![]),
    );
    scene.finalize_portals(viewport());

    assert_eq!(scene.overlay_close_handlers.len(), 1);
    let handle = &scene.overlay_close_handlers[0];
    assert_eq!(handle.overlay_id, OverlayId::new(42));
    assert_eq!(handle.layer, OverlayLayer::Menu);
    assert!(handle.close_on_outside_click);
    assert!(handle.close_on_escape);
    assert!(!handle.close_value);
}

#[test]
fn close_handle_not_registered_when_no_close_hooks() {
    let mut scene = ComputedScene::<()>::default();
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(40.0), dp(40.0));
    let overlay = Overlay::<()>::new(OverlayId::new(1), Anchor::Rect(anchor));

    let _ = emit_overlay(
        &mut scene,
        viewport(),
        overlay,
        (dp(80.0), dp(40.0)),
        OverlayContent::Primitives(vec![]),
    );
    scene.finalize_portals(viewport());

    assert_eq!(scene.overlay_close_handlers.len(), 0);
}

#[test]
fn close_handle_rect_matches_solved_rect() {
    let mut scene = ComputedScene::<()>::default();
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(40.0), dp(40.0));
    let overlay = Overlay::<()>::new(OverlayId::new(1), Anchor::Rect(anchor))
        .placement(Placement::bottom())
        .close_on_outside_click(true);

    let solved = emit_overlay(
        &mut scene,
        viewport(),
        overlay,
        (dp(80.0), dp(40.0)),
        OverlayContent::Primitives(vec![]),
    );
    scene.finalize_portals(viewport());

    assert_eq!(scene.overlay_close_handlers[0].rect, solved.rect);
}

#[test]
fn finalize_orders_overlays_by_layer_z_order() {
    let mut scene = ComputedScene::<()>::default();
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(40.0), dp(40.0));

    let make = |id, layer, color| {
        let overlay = Overlay::<()>::new(OverlayId::new(id), Anchor::Rect(anchor))
            .placement(Placement::bottom())
            .layer(layer)
            .close_on_outside_click(true);
        let prims = vec![shape(
            Rect::new(dp(0.0), dp(0.0), dp(10.0), dp(10.0)),
            color,
        )];
        (overlay, prims)
    };

    let red = Color::rgba(255, 0, 0, 255);
    let green = Color::rgba(0, 255, 0, 255);
    let blue = Color::rgba(0, 0, 255, 255);
    let yellow = Color::rgba(255, 255, 0, 255);

    for (overlay, prims) in [
        make(1, OverlayLayer::Modal, red),
        make(2, OverlayLayer::Tooltip, green),
        make(3, OverlayLayer::Menu, blue),
        make(4, OverlayLayer::Popover, yellow),
    ] {
        let _ = emit_overlay(
            &mut scene,
            viewport(),
            overlay,
            (dp(10.0), dp(10.0)),
            OverlayContent::Primitives(prims),
        );
    }
    scene.finalize_portals(viewport());

    let colors: Vec<_> = scene
        .scene
        .overlay_shapes
        .iter()
        .map(|shape| shape.color)
        .collect();
    assert_eq!(colors, vec![green, yellow, blue, red]);

    let layers: Vec<_> = scene
        .overlay_close_handlers
        .iter()
        .map(|handle| handle.layer)
        .collect();
    assert_eq!(
        layers,
        vec![
            OverlayLayer::Tooltip,
            OverlayLayer::Popover,
            OverlayLayer::Menu,
            OverlayLayer::Modal,
        ]
    );
}

#[test]
fn finalize_keeps_within_layer_emit_order() {
    let mut scene = ComputedScene::<()>::default();
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(40.0), dp(40.0));

    let red = Color::rgba(255, 0, 0, 255);
    let blue = Color::rgba(0, 0, 255, 255);

    for (id, color) in [(1u64, red), (2, blue)] {
        let overlay = Overlay::<()>::new(OverlayId::new(id), Anchor::Rect(anchor))
            .placement(Placement::bottom())
            .layer(OverlayLayer::Menu);
        let _ = emit_overlay(
            &mut scene,
            viewport(),
            overlay,
            (dp(10.0), dp(10.0)),
            OverlayContent::Primitives(vec![shape(
                Rect::new(dp(0.0), dp(0.0), dp(10.0), dp(10.0)),
                color,
            )]),
        );
    }
    scene.finalize_portals(viewport());

    let colors: Vec<_> = scene
        .scene
        .overlay_shapes
        .iter()
        .map(|shape| shape.color)
        .collect();
    assert_eq!(colors, vec![red, blue]);
}

#[test]
fn emit_overlay_registers_focus_scope_and_rebases_hit_scope_path() {
    let mut scene = ComputedScene::<()>::default();
    let anchor = Rect::new(dp(100.0), dp(100.0), dp(40.0), dp(40.0));
    let scope_id = WidgetId::from_raw(7);
    let focus_scope = FocusScopeState {
        scope_id,
        path: vec![scope_id],
        options: FocusScopeOptions::new().trap(true),
        active: true,
    };
    let focus_target_id = WidgetId::from_raw(8);
    let overlay = Overlay::<()>::new(OverlayId::new(scope_id.raw()), Anchor::Rect(anchor))
        .focus_scope(focus_scope.clone());
    let hits = vec![HitRegion {
        rect: Rect::new(dp(0.0), dp(0.0), dp(40.0), dp(20.0)),
        clip_rect: None,
        geometry: HitGeometry::Rect,
        scope_path: vec![WidgetId::from_raw(999)],
        focus: Some(FocusTargetMeta {
            widget_id: focus_target_id,
            tab_index: Some(0),
            order: 0,
            scope_path: vec![WidgetId::from_raw(999)],
            on_focus: None,
            on_blur: None,
        }),
        interaction: HitInteraction::Disabled {
            id: focus_target_id,
        },
        gpu_scroll_container: None,
    }];

    let _ = emit_overlay(
        &mut scene,
        viewport(),
        overlay,
        (dp(40.0), dp(20.0)),
        OverlayContent::Hits(hits),
    );
    scene.finalize_portals(viewport());

    assert_eq!(scene.focus_scopes.to_vec(), vec![focus_scope.clone()]);
    assert_eq!(scene.overlay_hit_regions.len(), 1);
    assert_eq!(scene.overlay_hit_regions[0].scope_path, focus_scope.path);
    assert_eq!(
        scene.overlay_hit_regions[0]
            .focus
            .as_ref()
            .expect("overlay focus metadata should be preserved")
            .scope_path,
        focus_scope.path
    );
}
