use super::*;

#[test]
fn hover_path_reuses_cached_computed_scene() {
    let invalidation = InvalidationSignal::new();
    let resolve_count = Arc::new(AtomicUsize::new(0));
    let child = {
        let resolve_count = resolve_count.clone();
        Signal::new(
            move || {
                resolve_count.fetch_add(1, Ordering::SeqCst);
                Text::new("hover").cursor(CursorStyle::Pointer)
            },
            invalidation.clone(),
        )
    };
    let tree = WidgetTree::new_legacy(Flex::new(Axis::Vertical).dynamic_child(child));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));

    let viewport = handler.viewport_rect();
    assert_eq!(handler.hover_path(viewport).len(), 1);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);

    assert_eq!(handler.hover_path(viewport).len(), 1);
    assert_eq!(resolve_count.load(Ordering::SeqCst), 1);
}

#[test]
fn clearing_pointer_position_preserves_cached_layout_for_hover_recompute() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Text::new("hover").cursor(CursorStyle::Pointer));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));

    let viewport = handler.viewport_rect();
    assert!(handler.handle_hover(viewport));
    let hover_epoch = handler.hover_epoch;
    let _ = handler.computed_scene();
    assert!(handler.cached_scene.is_some());

    handler.clear_pointer_position();

    assert!(handler.hovered_widgets.is_empty());
    assert_eq!(handler.hover_epoch, hover_epoch.wrapping_add(1));
    assert!(handler.cached_scene.is_some());
}

#[test]
fn scrollbar_hover_preserves_cached_layout_for_hover_recompute() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new("line 0\nline 1\nline 2\nline 3\nline 4\nline 5").height(dp(52.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.vertical_thumb.is_some())
        .copied()
        .expect("textarea scroll region with a vertical thumb should exist");
    let thumb = region
        .vertical_thumb
        .expect("vertical scrollbar thumb should exist");

    assert!(handler.cached_scene.is_some());

    handler.cursor_position = Some(Point {
        x: thumb.x + Dp::new(thumb.width.get() * 0.5),
        y: thumb.y + Dp::new(thumb.height.get() * 0.5),
    });

    assert!(handler.sync_scrollbar_hover());
    assert_eq!(
        handler.hovered_scrollbar.map(|handle| handle.id),
        Some(region.id)
    );
    assert!(handler.cached_scene.is_some());
}

#[test]
fn scene_cache_invalidates_when_units_change() {
    let invalidation = InvalidationSignal::new();
    let handler = test_handler(None, invalidation);
    let viewport = handler.viewport_rect();
    let cached = cached_scene_shell(&handler, viewport, UnitContext::new(1.0, 1.0));

    assert!(!handler.scene_cache_matches(
        &cached,
        viewport,
        UnitContext::new(1.0, 1.25),
        false,
        None,
    ));
}

#[test]
fn scene_layout_cache_survives_visual_only_animation_epoch_change() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(None, invalidation);
    let viewport = handler.viewport_rect();
    let cached = cached_scene_shell(&handler, viewport, UnitContext::new(1.0, 1.0));

    handler.animation_epoch = 1;

    assert!(handler.scene_layout_cache_matches(&cached, viewport, UnitContext::new(1.0, 1.0),));
}

#[test]
fn theme_animation_invalidates_cached_layout() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(None, invalidation);
    let viewport = handler.viewport_rect();
    let cached = cached_scene_shell(&handler, viewport, UnitContext::new(1.0, 1.0));

    handler.layout_animation_epoch = 1;

    assert!(!handler.scene_layout_cache_matches(&cached, viewport, UnitContext::new(1.0, 1.0),));
}

