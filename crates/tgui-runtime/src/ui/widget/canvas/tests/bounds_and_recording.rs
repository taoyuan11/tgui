use super::*;

#[test]
fn bounds_include_stroke_width() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas
            .next_item_id(7_u64)
            .set_stroke(CanvasStroke::new(dp(8.0), Color::WHITE))
            .begin_path()
            .move_to(10.0, 10.0)
            .line_to(30.0, 10.0)
            .line_to(30.0, 20.0)
            .close_path()
            .stroke();
    });

    let rendered = rendered_items(&scene);
    let bounds = rendered[0].hit_bounds.expect("bounds should exist");
    assert_eq!(bounds.min_x, 6.0);
    assert_eq!(bounds.max_x, 34.0);
}

#[test]
fn canvas_bounds_union_all_items() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas
            .next_item_id(1_u64)
            .begin_path()
            .move_to(0.0, 0.0)
            .line_to(20.0, 0.0)
            .line_to(20.0, 10.0)
            .close_path()
            .fill()
            .next_item_id(2_u64)
            .begin_path()
            .move_to(50.0, 25.0)
            .line_to(80.0, 25.0)
            .line_to(80.0, 40.0)
            .close_path()
            .fill();
    });

    let bounds = canvas_scene_bounds(&scene).expect("bounds should exist");
    assert_eq!(bounds.width(), 80.0);
    assert_eq!(bounds.height(), 40.0);
}

#[test]
fn canvas_bounds_include_shadow_expansion() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas
            .next_item_id(1_u64)
            .set_shadow(CanvasShadow::new(
                Color::BLACK,
                crate::ui::widget::Point::new(4.0, 6.0),
                dp(5.0),
            ))
            .fill_rect(0.0, 0.0, 20.0, 20.0);
    });

    let bounds = canvas_scene_bounds(&scene).expect("layout bounds should exist");
    assert!(bounds.max_x > 20.0);
    assert!(bounds.max_y > 20.0);
}

#[test]
fn gradients_with_many_stops_are_compressed_for_rendering() {
    let stops = (0..9)
        .map(|index| CanvasGradientStop::new(index as f32 / 8.0, Color::WHITE))
        .collect::<Vec<_>>();
    let gradient = CanvasBrush::LinearGradient(super::super::CanvasLinearGradient::new(
        crate::ui::widget::Point::new(0.0, 0.0),
        crate::ui::widget::Point::new(10.0, 0.0),
        stops,
    ));

    assert!(super::super::CanvasBrushData::from_brush(&gradient, 1.0).is_some());
}

#[test]
fn rounded_rect_fill_prefers_non_mesh_fast_path() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas
            .next_item_id(1_u64)
            .set_fill(Color::WHITE)
            .fill_round_rect(0.0, 0.0, 80.0, 40.0, 12.0);
    });
    let item = scene
        .items()
        .first()
        .expect("rounded rect item should exist");
    let super::super::CanvasItem::Path(path) = item else {
        panic!("rounded rect should record as a path");
    };
    let fill = path.fill.as_ref().map(Value::resolve);

    let output = tessellate_axis_aligned_rounded_rect(
        path,
        Point::ZERO,
        super::super::CanvasClipContext::default(),
        fill.as_ref(),
        None,
        1.0,
    )
    .expect("rounded rect should use fast path");

    assert!(output.meshes.is_empty());
    assert_eq!(output.commands.len(), 1);
    assert!(matches!(
        output.commands[0],
        crate::ui::widget::RenderCommand::Shape(_)
    ));
}

#[test]
fn canvas_recorder_auto_ids_are_stable() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas.fill_rect(0.0, 0.0, 20.0, 20.0);
        canvas.draw_text(Rect::new(0.0, 0.0, 40.0, 20.0), "hello");
        canvas.stroke_circle(20.0, 20.0, 8.0);
    });
    let rendered = rendered_items(&scene);

    assert_eq!(rendered[0].item_id, 1_u64.into());
    assert_eq!(rendered[1].item_id, 2_u64.into());
    assert_eq!(rendered[2].item_id, 3_u64.into());
}

#[test]
fn canvas_recorder_save_restore_restores_state() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas.set_opacity(0.25).translate(10.0, 5.0);
        canvas.save();
        canvas.set_opacity(0.9).translate(50.0, 0.0);
        canvas.fill_rect(0.0, 0.0, 10.0, 10.0);
        canvas.restore();
        canvas.fill_rect(0.0, 0.0, 10.0, 10.0);
    });
    let rendered = rendered_items(&scene);
    let first_bounds = rendered[0].hit_bounds.expect("first bounds");
    let second_bounds = rendered[1].hit_bounds.expect("second bounds");
    assert!(first_bounds.min_x > second_bounds.min_x);
}

#[test]
fn canvas_recorder_clip_scopes_items_inside_current_frame() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas.save();
        canvas.rect(0.0, 0.0, 40.0, 40.0).clip();
        canvas.fill_rect(10.0, 10.0, 20.0, 20.0);
        canvas.restore();
        canvas.fill_rect(50.0, 0.0, 20.0, 20.0);
    });
    let rendered = rendered_items(&scene);

    assert!(matches!(
        rendered[0].output.commands.first(),
        Some(RenderCommand::CanvasComposite(_))
    ));
    assert_eq!(rendered.len(), 2);
}

