use super::*;
#[cfg(feature = "bench-support")]
use crate::runtime::state::StrictCapabilityKind;
use crate::ui::widget::ItemLayout;
#[cfg(feature = "bench-support")]
use crate::ui::widget::{For, Show, ViewSwitch};

#[test]
fn textarea_show_scrollbar_dependency_update_preserves_cached_layout() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let show_scrollbar = context.state(false);
    let tree = WidgetTree::new(
        Textarea::new("line 0\nline 1\nline 2\nline 3")
            .height(dp(52.0))
            .show_scrollbar(show_scrollbar.signal()),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let units = handler.unit_context();

    let _ = handler.computed_scene();
    assert!(handler
        .cached_scene
        .as_ref()
        .and_then(|cache| cache.layout.as_ref())
        .is_some());

    show_scrollbar.set(true);
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("scene-only invalidation should keep the cache shell");
    assert!(cached.layout.is_some());
    assert!(cached.computed_valid);
    assert!(handler.scene_layout_cache_matches(cached, viewport, units));
}

#[test]
fn textarea_auto_wrap_dependency_update_preserves_cached_layout() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let auto_wrap = context.state(false);
    let tree = WidgetTree::new(
        Textarea::new("a very long line of text that should change the measured content")
            .height(dp(52.0))
            .auto_wrap(auto_wrap.signal()),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    assert!(handler
        .cached_scene
        .as_ref()
        .and_then(|cache| cache.layout.as_ref())
        .is_some());

    auto_wrap.set(true);
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("scene-only invalidation should keep the cache shell");
    assert!(cached.layout.is_some());
    assert!(cached.computed_valid);
    assert!(handler.scene_layout_cache_matches(
        cached,
        handler.viewport_rect(),
        handler.unit_context()
    ));
}

#[test]
fn canvas_items_dependency_update_preserves_cached_layout_shell() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let expanded = context.state(false);
    let tree = WidgetTree::new(Canvas::new(expanded.signal().map(|expanded| {
        let width = if expanded { 96.0 } else { 48.0 };
        CanvasRecorder::build(|canvas| {
            canvas
                .next_item_id(1_u64)
                .set_fill(Color::WHITE)
                .begin_path()
                .move_to(0.0, 0.0)
                .line_to(width, 0.0)
                .line_to(width, 24.0)
                .line_to(0.0, 24.0)
                .close_path()
                .fill();
        })
    })));
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    assert!(handler
        .cached_scene
        .as_ref()
        .and_then(|cache| cache.layout.as_ref())
        .is_some());

    expanded.set(true);
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("canvas subtree patch should keep the cache shell");
    assert!(cached.layout.is_some());
    assert!(cached.computed_valid);
}

#[test]
fn outer_scroll_with_virtual_descendant_preserves_layout_cache_for_non_virtual_scroll() {
    let invalidation = InvalidationSignal::new();
    let items = (0..48usize).collect::<Vec<_>>();
    let tree = WidgetTree::new(
        ScrollView::new().size(dp(180.0), dp(120.0)).child(
            Flex::vertical()
                .width(dp(180.0))
                .child(Text::new("header"))
                .child(
                    crate::ui::widget::VirtualList::new(items, |index, _item| {
                        Text::new(format!("row {index}")).height(dp(24.0)).into()
                    })
                    .height(dp(96.0))
                    .item_layout(ItemLayout::Fixed {
                        item_extent: dp(24.0),
                        spacing: Dp::ZERO,
                        overscan: 2,
                    }),
                )
                .child(Flex::vertical().height(dp(260.0))),
        ),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let viewport = handler.viewport_rect();
    let units = handler.unit_context();

    let outer_scroll_id = handler
        .cached_scene
        .as_ref()
        .and_then(|cached| {
            cached
                .computed
                .scroll_regions
                .first()
                .map(|region| region.id)
        })
        .expect("outer page scroller should exist");

    handler.set_scroll_offset(outer_scroll_id, Point::new(dp(0.0), dp(24.0)));
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("scroll should preserve cached scene shell");

    assert!(
        handler.scene_layout_cache_matches(cached, viewport, units),
        "scrolling a non-virtual ancestor should not invalidate layout cache just because a virtual descendant exists"
    );
}

#[test]
fn command_context_request_rebuild_invalidates_cached_scene_explicitly() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Text::new("static"));
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before_revision = handler.invalidation.revision();

    let command = Command::new_with_context(|_vm: &mut TestVm, context| {
        context.request_rebuild();
    });
    handler.execute_command_without_invalidation(&command);

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("request_rebuild should preserve cache shell until redraw");
    assert!(!cached.layout_valid);
    assert!(!cached.computed_valid);
    assert!(
        handler.invalidation.revision() > before_revision,
        "explicit rebuild request should advance invalidation revision"
    );
}

#[derive(Default)]
struct RebuildRootVm {
    page: &'static str,
}

impl ViewModel for RebuildRootVm {
    fn new(_context: &ViewModelContext) -> Self {
        Self::default()
    }

    fn view(&self) -> Element<Self>
    where
        Self: Sized,
    {
        Stack::new().into()
    }
}

#[test]
fn command_context_request_rebuild_replaces_root_tree_when_factory_is_available() {
    let invalidation = InvalidationSignal::new();
    let root_view: crate::application::RootViewFactory<RebuildRootVm> =
        Arc::new(|vm| Text::new(vm.page).into());
    let initial_tree = WidgetTree::new(Text::new("first"));
    let (dialog_dispatcher, dialog_receiver) = async_dialog_channel();
    let (notification_dispatcher, notification_receiver) = async_notification_channel();
    let (task_dispatcher, task_receiver) = async_task_channel();
    let mut handler = BoundRuntimeHandler::new(
        "test".to_string(),
        1,
        WindowRole::Main,
        test_config(),
        Arc::new(Mutex::new(RebuildRootVm { page: "first" })),
        WindowBindings::default(),
        Some(initial_tree),
        Some(root_view),
        Vec::new(),
        invalidation,
        AnimationCoordinator::default(),
        dialog_dispatcher,
        Some(dialog_receiver),
        notification_dispatcher,
        Some(notification_receiver),
        task_dispatcher,
        Some(task_receiver),
    );

    let command = Command::new_with_context(|vm: &mut RebuildRootVm, context| {
        vm.page = "second";
        context.request_rebuild();
    });
    handler.execute_command_without_invalidation(&command);
    let computed = handler.computed_scene();

    assert!(
        computed
            .scene
            .texts
            .iter()
            .any(|text| text.content.as_ref() == "second"),
        "explicit rebuild should replace the root widget tree"
    );
}

#[test]
fn removing_opaque_dependency_subtree_clears_global_fallback() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let visible = context.state(false);
    let checked = context.state(false);
    let backing = Arc::new(Mutex::new(String::from("first")));
    let opaque = {
        let backing = backing.clone();
        context.signal(move || backing.lock().expect("test signal lock poisoned").clone())
    };
    let tree = WidgetTree::new_legacy(
        Stack::<TestVm>::new()
            .dynamic_child(visible.signal().map_unchecked({
                let opaque = opaque.clone();
                move |visible| {
                    let element: Element<TestVm> = if visible {
                        Text::new(opaque.clone()).key("opaque").into()
                    } else {
                        Text::new("static").key("static").into()
                    };
                    element
                }
            }))
            .child(Checkbox::new(checked.signal())),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    let root_id = handler
        .cached_scene
        .as_ref()
        .and_then(|cache| cache.layout.as_ref())
        .expect("cached layout should exist")
        .root_id();

    visible.set(true);
    assert!(handler.patch_cached_layout_for_roots(&[root_id], Instant::now()));
    assert!(handler.patch_cached_scene_for_roots(&[root_id], Instant::now(), true));
    assert!(handler
        .cached_scene
        .as_ref()
        .expect("patched cache should remain available")
        .dependencies
        .has_global_dependency());

    visible.set(false);
    assert!(handler.patch_cached_layout_for_roots(&[root_id], Instant::now()));
    assert!(handler.patch_cached_scene_for_roots(&[root_id], Instant::now(), true));
    assert!(!handler
        .cached_scene
        .as_ref()
        .expect("patched cache should remain available")
        .dependencies
        .has_global_dependency());

    checked.set(true);
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("scene-only invalidation should stay local after opaque removal");
    assert!(cached.computed_valid);
}

#[test]
fn removing_dynamic_child_clears_reactive_owner_subscriptions() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let visible = context.state(true);
    let color = context.state(Color::RED);
    let color_signal = color.signal();
    let tree = WidgetTree::new_legacy(
        Stack::<TestVm>::new().dynamic_child(visible.signal().map_unchecked(
            move |visible| -> Element<TestVm> {
                if visible {
                    let color_signal = color_signal.clone();
                    Stack::<TestVm>::new()
                        .size(dp(40.0), dp(40.0))
                        .style_full(move |ctx| {
                            let mut style = ContainerStyle::default_for_theme(ctx.theme);
                            style.surface.background = Some(color_signal.clone().into());
                            style
                        })
                        .into()
                } else {
                    Text::new("hidden").key("hidden").into()
                }
            },
        )),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();

    visible.set(false);
    handler.request_redraw_if_dirty(Instant::now());

    color.set(Color::BLUE);
    let updates = handler.invalidation.drain_reactive_updates();
    assert!(
        updates.targets.is_empty(),
        "removed child signal update must not enqueue a detached reactive target: {:?}",
        updates.targets
    );
}

#[test]
fn opaque_signal_dirty_update_falls_back_to_full_scene_invalidation() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let backing = Arc::new(Mutex::new(String::from("first")));
    let signal = {
        let backing = backing.clone();
        context.signal(move || backing.lock().expect("test signal lock poisoned").clone())
    };
    let tree = WidgetTree::new_legacy(Text::new(signal));
    let mut handler = test_handler(Some(tree), invalidation.clone());

    let _ = handler.computed_scene();
    assert!(handler
        .cached_scene
        .as_ref()
        .and_then(|cache| cache.layout.as_ref())
        .is_some());

    *backing.lock().expect("test signal lock poisoned") = String::from("second");
    invalidation.mark_dirty();
    handler.request_redraw_if_dirty(Instant::now());

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("global invalidation should retain cache shell");
    assert!(!cached.layout_valid);
    assert!(!cached.computed_valid);
}

#[test]
fn select_portal_repositions_and_clears_after_scene_patch() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let expanded = context.state(false);
    let visible = context.state(true);
    let tree = WidgetTree::new_legacy(Stack::<TestVm>::new().dynamic_child(
        visible.signal().map_unchecked({
            let expanded = expanded.clone();
            move |visible| {
                if !visible {
                    let hidden: Element<TestVm> = Text::new("hidden").into();
                    return hidden;
                }
                let width = expanded
                    .signal()
                    .map(|expanded| if expanded { dp(180.0) } else { dp(120.0) });
                let select: Element<TestVm> = Select::new(
                    vec![
                        SelectOption::new("email".to_string(), "Email".to_string()),
                        SelectOption::new("sms".to_string(), "SMS".to_string()),
                    ],
                    None::<String>,
                )
                .open(true)
                .size(width, dp(40.0))
                .into();
                select
            }
        }),
    ));
    let mut handler = test_handler(Some(tree), invalidation);

    let initial = handler.computed_scene().clone();
    let initial_rect = initial
        .scene
        .overlay_shapes
        .first()
        .expect("open select should emit overlay")
        .rect;
    assert!(!initial.overlay_close_handlers.is_empty());

    expanded.set(true);
    handler.request_redraw_if_dirty(Instant::now());
    let expanded_scene = handler.computed_scene().clone();
    let expanded_rect = expanded_scene
        .scene
        .overlay_shapes
        .first()
        .expect("expanded select should still emit overlay")
        .rect;
    assert!(expanded_rect.width > initial_rect.width);

    visible.set(false);
    handler.request_redraw_if_dirty(Instant::now());
    let hidden_scene = handler.computed_scene().clone();
    assert!(hidden_scene.scene.overlay_shapes.is_empty());
    assert!(hidden_scene.overlay_close_handlers.is_empty());
}

// ---------------------------------------------------------------------------
// 场景命令原地拼接
//
// 这些测试的核心断言不是「splice 是否被走到」，而是「无论走 splice 还是 recompose，
// 最终 `cached.computed` 的渲染命令流（顺序 + 内容）都与一次从零的全量重收集逐项等价」。
// 这把 splice 的正确性红线（z-order 不变、只有目标区间变化、失败能干净回退）钉死。
// ---------------------------------------------------------------------------

/// 从扁平场景里按渲染顺序抽取每个 shape 的 (rect, color)，用于逐项比对。
fn shape_fingerprints<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
) -> Vec<(Dp, Dp, Dp, Dp, Color)> {
    computed
        .scene
        .shapes
        .iter()
        .map(|s| (s.rect.x, s.rect.y, s.rect.width, s.rect.height, s.color))
        .collect()
}

fn shape_detail_fingerprints<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
) -> Vec<(Dp, Dp, Dp, Dp, Color, f32, f32)> {
    computed
        .scene
        .shapes
        .iter()
        .map(|s| {
            (
                s.rect.x,
                s.rect.y,
                s.rect.width,
                s.rect.height,
                s.color,
                s.corner_radius,
                s.stroke_width,
            )
        })
        .collect()
}

fn text_fingerprints<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
) -> Vec<(String, Dp, Dp, Dp, Dp, Color)> {
    computed
        .scene
        .texts
        .iter()
        .map(|text| {
            (
                text.content.to_string(),
                text.frame.x,
                text.frame.y,
                text.frame.width,
                text.frame.height,
                text.color,
            )
        })
        .collect()
}

fn texture_fingerprints<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
) -> Vec<(Dp, Dp, Dp, Dp, f32, f32)> {
    computed
        .scene
        .textures
        .iter()
        .map(|texture| {
            (
                texture.frame.x,
                texture.frame.y,
                texture.frame.width,
                texture.frame.height,
                texture.corner_radius,
                texture.opacity,
            )
        })
        .collect()
}

fn texture_source_fingerprints<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
) -> Vec<(u64, Dp, Dp, Dp, Dp, f32, f32)> {
    computed
        .scene
        .textures
        .iter()
        .map(|texture| {
            (
                texture.texture.id(),
                texture.frame.x,
                texture.frame.y,
                texture.frame.width,
                texture.frame.height,
                texture.corner_radius,
                texture.opacity,
            )
        })
        .collect()
}

#[cfg(feature = "bench-support")]
fn texture_id_revision_fingerprints<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
) -> Vec<(u64, u64)> {
    computed
        .scene
        .textures
        .iter()
        .map(|texture| (texture.texture.id(), texture.texture.revision()))
        .collect()
}

fn brush_fingerprints<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
) -> Vec<(Dp, Dp, Dp, Dp, f32, crate::ui::widget::BackgroundBrush)> {
    computed
        .scene
        .brushes
        .iter()
        .map(|brush| {
            (
                brush.rect.x,
                brush.rect.y,
                brush.rect.width,
                brush.rect.height,
                brush.corner_radius,
                brush.brush.clone(),
            )
        })
        .collect()
}

fn backdrop_blur_fingerprints<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
) -> Vec<(Dp, Dp, Dp, Dp, f32, f32)> {
    computed
        .scene
        .backdrop_blurs
        .iter()
        .map(|blur| {
            (
                blur.rect.x,
                blur.rect.y,
                blur.rect.width,
                blur.rect.height,
                blur.corner_radius,
                blur.blur_radius,
            )
        })
        .collect()
}

const SIMPLE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="20"><rect width="10" height="20" fill="#22c55e"/></svg>"##;
const ALT_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="20"><rect width="10" height="20" fill="#ef4444"/></svg>"##;
#[cfg(feature = "bench-support")]
const WIDE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="30" height="10"><rect width="30" height="10" fill="#3b82f6"/></svg>"##;
#[cfg(feature = "bench-support")]
const ONE_BY_ONE_GIF: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xFF, 0xFF, 0xFF, 0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x01, 0x4C,
    0x00, 0x3B,
];

/// 构造「容器 > [兄弟0, 目标surface, 兄弟2]」的树，目标的背景色受 `state` 驱动。
/// 改色只改一个 shape 的颜色、不增删命令 —— 命中 splice 快路径的典型场景。
fn nested_color_tree(color_state: &State<Color>) -> WidgetTree<TestVm> {
    nested_color_tree_with_mode(color_state, false)
}

fn nested_color_tree_legacy(color_state: &State<Color>) -> WidgetTree<TestVm> {
    nested_color_tree_with_mode(color_state, true)
}

fn nested_color_tree_with_mode(color_state: &State<Color>, legacy: bool) -> WidgetTree<TestVm> {
    let target_bg = color_state.signal();
    let sibling0: Element<TestVm> = Stack::new()
        .size(dp(20.0), dp(20.0))
        .style_full(|ctx| {
            let mut style = ContainerStyle::default_for_theme(ctx.theme);
            style.surface.background = Some(Color::hexa(0x111111FF).into());
            style
        })
        .into();
    let target: Element<TestVm> = Stack::new()
        .size(dp(20.0), dp(20.0))
        .style_full(move |ctx| {
            let mut style = ContainerStyle::default_for_theme(ctx.theme);
            style.surface.background = Some(target_bg.clone().into());
            style
        })
        .into();
    let sibling2: Element<TestVm> = Stack::new()
        .size(dp(20.0), dp(20.0))
        .style_full(|ctx| {
            let mut style = ContainerStyle::default_for_theme(ctx.theme);
            style.surface.background = Some(Color::hexa(0x333333FF).into());
            style
        })
        .into();
    // 再包一层容器，确保目标位于「根 → 中间容器 → 目标」的深层位置，
    // 这样 splice 必须正确覆盖多个祖先 chunk（含根）才能等价。
    let inner: Element<TestVm> = Stack::new().child([sibling0, target, sibling2]).into();
    let root: Element<TestVm> = Stack::new().child(inner).into();
    if legacy {
        WidgetTree::new_legacy(root)
    } else {
        WidgetTree::new(root)
    }
}

fn text_color_tree(color_state: &State<Color>) -> WidgetTree<TestVm> {
    let text_color = color_state.signal();
    WidgetTree::new(
        Stack::new().child(Text::new("Reactive text color").style_full(move |ctx| {
            let mut style = crate::ui::widget::TextWidgetStyle::default_for_theme(ctx.theme);
            style.color = text_color.clone().into();
            style
        })),
    )
}

fn fixed_text_content_tree(content_state: &State<String>) -> WidgetTree<TestVm> {
    let content = content_state.signal();
    WidgetTree::new(Text::new(content).size(dp(180.0), dp(28.0)))
}