#[test]
fn theme_mode_change_invalidates_cached_layout_when_theme_changes() {
    let invalidation = InvalidationSignal::new();
    let mode = Signal::new(|| ThemeMode::Light, invalidation.clone());
    let (theme_set, _light, _dark) = custom_theme_set();
    let mut handler = test_handler_with_config(
        TestVm,
        None,
        invalidation.clone(),
        test_config_with_theme(ThemeSelection::System, theme_set),
    );
    handler.window_bindings.theme_mode = Some(mode);
    handler.sync_theme_binding();

    let viewport = handler.viewport_rect();
    let cached = cached_scene_shell(&handler, viewport, UnitContext::new(1.0, 1.0));

    handler.window_bindings.theme_mode = Some(Signal::new(|| ThemeMode::Dark, invalidation));
    handler.sync_theme_binding();

    assert!(!handler.scene_layout_cache_matches(&cached, viewport, UnitContext::new(1.0, 1.0),));
}

#[test]
fn animation_scene_invalidation_preserves_cached_layout() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Textarea::<TestVm>::new("line 0\nline 1\nline 2\nline 3\nline 4\nline 5").height(dp(52.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let units = handler.unit_context();

    let _ = handler.computed_scene();
    assert!(handler.cached_scene.is_some());
    assert!(!handler.text_input_regions.is_empty());

    handler.animation_epoch = handler.animation_epoch.wrapping_add(1);
    handler.invalidate_computed_scene();

    assert!(handler.cached_scene.is_some());
    assert!(handler.text_input_regions.is_empty());
    assert!(handler.scene_layout_cache_matches(
        handler
            .cached_scene
            .as_ref()
            .expect("cached scene should remain available"),
        viewport,
        units,
    ));
}

#[test]
fn visual_only_animation_refresh_prefers_scene_patch() {
    let invalidation = InvalidationSignal::new();
    let ctx = ViewModelContext::for_benchmarks();
    let opacity = ctx.state(0.0_f32);
    let tree = WidgetTree::new(
        Stack::<TestVm>::new()
            .opacity(opacity.project(|value| 0.3 + value * 0.7))
            .child(Text::new("animated visual only")),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    let before_root = handler
        .cached_scene
        .as_ref()
        .and_then(|cached| cached.layout.as_ref())
        .map(|layout| layout.root_id())
        .expect("cached layout should exist");

    opacity.set(1.0);
    handler.animation_epoch = handler.animation_epoch.wrapping_add(1);
    assert!(handler.patch_animation_scene_widgets(&[before_root.raw()], Instant::now()));
    assert!(handler.cached_scene.is_some());
    assert!(
        handler
            .cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .map(|layout| layout.root_id())
            == Some(before_root)
    );
}

#[derive(Debug, PartialEq)]
enum ButtonCommandFingerprint {
    Backdrop(crate::ui::widget::BackdropBlurPrimitive),
    Brush(crate::ui::widget::BrushPrimitive),
    Shape(
        Rect,
        Color,
        f32,
        f32,
        Option<Rect>,
        Option<crate::ui::widget::ClipMask>,
    ),
    Text(crate::ui::widget::TextPrimitive),
    TextDecoration(crate::ui::widget::TextDecorationPrimitive),
}

fn button_command_fingerprints(
    commands: &[crate::ui::widget::RenderCommand],
) -> Vec<ButtonCommandFingerprint> {
    commands
        .iter()
        .map(|command| match command {
            crate::ui::widget::RenderCommand::BackdropBlur(primitive) => {
                ButtonCommandFingerprint::Backdrop(*primitive)
            }
            crate::ui::widget::RenderCommand::Brush(primitive) => {
                ButtonCommandFingerprint::Brush(primitive.clone())
            }
            crate::ui::widget::RenderCommand::Shape(primitive) => ButtonCommandFingerprint::Shape(
                primitive.rect,
                primitive.color,
                primitive.corner_radius,
                primitive.stroke_width,
                primitive.clip_rect,
                primitive.clip_mask,
            ),
            crate::ui::widget::RenderCommand::Text(primitive) => {
                ButtonCommandFingerprint::Text((**primitive).clone())
            }
            crate::ui::widget::RenderCommand::TextDecoration(primitive) => {
                ButtonCommandFingerprint::TextDecoration(primitive.clone())
            }
            crate::ui::widget::RenderCommand::CanvasComposite(_)
            | crate::ui::widget::RenderCommand::Texture(_)
            | crate::ui::widget::RenderCommand::Mesh(_) => {
                panic!("simple Button hover scene emitted an unsupported command")
            }
            #[cfg(feature = "video")]
            crate::ui::widget::RenderCommand::VideoTexture(_) => {
                panic!("simple Button hover scene emitted a video command")
            }
        })
        .collect()
}

#[derive(Debug, PartialEq)]
enum HitGeometryFingerprint {
    Rect,
    Quad([Point; 4]),
    Triangles(Vec<[Point; 3]>),
}

fn hit_geometry_fingerprint(geometry: &crate::ui::widget::HitGeometry) -> HitGeometryFingerprint {
    match geometry {
        crate::ui::widget::HitGeometry::Rect => HitGeometryFingerprint::Rect,
        crate::ui::widget::HitGeometry::Quad(quad) => HitGeometryFingerprint::Quad(*quad),
        crate::ui::widget::HitGeometry::Triangles(triangles) => {
            HitGeometryFingerprint::Triangles(triangles.iter().copied().collect())
        }
    }
}

fn hit_region_fingerprints<VM>(
    regions: &[crate::ui::widget::HitRegion<VM>],
) -> Vec<(
    Rect,
    Option<Rect>,
    HitGeometryFingerprint,
    Vec<WidgetId>,
    Vec<WidgetId>,
    Option<(WidgetId, Option<i32>, usize, Vec<WidgetId>, bool, bool)>,
    crate::ui::widget::HitTargetId,
    Option<WidgetId>,
)> {
    regions
        .iter()
        .map(|region| {
            (
                region.rect,
                region.clip_rect,
                hit_geometry_fingerprint(&region.geometry),
                region.transform_chain.iter().copied().collect(),
                region.scope_path.clone(),
                region.focus.as_ref().map(|focus| {
                    (
                        focus.widget_id,
                        focus.tab_index,
                        focus.order,
                        focus.scope_path.clone(),
                        focus.on_focus.is_some(),
                        focus.on_blur.is_some(),
                    )
                }),
                region.interaction.target_id(),
                region.gpu_scroll_container,
            )
        })
        .collect()
}

fn scroll_region_fingerprints(
    regions: &[crate::ui::widget::ScrollRegion],
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

fn assert_button_hover_accessibility_equivalent(
    actual: &crate::ui::widget::ComputedScene<TestVm>,
    expected: &crate::ui::widget::ComputedScene<TestVm>,
) {
    assert_eq!(
        actual.accessibility_fragments.len(),
        expected.accessibility_fragments.len()
    );
    for (actual, expected) in actual
        .accessibility_fragments
        .iter()
        .zip(expected.accessibility_fragments.iter())
    {
        assert_eq!(
            actual.source_window_instance_id,
            expected.source_window_instance_id
        );
        assert_eq!(
            actual.source_publication_generation,
            expected.source_publication_generation
        );
        assert_eq!(
            actual
                .source_open
                .as_ref()
                .map(|open| open.resolve_untracked()),
            expected
                .source_open
                .as_ref()
                .map(|open| open.resolve_untracked())
        );
        assert_eq!(actual.owner_path, expected.owner_path);
        assert_eq!(actual.scope_path, expected.scope_path);
        assert_eq!(actual.clip_rect, expected.clip_rect);
        assert_eq!(
            actual.has_duplicate_widget_ids,
            expected.has_duplicate_widget_ids
        );
        assert_eq!(actual.resolved_root.id, expected.resolved_root.id);
        assert_eq!(
            std::mem::discriminant(&actual.resolved_root.kind),
            std::mem::discriminant(&expected.resolved_root.kind)
        );
        assert_eq!(actual.nodes.len(), expected.nodes.len());
        for (actual, expected) in actual.nodes.iter().zip(expected.nodes.iter()) {
            assert_eq!(actual.widget_id, expected.widget_id);
            assert_eq!(actual.resolved_path, expected.resolved_path);
            assert_eq!(actual.bounds, expected.bounds);
            assert_eq!(actual.clip_rect, expected.clip_rect);
            assert_eq!(actual.children, expected.children);
            assert_eq!(
                hit_region_fingerprints(&actual.hits),
                hit_region_fingerprints(&expected.hits)
            );
            assert_eq!(
                scroll_region_fingerprints(&actual.scroll_regions),
                scroll_region_fingerprints(&expected.scroll_regions)
            );
        }
    }
}

fn assert_button_hover_scene_equivalent(
    actual: &crate::ui::widget::ComputedScene<TestVm>,
    expected: &crate::ui::widget::ComputedScene<TestVm>,
) {
    super::super::table_tests::assert_data_grid_scene_equivalent(actual, expected);
    assert!(actual.is_simple_for_button_hover_recompose());
    assert!(expected.is_simple_for_button_hover_recompose());
    assert_eq!(
        button_command_fingerprints(&actual.scene.commands),
        button_command_fingerprints(&expected.scene.commands)
    );
    assert_eq!(
        actual.scene.overlay_command_sources,
        expected.scene.overlay_command_sources
    );
    assert_eq!(
        actual.scene.command_gpu_scroll_containers,
        expected.scene.command_gpu_scroll_containers
    );
    assert_eq!(
        actual.scene.overlay_command_gpu_scroll_containers,
        expected.scene.overlay_command_gpu_scroll_containers
    );
    assert_eq!(
        actual.scene.command_transform_chains,
        expected.scene.command_transform_chains
    );
    assert_eq!(
        actual.scene.overlay_command_transform_chains,
        expected.scene.overlay_command_transform_chains
    );
    assert_eq!(actual.overlay_layer_graph, expected.overlay_layer_graph);
    assert_eq!(
        actual.dependencies.all_owners(),
        expected.dependencies.all_owners()
    );
    assert_button_hover_accessibility_equivalent(actual, expected);
}

fn button_hover_fixture(second_tooltip: bool) -> (WidgetTree<TestVm>, WidgetId, WidgetId) {
    let first: Element<TestVm> = Button::new("First").size(dp(84.0), dp(40.0)).into();
    let first_id = first.id;
    let second: Element<TestVm> = if second_tooltip {
        Button::new("Second")
            .size(dp(84.0), dp(40.0))
            .tooltip(Tooltip::new("hint").delay(Duration::ZERO))
            .into()
    } else {
        Button::new("Second").size(dp(84.0), dp(40.0)).into()
    };
    let second_id = second.id;
    (
        WidgetTree::new(
            Flex::new(Axis::Horizontal)
                .size(dp(200.0), dp(56.0))
                .gap(dp(12.0))
                .child(first)
                .child(second),
        ),
        first_id,
        second_id,
    )
}

fn button_hit_center(handler: &mut BoundRuntimeHandler<TestVm>, id: WidgetId) -> Point {
    handler
        .computed_scene()
        .hit_regions
        .iter()
        .find_map(|region| {
            (region.interaction.target_id() == crate::ui::widget::HitTargetId::Widget(id))
                .then_some(Point::new(
                    region.rect.x + region.rect.width * 0.5,
                    region.rect.y + region.rect.height * 0.5,
                ))
        })
        .expect("Button should expose a visible hit region")
}

fn move_button_pointer(handler: &mut BoundRuntimeHandler<TestVm>, point: Point) {
    let _ = handler.handle_bound_window_event(
        &TestEventLoop,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(f64::from(point.x.get()), f64::from(point.y.get())),
            primary: true,
            source: PointerSource::Mouse,
        },
    );
}

fn send_button_pointer_event(
    handler: &mut BoundRuntimeHandler<TestVm>,
    point: Point,
    state: ElementState,
) {
    let _ = handler.handle_bound_window_event(
        &TestEventLoop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(f64::from(point.x.get()), f64::from(point.y.get())),
            state,
            button: ButtonSource::Mouse(crate::platform::event::MouseButton::Left),
            primary: true,
        },
    );
}

fn prime_button_pointer_focus(
    handler: &mut BoundRuntimeHandler<TestVm>,
    button: WidgetId,
    point: Point,
) {
    move_button_pointer(handler, point);
    let _ = handler.computed_scene();

    send_button_pointer_event(handler, point, ElementState::Pressed);
    handler.button_pressed_patch_pending = None;
    let _ = handler.computed_scene();

    send_button_pointer_event(handler, point, ElementState::Released);
    handler.button_pressed_patch_pending = None;
    let _ = handler.computed_scene();

    assert_eq!(handler.focused_widget_id(), Some(button));
    assert!(!handler.focus_visible);
    assert_eq!(handler.pressed_widget, None);
    assert_eq!(
        handler
            .cached_scene
            .as_ref()
            .expect("primed Button cache should exist")
            .pressed_widget,
        None
    );
}

#[test]
fn simple_button_hover_single_and_two_root_patches_match_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let (tree, first_id, second_id) = button_hover_fixture(false);
    let mut handler = test_handler(Some(tree), invalidation);
    handler.reduced_motion = true;
    let first = button_hit_center(&mut handler, first_id);
    let second = button_hit_center(&mut handler, second_id);
    handler.request_redraw_if_dirty(Instant::now());

    crate::runtime::scene_runtime::button_hover_patch_probe::reset();
    move_button_pointer(&mut handler, first);
    assert!(handler.button_hover_patch_pending.is_some());
    let _ = handler.computed_scene();
    assert_eq!(
        crate::runtime::scene_runtime::button_hover_patch_probe::hits(),
        1,
        "none→Button should patch the single simple Button root"
    );

    crate::runtime::scene_runtime::button_hover_patch_probe::reset();
    move_button_pointer(&mut handler, second);
    assert!(handler.button_hover_patch_pending.is_some());
    let retained = handler.computed_scene().clone();
    assert_eq!(
        crate::runtime::scene_runtime::button_hover_patch_probe::hits(),
        1,
        "Button A→B should patch both changed Button roots"
    );
    let cached_hover_epoch = handler
        .cached_scene
        .as_ref()
        .expect("retained Button hover cache")
        .hover_epoch;
    assert_eq!(cached_hover_epoch, handler.hover_epoch);

    handler.invalidate_computed_scene();
    let full = handler.computed_scene().clone();
    assert_button_hover_scene_equivalent(&retained, &full);
}

