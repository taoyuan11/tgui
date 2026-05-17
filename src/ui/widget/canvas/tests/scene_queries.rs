use super::*;

#[test]
fn canvas_scene_can_query_named_and_nested_items() {
    let scene = CanvasScene::from_items(vec![
        super::super::CanvasPath::new(1_u64, PathBuilder::new().rect(0.0, 0.0, 10.0, 10.0))
            .name_item("background")
            .into(),
        super::super::CanvasGroup::new(
            2_u64,
            super::super::CanvasGroupMode::Clip,
            super::super::CanvasGroupShape::path(PathBuilder::new().rect(0.0, 0.0, 40.0, 40.0)),
            vec![
                super::super::CanvasText::new(3_u64, Rect::new(4.0, 4.0, 20.0, 10.0), "hello")
                    .name_item("label")
                    .into(),
            ],
        )
        .name_item("root-group")
        .into(),
    ]);

    assert!(scene.contains_id(1_u64.into()));
    assert!(scene.contains_name("label"));
    assert_eq!(
        scene
            .find_named("root-group")
            .map(super::super::CanvasItem::id),
        Some(2_u64.into())
    );
    assert_eq!(
        scene
            .find(3_u64.into())
            .and_then(super::super::CanvasItem::name),
        Some("label")
    );
}

#[test]
fn canvas_scene_visit_reports_depth_and_paths() {
    let scene = CanvasScene::from_items(vec![super::super::CanvasGroup::new(
        1_u64,
        super::super::CanvasGroupMode::Mask,
        super::super::CanvasGroupShape::path(PathBuilder::new().circle(20.0, 20.0, 20.0)),
        vec![
            super::super::CanvasPath::new(2_u64, PathBuilder::new().rect(0.0, 0.0, 10.0, 10.0))
                .name_item("rect")
                .into(),
        ],
    )
    .into()]);

    let mut visited = Vec::new();
    scene.visit(|entry| {
        visited.push((entry.item.id().get(), entry.depth, entry.index_path));
    });

    assert_eq!(visited.len(), 2);
    assert_eq!(visited[0], (1, 0, vec![0]));
    assert_eq!(visited[1], (2, 1, vec![0, 0]));
}

#[test]
fn canvas_scene_remove_handles_nested_items() {
    let mut scene = CanvasScene::from_items(vec![super::super::CanvasGroup::new(
        1_u64,
        super::super::CanvasGroupMode::Clip,
        super::super::CanvasGroupShape::path(PathBuilder::new().rect(0.0, 0.0, 20.0, 20.0)),
        vec![super::super::CanvasImage::new(
            2_u64,
            Rect::new(0.0, 0.0, 20.0, 20.0),
            MediaSource::bytes(vec![1, 2, 3]),
        )
        .name_item("thumb")
        .into()],
    )
    .into()]);

    let removed = scene
        .remove(2_u64.into())
        .expect("nested item should be removed");
    assert_eq!(removed.name(), Some("thumb"));
    assert!(!scene.contains_id(2_u64.into()));
}

#[test]
fn canvas_recorder_item_names_are_recorded() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas
            .next_item_name("hero-card")
            .fill_rect(0.0, 0.0, 40.0, 20.0)
            .next_item_name("title")
            .draw_text(Rect::new(0.0, 0.0, 30.0, 10.0), "Hi");
    });

    assert_eq!(
        scene
            .find_named("hero-card")
            .map(super::super::CanvasItem::id),
        Some(1_u64.into())
    );
    assert_eq!(
        scene.find_named("title").map(super::super::CanvasItem::id),
        Some(2_u64.into())
    );
}

#[test]
fn canvas_scene_debug_exports_include_stats_and_names() {
    let scene = CanvasRecorder::build(|canvas| {
        canvas
            .next_item_name("surface")
            .fill_round_rect(0.0, 0.0, 80.0, 40.0, 12.0)
            .next_item_name("caption")
            .draw_text(Rect::new(8.0, 8.0, 60.0, 18.0), "Canvas");
    });

    let debug = scene.debug_info();
    let text = scene.export_debug_text();
    let json = scene.export_debug_json();

    assert_eq!(debug.stats.total_items, 2);
    assert_eq!(debug.stats.named_items, 2);
    assert!(text.contains("surface"));
    assert!(text.contains("caption"));
    assert!(json.contains("\"stats\""));
    assert!(json.contains("\"name\": \"surface\""));
}