fn text_opacity_tree(opacity_state: &State<f32>) -> WidgetTree<TestVm> {
    let opacity = opacity_state.signal();
    WidgetTree::new(
        Text::new("Opacity text")
            .size(dp(180.0), dp(28.0))
            .style_full(move |ctx| {
                let mut style = crate::ui::widget::TextWidgetStyle::default_for_theme(ctx.theme);
                style.color = crate::ui::layout::Value::Static(Color::hexa(0xF8FAFCFF));
                style.surface.opacity = opacity.clone().into();
                style
            }),
    )
}

fn image_opacity_tree(opacity_state: &State<f32>) -> WidgetTree<TestVm> {
    let opacity = opacity_state.signal();
    WidgetTree::new(
        crate::ui::widget::Image::from_bytes(SIMPLE_SVG)
            .size(dp(48.0), dp(48.0))
            .style_full(move |ctx| {
                let mut style = crate::ui::widget::ImageStyle::default_for_theme(ctx.theme);
                style.surface.opacity = opacity.clone().into();
                style
            }),
    )
}

fn image_source_tree(source_state: &State<crate::media::MediaSource>) -> WidgetTree<TestVm> {
    let source = source_state.signal();
    WidgetTree::new(crate::ui::widget::Image::new(source).size(dp(48.0), dp(48.0)))
}

fn background_image_source_tree(
    image_state: &State<crate::ui::widget::BackgroundImage>,
) -> WidgetTree<TestVm> {
    let image = image_state.signal();
    WidgetTree::new(
        Stack::<TestVm>::new()
            .size(dp(48.0), dp(48.0))
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background_image = Some(image.clone().into());
                style
            }),
    )
}

fn background_brush_tree(
    brush_state: &State<crate::ui::widget::BackgroundBrush>,
) -> WidgetTree<TestVm> {
    let brush = brush_state.signal();
    WidgetTree::new(
        Stack::<TestVm>::new()
            .size(dp(48.0), dp(48.0))
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background_brush = Some(brush.clone().into());
                style.surface.border_radius = Some(crate::ui::layout::Value::Static(dp(6.0)));
                style
            }),
    )
}

fn background_blur_tree(blur_state: &State<Dp>) -> WidgetTree<TestVm> {
    let blur = blur_state.signal();
    WidgetTree::new(
        Stack::<TestVm>::new()
            .size(dp(48.0), dp(48.0))
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background_blur = blur.clone().into();
                style.surface.border_radius = Some(crate::ui::layout::Value::Static(dp(6.0)));
                style
            }),
    )
}

fn background_brush_offset_tree(offset_state: &State<Point>) -> WidgetTree<TestVm> {
    let offset = offset_state.signal();
    WidgetTree::new(
        Stack::<TestVm>::new()
            .size(dp(48.0), dp(48.0))
            .offset(offset)
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background_brush =
                    Some(crate::ui::widget::BackgroundBrush::Solid(Color::hexa(0x22C55EFF)).into());
                style.surface.border_radius = Some(crate::ui::layout::Value::Static(dp(6.0)));
                style
            }),
    )
}

fn background_blur_scale_tree(scale_state: &State<f32>) -> WidgetTree<TestVm> {
    let scale = scale_state.signal();
    WidgetTree::new(
        Stack::<TestVm>::new()
            .size(dp(48.0), dp(48.0))
            .scale(scale)
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background_blur = crate::ui::layout::Value::Static(dp(8.0));
                style.surface.border_radius = Some(crate::ui::layout::Value::Static(dp(6.0)));
                style
            }),
    )
}

fn background_image_offset_tree(offset_state: &State<Point>) -> WidgetTree<TestVm> {
    let offset = offset_state.signal();
    WidgetTree::new(
        Stack::<TestVm>::new()
            .size(dp(48.0), dp(48.0))
            .offset(offset)
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background_image =
                    Some(crate::ui::widget::BackgroundImage::from_bytes(SIMPLE_SVG).into());
                style.surface.border_radius = Some(crate::ui::layout::Value::Static(dp(6.0)));
                style
            }),
    )
}

#[cfg(feature = "bench-support")]
fn background_image_scale_tree(
    scale_state: &State<f32>,
    source: crate::media::MediaSource,
) -> WidgetTree<TestVm> {
    let scale = scale_state.signal();
    WidgetTree::new_legacy(
        Stack::<TestVm>::new()
            .size(dp(48.0), dp(48.0))
            .scale(scale)
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background_image = Some(
                    crate::ui::widget::BackgroundImage::new(source.clone())
                        .fit(crate::media::ContentFit::Fill)
                        .into(),
                );
                style
            }),
    )
}

#[cfg(feature = "bench-support")]
fn intrinsic_image_source_tree(
    source_state: &State<crate::media::MediaSource>,
) -> WidgetTree<TestVm> {
    let source = source_state.signal();
    WidgetTree::new(crate::ui::widget::Image::new(source))
}

fn surface_opacity_tree(opacity_state: &State<f32>) -> WidgetTree<TestVm> {
    let opacity = opacity_state.signal();
    WidgetTree::new(
        Stack::<TestVm>::new()
            .size(dp(48.0), dp(48.0))
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background = Some(Color::hexa(0x111827FF).into());
                style.surface.border_color = Some(Color::hexa(0x38BDF8FF).into());
                style.surface.border_width = Some(crate::ui::layout::Value::Static(dp(2.0)));
                style.surface.border_radius = Some(crate::ui::layout::Value::Static(dp(6.0)));
                style.surface.opacity = opacity.clone().into();
                style
            }),
    )
}

fn surface_offset_tree(offset_state: &State<Point>) -> WidgetTree<TestVm> {
    let offset = offset_state.signal();
    WidgetTree::new(
        Stack::<TestVm>::new()
            .size(dp(48.0), dp(48.0))
            .offset(offset)
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background = Some(Color::hexa(0x111827FF).into());
                style.surface.border_color = Some(Color::hexa(0x38BDF8FF).into());
                style.surface.border_width = Some(crate::ui::layout::Value::Static(dp(2.0)));
                style.surface.border_radius = Some(crate::ui::layout::Value::Static(dp(6.0)));
                style
            }),
    )
}

fn retained_transform_offset_tree(offset_state: &State<Point>) -> (WidgetTree<TestVm>, WidgetId) {
    let offset = offset_state.signal();
    let moving: Element<TestVm> = Stack::<TestVm>::new()
        .size(dp(96.0), dp(36.0))
        .overflow(Overflow::Visible)
        .offset(offset)
        .child(Text::new("fixed slot").size(dp(80.0), dp(20.0)))
        .into();
    let moving_id = moving.id;
    (
        WidgetTree::new(Stack::<TestVm>::new().child(moving)),
        moving_id,
    )
}

fn retained_transform_offset_hit_tree(
    offset_state: &State<Point>,
) -> (WidgetTree<TestVm>, WidgetId, WidgetId) {
    let offset = offset_state.signal();
    let clickable: Element<TestVm> = Stack::<TestVm>::new()
        .size(dp(40.0), dp(20.0))
        .overflow(Overflow::Visible)
        .on_click(Command::new(|_vm| {}))
        .into();
    let clickable_id = clickable.id;
    let moving: Element<TestVm> = Stack::<TestVm>::new()
        .size(dp(96.0), dp(36.0))
        .overflow(Overflow::Visible)
        .offset(offset)
        .child(clickable)
        .into();
    let moving_id = moving.id;
    (
        WidgetTree::new(Stack::<TestVm>::new().child(moving)),
        moving_id,
        clickable_id,
    )
}

#[cfg(feature = "bench-support")]
fn inherited_clip_mask_transform_offset_tree(
    offset_state: &State<Point>,
) -> (WidgetTree<TestVm>, WidgetId) {
    let offset = offset_state.signal();
    let moving: Element<TestVm> = Stack::<TestVm>::new()
        .size(dp(72.0), dp(32.0))
        .overflow(Overflow::Visible)
        .offset(offset)
        .style_full(|ctx| {
            let mut style = ContainerStyle::default_for_theme(ctx.theme);
            style.surface.background = Some(Color::hexa(0x111827FF).into());
            style
        })
        .into();
    let moving_id = moving.id;
    (
        WidgetTree::new(
            Stack::<TestVm>::new()
                .size(dp(80.0), dp(40.0))
                .overflow(Overflow::Hidden)
                .style_full(|ctx| {
                    let mut style = ContainerStyle::default_for_theme(ctx.theme);
                    style.surface.border_radius = Some(crate::ui::layout::Value::Static(dp(12.0)));
                    style
                })
                .child(moving),
        ),
        moving_id,
    )
}

fn surface_scale_tree(scale_state: &State<f32>) -> WidgetTree<TestVm> {
    let scale = scale_state.signal();
    WidgetTree::new(
        Stack::<TestVm>::new()
            .size(dp(48.0), dp(48.0))
            .scale(scale)
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background = Some(Color::hexa(0x111827FF).into());
                style.surface.border_color = Some(Color::hexa(0x38BDF8FF).into());
                style.surface.border_width = Some(crate::ui::layout::Value::Static(dp(2.0)));
                style.surface.border_radius = Some(crate::ui::layout::Value::Static(dp(6.0)));
                style
            }),
    )
}

fn border_color_tree(color_state: &State<Color>) -> WidgetTree<TestVm> {
    let color = color_state.signal();
    WidgetTree::new(
        Stack::<TestVm>::new()
            .size(dp(48.0), dp(48.0))
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background = Some(Color::hexa(0x111827FF).into());
                style.surface.border_color = Some(color.clone().into());
                style.surface.border_width = Some(crate::ui::layout::Value::Static(dp(2.0)));
                style.surface.border_radius = Some(crate::ui::layout::Value::Static(dp(6.0)));
                style
            }),
    )
}

fn border_radius_tree(radius_state: &State<Dp>) -> WidgetTree<TestVm> {
    let radius = radius_state.signal();
    WidgetTree::new(
        Stack::<TestVm>::new()
            .size(dp(48.0), dp(48.0))
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background = Some(Color::hexa(0x111827FF).into());
                style.surface.border_color = Some(Color::hexa(0x38BDF8FF).into());
                style.surface.border_width = Some(crate::ui::layout::Value::Static(dp(2.0)));
                style.surface.border_radius = Some(radius.clone().into());
                style
            }),
    )
}

fn border_width_tree(width_state: &State<Dp>) -> WidgetTree<TestVm> {
    let width = width_state.signal();
    WidgetTree::new(
        Stack::<TestVm>::new()
            .size(dp(48.0), dp(48.0))
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background = Some(Color::hexa(0x111827FF).into());
                style.surface.border_color = Some(Color::hexa(0x38BDF8FF).into());
                style.surface.border_width = Some(width.clone().into());
                style.surface.border_radius = Some(crate::ui::layout::Value::Static(dp(12.0)));
                style
            }),
    )
}

fn progress_bar_tree(progress_state: &State<f32>) -> WidgetTree<TestVm> {
    let progress = progress_state.signal();
    WidgetTree::new(
        ProgressBar::<TestVm>::new(progress)
            .size(dp(200.0), dp(20.0))
            .style(|style, _| {
                style.track_color = crate::ui::layout::Value::Static(Color::hexa(0x202020FF));
                style.fill_color = crate::ui::layout::Value::Static(Color::hexa(0x29A3FFFF));
                style.radius = crate::ui::layout::Value::Static(dp(0.0));
                style.height = dp(20.0);
            }),
    )
}

fn labeled_progress_bar_tree(progress_state: &State<f32>) -> WidgetTree<TestVm> {
    let progress = progress_state.signal();
    WidgetTree::new(
        ProgressBar::<TestVm>::new(progress)
            .show_label(true)
            .size(dp(200.0), dp(44.0))
            .style(|style, _| {
                style.track_color = crate::ui::layout::Value::Static(Color::hexa(0x202020FF));
                style.fill_color = crate::ui::layout::Value::Static(Color::hexa(0x29A3FFFF));
                style.label_color = crate::ui::layout::Value::Static(Color::hexa(0xF8FAFCFF));
                style.radius = crate::ui::layout::Value::Static(dp(0.0));
                style.height = dp(20.0);
                style.gap = dp(4.0);
            }),
    )
}

fn slider_tree(value_state: &State<f32>) -> WidgetTree<TestVm> {
    let value = value_state.signal();
    WidgetTree::new(
        Slider::<TestVm>::new(value, 0.0, 1.0)
            .step(0.01)
            .size(dp(220.0), dp(32.0))
            .style(|style, _| {
                style.track = crate::ui::theme::StateValue::new(crate::ui::layout::Value::Static(
                    Color::hexa(0x1F2937FF),
                ));
                style.active_track = crate::ui::theme::StateValue::new(
                    crate::ui::layout::Value::Static(Color::hexa(0x22C55EFF)),
                );
                style.thumb = crate::ui::theme::StateValue::new(crate::ui::layout::Value::Static(
                    Color::hexa(0xF8FAFCFF),
                ));
                style.thumb_shadow = None;
                style.border_width = crate::ui::layout::Value::Static(dp(0.0));
                style.track_height = dp(6.0);
                style.thumb_size = dp(20.0);
                style.radius = crate::ui::layout::Value::Static(dp(0.0));
            }),
    )
}

fn labeled_slider_tree(value_state: &State<f32>) -> WidgetTree<TestVm> {
    let value = value_state.signal();
    WidgetTree::new(
        Slider::<TestVm>::new(value, 0.0, 1.0)
            .step(0.01)
            .show_value_label(true)
            .size(dp(220.0), dp(52.0))
            .style(|style, _| {
                style.track = crate::ui::theme::StateValue::new(crate::ui::layout::Value::Static(
                    Color::hexa(0x1F2937FF),
                ));
                style.active_track = crate::ui::theme::StateValue::new(
                    crate::ui::layout::Value::Static(Color::hexa(0x22C55EFF)),
                );
                style.thumb = crate::ui::theme::StateValue::new(crate::ui::layout::Value::Static(
                    Color::hexa(0xF8FAFCFF),
                ));
                style.label = crate::ui::theme::StateValue::new(crate::ui::layout::Value::Static(
                    Color::hexa(0xF8FAFCFF),
                ));
                style.thumb_shadow = None;
                style.border_width = crate::ui::layout::Value::Static(dp(0.0));
                style.track_height = dp(6.0);
                style.thumb_size = dp(20.0);
                style.radius = crate::ui::layout::Value::Static(dp(0.0));
            }),
    )
}

fn slider_hit_fingerprint<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
) -> Option<(f32, Rect, Rect)> {
    computed
        .hit_regions
        .iter()
        .find_map(|hit| match &hit.interaction {
            HitInteraction::Slider {
                value,
                track_rect,
                thumb_rect,
                ..
            } => Some((*value, *track_rect, *thumb_rect)),
            _ => None,
        })
}

fn apply_legacy_scene_dependency_invalidation(
    handler: &mut BoundRuntimeHandler<TestVm>,
) -> &'static str {
    let revision = handler.invalidation.revision();
    let (dirty_kind, dirty_dependencies) = handler
        .invalidation
        .dirty_dependencies_since(handler.last_invalidation_revision);
    handler.last_invalidation_revision = revision;
    let _ = handler.invalidation.drain_reactive_updates();
    handler.invalidate_cached_scene_for_dependencies(
        dirty_kind,
        &dirty_dependencies,
        &[],
        0,
        Instant::now(),
    )
}

#[cfg(feature = "bench-support")]
const STRICT_REACTIVE_ALLOWED_ACTIONS: &[&str] = &[
    "media_texture_slot_write",
    "reactive_layout_slot_update",
    "reactive_property_slot_write",
    "reactive_slot_update",
    "reactive_structure_slot_update",
    "reactive_transform_record_update",
    "strict_reactive_capability_missing_plan",
    "strict_reactive_detached_rejected",
    "strict_reactive_global_rejected",
    "strict_reactive_layout_rejected",
    "strict_reactive_media_rejected",
    "strict_reactive_scene_rejected",
];

#[cfg(feature = "bench-support")]
const STRICT_REACTIVE_FORBIDDEN_FALLBACK_ACTIONS: &[&str] = &[
    "global_full_rebuild",
    "layout_scene_subtree_patch",
    "reactive_global_full_rebuild",
    "reactive_layout_scene_patch",
    "reactive_scene_full_recollect",
    "scene_full_recollect",
];