#[test]
fn canvas_recorder_shortcuts_match_manual_paths() {
    let shortcut = CanvasRecorder::build(|canvas| {
        canvas.fill_round_rect(0.0, 0.0, 80.0, 40.0, 12.0);
    });
    let manual = CanvasRecorder::build(|canvas| {
        canvas.begin_path();
        canvas.rounded_rect(0.0, 0.0, 80.0, 40.0, 12.0);
        canvas.fill();
    });

    assert_eq!(
        canvas_scene_bounds(&shortcut).expect("shortcut bounds"),
        canvas_scene_bounds(&manual).expect("manual bounds")
    );
}

#[test]
fn command_bounds_match_lyon_reference_for_curves_and_subpaths() {
    let paths = [
        PathBuilder::new()
            .move_to(-10.0, 2.0)
            .line_to(8.0, 15.0)
            .quad_to(30.0, -40.0, 50.0, 4.0)
            .cubic_to(75.0, 90.0, 110.0, -70.0, 140.0, 12.0)
            .close(),
        PathBuilder::new()
            .move_to(2.0, 3.0)
            .cubic_to(-100.0, 50.0, 80.0, 120.0, 20.0, -15.0)
            .move_to(200.0, 100.0)
            .quad_to(260.0, -30.0, 320.0, 40.0)
            .line_to(280.0, 180.0),
        PathBuilder::new().move_to(5.0, 7.0),
    ];

    for path in paths {
        assert_eq!(path.control_bounds(), path.control_bounds_via_lyon());
    }
}

#[test]
fn disabling_canvas_hit_metadata_preserves_render_geometry() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas.set_fill(Color::rgb(40, 120, 220)).draw_path(
            PathBuilder::new()
                .move_to(4.0, 8.0)
                .cubic_to(24.0, -6.0, 48.0, 42.0, 72.0, 12.0)
                .line_to(50.0, 60.0)
                .close(),
        );
    });
    let media = test_media();
    let with_hits = rendered_items_with_context(&scene, true, &media);
    let without_hits = rendered_items_with_context(&scene, false, &media);

    assert_eq!(with_hits.len(), without_hits.len());
    assert!(with_hits[0].hit_bounds.is_some());
    assert!(matches!(
        with_hits[0].hit_geometry,
        Some(super::super::CanvasHitGeometry::Triangles(_))
    ));
    assert!(without_hits[0].hit_bounds.is_none());
    assert!(without_hits[0].hit_geometry.is_none());
    assert!(without_hits[0].text_hits.is_empty());

    let with_mesh = &with_hits[0].output.meshes[0];
    let without_mesh = &without_hits[0].output.meshes[0];
    assert_eq!(&*with_mesh.vertices, &*without_mesh.vertices);
    assert_eq!(&*with_mesh.triangles, &*without_mesh.triangles);
    assert_eq!(with_mesh.clip_rect, without_mesh.clip_rect);
    assert_eq!(with_mesh.clip_mask, without_mesh.clip_mask);
}

#[test]
fn borrowed_canvas_scene_collection_reuses_shadow_cache() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas
            .set_fill(Color::WHITE)
            .set_shadow(CanvasShadow::new(
                Color::BLACK,
                Point::new(3.0, 5.0),
                dp(4.0),
            ))
            .begin_path()
            .move_to(0.0, 0.0)
            .quad_to(24.0, -12.0, 48.0, 10.0)
            .line_to(36.0, 42.0)
            .close_path()
            .fill();
    });
    let media = test_media();
    let first = rendered_items_with_context(&scene, false, &media);
    let second = rendered_items_with_context(&scene, false, &media);

    let first_shadow = &first[0].output.textures[0].texture;
    let second_shadow = &second[0].output.textures[0].texture;
    assert_eq!(first_shadow.id(), second_shadow.id());
    assert_eq!(first_shadow.size(), second_shadow.size());
}

#[test]
fn canvas_shadow_opacity_reuses_canonical_texture_and_updates_primitive_alpha() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas
            .set_fill(Color::WHITE)
            .set_shadow(CanvasShadow::new(
                Color::hexa(0x112233B8),
                Point::new(3.0, 5.0),
                dp(6.0),
            ))
            .begin_path()
            .move_to(0.0, 0.0)
            .quad_to(24.0, -12.0, 48.0, 10.0)
            .line_to(36.0, 42.0)
            .close_path()
            .fill();
    });
    let media = test_media();
    let low = super::super::tessellate_canvas_scene_items(
        &scene,
        Point::ZERO,
        0.25,
        None,
        None,
        false,
        &FontManager::new(&FontCatalog::default()),
        &media,
        UnitContext::default(),
    );
    let high = super::super::tessellate_canvas_scene_items(
        &scene,
        Point::ZERO,
        0.85,
        None,
        None,
        false,
        &FontManager::new(&FontCatalog::default()),
        &media,
        UnitContext::default(),
    );

    let low_shadow = &low[0].output.textures[0];
    let high_shadow = &high[0].output.textures[0];
    assert_eq!(low_shadow.texture.id(), high_shadow.texture.id());
    assert_eq!(
        low_shadow.texture.revision(),
        high_shadow.texture.revision()
    );
    assert_eq!(low_shadow.texture.size(), high_shadow.texture.size());
    assert_eq!(low_shadow.texture.pixels(), high_shadow.texture.pixels());
    assert!((low_shadow.opacity - 0.25).abs() <= f32::EPSILON);
    assert!((high_shadow.opacity - 0.85).abs() <= f32::EPSILON);
}