#[test]
fn button_hover_patch_falls_back_for_invalidation_and_tooltip_overlay() {
    let invalidation = InvalidationSignal::new();
    let (tree, first_id, second_id) = button_hover_fixture(false);
    let mut handler = test_handler(Some(tree), invalidation.clone());
    handler.reduced_motion = true;
    let first = button_hit_center(&mut handler, first_id);
    let second = button_hit_center(&mut handler, second_id);
    handler.request_redraw_if_dirty(Instant::now());
    move_button_pointer(&mut handler, first);
    let _ = handler.computed_scene();
    move_button_pointer(&mut handler, second);
    assert!(handler.button_hover_patch_pending.is_some());
    invalidation.mark_dirty();
    crate::runtime::scene_runtime::button_hover_patch_probe::reset();
    let fallback = handler.computed_scene().clone();
    assert_eq!(
        crate::runtime::scene_runtime::button_hover_patch_probe::hits(),
        0
    );
    handler.invalidate_computed_scene();
    let full = handler.computed_scene().clone();
    assert_button_hover_scene_equivalent(&fallback, &full);

    let invalidation = InvalidationSignal::new();
    let (tree, first_id, second_id) = button_hover_fixture(true);
    let mut handler = test_handler(Some(tree), invalidation);
    handler.reduced_motion = true;
    let first = button_hit_center(&mut handler, first_id);
    let second = button_hit_center(&mut handler, second_id);
    handler.request_redraw_if_dirty(Instant::now());
    move_button_pointer(&mut handler, first);
    let _ = handler.computed_scene();
    move_button_pointer(&mut handler, second);
    assert!(
        handler.button_hover_patch_pending.is_none(),
        "tooltip-bearing Button must reject the retained hover candidate"
    );
    crate::runtime::scene_runtime::button_hover_patch_probe::reset();
    let _ = handler.computed_scene();
    assert_eq!(
        crate::runtime::scene_runtime::button_hover_patch_probe::hits(),
        0
    );
}