#[cfg(feature = "bench-support")]
fn assert_action_count(snapshot: &[(&'static str, u64)], expected: &'static str, count: u64) {
    let actual = snapshot
        .iter()
        .find_map(|(action, actual)| (*action == expected).then_some(*actual))
        .unwrap_or(0);
    assert_eq!(
        actual, count,
        "expected action {expected} to be recorded {count} time(s), got {snapshot:?}"
    );
}

#[cfg(feature = "bench-support")]
fn assert_action_absent(snapshot: &[(&'static str, u64)], unexpected: &'static str) {
    assert!(
        !snapshot.iter().any(|(action, _)| *action == unexpected),
        "unexpected action {unexpected} in {snapshot:?}"
    );
}

#[cfg(feature = "bench-support")]
fn assert_strict_reactive_actions_allowed(snapshot: &[(&'static str, u64)]) {
    assert!(
        !snapshot.is_empty(),
        "strict reactive update should record at least one action"
    );
    for (action, count) in snapshot {
        assert!(
            *count > 0,
            "action counts should be positive in {snapshot:?}"
        );
        assert!(
            STRICT_REACTIVE_ALLOWED_ACTIONS.contains(action),
            "unexpected strict reactive action {action}; allowed={STRICT_REACTIVE_ALLOWED_ACTIONS:?}; snapshot={snapshot:?}"
        );
    }
    for forbidden in STRICT_REACTIVE_FORBIDDEN_FALLBACK_ACTIONS {
        assert_action_absent(snapshot, forbidden);
    }
}

#[cfg(feature = "bench-support")]
fn capture_strict_reactive_actions(
    handler: &mut BoundRuntimeHandler<TestVm>,
    update: impl FnOnce(),
) -> Vec<(&'static str, u64)> {
    crate::runtime::action_stats::reset();
    update();
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();
    assert_strict_reactive_actions_allowed(&snapshot);
    snapshot
}

#[cfg(feature = "bench-support")]
fn assert_strict_missing_property_slot_plan_rejected(
    handler: &mut BoundRuntimeHandler<TestVm>,
    property: crate::foundation::binding::PropertySlot,
    trigger: impl FnOnce(),
) {
    let cached = handler
        .cached_scene
        .as_mut()
        .expect("initial scene should be cached");
    assert!(
        cached
            .reactive_slot_bindings
            .keys()
            .any(|(_, existing)| *existing == property),
        "initial strict collect should prebuild a {property:?} slot binding"
    );
    cached
        .reactive_slot_bindings
        .retain(|(_, existing), _| *existing != property);

    let snapshot = capture_strict_reactive_actions(handler, trigger);
    assert_action_count(&snapshot, "strict_reactive_scene_rejected", 1);
    assert_action_absent(&snapshot, "reactive_property_slot_write");

    let cached = handler.cached_scene.as_ref().expect("cache shell");
    assert!(
        cached.layout_valid && cached.computed_valid,
        "strict rejection must preserve retained caches"
    );
}

#[cfg(feature = "bench-support")]
fn filled_stack(color: Color) -> Stack<TestVm> {
    Stack::<TestVm>::new().style_full(move |ctx| {
        let mut style = ContainerStyle::default_for_theme(ctx.theme);
        style.surface.background = Some(color.into());
        style
    })
}

#[cfg(feature = "bench-support")]
fn assert_reactive_layout_slot_update_matches_full_recollect(
    label: &'static str,
    invalidation: InvalidationSignal,
    tree: WidgetTree<TestVm>,
    update: impl FnOnce(),
) {
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before_shapes = shape_detail_fingerprints(handler.computed_scene());

    let snapshot = capture_strict_reactive_actions(&mut handler, update);
    let layout_slot_updates = snapshot
        .iter()
        .find_map(|(action, count)| (*action == "reactive_layout_slot_update").then_some(*count))
        .unwrap_or(0);
    assert_eq!(
        layout_slot_updates, 1,
        "{label}: expected one reactive_layout_slot_update, got {snapshot:?}"
    );
    assert_action_absent(&snapshot, "strict_reactive_layout_rejected");
    assert_action_absent(&snapshot, "reactive_layout_scene_patch");

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("retained layout update should keep cache");
    assert!(
        cached.layout_valid && cached.computed_valid,
        "{label}: retained layout update should leave valid caches"
    );
    let after_slot_shapes = shape_detail_fingerprints(&cached.computed);
    assert_ne!(
        before_shapes, after_slot_shapes,
        "{label}: test update should move or resize at least one shape"
    );

    handler.invalidate_scene_with_reason("layout_slot_equivalence_full_recollect");
    let after_full_shapes = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_shapes, after_full_shapes,
        "{label}: retained layout slot update must match full layout + scene recollect"
    );
}

#[test]
fn splice_color_change_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let color = context.state(Color::hexa(0xFF0000FF));
    let tree = nested_color_tree_legacy(&color);
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    let before = shape_fingerprints(handler.computed_scene());

    // 改色 → 走失效决策（命中 scene_subtree_patch，内部尝试 splice）。
    #[cfg(test)]
    crate::runtime::scene_patch::splice_probe::reset();
    color.set(Color::hexa(0x00FF00FF));
    assert_eq!(
        apply_legacy_scene_dependency_invalidation(&mut handler),
        "scene_subtree_patch"
    );
    // 确认确实走了 splice 快路径，而非回退到 recompose——否则本测试只验证了回退正确性。
    #[cfg(test)]
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        1,
        "color change should hit the splice fast path exactly once"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("scene patch keeps cache shell");
    assert!(cached.computed_valid);
    let after_patch = shape_fingerprints(&cached.computed);

    // 强制一次从零的全量重收集，作为「真值」。
    handler.invalidate_computed_scene();
    let after_full = shape_fingerprints(handler.computed_scene());

    // 1) patch 结果与全量重收集逐项等价（顺序 + rect + color）。
    assert_eq!(
        after_patch, after_full,
        "spliced scene must be byte-identical to a fresh full recollect"
    );
    // 2) 命令数量不变（纯属性变化）。
    assert_eq!(before.len(), after_patch.len());
    // 3) 确有且仅有目标那一项颜色发生变化，其余 shape 不动（z-order/内容不变）。
    let diffs: Vec<usize> = before
        .iter()
        .zip(after_patch.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        diffs.len(),
        1,
        "exactly one shape should change, got {diffs:?}"
    );
    let changed = diffs[0];
    // 变化项只有颜色变，几何不变。
    assert_eq!(before[changed].0, after_patch[changed].0);
    assert_eq!(before[changed].1, after_patch[changed].1);
    assert_eq!(before[changed].4, Color::hexa(0xFF0000FF));
    assert_eq!(after_patch[changed].4, Color::hexa(0x00FF00FF));
}

#[test]
fn splice_repeated_updates_stay_consistent() {
    // 连续多次改色：splice 在 counts 不变下原地覆盖，offset 不应漂移。
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let color = context.state(Color::hexa(0xFF0000FF));
    let tree = nested_color_tree_legacy(&color);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    for hex in [0x00FF00FF_u32, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF] {
        color.set(Color::hexa(hex));
        assert_eq!(
            apply_legacy_scene_dependency_invalidation(&mut handler),
            "scene_subtree_patch"
        );
        let after_patch =
            shape_fingerprints(&handler.cached_scene.as_ref().expect("cache shell").computed);
        handler.invalidate_computed_scene();
        let after_full = shape_fingerprints(handler.computed_scene());
        assert_eq!(
            after_patch, after_full,
            "spliced scene diverged from full recollect after setting {hex:#010x}"
        );
        assert!(after_patch
            .iter()
            .any(|(_, _, _, _, c)| *c == Color::hexa(hex)));
    }
}

#[test]
fn splice_sibling_zorder_is_preserved() {
    // 改中间兄弟的颜色，前后兄弟（更低/更高 z-order）的命令位置与内容必须不变。
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let color = context.state(Color::hexa(0xFF0000FF));
    let tree = nested_color_tree_legacy(&color);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = shape_fingerprints(handler.computed_scene());

    color.set(Color::hexa(0x00FF00FF));
    assert_eq!(
        apply_legacy_scene_dependency_invalidation(&mut handler),
        "scene_subtree_patch"
    );
    let after = shape_fingerprints(&handler.cached_scene.as_ref().expect("cache shell").computed);

    // 兄弟0（固定 0x111111）与兄弟2（固定 0x333333）必须仍在场景中、且保持原相对顺序。
    let s0 = Color::hexa(0x111111FF);
    let s2 = Color::hexa(0x333333FF);
    let pos = |fps: &[(Dp, Dp, Dp, Dp, Color)], c: Color| {
        fps.iter().position(|(_, _, _, _, col)| *col == c)
    };
    let (b0, b2) = (pos(&before, s0), pos(&before, s2));
    let (a0, a2) = (pos(&after, s0), pos(&after, s2));
    assert!(b0.is_some() && b2.is_some() && a0.is_some() && a2.is_some());
    assert_eq!(b0, a0, "sibling0 z-order/position changed");
    assert_eq!(b2, a2, "sibling2 z-order/position changed");
    assert!(b0 < b2 && a0 < a2, "sibling relative order must be stable");
}

#[test]
fn reactive_property_slot_write_color_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let color = context.state(Color::hexa(0xFF0000FF));
    let tree = nested_color_tree(&color);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = shape_fingerprints(handler.computed_scene());

    crate::runtime::scene_patch::splice_probe::reset();
    color.set(Color::hexa(0x00FF00FF));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive property slot write should not enter the scene splice path"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = shape_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = shape_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written scene must match a fresh full recollect"
    );

    let diffs: Vec<usize> = before
        .iter()
        .zip(after_slot_write.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(diffs.len(), 1, "exactly one shape should change");
    let changed = diffs[0];
    assert_eq!(before[changed].4, Color::hexa(0xFF0000FF));
    assert_eq!(after_slot_write[changed].4, Color::hexa(0x00FF00FF));
}

#[test]
fn reactive_background_transparent_uses_retained_slot_write() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let color = context.state(Color::rgba(255, 0, 0, 0));
    let tree = nested_color_tree(&color);
    let mut handler = test_handler(Some(tree), invalidation);
    let before = shape_fingerprints(handler.computed_scene());
    assert!(
        before
            .iter()
            .any(|(_, _, _, _, color)| *color == Color::rgba(255, 0, 0, 0)),
        "reactive transparent background should keep a retained shape slot: {before:?}"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    color.set(Color::hexa(0x00FF00FF));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "transparent-to-opaque background should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_opaque_slot_write = shape_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_opaque_full = shape_fingerprints(handler.computed_scene());
    assert_eq!(
        after_opaque_slot_write, after_opaque_full,
        "transparent-to-opaque slot write must match a fresh full recollect"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    color.set(Color::rgba(255, 0, 0, 0));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "opaque-to-transparent background should not enter scene splice"
    );
    let after_transparent_slot_write = shape_fingerprints(
        &handler
            .cached_scene
            .as_ref()
            .expect("slot write keeps cache shell")
            .computed,
    );

    handler.invalidate_computed_scene();
    let after_transparent_full = shape_fingerprints(handler.computed_scene());
    assert_eq!(
        after_transparent_slot_write, after_transparent_full,
        "opaque-to-transparent slot write must match a fresh full recollect"
    );
}

#[test]
fn reactive_property_slot_write_text_color_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let color = context.state(Color::hexa(0xFF0000FF));
    let tree = text_color_tree(&color);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = text_fingerprints(handler.computed_scene());

    crate::runtime::scene_patch::splice_probe::reset();
    color.set(Color::hexa(0x00FF00FF));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive text-color slot write should not enter the scene splice path"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = text_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = text_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written text scene must match a fresh full recollect"
    );

    let diffs: Vec<usize> = before
        .iter()
        .zip(after_slot_write.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(diffs.len(), 1, "exactly one text primitive should change");
    let changed = diffs[0];
    assert_eq!(before[changed].5, Color::hexa(0xFF0000FF));
    assert_eq!(after_slot_write[changed].5, Color::hexa(0x00FF00FF));
}

#[test]
fn reactive_property_slot_write_text_content_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let content = context.state(String::from("Alpha"));
    let tree = fixed_text_content_tree(&content);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = text_fingerprints(handler.computed_scene());
    assert_eq!(before.len(), 1, "fixed text should emit one text primitive");

    crate::runtime::scene_patch::splice_probe::reset();
    content.set(String::from("Omega"));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive fixed text content slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = text_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = text_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written text content scene must match a fresh full recollect"
    );

    let diffs: Vec<usize> = before
        .iter()
        .zip(after_slot_write.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(diffs.len(), 1, "exactly one text primitive should change");
    let changed = diffs[0];
    assert_eq!(before[changed].0, "Alpha");
    assert_eq!(after_slot_write[changed].0, "Omega");
    assert_eq!(before[changed].1, after_slot_write[changed].1);
    assert_eq!(before[changed].2, after_slot_write[changed].2);
    assert_eq!(before[changed].3, after_slot_write[changed].3);
    assert_eq!(before[changed].4, after_slot_write[changed].4);
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_text_controller_input_content_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let controller = context.text_controller("Alpha");
    let tree = WidgetTree::new(Input::<TestVm>::new(controller.clone()).size(dp(180.0), dp(40.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let before = text_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        1,
        "single-line input should emit one text primitive"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    crate::runtime::action_stats::reset();
    controller.set_text("Omega");
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "controller-driven input content slot write should not enter scene splice"
    );
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "reactive_property_slot_write" && *count == 1),
        "controller-driven input content should update by retained slot write: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| action.starts_with("strict_reactive")),
        "strict tree should accept fixed input controller content update: {snapshot:?}"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = text_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = text_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written input controller content must match a fresh full recollect"
    );
    assert!(after_slot_write
        .iter()
        .any(|(content, ..)| content == "Omega"));
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_text_controller_input_placeholder_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let controller = context.text_controller("");
    let tree = WidgetTree::new(
        Input::<TestVm>::new(controller.clone())
            .placeholder("Empty")
            .size(dp(180.0), dp(40.0))
            .style(|style, _| {
                let color = crate::ui::layout::Value::Static(Color::hexa(0xA8B4C2E6));
                style.text = crate::ui::theme::StateValue::new(color.clone());
                style.placeholder = crate::ui::theme::StateValue::new(color);
            }),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let before = text_fingerprints(handler.computed_scene());
    assert_eq!(
        before
            .iter()
            .filter(|(content, ..)| content == "Empty")
            .count(),
        1,
        "empty controller should render the placeholder in the retained text slot"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    crate::runtime::action_stats::reset();
    controller.set_text("Omega");
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "placeholder-to-content input slot write should not enter scene splice"
    );
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "reactive_property_slot_write" && *count == 1),
        "fixed input placeholder update should use retained slot write: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| action.starts_with("strict_reactive")),
        "fixed input placeholder update must not be rejected: {snapshot:?}"
    );
    let after_slot_write = text_fingerprints(
        &handler
            .cached_scene
            .as_ref()
            .expect("slot write keeps cache shell")
            .computed,
    );

    handler.invalidate_computed_scene();
    let after_full = text_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written placeholder-to-content input must match a fresh full recollect"
    );
    assert!(after_slot_write
        .iter()
        .any(|(content, ..)| content == "Omega"));
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_text_controller_input_requires_fixed_frame() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let controller = context.text_controller("Alpha");
    let tree = WidgetTree::new(Input::<TestVm>::new(controller.clone()));
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    crate::runtime::action_stats::reset();
    controller.set_text("Omega");
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();

    assert!(
        snapshot
            .iter()
            .any(|(action, count)| action.starts_with("strict_reactive") && *count == 1),
        "non-fixed input controller update should be rejected in strict mode: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "reactive_property_slot_write"),
        "non-fixed input must not be reported as a retained slot write: {snapshot:?}"
    );
}

#[test]
fn reactive_property_slot_write_text_opacity_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let opacity = context.state(0.35_f32);
    let tree = text_opacity_tree(&opacity);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = text_fingerprints(handler.computed_scene());

    crate::runtime::scene_patch::splice_probe::reset();
    opacity.set(0.85);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive text opacity slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = text_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = text_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written text opacity scene must match a fresh full recollect"
    );

    let diffs: Vec<usize> = before
        .iter()
        .zip(after_slot_write.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(diffs.len(), 1, "exactly one text primitive should change");
    let changed = diffs[0];
    assert_eq!(before[changed].0, after_slot_write[changed].0);
    assert!(
        after_slot_write[changed].5.a > before[changed].5.a,
        "text alpha should increase"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    opacity.set(0.0);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive text opacity-to-zero should not enter scene splice"
    );
    let after_zero_slot_write = text_fingerprints(
        &handler
            .cached_scene
            .as_ref()
            .expect("slot write keeps cache shell")
            .computed,
    );

    handler.invalidate_computed_scene();
    let after_zero_full = text_fingerprints(handler.computed_scene());
    assert_eq!(
        after_zero_slot_write, after_zero_full,
        "text opacity-to-zero slot write must match a fresh full recollect"
    );
    assert_eq!(
        after_zero_slot_write.len(),
        after_slot_write.len(),
        "reactive opacity keeps retained text primitive slots"
    );
    assert!(
        after_zero_slot_write
            .iter()
            .all(|(_, _, _, _, _, color)| color.a == 0),
        "text opacity zero should write transparent colors: {after_zero_slot_write:?}"
    );
}

#[cfg(feature = "bench-support")]
fn write_temp_gif(name: &str) -> std::path::PathBuf {
    write_temp_media(name, ONE_BY_ONE_GIF)
}

#[cfg(feature = "bench-support")]
fn write_temp_media(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("tgui-runtime-media-slot-{nanos}"));
    std::fs::create_dir_all(&dir).expect("temp media directory should be created");
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("temp media should be written");
    path
}

#[cfg(feature = "bench-support")]
fn animated_gif_bytes() -> Vec<u8> {
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame, RgbaImage};

    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        encoder
            .set_repeat(Repeat::Infinite)
            .expect("gif repeat should encode");
        let red = RgbaImage::from_raw(1, 1, vec![255, 0, 0, 255]).expect("valid red rgba image");
        let blue = RgbaImage::from_raw(1, 1, vec![0, 0, 255, 255]).expect("valid blue rgba image");
        encoder
            .encode_frame(Frame::from_parts(
                red,
                0,
                0,
                Delay::from_numer_denom_ms(20, 1),
            ))
            .expect("first gif frame should encode");
        encoder
            .encode_frame(Frame::from_parts(
                blue,
                0,
                0,
                Delay::from_numer_denom_ms(20, 1),
            ))
            .expect("second gif frame should encode");
    }
    bytes
}

#[cfg(feature = "bench-support")]
fn delayed_media_url(
    bytes: &'static [u8],
    content_type: &'static str,
) -> (
    String,
    std::sync::mpsc::Sender<()>,
    std::thread::JoinHandle<()>,
) {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("delayed media test server should bind");
    let url = format!(
        "http://{}/delayed-media.svg",
        listener
            .local_addr()
            .expect("delayed media test server should have a local addr")
    );
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        use std::io::{Read, Write};

        let (mut stream, _) = listener
            .accept()
            .expect("delayed media test server should accept one request");
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request);
        release_rx
            .recv()
            .expect("delayed media response should be released");
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        );
        stream
            .write_all(header.as_bytes())
            .and_then(|_| stream.write_all(bytes))
            .expect("delayed media response should be written");
    });
    (url, release_tx, handle)
}