#[test]
fn canvas_scene_query_point_and_stable_export_work() {
    let scene = CanvasScene::from_items(vec![super::super::CanvasGroup::new(
        1_u64,
        super::super::CanvasGroupMode::Clip,
        super::super::CanvasGroupShape::path(PathBuilder::new().rect(0.0, 0.0, 100.0, 100.0)),
        vec![
            super::super::CanvasPath::new(2_u64, PathBuilder::new().rect(10.0, 10.0, 60.0, 40.0))
                .name_item("card")
                .fill(Color::WHITE)
                .into(),
        ],
    )
    .name_item("root")
    .into()]);

    let hit = scene
        .query_point(Point::new(20.0, 20.0))
        .expect("point should hit nested item");
    let all_hits = scene.query_point_all(Point::new(20.0, 20.0));
    let stable = scene.export_json();

    assert_eq!(hit.item_id, 2_u64.into());
    assert_eq!(hit.name.as_deref(), Some("card"));
    assert_eq!(all_hits[0].item_id, 2_u64.into());
    assert!(all_hits.iter().any(|entry| entry.item_id == 1_u64.into()));
    assert!(stable.contains("\"format\": \"tgui.canvas.scene\""));
    assert!(stable.contains("\"version\": 1"));
    assert!(stable.contains("\"kind\": \"group\""));
    assert!(stable.contains("\"name\": \"card\""));
}

#[test]
fn canvas_scene_query_point_returns_text_hit_for_text_items() {
    let scene = CanvasScene::from_items(vec![super::super::CanvasText::new(
        1_u64,
        Rect::new(0.0, 0.0, 120.0, 32.0),
        "Hello",
    )
    .name_item("label")
    .into()]);

    let hit = scene
        .query_point(Point::new(6.0, 10.0))
        .expect("point should hit text");

    assert_eq!(hit.item_id, 1_u64.into());
    assert!(hit.text_hit.is_some());
    let text_hit = hit.text_hit.expect("text hit should exist");
    assert!(text_hit.utf8_end > text_hit.utf8_start);
}

#[test]
fn stable_export_escapes_control_characters() {
    let scene = CanvasScene::from_items(vec![super::super::CanvasText::new(
        1_u64,
        Rect::new(0.0, 0.0, 120.0, 32.0),
        "line\u{0001}\u{0008}\u{000C}end",
    )
    .name_item("bad\u{0002}name")
    .into()]);

    let json = scene.export_json();

    assert!(json.contains("bad\\u0002name"));
    assert!(json.contains("line\\u0001\\b\\fend"));
}

#[test]
fn canvas_scene_query_options_drive_explicit_query_context() {
    let scene = CanvasScene::from_items(vec![super::super::CanvasText::new(
        1_u64,
        Rect::new(0.0, 0.0, 120.0, 32.0),
        "Hello",
    )
    .name_item("label")
    .into()]);
    let options = super::super::CanvasSceneQueryOptions::new()
        .scale_factor(1.5)
        .font_scale(1.25);

    let hit = scene
        .query_point_with(&options, Point::new(6.0, 10.0))
        .expect("point should hit text with explicit context");
    let all_hits = scene.query_point_all_with(&options, Point::new(6.0, 10.0));

    assert_eq!(hit.item_id, 1_u64.into());
    assert!(hit.text_hit.is_some());
    assert_eq!(all_hits.len(), 1);
}

#[test]
fn runtime_query_context_bridge_reuses_runtime_inputs() {
    let scene = CanvasScene::from_items(vec![super::super::CanvasText::new(
        1_u64,
        Rect::new(0.0, 0.0, 120.0, 32.0),
        "Hello",
    )
    .name_item("label")
    .into()]);
    let font_manager = FontManager::new(&FontCatalog::default());
    let units = UnitContext::new(1.5, 1.25);

    let hit = scene
        .query_point_with_runtime_context(&font_manager, units, Point::new(6.0, 10.0))
        .expect("point should hit text with runtime context");
    let all_hits =
        scene.query_point_all_with_runtime_context(&font_manager, units, Point::new(6.0, 10.0));

    assert_eq!(hit.item_id, 1_u64.into());
    assert!(hit.text_hit.is_some());
    assert_eq!(all_hits.len(), 1);
}