#[test]
fn first_button_press_falls_back_when_pointer_focus_changes() {
    let invalidation = InvalidationSignal::new();
    let (tree, first_id, _) = button_hover_fixture(false);
    let mut handler = test_handler(Some(tree), invalidation);
    handler.reduced_motion = true;
    let first = button_hit_center(&mut handler, first_id);
    handler.request_redraw_if_dirty(Instant::now());
    move_button_pointer(&mut handler, first);
    let _ = handler.computed_scene();
    assert_eq!(handler.focused_widget_id(), None);

    crate::runtime::scene_runtime::button_pressed_patch_probe::reset();
    send_button_pointer_event(&mut handler, first, ElementState::Pressed);

    assert_eq!(handler.focused_widget_id(), Some(first_id));
    assert!(!handler.focus_visible);
    assert_eq!(handler.pressed_widget, Some(first_id));
    let fallback = handler.computed_scene().clone();
    assert_eq!(
        crate::runtime::scene_runtime::button_pressed_patch_probe::hits(),
        0
    );
    assert_eq!(
        handler
            .cached_scene
            .as_ref()
            .expect("fallback Button cache should exist")
            .pressed_widget,
        Some(first_id)
    );

    handler.invalidate_computed_scene();
    let full = handler.computed_scene().clone();
    assert_button_hover_scene_equivalent(&fallback, &full);
}