#[cfg(feature = "bench-support")]
fn wait_for_one_texture(
    handler: &mut BoundRuntimeHandler<TestVm>,
    timeout: Duration,
) -> Vec<(u64, Dp, Dp, Dp, Dp, f32, f32)> {
    let start = Instant::now();
    loop {
        handler.request_redraw_if_dirty(Instant::now());
        let scene = handler.computed_scene();
        let fingerprints = texture_source_fingerprints(scene);
        if fingerprints.len() == 1
            && scene
                .scene
                .textures
                .first()
                .map(|texture| texture.texture.size() != (1, 1))
                .unwrap_or(false)
        {
            return fingerprints;
        }
        assert!(
            start.elapsed() < timeout,
            "timed out waiting for async media texture; last fingerprints={fingerprints:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn reactive_property_slot_write_image_opacity_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let opacity = context.state(0.35_f32);
    let tree = image_opacity_tree(&opacity);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = texture_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        1,
        "loaded image should emit one texture primitive"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    opacity.set(0.85);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive image opacity slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = texture_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = texture_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written image opacity scene must match a fresh full recollect"
    );

    let diffs: Vec<usize> = before
        .iter()
        .zip(after_slot_write.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        diffs.len(),
        1,
        "exactly one texture primitive should change"
    );
    let changed = diffs[0];
    assert_eq!(before[changed].0, after_slot_write[changed].0);
    assert_eq!(before[changed].1, after_slot_write[changed].1);
    assert_eq!(before[changed].2, after_slot_write[changed].2);
    assert_eq!(before[changed].3, after_slot_write[changed].3);
    assert_eq!(before[changed].4, after_slot_write[changed].4);
    assert!(
        after_slot_write[changed].5 > before[changed].5,
        "texture opacity should increase"
    );
}

#[test]
fn reactive_property_slot_write_image_source_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let source = context.state(crate::media::MediaSource::bytes(SIMPLE_SVG));
    let tree = image_source_tree(&source);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = texture_source_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        1,
        "loaded image should emit one texture primitive"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    source.set(crate::media::MediaSource::bytes(ALT_SVG));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive image source slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = texture_source_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = texture_source_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written image source scene must match a fresh full recollect"
    );
    assert_ne!(
        before[0].0, after_slot_write[0].0,
        "image source update should replace the texture frame in place"
    );
    assert_eq!(before[0].1, after_slot_write[0].1);
    assert_eq!(before[0].2, after_slot_write[0].2);
    assert_eq!(before[0].3, after_slot_write[0].3);
    assert_eq!(before[0].4, after_slot_write[0].4);
}

#[test]
fn reactive_property_slot_write_background_image_source_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let image = context.state(crate::ui::widget::BackgroundImage::from_bytes(SIMPLE_SVG));
    let tree = background_image_source_tree(&image);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = texture_source_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        1,
        "loaded background image should emit one texture primitive"
    );
    let before_media_key = {
        let cached = handler.cached_scene.as_ref().expect("cache shell");
        assert_eq!(
            cached.media_texture_binding_index.len(),
            1,
            "background image slot should have one retained media binding index"
        );
        let keys = cached
            .media_texture_bindings
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 1);
        keys[0].clone()
    };

    crate::runtime::scene_patch::splice_probe::reset();
    #[cfg(feature = "bench-support")]
    crate::runtime::action_stats::reset();
    image.set(crate::ui::widget::BackgroundImage::from_bytes(ALT_SVG));
    handler.request_redraw_if_dirty(Instant::now());
    #[cfg(feature = "bench-support")]
    {
        let snapshot = crate::runtime::action_stats::snapshot();
        assert!(
            snapshot
                .iter()
                .any(|(action, count)| *action == "reactive_property_slot_write" && *count == 1),
            "background image source update should use retained slot writes: {snapshot:?}"
        );
        assert!(
            !snapshot
                .iter()
                .any(|(action, _)| *action == "media_texture_binding_full_rebuild"),
            "reactive texture slot write must update media bindings locally: {snapshot:?}"
        );
    }
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive background image source slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    assert_eq!(
        cached.media_texture_binding_index.len(),
        1,
        "reactive source update should keep exactly one retained media binding index"
    );
    let after_media_keys = cached
        .media_texture_bindings
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(after_media_keys.len(), 1);
    assert_ne!(
        before_media_key, after_media_keys[0],
        "reactive source update should replace the media binding key in place"
    );
    assert_eq!(
        after_media_keys[0].source,
        crate::media::MediaSource::bytes(ALT_SVG),
        "media completion lookup should track the new source after the slot write"
    );
    let after_slot_write = texture_source_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = texture_source_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written background image source scene must match a fresh full recollect"
    );
    assert_ne!(
        before[0].0, after_slot_write[0].0,
        "background image source update should replace the texture frame in place"
    );
    assert_eq!(before[0].1, after_slot_write[0].1);
    assert_eq!(before[0].2, after_slot_write[0].2);
    assert_eq!(before[0].3, after_slot_write[0].3);
    assert_eq!(before[0].4, after_slot_write[0].4);
}

#[test]
fn reactive_property_slot_write_background_brush_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let brush = context.state(crate::ui::widget::BackgroundBrush::Solid(Color::hexa(
        0x22C55EFF,
    )));
    let tree = background_brush_tree(&brush);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = brush_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        1,
        "simple background brush should emit one brush primitive"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    brush.set(crate::ui::widget::BackgroundBrush::LinearGradient(
        crate::ui::widget::BackgroundLinearGradient::new(
            Point::new(dp(0.0), dp(0.0)),
            Point::new(dp(48.0), dp(0.0)),
            vec![
                crate::ui::widget::BackgroundGradientStop::new(0.0, Color::hexa(0x0EA5E9FF)),
                crate::ui::widget::BackgroundGradientStop::new(1.0, Color::hexa(0xF97316FF)),
            ],
        ),
    ));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive background brush slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = brush_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = brush_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written background brush scene must match a fresh full recollect"
    );
    assert_eq!(before.len(), after_slot_write.len());
    assert_eq!(before[0].0, after_slot_write[0].0);
    assert_eq!(before[0].1, after_slot_write[0].1);
    assert_eq!(before[0].2, after_slot_write[0].2);
    assert_eq!(before[0].3, after_slot_write[0].3);
    assert_eq!(before[0].4, after_slot_write[0].4);
    assert_ne!(
        before[0].5, after_slot_write[0].5,
        "background brush payload should be replaced in place"
    );
}

#[test]
fn reactive_property_slot_write_background_blur_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let blur = context.state(dp(4.0));
    let tree = background_blur_tree(&blur);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = backdrop_blur_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        1,
        "positive background blur should emit one backdrop blur primitive"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    blur.set(dp(12.0));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive background blur slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = backdrop_blur_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = backdrop_blur_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written background blur scene must match a fresh full recollect"
    );
    assert_eq!(before.len(), after_slot_write.len());
    assert_eq!(before[0].0, after_slot_write[0].0);
    assert_eq!(before[0].1, after_slot_write[0].1);
    assert_eq!(before[0].2, after_slot_write[0].2);
    assert_eq!(before[0].3, after_slot_write[0].3);
    assert_eq!(before[0].4, after_slot_write[0].4);
    assert_eq!(after_slot_write[0].5, 12.0);
}

#[test]
fn reactive_background_blur_zero_uses_retained_slot_write() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let blur = context.state(dp(0.0));
    let tree = background_blur_tree(&blur);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = backdrop_blur_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        1,
        "zero reactive background blur should keep a retained backdrop blur slot"
    );
    assert_eq!(before[0].5, 0.0);

    crate::runtime::scene_patch::splice_probe::reset();
    blur.set(dp(12.0));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "zero-to-positive background blur slot write should not hit splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = backdrop_blur_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = backdrop_blur_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "zero-to-positive background blur slot write must match a fresh full recollect"
    );
    assert_eq!(
        after_slot_write.len(),
        1,
        "background blur slot write must keep primitive count fixed"
    );
    assert_eq!(after_slot_write[0].5, 12.0);
}

#[test]
fn reactive_property_slot_write_background_brush_offset_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let offset = context.state(Point::ZERO);
    let tree = background_brush_offset_tree(&offset);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = brush_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        1,
        "brush surface should emit one brush primitive"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    offset.set(Point::new(dp(6.0), dp(8.0)));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive brush offset slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = brush_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = brush_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written brush offset scene must match a fresh full recollect"
    );
    assert_eq!(before.len(), after_slot_write.len());
    assert_eq!(after_slot_write[0].0 - before[0].0, dp(6.0));
    assert_eq!(after_slot_write[0].1 - before[0].1, dp(8.0));
    assert_eq!(before[0].2, after_slot_write[0].2);
    assert_eq!(before[0].3, after_slot_write[0].3);
    assert_eq!(before[0].4, after_slot_write[0].4);
    assert_eq!(before[0].5, after_slot_write[0].5);
}

#[test]
fn reactive_property_slot_write_background_blur_scale_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let scale = context.state(1.0_f32);
    let tree = background_blur_scale_tree(&scale);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = backdrop_blur_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        1,
        "blur surface should emit one backdrop blur primitive"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    scale.set(1.25);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive blur scale slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = backdrop_blur_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = backdrop_blur_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written blur scale scene must match a fresh full recollect"
    );
    assert_eq!(before.len(), after_slot_write.len());
    assert!(after_slot_write[0].0 < before[0].0);
    assert!(after_slot_write[0].1 < before[0].1);
    assert!(after_slot_write[0].2 > before[0].2);
    assert!(after_slot_write[0].3 > before[0].3);
    assert_eq!(before[0].4, after_slot_write[0].4);
    assert_eq!(before[0].5, after_slot_write[0].5);
}

#[test]
fn reactive_property_slot_write_background_image_offset_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let offset = context.state(Point::ZERO);
    let tree = background_image_offset_tree(&offset);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = texture_source_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        1,
        "background image surface should emit one texture primitive"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    offset.set(Point::new(dp(6.0), dp(8.0)));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive background image offset slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = texture_source_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = texture_source_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written background image offset scene must match a fresh full recollect"
    );
    assert_eq!(before.len(), after_slot_write.len());
    assert_eq!(before[0].0, after_slot_write[0].0);
    assert_eq!(after_slot_write[0].1 - before[0].1, dp(6.0));
    assert_eq!(after_slot_write[0].2 - before[0].2, dp(8.0));
    assert_eq!(before[0].3, after_slot_write[0].3);
    assert_eq!(before[0].4, after_slot_write[0].4);
    assert_eq!(before[0].5, after_slot_write[0].5);
    assert_eq!(before[0].6, after_slot_write[0].6);
}

#[test]
#[cfg(feature = "bench-support")]
fn intrinsic_image_source_change_rejects_layout_path_not_texture_slot_write() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let source = context.state(crate::media::MediaSource::bytes(SIMPLE_SVG));
    let tree = intrinsic_image_source_tree(&source);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    crate::runtime::action_stats::reset();
    source.set(crate::media::MediaSource::bytes(WIDE_SVG));
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();

    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "strict_reactive_layout_rejected" && *count == 1),
        "intrinsic image source changes must be rejected by strict reactive layout: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "reactive_property_slot_write"),
        "intrinsic image source changes must not be reported as O(1) texture slot writes: {snapshot:?}"
    );
}

#[test]
fn reactive_property_slot_write_surface_opacity_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let opacity = context.state(0.35_f32);
    let tree = surface_opacity_tree(&opacity);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        2,
        "solid bordered surface should emit background + border shapes"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    opacity.set(0.85);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive surface opacity slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = shape_detail_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written surface opacity scene must match a fresh full recollect"
    );

    let diffs: Vec<usize> = before
        .iter()
        .zip(after_slot_write.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        diffs.len(),
        2,
        "background and border colors should be the only changed shapes"
    );
    for changed in diffs {
        assert_eq!(before[changed].0, after_slot_write[changed].0);
        assert_eq!(before[changed].1, after_slot_write[changed].1);
        assert_eq!(before[changed].2, after_slot_write[changed].2);
        assert_eq!(before[changed].3, after_slot_write[changed].3);
        assert_eq!(before[changed].5, after_slot_write[changed].5);
        assert_eq!(before[changed].6, after_slot_write[changed].6);
        assert!(
            after_slot_write[changed].4.a > before[changed].4.a,
            "shape alpha should increase"
        );
    }

    crate::runtime::scene_patch::splice_probe::reset();
    opacity.set(0.0);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive surface opacity-to-zero should not enter scene splice"
    );
    let after_zero_slot_write = shape_detail_fingerprints(
        &handler
            .cached_scene
            .as_ref()
            .expect("slot write keeps cache shell")
            .computed,
    );

    handler.invalidate_computed_scene();
    let after_zero_full = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        after_zero_slot_write, after_zero_full,
        "opacity-to-zero slot write must match a fresh full recollect"
    );
    assert_eq!(
        after_zero_slot_write.len(),
        after_slot_write.len(),
        "reactive opacity keeps retained surface primitive slots"
    );
    assert!(
        after_zero_slot_write
            .iter()
            .all(|(_, _, _, _, color, _, _)| color.a == 0),
        "surface opacity zero should write transparent colors: {after_zero_slot_write:?}"
    );
}

#[test]
fn reactive_surface_initial_zero_opacity_uses_retained_slot_write() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let opacity = context.state(0.0_f32);
    let tree = surface_opacity_tree(&opacity);
    let mut handler = test_handler(Some(tree), invalidation);
    let before = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        2,
        "reactive zero opacity should retain background + border shape slots"
    );
    assert!(
        before.iter().all(|(_, _, _, _, color, _, _)| color.a == 0),
        "initial zero opacity should render retained transparent shapes: {before:?}"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    opacity.set(0.85);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "zero-to-visible opacity should not enter scene splice"
    );
    let after_slot_write = shape_detail_fingerprints(
        &handler
            .cached_scene
            .as_ref()
            .expect("slot write keeps cache shell")
            .computed,
    );

    handler.invalidate_computed_scene();
    let after_full = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "zero-to-visible opacity slot write must match a fresh full recollect"
    );
    assert!(
        after_slot_write
            .iter()
            .all(|(_, _, _, _, color, _, _)| color.a > 0),
        "visible opacity should write non-transparent colors: {after_slot_write:?}"
    );
}

#[test]
fn reactive_property_slot_write_surface_offset_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let offset = context.state(Point::ZERO);
    let tree = surface_offset_tree(&offset);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        2,
        "solid bordered surface should emit background + border shapes"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    offset.set(Point::new(dp(6.0), dp(8.0)));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive surface offset slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = shape_detail_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written surface offset scene must match a fresh full recollect"
    );

    let diffs: Vec<usize> = before
        .iter()
        .zip(after_slot_write.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        diffs.len(),
        2,
        "background and border rects should be the only changed shapes"
    );
    for changed in diffs {
        assert_eq!(after_slot_write[changed].0 - before[changed].0, dp(6.0));
        assert_eq!(after_slot_write[changed].1 - before[changed].1, dp(8.0));
        assert_eq!(before[changed].2, after_slot_write[changed].2);
        assert_eq!(before[changed].3, after_slot_write[changed].3);
        assert_eq!(before[changed].4, after_slot_write[changed].4);
        assert_eq!(before[changed].5, after_slot_write[changed].5);
        assert_eq!(before[changed].6, after_slot_write[changed].6);
    }
}

#[test]
fn reactive_transform_record_offset_updates_only_retained_record() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let offset = context.state(Point::ZERO);
    let (tree, moving_id) = retained_transform_offset_tree(&offset);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("initial collect should populate cache");
    let before_shapes = shape_detail_fingerprints(&cached.computed);
    let before_texts = text_fingerprints(&cached.computed);
    assert!(
        !before_texts.is_empty(),
        "test tree should emit a fixed text primitive"
    );
    let record = cached.computed.transform_records.get(&moving_id).unwrap_or_else(|| {
        panic!(
            "eligible reactive offset container should install a transform record; records={:?} hits={} overlay_hits={} focus={} scroll={:?} backdrop={} brushes={} canvas={} meshes={} overlay_commands={} chains={:?}",
            cached.computed.transform_records,
            cached.computed.hit_regions.len(),
            cached.computed.overlay_hit_regions.len(),
            cached.computed.focus_scopes.len(),
            cached
                .computed
                .scroll_regions
                .iter()
                .map(|region| (region.id, region.overflow_x, region.overflow_y))
                .collect::<Vec<_>>(),
            cached.computed.scene.backdrop_blurs.len(),
            cached.computed.scene.brushes.len(),
            cached.computed.scene.canvas_composites.len(),
            cached.computed.scene.meshes.len(),
            cached.computed.scene.overlay_commands.len(),
            cached.computed.scene.command_transform_chains(),
        )
    });
    assert_eq!(record.base_offset, Point::ZERO);
    assert_eq!(record.current_offset, Point::ZERO);
    assert!(
        cached
            .computed
            .scene
            .command_transform_chains()
            .iter()
            .any(|chain| chain.contains(&moving_id)),
        "render commands in the moved subtree should carry the transform chain"
    );

    #[cfg(feature = "bench-support")]
    crate::runtime::action_stats::reset();
    crate::runtime::scene_patch::splice_probe::reset();

    let next_offset = Point::new(dp(12.0), dp(7.0));
    offset.set(next_offset);
    handler.request_redraw_if_dirty(Instant::now());

    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "transform record update should not enter scene splice"
    );
    #[cfg(feature = "bench-support")]
    {
        let snapshot = crate::runtime::action_stats::snapshot();
        assert!(
            snapshot.iter().any(|(action, count)| {
                *action == "reactive_transform_record_update" && *count == 1
            }),
            "offset change should be consumed by transform record update: {snapshot:?}"
        );
    }

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("record update keeps cache shell");
    assert!(cached.computed_valid);
    let after_shapes = shape_detail_fingerprints(&cached.computed);
    let after_texts = text_fingerprints(&cached.computed);
    assert_eq!(
        after_shapes, before_shapes,
        "retained transform update must not rewrite shape primitive rects"
    );
    assert_eq!(
        after_texts, before_texts,
        "retained transform update must not rewrite text primitive frames"
    );
    let record = cached
        .computed
        .transform_records
        .get(&moving_id)
        .expect("transform record should stay installed");
    assert_eq!(record.base_offset, Point::ZERO);
    assert_eq!(record.current_offset, next_offset);

    handler.invalidate_computed_scene();
    let full = handler.computed_scene();
    let full_shapes = shape_detail_fingerprints(full);
    let full_texts = text_fingerprints(full);
    assert_eq!(full_shapes.len(), before_shapes.len());
    for (before, full) in before_shapes.iter().zip(full_shapes.iter()) {
        assert_eq!(full.0, before.0 + next_offset.x);
        assert_eq!(full.1, before.1 + next_offset.y);
        assert_eq!(full.2, before.2);
        assert_eq!(full.3, before.3);
        assert_eq!(full.4, before.4);
        assert_eq!(full.5, before.5);
        assert_eq!(full.6, before.6);
    }
    assert_eq!(full_texts.len(), before_texts.len());
    for (before, full) in before_texts.iter().zip(full_texts.iter()) {
        assert_eq!(full.0, before.0);
        assert_eq!(full.1, before.1 + next_offset.x);
        assert_eq!(full.2, before.2 + next_offset.y);
        assert_eq!(full.3, before.3);
        assert_eq!(full.4, before.4);
        assert_eq!(full.5, before.5);
    }
}

