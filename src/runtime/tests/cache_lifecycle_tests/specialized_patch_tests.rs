use super::*;

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
    let tree = WidgetTree::new(
        Stack::<TestVm>::new()
            .child(visible.signal().map({
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
fn opaque_signal_dirty_update_falls_back_to_full_scene_invalidation() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let backing = Arc::new(Mutex::new(String::from("first")));
    let signal = {
        let backing = backing.clone();
        context.signal(move || backing.lock().expect("test signal lock poisoned").clone())
    };
    let tree = WidgetTree::new(Text::new(signal));
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
    let tree = WidgetTree::new(Stack::<TestVm>::new().child(visible.signal().map({
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
    })));
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
// Phase 1 · 场景命令原地拼接（fine-grained-splice）
//
// 这些测试的核心断言不是「splice 是否被走到」，而是「无论走 splice 还是 recompose，
// 最终 `cached.computed` 的渲染命令流（顺序 + 内容）都与一次从零的全量重收集逐项等价」。
// 这把 Phase 1 的正确性红线（z-order 不变、只有目标区间变化、失败能干净回退）钉死。
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

/// 构造「容器 > [兄弟0, 目标surface, 兄弟2]」的树，目标的背景色受 `state` 驱动。
/// 改色只改一个 shape 的颜色、不增删命令 —— 命中 splice 快路径的典型场景。
fn nested_color_tree(color_state: &State<Color>) -> WidgetTree<TestVm> {
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
    WidgetTree::new(root)
}

#[test]
fn splice_color_change_matches_full_recollect() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let color = context.state(Color::hexa(0xFF0000FF));
    let tree = nested_color_tree(&color);
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    let before = shape_fingerprints(handler.computed_scene());

    // 改色 → 走失效决策（命中 scene_subtree_patch，内部尝试 splice）。
    #[cfg(test)]
    crate::runtime::scene_patch::splice_probe::reset();
    color.set(Color::hexa(0x00FF00FF));
    handler.request_redraw_if_dirty(Instant::now());
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
    let tree = nested_color_tree(&color);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();

    for hex in [0x00FF00FF_u32, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF] {
        color.set(Color::hexa(hex));
        handler.request_redraw_if_dirty(Instant::now());
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
    let tree = nested_color_tree(&color);
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    let before = shape_fingerprints(handler.computed_scene());

    color.set(Color::hexa(0x00FF00FF));
    handler.request_redraw_if_dirty(Instant::now());
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
#[cfg(feature = "bench-support")]
fn action_stats_records_scene_subtree_patch_for_color_change() {
    // Phase 0 度量护栏：改一个深层叶子的颜色，失效决策应命中 `scene_subtree_patch`，
    // 且 action_stats 计数器把该命中记一次（用于单属性更新的命中分布基线）。
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
        vec![("scene_subtree_patch", 1)],
        "single deep-leaf color change should record exactly one scene_subtree_patch hit"
    );
}

// ---------------------------------------------------------------------------
// Phase 4 · 纯滚动快路径（transform-only-scroll）
//
// 核心断言与 Phase 1 一致：无论走纯滚动快路径还是整帧重收集，最终 `cached.computed`
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

    #[cfg(feature = "transform-only-scroll")]
    crate::runtime::scene_runtime::scroll_fast_path_probe::reset();

    // 滚动该容器，再求场景。
    handler.set_scroll_offset(scroll_id, Point::new(dp(0.0), dp(15.0)));
    let after_patch = shape_fingerprints(handler.computed_scene());

    // 特性开启时，断言确实走了纯滚动快路径（而非回退整帧重收集）。
    #[cfg(feature = "transform-only-scroll")]
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