#[test]
fn prefocused_button_pressed_and_released_patches_match_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let (tree, first_id, _) = button_hover_fixture(false);
    let mut handler = test_handler(Some(tree), invalidation);
    handler.reduced_motion = true;
    let first = button_hit_center(&mut handler, first_id);
    handler.request_redraw_if_dirty(Instant::now());
    prime_button_pointer_focus(&mut handler, first_id, first);
    let hover_scene = handler.computed_scene().clone();

    crate::runtime::scene_runtime::button_pressed_patch_probe::reset();
    send_button_pointer_event(&mut handler, first, ElementState::Pressed);
    let down_pending = handler
        .button_pressed_patch_pending
        .expect("pre-focused Button press should produce a retained candidate");
    assert_eq!(down_pending.button, first_id);
    assert_eq!(down_pending.source_pressed_widget, None);
    assert_eq!(down_pending.next_pressed_widget, Some(first_id));
    let retained_down = handler.computed_scene().clone();
    assert_eq!(
        crate::runtime::scene_runtime::button_pressed_patch_probe::hits(),
        1,
        "pre-focused Button press should patch the single Button root"
    );
    assert_eq!(
        handler
            .cached_scene
            .as_ref()
            .expect("pressed Button cache should exist")
            .pressed_widget,
        Some(first_id)
    );
    assert_ne!(
        button_command_fingerprints(&hover_scene.scene.commands),
        button_command_fingerprints(&retained_down.scene.commands),
        "pressed Button visuals should differ from the focused-hover state"
    );
    handler.invalidate_computed_scene();
    let full_down = handler.computed_scene().clone();
    assert_button_hover_scene_equivalent(&retained_down, &full_down);

    crate::runtime::scene_runtime::button_pressed_patch_probe::reset();
    send_button_pointer_event(&mut handler, first, ElementState::Released);
    assert!(
        handler.button_pressed_patch_pending.is_none(),
        "release hover recomputation should consume the retained candidate immediately"
    );
    assert_eq!(
        crate::runtime::scene_runtime::button_pressed_patch_probe::hits(),
        1,
        "same-position Button release should patch the single Button root"
    );
    let retained_up = handler.computed_scene().clone();
    assert_eq!(
        handler
            .cached_scene
            .as_ref()
            .expect("released Button cache should exist")
            .pressed_widget,
        None
    );
    assert_eq!(
        button_command_fingerprints(&retained_up.scene.commands),
        button_command_fingerprints(&hover_scene.scene.commands),
        "released Button visuals should return to the focused-hover state"
    );
    handler.invalidate_computed_scene();
    let full_up = handler.computed_scene().clone();
    assert_button_hover_scene_equivalent(&retained_up, &full_up);
}