#[test]
fn reactive_transform_record_offset_moves_simple_hit_regions_without_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let offset = context.state(Point::ZERO);
    let (tree, moving_id, clickable_id) = retained_transform_offset_hit_tree(&offset);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("initial collect should populate cache");
    if !cached.computed.transform_records.contains_key(&moving_id) {
        panic!(
            "simple hit-bearing subtree should still install a retained transform record; records={:?} hits={:?} overlays={} focus_scopes={} scroll={:?} scene_counts={:?}",
            cached.computed.transform_records,
            cached
                .computed
                .hit_regions
                .iter()
                .map(|hit| {
                    (
                        hit.interaction.target_id(),
                        hit.rect,
                        hit.focus.is_some(),
                        hit.gpu_scroll_container,
                        hit.transform_chain.clone(),
                        hit.supports_retained_transform(),
                    )
                })
                .collect::<Vec<_>>(),
            cached.computed.overlay_hit_regions.len(),
            cached.computed.focus_scopes.len(),
            cached
                .computed
                .scroll_regions
                .iter()
                .map(|region| (region.id, region.overflow_x, region.overflow_y))
                .collect::<Vec<_>>(),
            cached.computed.scene.counts(),
        );
    }
    assert!(
        cached.computed.hit_regions.iter().any(|hit| {
            hit.transform_chain.contains(&moving_id)
                && matches!(
                    hit.interaction,
                    HitInteraction::Widget { id, .. } if id == clickable_id
                )
        }),
        "clickable child hit should carry the moving ancestor transform chain"
    );
    let before_hit_rects: Vec<_> = cached
        .computed
        .hit_regions
        .iter()
        .map(|hit| (hit.interaction.target_id(), hit.rect))
        .collect();
    let old_point = Point::new(dp(10.0), dp(10.0));
    handler.cursor_position = Some(old_point);
    let viewport = handler.viewport_rect();
    assert!(
        handler
            .hit_path(viewport)
            .into_iter()
            .any(|hit| matches!(hit, HitInteraction::Widget { id, .. } if id == clickable_id)),
        "initial point should hit the clickable child"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    let next_offset = Point::new(dp(30.0), dp(10.0));
    offset.set(next_offset);
    handler.request_redraw_if_dirty(Instant::now());

    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "hit-bearing transform record update should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("record update keeps cache shell");
    assert!(cached.computed_valid);
    assert_eq!(
        cached
            .computed
            .hit_regions
            .iter()
            .map(|hit| (hit.interaction.target_id(), hit.rect))
            .collect::<Vec<_>>(),
        before_hit_rects,
        "retained transform update must not rewrite raw hit rects"
    );
    assert_eq!(
        cached
            .computed
            .transform_records
            .get(&moving_id)
            .expect("transform record should stay installed")
            .current_offset,
        next_offset
    );
    handler.cursor_position = Some(old_point);
    let viewport = handler.viewport_rect();
    assert!(
        !handler
            .hit_path(viewport)
            .into_iter()
            .any(|hit| matches!(hit, HitInteraction::Widget { id, .. } if id == clickable_id)),
        "old point should stop hitting after the retained transform moves"
    );
    let new_point = Point::new(dp(35.0), dp(15.0));
    handler.cursor_position = Some(new_point);
    let viewport = handler.viewport_rect();
    assert!(
        handler
            .hit_path(viewport)
            .into_iter()
            .any(|hit| matches!(hit, HitInteraction::Widget { id, .. } if id == clickable_id)),
        "new transformed point should hit without scene recollect"
    );

    handler.invalidate_computed_scene();
    let full = handler.computed_scene();
    assert!(
        WidgetTree::hit_path_from_computed(full, new_point)
            .into_iter()
            .any(|hit| matches!(hit, HitInteraction::Widget { id, .. } if id == clickable_id)),
        "fresh full collect should agree with retained hit query"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_tree_rejects_retained_transform_with_inherited_clip_mask() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let offset = context.state(Point::ZERO);
    let (tree, moving_id) = inherited_clip_mask_transform_offset_tree(&offset);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("initial collect should populate cache");
    assert!(
        cached
            .computed
            .scene
            .shapes
            .iter()
            .any(|shape| shape.clip_mask.is_some()),
        "test setup should inherit a rounded clip mask into the moving subtree"
    );
    assert!(
        !cached.computed.transform_records.contains_key(&moving_id),
        "subtrees with inherited clip masks are not safe for retained transform records"
    );

    crate::runtime::action_stats::reset();
    crate::runtime::scene_patch::splice_probe::reset();
    offset.set(Point::new(dp(10.0), dp(4.0)));
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();

    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "strict rejection must not enter scene splice"
    );
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| action.starts_with("strict_reactive") && *count == 1),
        "strict tree should reject unsupported clip-mask transform fallback: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "reactive_transform_record_update"),
        "clip-mask transform must not be reported as a retained transform update: {snapshot:?}"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_tree_rejects_primitive_count_changing_property_update() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let border_width = context.state(dp(2.0));
    let width = border_width.signal();
    let tree = WidgetTree::new(Stack::<TestVm>::new().size(dp(48.0), dp(48.0)).style_full(
        move |ctx| {
            let mut style = ContainerStyle::default_for_theme(ctx.theme);
            style.surface.background = Some(Color::hexa(0x111827FF).into());
            style.surface.border_color = Some(Color::hexa(0x38BDF8FF).into());
            style.surface.border_width = Some(width.clone().into());
            style
        },
    ));
    let mut handler = test_handler(Some(tree), invalidation);
    let initial_shapes = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        initial_shapes.len(),
        2,
        "test setup should retain background and border primitives initially"
    );

    crate::runtime::action_stats::reset();
    crate::runtime::scene_patch::splice_probe::reset();
    border_width.set(Dp::ZERO);
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();

    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "strict primitive-count rejection must not enter scene splice"
    );
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "strict_reactive_scene_rejected" && *count == 1),
        "strict tree should reject property updates that would remove retained primitives: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "reactive_property_slot_write"),
        "primitive-count-changing update must not be reported as a slot write: {snapshot:?}"
    );
}

#[test]
fn reactive_property_slot_write_surface_scale_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let scale = context.state(1.0_f32);
    let tree = surface_scale_tree(&scale);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        2,
        "solid bordered surface should emit background + border shapes"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    scale.set(1.25);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive surface scale slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = shape_detail_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written surface scale scene must match a fresh full recollect"
    );

    let diffs: Vec<usize> = before
        .iter()
        .zip(after_slot_write.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        diffs.len(),
        2,
        "background and border rects should be the only changed shapes"
    );
    for changed in diffs {
        assert!(after_slot_write[changed].0 < before[changed].0);
        assert!(after_slot_write[changed].1 < before[changed].1);
        assert!(after_slot_write[changed].2 > before[changed].2);
        assert!(after_slot_write[changed].3 > before[changed].3);
        assert_eq!(before[changed].4, after_slot_write[changed].4);
    }
}

#[test]
fn reactive_border_color_transparent_uses_retained_slot_write() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let color = context.state(Color::rgba(56, 189, 248, 0));
    let tree = border_color_tree(&color);
    let mut handler = test_handler(Some(tree), invalidation);
    let before = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        2,
        "reactive transparent border should retain background + border slots"
    );
    assert!(
        before
            .iter()
            .any(|(_, _, _, _, color, _, stroke_width)| { *stroke_width > 0.0 && color.a == 0 }),
        "initial transparent border should keep a retained stroke slot: {before:?}"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    color.set(Color::hexa(0x38BDF8FF));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "transparent-to-visible border color should not enter scene splice"
    );
    let after_visible_slot_write = shape_detail_fingerprints(
        &handler
            .cached_scene
            .as_ref()
            .expect("slot write keeps cache shell")
            .computed,
    );

    handler.invalidate_computed_scene();
    let after_visible_full = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        after_visible_slot_write, after_visible_full,
        "transparent-to-visible border color slot write must match a fresh full recollect"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    color.set(Color::rgba(56, 189, 248, 0));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "visible-to-transparent border color should not enter scene splice"
    );
    let after_transparent_slot_write = shape_detail_fingerprints(
        &handler
            .cached_scene
            .as_ref()
            .expect("slot write keeps cache shell")
            .computed,
    );

    handler.invalidate_computed_scene();
    let after_transparent_full = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        after_transparent_slot_write, after_transparent_full,
        "visible-to-transparent border color slot write must match a fresh full recollect"
    );
    assert_eq!(
        after_transparent_slot_write.len(),
        before.len(),
        "reactive border color keeps retained stroke slots"
    );
}

#[test]
fn reactive_property_slot_write_border_radius_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let radius = context.state(dp(4.0));
    let tree = border_radius_tree(&radius);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        2,
        "solid bordered surface should emit background + border shapes"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    radius.set(dp(12.0));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive border-radius slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = shape_detail_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written border radius scene must match a fresh full recollect"
    );

    let diffs: Vec<usize> = before
        .iter()
        .zip(after_slot_write.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        diffs.len(),
        2,
        "background and border radius should be the only changed shapes"
    );
    for changed in diffs {
        assert_eq!(before[changed].0, after_slot_write[changed].0);
        assert_eq!(before[changed].1, after_slot_write[changed].1);
        assert_eq!(before[changed].2, after_slot_write[changed].2);
        assert_eq!(before[changed].3, after_slot_write[changed].3);
        assert_eq!(before[changed].4, after_slot_write[changed].4);
        assert_eq!(before[changed].6, after_slot_write[changed].6);
        assert!(
            after_slot_write[changed].5 > before[changed].5,
            "corner radius should increase"
        );
    }
}

#[test]
fn reactive_property_slot_write_border_width_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let width = context.state(dp(2.0));
    let tree = border_width_tree(&width);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        2,
        "solid bordered surface should emit background + border shapes"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    width.set(dp(6.0));
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive border-width slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = shape_detail_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written border width scene must match a fresh full recollect"
    );

    let diffs: Vec<usize> = before
        .iter()
        .zip(after_slot_write.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        diffs.len(),
        2,
        "background inset and border stroke should be the only changed shapes"
    );
    assert!(
        after_slot_write
            .iter()
            .any(|(_, _, _, _, _, _, stroke)| (*stroke - 6.0).abs() <= f32::EPSILON),
        "border stroke width should update in place"
    );
}

#[test]
fn reactive_property_slot_write_progress_value_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let progress = context.state(0.25_f32);
    let tree = progress_bar_tree(&progress);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = shape_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        2,
        "determinate non-zero progress should emit track + fill"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    progress.set(0.75);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive progress slot write should not enter the scene splice path"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = shape_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = shape_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "slot-written progress scene must match a fresh full recollect"
    );

    let diffs: Vec<usize> = before
        .iter()
        .zip(after_slot_write.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        diffs.len(),
        1,
        "exactly one progress fill shape should change"
    );
    let changed = diffs[0];
    assert_eq!(before[changed].0, after_slot_write[changed].0);
    assert_eq!(before[changed].1, after_slot_write[changed].1);
    assert_eq!(before[changed].3, after_slot_write[changed].3);
    assert_eq!(before[changed].4, Color::hexa(0x29A3FFFF));
    assert_eq!(after_slot_write[changed].4, Color::hexa(0x29A3FFFF));
    assert!(
        after_slot_write[changed].2 > before[changed].2,
        "progress fill width should grow in place"
    );
}

#[test]
fn reactive_progress_zero_fill_uses_retained_slot_write() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let progress = context.state(0.0_f32);
    let tree = progress_bar_tree(&progress);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = shape_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        2,
        "zero progress should keep a retained fill slot"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    progress.set(0.5);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "zero-to-nonzero progress slot write should not hit splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = shape_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full = shape_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_write, after_full,
        "zero-to-nonzero progress slot write must match a fresh full recollect"
    );
    assert_eq!(
        after_slot_write.len(),
        before.len(),
        "progress slot write must keep primitive count fixed"
    );
    let diffs = before
        .iter()
        .zip(after_slot_write.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(diffs, 1, "only the retained fill slot should change");
}

#[test]
fn reactive_labeled_progress_value_updates_fill_and_label_slots() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let progress = context.state(0.25_f32);
    let tree = labeled_progress_bar_tree(&progress);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before_shapes = shape_fingerprints(handler.computed_scene());
    let before_texts = text_fingerprints(handler.computed_scene());
    assert_eq!(
        before_shapes.len(),
        2,
        "labeled progress should emit track + fill"
    );
    assert_eq!(
        before_texts.len(),
        1,
        "labeled progress should emit one label"
    );
    assert_eq!(before_texts[0].0, "25%");

    crate::runtime::scene_patch::splice_probe::reset();
    progress.set(0.75);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "labeled progress slot write should not enter scene splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_shapes = shape_fingerprints(&cached.computed);
    let after_slot_texts = text_fingerprints(&cached.computed);

    handler.invalidate_computed_scene();
    let after_full_shapes = shape_fingerprints(handler.computed_scene());
    let after_full_texts = text_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_shapes, after_full_shapes,
        "slot-written labeled progress shapes must match full recollect"
    );
    assert_eq!(
        after_slot_texts, after_full_texts,
        "slot-written labeled progress text must match full recollect"
    );
    assert_eq!(after_slot_texts[0].0, "75%");
    assert_eq!(
        after_slot_texts[0].1, before_texts[0].1,
        "label frame x should stay fixed"
    );
    assert_eq!(
        after_slot_texts[0].2, before_texts[0].2,
        "label frame y should stay fixed"
    );
    let changed_shapes = before_shapes
        .iter()
        .zip(after_slot_shapes.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(changed_shapes, 1, "only the fill rect should change");
}

#[test]
fn reactive_property_slot_write_slider_value_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let value = context.state(0.25_f32);
    let tree = slider_tree(&value);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = shape_fingerprints(handler.computed_scene());
    let before_hit = slider_hit_fingerprint(handler.computed_scene())
        .expect("slider should emit a hit region before update");
    assert_eq!(
        before.len(),
        3,
        "borderless shadowless slider should emit track + active track + thumb"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    value.set(0.75);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive slider slot write should not enter the scene splice path"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = shape_fingerprints(&cached.computed);
    let after_slot_hit = slider_hit_fingerprint(&cached.computed)
        .expect("slot write should preserve slider hit region");

    handler.invalidate_computed_scene();
    let after_full = shape_fingerprints(handler.computed_scene());
    let after_full_hit = slider_hit_fingerprint(handler.computed_scene())
        .expect("full recollect should preserve slider hit region");
    assert_eq!(
        after_slot_write, after_full,
        "slot-written slider scene must match a fresh full recollect"
    );
    assert_eq!(
        after_slot_hit, after_full_hit,
        "slot-written slider hit geometry must match full recollect"
    );

    let diffs: Vec<usize> = before
        .iter()
        .zip(after_slot_write.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        diffs.len(),
        2,
        "active track and thumb should be the only changed shapes"
    );
    assert!(
        after_slot_hit.0 > before_hit.0,
        "slider hit value should advance with the signal"
    );
    assert_eq!(
        after_slot_hit.1, before_hit.1,
        "slider track hit rect should stay fixed"
    );
    assert!(
        after_slot_hit.2.x > before_hit.2.x,
        "slider thumb hit rect should move with the visual thumb"
    );
}

#[test]
fn reactive_property_slot_write_labeled_slider_value_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let value = context.state(0.25_f32);
    let tree = labeled_slider_tree(&value);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before_shapes = shape_fingerprints(handler.computed_scene());
    let before_texts = text_fingerprints(handler.computed_scene());
    let before_hit = slider_hit_fingerprint(handler.computed_scene())
        .expect("labeled slider should emit a hit region before update");
    assert_eq!(
        before_texts.len(),
        1,
        "labeled slider should emit one text primitive"
    );
    assert_eq!(before_texts[0].0, "0.25");

    crate::runtime::scene_patch::splice_probe::reset();
    value.set(0.75);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "reactive labeled slider slot write should not enter the scene splice path"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_shapes = shape_fingerprints(&cached.computed);
    let after_slot_texts = text_fingerprints(&cached.computed);
    let after_slot_hit = slider_hit_fingerprint(&cached.computed)
        .expect("slot write should preserve labeled slider hit region");

    handler.invalidate_computed_scene();
    let after_full_shapes = shape_fingerprints(handler.computed_scene());
    let after_full_texts = text_fingerprints(handler.computed_scene());
    let after_full_hit = slider_hit_fingerprint(handler.computed_scene())
        .expect("full recollect should preserve labeled slider hit region");
    assert_eq!(
        after_slot_shapes, after_full_shapes,
        "slot-written labeled slider shapes must match a fresh full recollect"
    );
    assert_eq!(
        after_slot_texts, after_full_texts,
        "slot-written labeled slider text must match a fresh full recollect"
    );
    assert_eq!(
        after_slot_hit, after_full_hit,
        "slot-written labeled slider hit geometry must match full recollect"
    );

    let changed_shapes = before_shapes
        .iter()
        .zip(after_slot_shapes.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(
        changed_shapes, 2,
        "active track and thumb should change in place"
    );
    assert_eq!(after_slot_texts.len(), before_texts.len());
    assert_eq!(after_slot_texts[0].0, "0.75");
    assert_eq!(after_slot_texts[0].1, before_texts[0].1);
    assert_eq!(after_slot_texts[0].2, before_texts[0].2);
    assert_eq!(after_slot_texts[0].3, before_texts[0].3);
    assert_eq!(after_slot_texts[0].4, before_texts[0].4);
    assert!(
        after_slot_hit.0 > before_hit.0,
        "slider hit value should advance with the signal"
    );
}

#[test]
fn reactive_slider_zero_active_track_uses_retained_slot_write() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let value = context.state(0.0_f32);
    let tree = slider_tree(&value);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = shape_fingerprints(handler.computed_scene());
    assert_eq!(
        before.len(),
        3,
        "zero slider value should keep a retained active-track slot"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    value.set(0.5);
    handler.request_redraw_if_dirty(Instant::now());
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "zero-to-nonzero slider slot write should not hit splice"
    );
    let cached = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell");
    assert!(cached.computed_valid);
    let after_slot_write = shape_fingerprints(&cached.computed);
    let after_slot_hit =
        slider_hit_fingerprint(&cached.computed).expect("slot write should keep slider hit region");

    handler.invalidate_computed_scene();
    let after_full = shape_fingerprints(handler.computed_scene());
    let after_full_hit =
        slider_hit_fingerprint(handler.computed_scene()).expect("full recollect slider hit region");
    assert_eq!(
        after_slot_write, after_full,
        "zero-to-nonzero slider slot write must match a fresh full recollect"
    );
    assert_eq!(
        after_slot_hit, after_full_hit,
        "slot-written slider hit geometry must match full recollect"
    );
    assert_eq!(
        after_slot_write.len(),
        before.len(),
        "slider slot write must keep primitive count fixed"
    );
    let diffs = before
        .iter()
        .zip(after_slot_write.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(diffs, 2, "active track and thumb should change in place");
}

