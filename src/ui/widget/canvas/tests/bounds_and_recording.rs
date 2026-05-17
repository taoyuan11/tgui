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