#[test]
fn button_release_after_pointer_exit_falls_back() {
    let invalidation = InvalidationSignal::new();
    let (tree, first_id, _) = button_hover_fixture(false);
    let mut handler = test_handler(Some(tree), invalidation);
    handler.reduced_motion = true;
    let first = button_hit_center(&mut handler, first_id);
    handler.request_redraw_if_dirty(Instant::now());
    prime_button_pointer_focus(&mut handler, first_id, first);

    send_button_pointer_event(&mut handler, first, ElementState::Pressed);
    let _ = handler.computed_scene();
    assert_eq!(handler.pressed_widget, Some(first_id));

    let outside = Point::new(dp(190.0), dp(100.0));
    crate::runtime::scene_runtime::button_pressed_patch_probe::reset();
    send_button_pointer_event(&mut handler, outside, ElementState::Released);
    assert!(handler.button_pressed_patch_pending.is_none());
    assert!(handler.hovered_widgets.is_empty());
    let fallback = handler.computed_scene().clone();
    assert_eq!(
        crate::runtime::scene_runtime::button_pressed_patch_probe::hits(),
        0
    );
    assert_eq!(handler.pressed_widget, None);
    assert_eq!(
        handler
            .cached_scene
            .as_ref()
            .expect("pointer-exit fallback cache should exist")
            .pressed_widget,
        None
    );

    handler.invalidate_computed_scene();
    let full = handler.computed_scene().clone();
    assert_button_hover_scene_equivalent(&fallback, &full);
}