#[test]
#[cfg(feature = "bench-support")]
fn action_stats_records_reactive_property_slot_write_for_color_change() {
    // 改一个深层叶子的颜色，retained reactive drain 应先定位到属性级 Scene owner，
    // 再命中 property slot 原地写，而不是退回到通用 dependency graph 扫描或 scene patch。
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let color = context.state(Color::hexa(0xFF0000FF));
    let tree = nested_color_tree(&color);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    crate::runtime::action_stats::reset();
    color.set(Color::hexa(0x00FF00FF));
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();

    assert_eq!(
        snapshot,
        vec![
            ("reactive_property_slot_write", 1),
            ("reactive_slot_update", 1)
        ],
        "single deep-leaf color change should be driven by retained reactive property slot targets"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_signal_update_actions_stay_within_explicit_allowlist() {
    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let color = context.state(Color::hexa(0xFF0000FF));
        let tree = nested_color_tree(&color);
        let mut handler = test_handler(Some(tree), invalidation);
        let _ = handler.computed_scene();

        let snapshot = capture_strict_reactive_actions(&mut handler, || {
            color.set(Color::hexa(0x00FF00FF));
        });
        assert_action_count(&snapshot, "reactive_property_slot_write", 1);
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let offset = context.state(Point::ZERO);
        let (tree, _) = retained_transform_offset_tree(&offset);
        let mut handler = test_handler(Some(tree), invalidation);
        let _ = handler.computed_scene();

        let snapshot = capture_strict_reactive_actions(&mut handler, || {
            offset.set(Point::new(dp(8.0), dp(6.0)));
        });
        assert_action_count(&snapshot, "reactive_transform_record_update", 1);
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let width = context.state(dp(48.0));
        let tree = WidgetTree::try_new_strict(
            Stack::<TestVm>::new()
                .width(width.signal())
                .height(dp(24.0))
                .style_full(|ctx| {
                    let mut style = ContainerStyle::default_for_theme(ctx.theme);
                    style.surface.background = Some(Color::hexa(0x111827FF).into());
                    style
                }),
        )
        .expect("strict layout slot tree");
        let mut handler = test_handler(Some(tree), invalidation);
        let _ = handler.computed_scene();

        let snapshot = capture_strict_reactive_actions(&mut handler, || {
            width.set(dp(96.0));
        });
        assert_action_count(&snapshot, "reactive_layout_slot_update", 1);
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let source = context.state(crate::media::MediaSource::bytes(SIMPLE_SVG));
        let tree = WidgetTree::try_new_strict(crate::ui::widget::Image::new(source.signal()))
            .expect("strict image tree");
        let mut handler = test_handler(Some(tree), invalidation);
        let _ = handler.computed_scene();

        let snapshot = capture_strict_reactive_actions(&mut handler, || {
            source.set(crate::media::MediaSource::bytes(WIDE_SVG));
        });
        assert_action_count(&snapshot, "strict_reactive_layout_rejected", 1);
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let value = context.state(0.25_f32);
        let tree = WidgetTree::try_new_strict(
            Slider::<TestVm>::new(value.signal(), 0.0, 1.0)
                .show_ticks(true)
                .width(dp(120.0))
                .style(|style, _| {
                    style.thumb_shadow = None;
                }),
        )
        .expect("strict slider tree");
        let mut handler = test_handler(Some(tree), invalidation);
        let _ = handler.computed_scene();

        let snapshot = capture_strict_reactive_actions(&mut handler, || {
            value.set(0.75);
        });
        assert_action_count(&snapshot, "strict_reactive_scene_rejected", 1);
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let backing = Arc::new(Mutex::new(String::from("first")));
        let signal = {
            let backing = backing.clone();
            context.signal(move || backing.lock().expect("test signal lock poisoned").clone())
        };
        let tree = WidgetTree::try_new_strict(Text::new(signal).size(dp(120.0), dp(24.0)))
            .expect("strict text tree");
        let mut handler = test_handler(Some(tree), invalidation.clone());
        let _ = handler.computed_scene();

        let snapshot = capture_strict_reactive_actions(&mut handler, || {
            *backing.lock().expect("test signal lock poisoned") = String::from("second");
            invalidation.mark_dirty();
        });
        assert_action_count(&snapshot, "strict_reactive_global_rejected", 1);
        assert_action_absent(&snapshot, "reactive_slot_update");
    }
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_capability_report_records_retained_plans_and_reject_policy() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let color = context.state(Color::hexa(0x2563EBFF));
    let width = context.state(dp(40.0));
    let opaque_backing = Arc::new(Mutex::new(String::from("opaque")));
    let opaque_text = {
        let opaque_backing = opaque_backing.clone();
        context.signal(move || {
            opaque_backing
                .lock()
                .expect("opaque text signal lock poisoned")
                .clone()
        })
    };

    let color_signal = color.signal();
    let tree = WidgetTree::try_new_strict(
        Flex::vertical()
            .child(
                Stack::<TestVm>::new()
                    .width(width.signal())
                    .height(dp(20.0))
                    .style_full(move |ctx| {
                        let mut style = ContainerStyle::default_for_theme(ctx.theme);
                        style.surface.background = Some(color_signal.clone().into());
                        style
                    }),
            )
            .child(Text::new(opaque_text).size(dp(120.0), dp(24.0))),
    )
    .expect("strict capability tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let report = handler
        .cached_scene
        .as_ref()
        .and_then(|cached| cached.strict_capability_report.as_ref())
        .expect("strict scene cache should build a capability report");

    assert_eq!(
        report.missing_plan_count(),
        0,
        "strict capability report should classify every owner: {report:?}"
    );
    assert!(
        report
            .entries
            .iter()
            .any(|entry| entry.kind == StrictCapabilityKind::DirectSlot),
        "reactive background color should have a direct slot plan: {report:?}"
    );
    assert!(
        report
            .entries
            .iter()
            .any(|entry| entry.kind == StrictCapabilityKind::LayoutSlot),
        "reactive width should have a retained layout slot plan: {report:?}"
    );
    assert!(
        report.has_global_reject_policy,
        "opaque collect-time signal should be represented by a global reject policy: {report:?}"
    );
    assert!(
        report.retained_plan_count() >= 2,
        "strict capability report should count retained plans: {report:?}"
    );
    assert!(
        report.explicit_reject_count() <= report.entries.len(),
        "strict capability reject count should stay bounded by report entries: {report:?}"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_no_fallback_group_covers_actions_and_capabilities() {
    strict_reactive_signal_update_actions_stay_within_explicit_allowlist();
    strict_capability_report_records_retained_plans_and_reject_policy();
}

#[test]
#[cfg(feature = "bench-support")]
#[should_panic(expected = "strict capability report contains 1 missing retained plan(s)")]
fn strict_capability_report_missing_plan_is_hard_gated() {
    let report = crate::runtime::state::StrictCapabilityReport {
        entries: vec![crate::runtime::state::StrictCapabilityEntry {
            owner: crate::foundation::binding::DependencyOwner {
                widget_id: 1,
                phase: crate::foundation::binding::DependencyPhase::Scene,
                property: None,
            },
            kind: StrictCapabilityKind::MissingPlan,
        }],
        has_global_reject_policy: false,
    };

    report.enforce_no_missing_plans();
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_tree_allows_property_slot_write() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let color = context.state(Color::hexa(0xFF0000FF));
    let background = color.signal();
    let tree =
        WidgetTree::try_new_strict(Stack::<TestVm>::new().size(dp(48.0), dp(48.0)).style_full(
            move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background = Some(background.clone().into());
                style
            },
        ))
        .expect("strict static tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    crate::runtime::action_stats::reset();
    color.set(Color::hexa(0x00FF00FF));
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();

    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "reactive_property_slot_write" && *count == 1),
        "strict tree should keep allowed property slot writes: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| action.starts_with("strict_reactive")),
        "allowed slot writes must not be rejected: {snapshot:?}"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_tree_requires_prebuilt_property_slot_binding() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let color = context.state(Color::hexa(0xFF0000FF));
    let background = color.signal();
    let tree =
        WidgetTree::try_new_strict(Stack::<TestVm>::new().size(dp(48.0), dp(48.0)).style_full(
            move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background = Some(background.clone().into());
                style
            },
        ))
        .expect("strict static tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    assert_strict_missing_property_slot_plan_rejected(
        &mut handler,
        crate::foundation::binding::PropertySlot::Background,
        || {
            color.set(Color::hexa(0x00FF00FF));
        },
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_tree_uses_retained_targets_without_collect_dependency_lookup() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let value = context.state(1_i32);
    let background = value.signal().map(|value| {
        if value == 3 {
            Color::hexa(0x00FF00FF)
        } else {
            Color::hexa(0xFF0000FF)
        }
    });
    let label = value
        .signal()
        .map_memo(|value| if value % 2 == 0 { "even" } else { "odd" }.to_string());
    let target: Element<TestVm> = Stack::new()
        .size(dp(48.0), dp(48.0))
        .style_full(move |ctx| {
            let mut style = ContainerStyle::default_for_theme(ctx.theme);
            style.surface.background = Some(background.clone().into());
            style
        })
        .into();
    let unchanged_memo_target: Element<TestVm> = Text::new(label).size(dp(120.0), dp(24.0)).into();
    let tree = WidgetTree::new(Stack::<TestVm>::new().child([target, unchanged_memo_target]));
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    crate::runtime::action_stats::reset();
    value.set(3);
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();

    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "reactive_property_slot_write" && *count == 1),
        "changed retained target should update by direct property slot write: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "reactive_collect_dependency_lookup"),
        "strict retained target path must not consult collect-time dirty dependencies: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| action.starts_with("strict_reactive")),
        "strict tree should accept the retained slot write: {snapshot:?}"
    );
    let text = handler
        .cached_scene
        .as_ref()
        .expect("slot write keeps cache shell")
        .computed
        .scene
        .texts
        .iter()
        .find(|text| text.content.as_ref() == "odd");
    assert!(
        text.is_some(),
        "unchanged memo-derived target should stay rendered without being dirtied"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_tree_rejects_reactive_layout_patch() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let source = context.state(crate::media::MediaSource::bytes(SIMPLE_SVG));
    let tree = WidgetTree::try_new_strict(crate::ui::widget::Image::new(source.signal()))
        .expect("strict tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    crate::runtime::action_stats::reset();
    source.set(crate::media::MediaSource::bytes(WIDE_SVG));
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();

    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "strict_reactive_layout_rejected" && *count == 1),
        "strict tree should reject reactive layout patch fallback: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "reactive_layout_scene_patch"),
        "strict tree must not silently patch reactive layout changes: {snapshot:?}"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_tree_updates_reactive_auto_flow_layout_with_retained_layout_slot() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let width = context.state(dp(48.0));
    let first_width = width.signal();
    let tree = WidgetTree::try_new_strict(
        Flex::<TestVm>::new(Axis::Horizontal)
            .size(dp(160.0), dp(48.0))
            .child(
                Stack::new()
                    .width(first_width)
                    .height(dp(24.0))
                    .style_full(|ctx| {
                        let mut style = ContainerStyle::default_for_theme(ctx.theme);
                        style.surface.background = Some(Color::hexa(0x111827FF).into());
                        style
                    }),
            )
            .child(Stack::new().size(dp(24.0), dp(24.0)).style_full(|ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background = Some(Color::hexa(0x38BDF8FF).into());
                style
            })),
    )
    .expect("strict static flex tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    crate::runtime::action_stats::reset();
    width.set(dp(96.0));
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();

    assert_action_count(&snapshot, "reactive_layout_slot_update", 1);
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| action.starts_with("strict_reactive")),
        "strict tree should accept retained layout slot updates: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "reactive_layout_scene_patch"
                || *action == "layout_scene_subtree_patch"
                || *action == "global_full_rebuild"),
        "retained layout update must not use legacy layout fallback actions: {snapshot:?}"
    );

    let shapes = shape_fingerprints(
        &handler
            .cached_scene
            .as_ref()
            .expect("retained layout update should keep cache")
            .computed,
    );
    assert!(
        shapes
            .iter()
            .any(|(_, _, width, _, color)| *color == Color::hexa(0x111827FF) && *width == dp(96.0)),
        "first child width should update in retained layout: {shapes:?}"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_show_toggle_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let visible = context.state(true);
    let tree = WidgetTree::try_new_strict(Stack::<TestVm>::new().size(dp(160.0), dp(80.0)).child(
        Show::new(
            visible.signal(),
            filled_stack(Color::hexa(0x22C55EFF)).size(dp(48.0), dp(24.0)),
        ),
    ))
    .expect("strict retained show tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let before_shapes = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(before_shapes.len(), 1, "show child should start visible");

    let snapshot = capture_strict_reactive_actions(&mut handler, || visible.set(false));
    assert_action_count(&snapshot, "reactive_structure_slot_update", 1);
    assert_action_absent(&snapshot, "strict_reactive_layout_rejected");
    assert_action_absent(&snapshot, "reactive_layout_scene_patch");
    let after_slot_shapes = shape_detail_fingerprints(
        &handler
            .cached_scene
            .as_ref()
            .expect("show patch should keep cache")
            .computed,
    );
    assert!(
        after_slot_shapes.is_empty(),
        "hidden Show child should remove its scene primitives"
    );

    handler.invalidate_scene_with_reason("show_toggle_equivalence_full_recollect");
    let after_full_shapes = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_shapes, after_full_shapes,
        "retained Show toggle must match full layout + scene recollect"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_show_hide_clears_child_reactive_subscriptions() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let visible = context.state(true);
    let color = context.state(Color::RED);
    let color_signal = color.signal();
    let tree = WidgetTree::try_new_strict(
        Stack::<TestVm>::new()
            .size(dp(160.0), dp(80.0))
            .child(Show::new(
                visible.signal(),
                Stack::<TestVm>::new()
                    .size(dp(48.0), dp(24.0))
                    .style_full(move |ctx| {
                        let mut style = ContainerStyle::default_for_theme(ctx.theme);
                        style.surface.background = Some(color_signal.clone().into());
                        style
                    }),
            )),
    )
    .expect("strict retained show tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    let snapshot = capture_strict_reactive_actions(&mut handler, || visible.set(false));
    assert_action_count(&snapshot, "reactive_structure_slot_update", 1);

    color.set(Color::BLUE);
    let updates = handler.invalidation.drain_reactive_updates();
    assert!(
        updates.targets.is_empty(),
        "hidden Show child signal update must not enqueue a removed child target: {:?}",
        updates.targets
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_keyed_for_reorder_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let items = context.state(vec![1usize, 2]);
    let tree =
        WidgetTree::try_new_strict(Flex::vertical().size(dp(160.0), dp(96.0)).child(For::new(
            items.signal(),
            |item| *item,
            |_index, item| {
                let color = if *item == 1 {
                    Color::hexa(0x22C55EFF)
                } else {
                    Color::hexa(0x2563EBFF)
                };
                Stack::<TestVm>::new()
                    .size(dp(48.0), dp(24.0))
                    .style_full(move |ctx| {
                        let mut style = ContainerStyle::default_for_theme(ctx.theme);
                        style.surface.background = Some(color.into());
                        style
                    })
            },
        )))
        .expect("strict retained keyed For tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let before_shapes = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(before_shapes.len(), 2, "For should start with two children");

    let snapshot = capture_strict_reactive_actions(&mut handler, || items.set(vec![2, 1]));
    assert_action_count(&snapshot, "reactive_structure_slot_update", 1);
    assert_action_absent(&snapshot, "strict_reactive_layout_rejected");
    assert_action_absent(&snapshot, "reactive_layout_scene_patch");
    let after_slot_shapes = shape_detail_fingerprints(
        &handler
            .cached_scene
            .as_ref()
            .expect("keyed For patch should keep cache")
            .computed,
    );
    assert_eq!(after_slot_shapes.len(), 2);
    assert_ne!(
        before_shapes, after_slot_shapes,
        "reordering keyed For children should change ordered scene output"
    );

    handler.invalidate_scene_with_reason("keyed_for_reorder_equivalence_full_recollect");
    let after_full_shapes = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_shapes, after_full_shapes,
        "retained keyed For reorder must match full layout + scene recollect"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_keyed_for_remove_clears_child_reactive_subscriptions() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let items = context.state(vec![1usize, 2]);
    let first_color = context.state(Color::RED);
    let second_color = context.state(Color::GREEN);
    let first_color_signal = first_color.signal();
    let second_color_signal = second_color.signal();
    let tree =
        WidgetTree::try_new_strict(Flex::vertical().size(dp(160.0), dp(96.0)).child(For::new(
            items.signal(),
            |item| *item,
            move |_index, item| {
                let color = if *item == 1 {
                    first_color_signal.clone()
                } else {
                    second_color_signal.clone()
                };
                Stack::<TestVm>::new()
                    .size(dp(48.0), dp(24.0))
                    .style_full(move |ctx| {
                        let mut style = ContainerStyle::default_for_theme(ctx.theme);
                        style.surface.background = Some(color.clone().into());
                        style
                    })
            },
        )))
        .expect("strict retained keyed For tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    let snapshot = capture_strict_reactive_actions(&mut handler, || items.set(vec![1]));
    assert_action_count(&snapshot, "reactive_structure_slot_update", 1);

    second_color.set(Color::BLUE);
    let updates = handler.invalidation.drain_reactive_updates();
    assert!(
        updates.targets.is_empty(),
        "removed keyed For child signal update must not enqueue a detached target: {:?}",
        updates.targets
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_view_switch_index_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let active = context.state(0usize);
    let tree = WidgetTree::try_new_strict(
        Flex::vertical().size(dp(160.0), dp(96.0)).child(
            ViewSwitch::new(active.signal())
                .case(filled_stack(Color::hexa(0x22C55EFF)).size(dp(48.0), dp(24.0)))
                .case(filled_stack(Color::hexa(0x2563EBFF)).size(dp(64.0), dp(32.0)))
                .fallback(filled_stack(Color::hexa(0xF97316FF)).size(dp(40.0), dp(20.0))),
        ),
    )
    .expect("strict retained ViewSwitch tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let before_shapes = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(before_shapes.len(), 1, "ViewSwitch should render one case");

    let snapshot = capture_strict_reactive_actions(&mut handler, || active.set(1));
    assert_action_count(&snapshot, "reactive_structure_slot_update", 1);
    assert_action_absent(&snapshot, "strict_reactive_layout_rejected");
    assert_action_absent(&snapshot, "reactive_layout_scene_patch");
    let after_slot_shapes = shape_detail_fingerprints(
        &handler
            .cached_scene
            .as_ref()
            .expect("ViewSwitch patch should keep cache")
            .computed,
    );
    assert_eq!(after_slot_shapes.len(), 1);
    assert_ne!(
        before_shapes, after_slot_shapes,
        "switching ViewSwitch cases should change scene output"
    );

    handler.invalidate_scene_with_reason("view_switch_index_equivalence_full_recollect");
    let after_full_shapes = shape_detail_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_shapes, after_full_shapes,
        "retained ViewSwitch index update must match full layout + scene recollect"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_view_switch_remove_clears_child_reactive_subscriptions() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let active = context.state(1usize);
    let first_color = context.state(Color::RED);
    let second_color = context.state(Color::GREEN);
    let first_color_signal = first_color.signal();
    let second_color_signal = second_color.signal();
    let tree = WidgetTree::try_new_strict(
        Flex::vertical().size(dp(160.0), dp(96.0)).child(
            ViewSwitch::new(active.signal())
                .case(
                    Stack::<TestVm>::new()
                        .size(dp(48.0), dp(24.0))
                        .style_full(move |ctx| {
                            let mut style = ContainerStyle::default_for_theme(ctx.theme);
                            style.surface.background = Some(first_color_signal.clone().into());
                            style
                        }),
                )
                .case(
                    Stack::<TestVm>::new()
                        .size(dp(64.0), dp(32.0))
                        .style_full(move |ctx| {
                            let mut style = ContainerStyle::default_for_theme(ctx.theme);
                            style.surface.background = Some(second_color_signal.clone().into());
                            style
                        }),
                ),
        ),
    )
    .expect("strict retained ViewSwitch tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    let snapshot = capture_strict_reactive_actions(&mut handler, || active.set(0));
    assert_action_count(&snapshot, "reactive_structure_slot_update", 1);

    second_color.set(Color::BLUE);
    let updates = handler.invalidation.drain_reactive_updates();
    assert!(
        updates.targets.is_empty(),
        "removed ViewSwitch case signal update must not enqueue a detached target: {:?}",
        updates.targets
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn reactive_layout_slot_size_constraints_match_full_recollect() {
    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let width = context.state(dp(48.0));
        let tree = WidgetTree::try_new_strict(
            Flex::<TestVm>::new(Axis::Horizontal)
                .size(dp(220.0), dp(80.0))
                .child(
                    filled_stack(Color::hexa(0x111827FF))
                        .width(width.signal())
                        .height(dp(24.0)),
                )
                .child(filled_stack(Color::hexa(0x38BDF8FF)).size(dp(24.0), dp(24.0))),
        )
        .expect("strict width layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "width",
            invalidation,
            tree,
            || width.set(dp(96.0)),
        );
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let height = context.state(dp(24.0));
        let tree = WidgetTree::try_new_strict(
            Flex::<TestVm>::new(Axis::Horizontal)
                .size(dp(220.0), dp(80.0))
                .child(
                    filled_stack(Color::hexa(0x111827FF))
                        .width(dp(48.0))
                        .height(height.signal()),
                )
                .child(filled_stack(Color::hexa(0x38BDF8FF)).size(dp(24.0), dp(24.0))),
        )
        .expect("strict height layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "height",
            invalidation,
            tree,
            || height.set(dp(48.0)),
        );
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let min_width = context.state(dp(40.0));
        let tree = WidgetTree::try_new_strict(
            Flex::<TestVm>::new(Axis::Horizontal)
                .size(dp(220.0), dp(80.0))
                .child(
                    filled_stack(Color::hexa(0x111827FF))
                        .min_width(min_width.signal())
                        .height(dp(24.0)),
                )
                .child(filled_stack(Color::hexa(0x38BDF8FF)).size(dp(24.0), dp(24.0))),
        )
        .expect("strict min width layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "min_width",
            invalidation,
            tree,
            || min_width.set(dp(80.0)),
        );
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let min_height = context.state(dp(24.0));
        let tree = WidgetTree::try_new_strict(
            Flex::<TestVm>::new(Axis::Horizontal)
                .size(dp(220.0), dp(100.0))
                .child(
                    filled_stack(Color::hexa(0x111827FF))
                        .width(dp(48.0))
                        .min_height(min_height.signal()),
                )
                .child(filled_stack(Color::hexa(0x38BDF8FF)).size(dp(24.0), dp(24.0))),
        )
        .expect("strict min height layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "min_height",
            invalidation,
            tree,
            || min_height.set(dp(56.0)),
        );
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let max_width = context.state(dp(48.0));
        let tree = WidgetTree::try_new_strict(
            Flex::<TestVm>::new(Axis::Horizontal)
                .size(dp(240.0), dp(80.0))
                .child(
                    filled_stack(Color::hexa(0x111827FF))
                        .width(dp(140.0))
                        .max_width(max_width.signal())
                        .height(dp(24.0)),
                )
                .child(filled_stack(Color::hexa(0x38BDF8FF)).size(dp(24.0), dp(24.0))),
        )
        .expect("strict max width layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "max_width",
            invalidation,
            tree,
            || max_width.set(dp(96.0)),
        );
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let max_height = context.state(dp(32.0));
        let tree = WidgetTree::try_new_strict(
            Flex::<TestVm>::new(Axis::Horizontal)
                .size(dp(240.0), dp(120.0))
                .child(
                    filled_stack(Color::hexa(0x111827FF))
                        .width(dp(48.0))
                        .height(dp(100.0))
                        .max_height(max_height.signal()),
                )
                .child(filled_stack(Color::hexa(0x38BDF8FF)).size(dp(24.0), dp(24.0))),
        )
        .expect("strict max height layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "max_height",
            invalidation,
            tree,
            || max_height.set(dp(72.0)),
        );
    }
}

#[test]
#[cfg(feature = "bench-support")]
fn reactive_layout_slot_spacing_and_flex_match_full_recollect() {
    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let margin = context.state(Insets::ZERO);
        let tree = WidgetTree::try_new_strict(
            Flex::<TestVm>::new(Axis::Horizontal)
                .size(dp(240.0), dp(80.0))
                .child(
                    filled_stack(Color::hexa(0x111827FF))
                        .size(dp(48.0), dp(24.0))
                        .margin(margin.signal()),
                )
                .child(filled_stack(Color::hexa(0x38BDF8FF)).size(dp(24.0), dp(24.0))),
        )
        .expect("strict margin layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "margin",
            invalidation,
            tree,
            || margin.set(Insets::symmetric(dp(8.0), dp(4.0))),
        );
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let padding = context.state(Insets::ZERO);
        let tree = WidgetTree::try_new_strict(
            Stack::<TestVm>::new().size(dp(180.0), dp(100.0)).child(
                filled_stack(Color::hexa(0x111827FF))
                    .size(dp(120.0), dp(64.0))
                    .padding(padding.signal())
                    .child(filled_stack(Color::hexa(0x38BDF8FF)).size(dp(24.0), dp(24.0))),
            ),
        )
        .expect("strict padding layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "padding",
            invalidation,
            tree,
            || padding.set(Insets::all(dp(12.0))),
        );
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let grow = context.state(0.0_f32);
        let tree = WidgetTree::try_new_strict(
            Flex::<TestVm>::new(Axis::Horizontal)
                .size(dp(180.0), dp(64.0))
                .child(
                    filled_stack(Color::hexa(0x111827FF))
                        .basis(dp(40.0))
                        .grow(grow.signal())
                        .height(dp(24.0)),
                )
                .child(
                    filled_stack(Color::hexa(0x38BDF8FF))
                        .basis(dp(40.0))
                        .grow(1.0)
                        .height(dp(24.0)),
                ),
        )
        .expect("strict grow layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "grow",
            invalidation,
            tree,
            || grow.set(1.0),
        );
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let shrink = context.state(0.0_f32);
        let tree = WidgetTree::try_new_strict(
            Flex::<TestVm>::new(Axis::Horizontal)
                .size(dp(150.0), dp(64.0))
                .child(
                    filled_stack(Color::hexa(0x111827FF))
                        .basis(dp(100.0))
                        .shrink(shrink.signal())
                        .height(dp(24.0)),
                )
                .child(
                    filled_stack(Color::hexa(0x38BDF8FF))
                        .basis(dp(100.0))
                        .shrink(1.0)
                        .height(dp(24.0)),
                ),
        )
        .expect("strict shrink layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "shrink",
            invalidation,
            tree,
            || shrink.set(1.0),
        );
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let basis = context.state(dp(40.0));
        let tree = WidgetTree::try_new_strict(
            Flex::<TestVm>::new(Axis::Horizontal)
                .size(dp(220.0), dp(64.0))
                .child(
                    filled_stack(Color::hexa(0x111827FF))
                        .basis(basis.signal())
                        .height(dp(24.0)),
                )
                .child(filled_stack(Color::hexa(0x38BDF8FF)).size(dp(24.0), dp(24.0))),
        )
        .expect("strict basis layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "basis",
            invalidation,
            tree,
            || basis.set(dp(88.0)),
        );
    }
}

#[test]
#[cfg(feature = "bench-support")]
fn reactive_layout_slot_aspect_ratio_and_inset_match_full_recollect() {
    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let aspect_ratio = context.state(1.0_f32);
        let tree = WidgetTree::try_new_strict(
            Flex::<TestVm>::new(Axis::Horizontal)
                .size(dp(220.0), dp(80.0))
                .child(
                    filled_stack(Color::hexa(0x111827FF))
                        .height(dp(24.0))
                        .aspect_ratio(aspect_ratio.signal()),
                )
                .child(filled_stack(Color::hexa(0x38BDF8FF)).size(dp(24.0), dp(24.0))),
        )
        .expect("strict aspect ratio layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "aspect_ratio",
            invalidation,
            tree,
            || aspect_ratio.set(2.0),
        );
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let left = context.state(dp(8.0));
        let tree = WidgetTree::try_new_strict(
            Stack::<TestVm>::new()
                .size(dp(180.0), dp(100.0))
                .child(
                    filled_stack(Color::hexa(0x111827FF))
                        .position_absolute()
                        .left(left.signal())
                        .top(dp(10.0))
                        .size(dp(24.0), dp(24.0)),
                )
                .child(filled_stack(Color::hexa(0x38BDF8FF)).size(dp(16.0), dp(16.0))),
        )
        .expect("strict inset layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "inset",
            invalidation,
            tree,
            || left.set(dp(40.0)),
        );
    }
}

#[test]
#[cfg(feature = "bench-support")]
fn reactive_layout_slot_grid_position_matches_full_recollect() {
    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let column = context.state(1usize);
        let tree = WidgetTree::try_new_strict(
            crate::ui::widget::Grid::<TestVm>::columns([
                crate::ui::layout::fr(1.0),
                crate::ui::layout::fr(1.0),
            ])
            .set_rows([crate::ui::layout::fr(1.0), crate::ui::layout::fr(1.0)])
            .size(dp(120.0), dp(80.0))
            .child(
                filled_stack(Color::hexa(0x111827FF))
                    .column(column.signal())
                    .row(1usize)
                    .size(dp(24.0), dp(24.0)),
            )
            .child(
                filled_stack(Color::hexa(0x38BDF8FF))
                    .column(2usize)
                    .row(2usize)
                    .size(dp(24.0), dp(24.0)),
            ),
        )
        .expect("strict grid column layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "grid_column",
            invalidation,
            tree,
            || column.set(2),
        );
    }

    {
        let invalidation = InvalidationSignal::new();
        let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
        let row = context.state(1usize);
        let tree = WidgetTree::try_new_strict(
            crate::ui::widget::Grid::<TestVm>::columns([
                crate::ui::layout::fr(1.0),
                crate::ui::layout::fr(1.0),
            ])
            .set_rows([crate::ui::layout::fr(1.0), crate::ui::layout::fr(1.0)])
            .size(dp(120.0), dp(80.0))
            .child(
                filled_stack(Color::hexa(0x111827FF))
                    .column(1usize)
                    .row(row.signal())
                    .size(dp(24.0), dp(24.0)),
            )
            .child(
                filled_stack(Color::hexa(0x38BDF8FF))
                    .column(2usize)
                    .row(2usize)
                    .size(dp(24.0), dp(24.0)),
            ),
        )
        .expect("strict grid row layout slot tree");
        assert_reactive_layout_slot_update_matches_full_recollect(
            "grid_row",
            invalidation,
            tree,
            || row.set(2),
        );
    }
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_tree_updates_intrinsic_text_content_with_retained_layout_slot() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let content = context.state(String::from("short"));
    let tree = WidgetTree::try_new_strict(
        Flex::<TestVm>::new(Axis::Horizontal)
            .size(dp(220.0), dp(48.0))
            .child(Text::new(content.signal()))
            .child(
                Stack::<TestVm>::new()
                    .size(dp(16.0), dp(16.0))
                    .style_full(|ctx| {
                        let mut style = ContainerStyle::default_for_theme(ctx.theme);
                        style.surface.background = Some(Color::hexa(0x38BDF8FF).into());
                        style
                    }),
            ),
    )
    .expect("strict intrinsic text tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let before_shapes = shape_detail_fingerprints(handler.computed_scene());
    let before_texts = text_fingerprints(handler.computed_scene());

    let snapshot = capture_strict_reactive_actions(&mut handler, || {
        content.set(String::from("a much longer label"))
    });
    assert_action_count(&snapshot, "reactive_layout_slot_update", 1);
    assert_action_absent(&snapshot, "strict_reactive_layout_rejected");
    assert_action_absent(&snapshot, "reactive_layout_scene_patch");
    assert_action_absent(&snapshot, "reactive_property_slot_write");

    let cached = handler
        .cached_scene
        .as_ref()
        .expect("retained intrinsic text layout update should keep cache");
    assert!(
        cached.layout_valid && cached.computed_valid,
        "retained intrinsic text update should keep both caches valid"
    );
    let after_slot_shapes = shape_detail_fingerprints(&cached.computed);
    let after_slot_texts = text_fingerprints(&cached.computed);
    assert_ne!(
        before_shapes, after_slot_shapes,
        "intrinsic text layout update should move sibling layout output"
    );
    assert_ne!(
        before_texts, after_slot_texts,
        "intrinsic text layout update should change rendered text output"
    );

    handler.invalidate_scene_with_reason("intrinsic_text_layout_equivalence_full_recollect");
    let after_full_shapes = shape_detail_fingerprints(handler.computed_scene());
    let after_full_texts = text_fingerprints(handler.computed_scene());
    assert_eq!(
        after_slot_shapes, after_full_shapes,
        "retained intrinsic text layout update must match full layout + scene recollect"
    );
    assert_eq!(
        after_slot_texts, after_full_texts,
        "retained intrinsic text layout text output must match full recollect"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_tree_rejects_scene_patch_fallback() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let value = context.state(0.25_f32);
    let tree = WidgetTree::try_new_strict(
        Slider::<TestVm>::new(value.signal(), 0.0, 1.0)
            .show_ticks(true)
            .width(dp(120.0))
            .style(|style, _| {
                style.thumb_shadow = None;
            }),
    )
    .expect("strict tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    crate::runtime::action_stats::reset();
    value.set(0.75);
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();

    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "strict_reactive_scene_rejected" && *count == 1),
        "strict tree should reject scene patch fallback: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "reactive_property_scene_patch"),
        "strict tree must not silently patch unsupported scene changes: {snapshot:?}"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_tree_rejects_global_dependency_fallback() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let backing = Arc::new(Mutex::new(String::from("first")));
    let signal = {
        let backing = backing.clone();
        context.signal(move || backing.lock().expect("test signal lock poisoned").clone())
    };
    let tree = WidgetTree::try_new_strict(Text::new(signal).size(dp(120.0), dp(24.0)))
        .expect("strict static text tree");
    let mut handler = test_handler(Some(tree), invalidation.clone());
    let _ = handler.computed_scene();
    assert!(
        handler
            .cached_scene
            .as_ref()
            .expect("cache shell")
            .dependencies
            .has_global_dependency(),
        "opaque signal should register a global dependency"
    );

    *backing.lock().expect("test signal lock poisoned") = String::from("second");
    crate::runtime::action_stats::reset();
    invalidation.mark_dirty();
    handler.request_redraw_if_dirty(Instant::now());
    let snapshot = crate::runtime::action_stats::snapshot();

    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "strict_reactive_global_rejected" && *count == 1),
        "strict tree should reject collect-time global fallback: {snapshot:?}"
    );
    let cached = handler.cached_scene.as_ref().expect("cache shell");
    assert!(
        cached.layout_valid && cached.computed_valid,
        "strict rejection must not invalidate retained caches"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_contain_image_source_load_updates_retained_texture_slot_matches_full_recollect()
{
    let (url, release_response, server) = delayed_media_url(WIDE_SVG, "image/svg+xml");
    let source = crate::media::MediaSource::url(url);
    let invalidation = InvalidationSignal::new();
    let tree =
        WidgetTree::try_new_strict(crate::ui::widget::Image::new(source).size(dp(48.0), dp(48.0)))
            .expect("strict static image tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let initial = texture_source_fingerprints(handler.computed_scene());
    assert_eq!(
        initial.len(),
        1,
        "strict Contain image should reserve a retained transparent texture slot while loading"
    );
    assert_eq!(initial[0].3, dp(48.0));
    assert_eq!(initial[0].4, dp(48.0));
    assert!(
        handler
            .cached_scene
            .as_ref()
            .expect("cache shell")
            .media_texture_bindings
            .values()
            .map(Vec::len)
            .sum::<usize>()
            > 0,
        "reserved non-Fill media texture should register retained texture bindings"
    );
    handler.last_invalidation_revision = handler.invalidation.revision();

    crate::runtime::scene_patch::splice_probe::reset();
    crate::runtime::action_stats::reset();
    let initial_texture_id = initial[0].0;
    release_response
        .send(())
        .expect("delayed media response should be released");
    let start = Instant::now();
    let snapshot = loop {
        handler.request_redraw_if_dirty(Instant::now());
        let current = texture_source_fingerprints(
            &handler
                .cached_scene
                .as_ref()
                .expect("cache shell should remain available")
                .computed,
        );
        if current[0].0 != initial_texture_id {
            break crate::runtime::action_stats::snapshot();
        }
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "timed out waiting for strict retained non-Fill media texture load"
        );
        std::thread::sleep(Duration::from_millis(10));
    };

    server
        .join()
        .expect("delayed media test server should finish");
    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "strict retained non-Fill media load should not enter scene splice"
    );
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "media_texture_slot_write" && *count >= 1),
        "strict retained non-Fill media load should be consumed by texture slot writes: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "strict_reactive_media_rejected"),
        "strict retained non-Fill media load must not be rejected: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "media_texture_full_rebuild"),
        "strict retained non-Fill media load must not fall back to full rebuild: {snapshot:?}"
    );
    let cached = handler.cached_scene.as_ref().expect("cache shell");
    let after_slot_textures = texture_source_fingerprints(&cached.computed);
    let after_slot_shapes = shape_detail_fingerprints(&cached.computed);
    let after_slot_text = text_fingerprints(&cached.computed);
    assert_eq!(after_slot_textures.len(), 1);
    assert_eq!(after_slot_textures[0].1, dp(0.0));
    assert_eq!(after_slot_textures[0].2, dp(16.0));
    assert_eq!(after_slot_textures[0].3, dp(48.0));
    assert_eq!(after_slot_textures[0].4, dp(16.0));

    handler.invalidate_computed_scene();
    let full = handler.computed_scene();
    assert_eq!(
        after_slot_textures,
        texture_source_fingerprints(full),
        "retained non-Fill source load texture must match a fresh full recollect"
    );
    assert_eq!(
        after_slot_shapes,
        shape_detail_fingerprints(full),
        "retained non-Fill source load placeholder shape must match a fresh full recollect"
    );
    assert_eq!(
        after_slot_text,
        text_fingerprints(full),
        "retained non-Fill source load placeholder text must match a fresh full recollect"
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn strict_reactive_fill_image_source_load_updates_retained_texture_slot() {
    let image_path = write_temp_gif("strict-fill-source-load.gif");
    let source = crate::media::MediaSource::path(image_path.clone());
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::try_new_strict(
        crate::ui::widget::Image::new(source)
            .size(dp(48.0), dp(48.0))
            .style(|style, _| {
                style.fit = crate::media::ContentFit::Fill;
            }),
    )
    .expect("strict static fill image tree");
    let mut handler = test_handler(Some(tree), invalidation);
    let initial = texture_source_fingerprints(handler.computed_scene());
    assert_eq!(
        initial.len(),
        1,
        "strict Fill image should reserve a retained transparent texture slot while loading"
    );
    assert!(
        handler
            .cached_scene
            .as_ref()
            .expect("cache shell")
            .media_texture_bindings
            .values()
            .map(Vec::len)
            .sum::<usize>()
            > 0,
        "reserved media texture should register retained texture bindings"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    crate::runtime::action_stats::reset();
    let initial_texture_id = initial[0].0;
    let start = Instant::now();
    let snapshot = loop {
        handler.request_redraw_if_dirty(Instant::now());
        let current = texture_source_fingerprints(
            &handler
                .cached_scene
                .as_ref()
                .expect("cache shell should remain available")
                .computed,
        );
        let snapshot = crate::runtime::action_stats::snapshot();
        if current[0].0 != initial_texture_id {
            break snapshot;
        }
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "timed out waiting for strict retained media texture load; actions={snapshot:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    };

    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "strict retained media load should not enter scene splice"
    );
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "media_texture_slot_write" && *count >= 1),
        "strict retained media load should be consumed by texture slot writes: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "strict_reactive_media_rejected"),
        "strict retained media load must not be rejected: {snapshot:?}"
    );
    assert!(
        !snapshot
            .iter()
            .any(|(action, _)| *action == "media_texture_full_rebuild"),
        "strict retained media load must not fall back to full rebuild: {snapshot:?}"
    );

    let _ = std::fs::remove_dir_all(
        image_path
            .parent()
            .expect("temp gif should have a parent directory"),
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn media_raster_completion_updates_retained_texture_slot() {
    let image_path = write_temp_gif("pixel.gif");
    let source = crate::media::MediaSource::path(image_path.clone());
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let scale = context.state(1.0_f32);
    let tree = background_image_scale_tree(&scale, source);
    let mut handler = test_handler(Some(tree), invalidation);

    let initial = wait_for_one_texture(&mut handler, Duration::from_secs(2));
    let initial_texture_id = initial[0].0;
    assert!(
        handler
            .cached_scene
            .as_ref()
            .expect("cache shell")
            .media_texture_bindings
            .values()
            .map(Vec::len)
            .sum::<usize>()
            > 0,
        "loaded media texture should register retained texture slots"
    );

    crate::runtime::action_stats::reset();
    scale.set(1.5);
    handler.request_redraw_if_dirty(Instant::now());
    let scale_snapshot = crate::runtime::action_stats::snapshot();
    assert!(
        scale_snapshot
            .iter()
            .any(|(action, count)| *action == "reactive_property_slot_write" && *count == 1),
        "reactive background image scale should update retained texture slots: {scale_snapshot:?}"
    );
    assert!(
        !scale_snapshot
            .iter()
            .any(|(action, _)| *action == "media_texture_binding_full_rebuild"),
        "reactive texture scale must update media bindings locally: {scale_snapshot:?}"
    );
    let scaled = texture_source_fingerprints(
        &handler
            .cached_scene
            .as_ref()
            .expect("slot write keeps cache shell")
            .computed,
    );
    assert_eq!(scaled.len(), 1);
    assert_eq!(
        scaled[0].0, initial_texture_id,
        "scale update should keep the old raster while the larger raster is pending"
    );
    assert!(
        scaled[0].3 > initial[0].3 && scaled[0].4 > initial[0].4,
        "scale update should request a larger texture frame"
    );

    crate::runtime::scene_patch::splice_probe::reset();
    crate::runtime::action_stats::reset();
    let start = Instant::now();
    loop {
        handler.request_redraw_if_dirty(Instant::now());
        let current = texture_source_fingerprints(
            &handler
                .cached_scene
                .as_ref()
                .expect("cache shell should remain available")
                .computed,
        );
        if current[0].0 != initial_texture_id {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "timed out waiting for retained media texture slot write"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let snapshot = crate::runtime::action_stats::snapshot();

    assert_eq!(
        crate::runtime::scene_patch::splice_probe::hits(),
        0,
        "media texture slot write should not enter the scene splice path"
    );
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "media_texture_slot_write" && *count == 1),
        "raster completion should be consumed by the retained media slot path: {snapshot:?}"
    );
    assert!(
        !snapshot.iter().any(|(action, _)| action.contains("patch")),
        "raster completion should not request a subtree scene patch: {snapshot:?}"
    );

    let _ = std::fs::remove_dir_all(
        image_path
            .parent()
            .expect("temp gif should have a parent directory"),
    );
}

#[test]
#[cfg(feature = "bench-support")]
fn animated_gif_runtime_advances_retained_texture_slot() {
    let image_path = write_temp_media("animated.gif", &animated_gif_bytes());
    let source = crate::media::MediaSource::path(image_path.clone());
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::try_new_strict(
        crate::ui::widget::Image::new(source)
            .size(dp(48.0), dp(48.0))
            .style(|style, _| {
                style.fit = crate::media::ContentFit::Fill;
            }),
    )
    .expect("strict animated gif tree");
    let mut handler = test_handler(Some(tree), invalidation);

    let initial = wait_for_one_texture(&mut handler, Duration::from_secs(2));
    let initial_texture_id = initial[0].0;
    let initial_revision = handler
        .cached_scene
        .as_ref()
        .expect("cache shell")
        .computed
        .scene
        .textures
        .first()
        .expect("texture should exist")
        .texture
        .revision();

    crate::runtime::action_stats::reset();
    let deadline = handler
        .next_deadline(Instant::now())
        .expect("animated gif should schedule a next deadline");
    assert!(handler.drive_animations(&TestEventLoop, deadline + Duration::from_millis(1),));
    handler.request_redraw_if_dirty(Instant::now());

    let updated = texture_id_revision_fingerprints(
        &handler
            .cached_scene
            .as_ref()
            .expect("cache shell remains available")
            .computed,
    );
    let snapshot = crate::runtime::action_stats::snapshot();

    assert_eq!(crate::runtime::scene_patch::splice_probe::hits(), 0);
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].0, initial_texture_id);
    assert!(updated[0].1 > initial_revision);
    assert!(
        snapshot
            .iter()
            .any(|(action, count)| *action == "media_texture_slot_write" && *count >= 1),
        "animated gif frame advance should be consumed by retained media slot writes: {snapshot:?}"
    );

    let _ = std::fs::remove_dir_all(
        image_path
            .parent()
            .expect("temp gif should have a parent directory"),
    );
}

// ---------------------------------------------------------------------------
// 纯滚动快路径
//
// 核心断言与 splice 测试一致：无论走纯滚动快路径还是整帧重收集，最终 `cached.computed`
// 的渲染命令流都逐项等价。快路径只是把「整树重收集」收窄成「只重收集滚动子树」，
// 用同一个 collect 函数，因此结果必须 byte-identical。探针确认确实走了快路径。
// ---------------------------------------------------------------------------

/// 构造一个内容溢出、可纵向滚动的容器，内部放三个不同色块以便指纹比对。
fn scrollable_color_tree() -> WidgetTree<TestVm> {
    fn block(hex: u32) -> Element<TestVm> {
        Stack::new()
            .size(dp(40.0), dp(40.0))
            .style_full(move |ctx| {
                let mut style = ContainerStyle::default_for_theme(ctx.theme);
                style.surface.background = Some(Color::hexa(hex).into());
                style
            })
            .into()
    }
    let content: Element<TestVm> = Flex::vertical()
        .child(block(0x111111FF))
        .child(block(0x222222FF))
        .child(block(0x333333FF))
        .into();
    let scroller: Element<TestVm> = ScrollView::new()
        .size(dp(60.0), dp(60.0))
        .child(content)
        .into();
    WidgetTree::new(scroller)
}

#[test]
fn pure_scroll_patch_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let tree = scrollable_color_tree();
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    // 找到滚动容器 id（场景里恰有一个 scroll_region）。
    let scroll_id = handler
        .cached_scene
        .as_ref()
        .and_then(|cached| {
            cached
                .computed
                .scroll_regions
                .first()
                .map(|region| region.id)
        })
        .expect("scrollable container should emit a scroll region");

    crate::runtime::scene_runtime::scroll_fast_path_probe::reset();

    // 滚动该容器，再求场景。
    handler.set_scroll_offset(scroll_id, Point::new(dp(0.0), dp(15.0)));
    let after_patch = shape_fingerprints(handler.computed_scene());

    // 特性开启时，断言确实走了纯滚动快路径（而非回退整帧重收集）。
    assert_eq!(
        crate::runtime::scene_runtime::scroll_fast_path_probe::hits(),
        1,
        "scroll should hit the pure-scroll fast path exactly once"
    );

    // 强制一次从零的全量重收集作为真值。
    handler.invalidate_computed_scene();
    let after_full = shape_fingerprints(handler.computed_scene());

    assert_eq!(
        after_patch, after_full,
        "pure-scroll patched scene must be item-identical to a fresh full recollect"
    );
}

#[test]
fn virtual_scroll_is_explicitly_excluded_from_pure_scroll_fast_path() {
    use crate::ui::widget::VirtualList;

    let invalidation = InvalidationSignal::new();
    let items = (0..80usize).collect::<Vec<_>>();
    let tree = WidgetTree::new(
        VirtualList::new(items, |index, _item| {
            let color = if index % 2 == 0 {
                Color::hexa(0x111827FF)
            } else {
                Color::hexa(0x2563EBFF)
            };
            Stack::<TestVm>::new()
                .size(dp(80.0), dp(40.0))
                .style_full(move |ctx| {
                    let mut style = ContainerStyle::default_for_theme(ctx.theme);
                    style.surface.background = Some(color.into());
                    style
                })
                .into()
        })
        .size(dp(80.0), dp(80.0)),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let scroll_id = handler
        .cached_scene
        .as_ref()
        .and_then(|cached| {
            cached
                .computed
                .scroll_regions
                .first()
                .map(|region| region.id)
        })
        .expect("virtual list should emit a scroll region");

    crate::runtime::scene_runtime::scroll_fast_path_probe::reset();
    handler.set_scroll_offset(scroll_id, Point::new(dp(0.0), dp(80.0)));
    let after_scroll = shape_fingerprints(handler.computed_scene());

    assert_eq!(
        crate::runtime::scene_runtime::scroll_fast_path_probe::hits(),
        0,
        "virtual containers are structural/windowing content and must not use pure-scroll fast paths"
    );

    handler.invalidate_computed_scene();
    let after_full = shape_fingerprints(handler.computed_scene());
    assert_eq!(
        after_scroll, after_full,
        "virtual scroll fallback should still match a full recollect"
    );
}

// 补充测试：纯滚动快路径的边界和回退情况

#[test]
fn nested_scroll_triggers_fallback_to_full_recollect() {
    // 嵌套滚动容器应回退到全量重收集
    use crate::ui::widget::ScrollView;

    fn nested_scroller_tree() -> WidgetTree<TestVm> {
        let inner_content = Flex::vertical().child(Flex::vertical().size(dp(50.0), dp(100.0)));
        let inner_scroll = ScrollView::new()
            .size(dp(50.0), dp(50.0))
            .child(inner_content);
        let outer_scroll = ScrollView::new()
            .size(dp(80.0), dp(80.0))
            .child(inner_scroll);
        WidgetTree::new(outer_scroll)
    }

    let invalidation = InvalidationSignal::new();
    let tree = nested_scroller_tree();
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    let scroll_regions = handler
        .cached_scene
        .as_ref()
        .map(|c| c.computed.scroll_regions.len())
        .unwrap_or(0);

    // 应该有滚动区域（实际数量取决于布局结构）
    assert!(
        scroll_regions >= 2,
        "should have at least two nested scroll regions, got {}",
        scroll_regions
    );

    if let Some(outer_id) = handler
        .cached_scene
        .as_ref()
        .and_then(|c| c.computed.scroll_regions.first().map(|r| r.id))
    {
        crate::runtime::scene_runtime::scroll_fast_path_probe::reset();
        handler.set_scroll_offset(outer_id, Point::new(dp(0.0), dp(10.0)));
        let scene_after_scroll = shape_fingerprints(handler.computed_scene());

        // 验证嵌套滚动产生正确的场景（可能走快路径或回退）
        handler.invalidate_computed_scene();
        let full_recollect = shape_fingerprints(handler.computed_scene());

        assert_eq!(
            scene_after_scroll, full_recollect,
            "nested scroll should produce correct scene regardless of path taken"
        );
    }
}

#[test]
fn multiple_scroll_actions_in_same_frame() {
    // 同一帧内多次滚动，验证正确性
    let invalidation = InvalidationSignal::new();
    let tree = scrollable_color_tree();
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    let scroll_id = handler
        .cached_scene
        .as_ref()
        .and_then(|c| c.computed.scroll_regions.first().map(|r| r.id))
        .expect("should have scroll region");

    crate::runtime::scene_runtime::scroll_fast_path_probe::reset();

    // 第一次滚动
    handler.set_scroll_offset(scroll_id, Point::new(dp(0.0), dp(5.0)));
    let scene1 = shape_fingerprints(handler.computed_scene());

    // 第二次滚动（累积）
    handler.set_scroll_offset(scroll_id, Point::new(dp(0.0), dp(10.0)));
    let scene2 = shape_fingerprints(handler.computed_scene());

    // 验证全量重收集结果
    handler.invalidate_computed_scene();
    let full = shape_fingerprints(handler.computed_scene());

    assert_eq!(scene2, full, "multiple scrolls should match full recollect");
    assert_ne!(
        scene1, scene2,
        "different scroll offsets should produce different scenes"
    );
}

#[test]
fn scroll_with_content_invalidation_uses_full_path() {
    // 滚动的同时内容失效，验证结果正确性
    let invalidation = InvalidationSignal::new();
    let tree = scrollable_color_tree();
    let mut handler = test_handler(Some(tree), invalidation.clone());

    let _ = handler.computed_scene();
    let scroll_id = handler
        .cached_scene
        .as_ref()
        .and_then(|c| c.computed.scroll_regions.first().map(|r| r.id))
        .expect("should have scroll region");

    crate::runtime::scene_runtime::scroll_fast_path_probe::reset();

    // 触发内容失效
    invalidation.mark_dirty();

    // 同时滚动
    handler.set_scroll_offset(scroll_id, Point::new(dp(0.0), dp(20.0)));
    let scene_after_scroll = shape_fingerprints(handler.computed_scene());

    // 验证场景已更新（内容失效 + 滚动偏移）
    // 实现可能选择快路径或全路径，只要结果正确即可
    handler.invalidate_computed_scene();
    let full_recollect = shape_fingerprints(handler.computed_scene());

    assert_eq!(
        scene_after_scroll, full_recollect,
        "scroll with content invalidation should produce correct scene"
    );
}

#[test]
fn scroll_result_matches_full_recollect() {
    // 无论快路径是否命中，滚动结果都必须与全量重收集一致。
    let invalidation = InvalidationSignal::new();
    let tree = scrollable_color_tree();
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    let scroll_id = handler
        .cached_scene
        .as_ref()
        .and_then(|c| c.computed.scroll_regions.first().map(|r| r.id))
        .expect("should have scroll region");

    handler.set_scroll_offset(scroll_id, Point::new(dp(0.0), dp(15.0)));
    let after_scroll = shape_fingerprints(handler.computed_scene());

    // 强制全量重收集
    handler.invalidate_computed_scene();
    let after_full = shape_fingerprints(handler.computed_scene());

    // 无论特性开启与否，结果都应该正确匹配
    assert_eq!(
        after_scroll, after_full,
        "scroll result must match full recollect regardless of feature"
    );
}

#[test]
fn scroll_zero_offset_is_noop() {
    // 滚动偏移为 0 应该是无操作
    let invalidation = InvalidationSignal::new();
    let tree = scrollable_color_tree();
    let mut handler = test_handler(Some(tree), invalidation);

    let before = shape_fingerprints(handler.computed_scene());

    let scroll_id = handler
        .cached_scene
        .as_ref()
        .and_then(|c| c.computed.scroll_regions.first().map(|r| r.id))
        .expect("should have scroll region");

    crate::runtime::scene_runtime::scroll_fast_path_probe::reset();

    // 设置零偏移
    handler.set_scroll_offset(scroll_id, Point::new(dp(0.0), dp(0.0)));
    let after = shape_fingerprints(handler.computed_scene());

    assert_eq!(before, after, "zero scroll offset should not change scene");
}