#[test]
fn button_pressed_patch_falls_back_for_pending_tooltip_wakeup() {
    let invalidation = InvalidationSignal::new();
    let (tree, button_id, _) = button_hover_fixture(false);
    let mut handler = test_handler(Some(tree), invalidation);
    handler.reduced_motion = true;
    let point = button_hit_center(&mut handler, button_id);
    handler.request_redraw_if_dirty(Instant::now());
    prime_button_pointer_focus(&mut handler, button_id, point);
    handler.next_tooltip_wakeup_deadline = Some(Instant::now() + Duration::from_secs(60));

    crate::runtime::scene_runtime::button_pressed_patch_probe::reset();
    send_button_pointer_event(&mut handler, point, ElementState::Pressed);
    let down_fallback = handler.computed_scene().clone();
    assert_eq!(
        crate::runtime::scene_runtime::button_pressed_patch_probe::hits(),
        0,
        "pending Tooltip wakeup must reject the pressed patch"
    );
    assert_eq!(handler.pressed_widget, Some(button_id));
    handler.invalidate_computed_scene();
    let down_full = handler.computed_scene().clone();
    assert_button_hover_scene_equivalent(&down_fallback, &down_full);

    handler.next_tooltip_wakeup_deadline = Some(Instant::now() + Duration::from_secs(60));
    crate::runtime::scene_runtime::button_pressed_patch_probe::reset();
    send_button_pointer_event(&mut handler, point, ElementState::Released);
    assert!(handler.button_pressed_patch_pending.is_none());
    assert_eq!(
        crate::runtime::scene_runtime::button_pressed_patch_probe::hits(),
        0,
        "pending Tooltip wakeup must reject the released patch"
    );
    let up_fallback = handler.computed_scene().clone();
    assert_eq!(handler.pressed_widget, None);
    handler.invalidate_computed_scene();
    let up_full = handler.computed_scene().clone();
    assert_button_hover_scene_equivalent(&up_fallback, &up_full);
}

#[test]
fn button_pressed_patch_falls_back_while_animation_engine_is_active() {
    let invalidation = InvalidationSignal::new();
    let (tree, button_id, _) = button_hover_fixture(false);
    let mut handler = test_handler(Some(tree), invalidation);
    handler.reduced_motion = true;
    let point = button_hit_center(&mut handler, button_id);
    handler.request_redraw_if_dirty(Instant::now());
    prime_button_pointer_focus(&mut handler, button_id, point);

    let animation_key = crate::animation::AnimationKey::Widget {
        id: WidgetId::next().raw(),
        property: crate::animation::WidgetProperty::Opacity,
    };
    let start = Instant::now();
    let transition = crate::animation::Transition::linear(Duration::from_secs(60));
    let _ = handler
        .animation_engine
        .resolve_f32(animation_key, 0.0, Some(transition), start);
    let _ = handler.animation_engine.resolve_f32(
        animation_key,
        1.0,
        Some(transition),
        start + Duration::from_millis(1),
    );
    assert!(handler.animation_engine.has_active_animations());

    crate::runtime::scene_runtime::button_pressed_patch_probe::reset();
    send_button_pointer_event(&mut handler, point, ElementState::Pressed);
    assert!(
        handler.button_pressed_patch_pending.is_none(),
        "active animation must reject the pressed candidate"
    );
    let fallback = handler.computed_scene().clone();
    assert_eq!(
        crate::runtime::scene_runtime::button_pressed_patch_probe::hits(),
        0
    );
    assert!(handler.animation_engine.has_active_animations());

    handler.invalidate_computed_scene();
    let full = handler.computed_scene().clone();
    assert_button_hover_scene_equivalent(&fallback, &full);
}

#[test]
fn button_pressed_patch_falls_back_for_revision_and_root_rebuild_changes() {
    let invalidation = InvalidationSignal::new();
    let (tree, first_id, _) = button_hover_fixture(false);
    let mut handler = test_handler(Some(tree), invalidation.clone());
    handler.reduced_motion = true;
    let first = button_hit_center(&mut handler, first_id);
    handler.request_redraw_if_dirty(Instant::now());
    prime_button_pointer_focus(&mut handler, first_id, first);

    send_button_pointer_event(&mut handler, first, ElementState::Pressed);
    assert!(handler.button_pressed_patch_pending.is_some());
    invalidation.mark_dirty();
    crate::runtime::scene_runtime::button_pressed_patch_probe::reset();
    let revision_fallback = handler.computed_scene().clone();
    assert_eq!(
        crate::runtime::scene_runtime::button_pressed_patch_probe::hits(),
        0
    );
    handler.invalidate_computed_scene();
    let revision_full = handler.computed_scene().clone();
    assert_button_hover_scene_equivalent(&revision_fallback, &revision_full);

    let invalidation = InvalidationSignal::new();
    let (tree, first_id, _) = button_hover_fixture(false);
    let mut handler = test_handler(Some(tree), invalidation.clone());
    handler.reduced_motion = true;
    let first = button_hit_center(&mut handler, first_id);
    handler.request_redraw_if_dirty(Instant::now());
    prime_button_pointer_focus(&mut handler, first_id, first);

    send_button_pointer_event(&mut handler, first, ElementState::Pressed);
    assert!(handler.button_pressed_patch_pending.is_some());
    invalidation.request_root_rebuild();
    crate::runtime::scene_runtime::button_pressed_patch_probe::reset();
    let root_rebuild_fallback = handler.computed_scene().clone();
    assert_eq!(
        crate::runtime::scene_runtime::button_pressed_patch_probe::hits(),
        0
    );
    handler.invalidate_computed_scene();
    let root_rebuild_full = handler.computed_scene().clone();
    assert_button_hover_scene_equivalent(&root_rebuild_fallback, &root_rebuild_full);
}
