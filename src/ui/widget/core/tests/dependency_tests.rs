use super::*;
use crate::ui::widget::r#virtual::{VirtualCacheState, VirtualViewportHint};

#[test]
fn text_signal_records_layout_and_scene_dependencies() {
    let ctx = test_context();
    let content = ctx.state(String::from("tracked"));
    let text: Element<()> = Text::new(content.signal()).into();
    let widget_id = text.id;
    let tree = WidgetTree::new(text);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let layout = tree.build_scene_layout(
        &font_manager,
        &Theme::default(),
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(layout.dependencies().contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Layout,
        property: Some(crate::foundation::binding::PropertySlot::TextContent),
    }));
    assert!(!layout.dependencies().contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Layout,
        property: None,
    }));

    let computed = tree.collect_scene_from_layout(
        &font_manager,
        &layout,
        &Theme::default(),
        &media,
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(computed.dependencies.contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Scene,
        property: Some(crate::foundation::binding::PropertySlot::TextContent),
    }));
}

#[test]
fn background_blur_signal_records_property_scene_dependency() {
    let ctx = test_context();
    let blur = ctx.state(dp(8.0));
    let blur_signal = blur.signal();
    let element: Element<()> = Stack::new()
        .size(dp(48.0), dp(48.0))
        .style_full(move |ctx| {
            let mut style = ContainerStyle::default_for_theme(ctx.theme);
            style.surface.background_blur = blur_signal.clone().into();
            style
        })
        .into();
    let widget_id = element.id;
    let tree = WidgetTree::new(element);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let layout = tree.build_scene_layout(
        &font_manager,
        &Theme::default(),
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    let computed = tree.collect_scene_from_layout(
        &font_manager,
        &layout,
        &Theme::default(),
        &media,
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(computed.dependencies.contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Scene,
        property: Some(crate::foundation::binding::PropertySlot::BackgroundBlur),
    }));
    assert!(!computed.dependencies.contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Scene,
        property: None,
    }));
}

#[test]
fn dynamic_children_signal_records_structure_dependency() {
    let ctx = test_context();
    let show = ctx.state(true);
    let container: Element<()> = Stack::new()
        .dynamic_child(show.signal().map_unchecked(|show| {
            if show {
                Text::new("shown")
            } else {
                Text::new("hidden")
            }
        }))
        .into();
    let widget_id = container.id;
    let tree = WidgetTree::new_legacy(container);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let layout = tree.build_scene_layout(
        &font_manager,
        &Theme::default(),
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(layout.dependencies().contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Structure,
        property: None,
    }));
}

#[test]
fn layout_width_signal_records_property_layout_dependency() {
    let ctx = test_context();
    let width = ctx.state(dp(48.0));
    let element: Element<()> = Stack::new().width(width.signal()).height(dp(24.0)).into();
    let widget_id = element.id;
    let tree = WidgetTree::new(element);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let layout = tree.build_scene_layout(
        &font_manager,
        &Theme::default(),
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(layout.dependencies().contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Layout,
        property: Some(crate::foundation::binding::PropertySlot::Width),
    }));
    assert!(!layout.dependencies().contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Layout,
        property: None,
    }));
}

#[test]
fn keyed_dynamic_children_reuse_widget_ids_across_reorder_patch() {
    let ctx = test_context();
    let reversed = ctx.state(false);
    let container: Element<()> = Stack::<()>::new()
        .dynamic_child(reversed.signal().map_unchecked(|reversed| {
            if reversed {
                vec![
                    Element::from(Text::new("second").key("second")),
                    Element::from(Text::new("first").key("first")),
                ]
            } else {
                vec![
                    Element::from(Text::new("first").key("first")),
                    Element::from(Text::new("second").key("second")),
                ]
            }
        }))
        .into();
    let container_id = container.id;
    let tree = WidgetTree::new_legacy(container);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let viewport = Rect::new(0.0, 0.0, 200.0, 120.0);
    let mut animations = AnimationEngine::default();

    let mut layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );

    let initial_ids = match &layout.resolved_root.kind {
        ResolvedWidgetKind::Container { children, .. } => {
            children.iter().map(|child| child.id).collect::<Vec<_>>()
        }
        _ => panic!("stack root should resolve to a container"),
    };

    reversed.set(true);
    let removed = layout
        .patch_layout_roots(
            &[container_id],
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
            Instant::now(),
        )
        .expect("keyed reorder should patch successfully");

    assert!(removed.is_empty());
    let reordered_ids = match &layout.resolved_root.kind {
        ResolvedWidgetKind::Container { children, .. } => {
            children.iter().map(|child| child.id).collect::<Vec<_>>()
        }
        _ => panic!("stack root should remain a container"),
    };
    assert_eq!(reordered_ids, vec![initial_ids[1], initial_ids[0]]);
}

#[test]
fn strict_keyed_for_children_reuse_widget_ids_across_reorder_patch() {
    use crate::ui::widget::For;

    let ctx = test_context();
    let items = ctx.state(vec![1usize, 2]);
    let container: Element<()> = Stack::<()>::new()
        .child(For::new(
            items.signal(),
            |item| *item,
            |_index, item| Text::new(format!("item {item}")),
        ))
        .into();
    let container_id = container.id;
    let tree = WidgetTree::try_new_strict(container).expect("strict keyed For tree");
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let viewport = Rect::new(0.0, 0.0, 200.0, 120.0);
    let mut animations = AnimationEngine::default();

    let mut layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );

    let initial_ids = match &layout.resolved_root.kind {
        ResolvedWidgetKind::Container { children, .. } => {
            children.iter().map(|child| child.id).collect::<Vec<_>>()
        }
        _ => panic!("stack root should resolve to a container"),
    };

    items.set(vec![2, 1]);
    let removed = layout
        .patch_layout_roots(
            &[container_id],
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
            Instant::now(),
        )
        .expect("keyed For reorder should patch successfully");

    assert!(removed.is_empty());
    let reordered_ids = match &layout.resolved_root.kind {
        ResolvedWidgetKind::Container { children, .. } => {
            children.iter().map(|child| child.id).collect::<Vec<_>>()
        }
        _ => panic!("stack root should remain a container"),
    };
    assert_eq!(reordered_ids, vec![initial_ids[1], initial_ids[0]]);
}

#[test]
fn canvas_items_signal_records_layout_and_scene_dependencies() {
    let ctx = test_context();
    let expanded = ctx.state(false);
    let canvas: Element<()> = Canvas::new(expanded.signal().map(|expanded| {
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
    }))
    .into();
    let widget_id = canvas.id;
    let tree = WidgetTree::new(canvas);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let layout = tree.build_scene_layout(
        &font_manager,
        &Theme::default(),
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(!layout.dependencies().has_global_dependency());
    assert!(layout.dependencies().contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Layout,
        property: None,
    }));

    let computed = tree.collect_scene_from_layout(
        &font_manager,
        &layout,
        &Theme::default(),
        &media,
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(!computed.dependencies.has_global_dependency());
    assert!(computed.dependencies.contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Scene,
        property: None,
    }));
}

#[test]
fn multiline_textarea_layout_is_content_independent() {
    let ctx = test_context();
    let auto_wrap = ctx.state(true);
    let textarea: Element<()> = Textarea::new("tracked text")
        .auto_wrap(auto_wrap.signal())
        .into();
    let tree = WidgetTree::new(textarea);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let layout = tree.build_scene_layout(
        &font_manager,
        &Theme::default(),
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(!layout.dependencies().has_global_dependency());
    assert_eq!(layout.dependencies().dependency_count(), 0);
}

#[test]
fn textarea_non_focused_render_reuses_stable_layout_snapshot() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let content = "line 0\nline 1\nline 2";
    let textarea: Element<()> = Textarea::new(content).height(dp(52.0)).into();
    let widget_id = textarea.id;
    let tree = WidgetTree::new(textarea);
    let viewport = Rect::new(0.0, 0.0, 220.0, 52.0);
    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );

    let baseline = tree.collect_scene_from_layout_with_focus_value(
        &font_manager,
        &layout,
        &theme,
        &media,
        &mut animations,
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
        false,
    );
    let baseline_region = baseline
        .scroll_regions
        .iter()
        .find(|region| region.id == widget_id)
        .expect("textarea scroll region should exist");

    let style = TextareaStyle::default_for_theme(&theme);
    let text = super::text_with_typography(content, &style.text_style);
    let (font_size, line_height, letter_spacing) =
        resolved_text_metrics(&text, &theme, UnitContext::default());
    let request = TextFontRequest {
        preferred_font: text.font_family.as_deref().or(theme
            .typography
            .body
            .font_family
            .as_deref()),
        weight: text.font_weight.unwrap_or(theme.typography.body.weight),
    };
    let alternate_layout = font_manager.measure_text_layout(
        content,
        request,
        font_size,
        line_height * 2.0,
        letter_spacing,
    );
    let overrides = HashMap::from([(
        widget_id,
        super::TextInputLayoutOverride {
            revision: 1,
            text: content,
            layout: &alternate_layout,
        },
    )]);

    let overridden = tree.collect_scene_from_layout_with_focus_value(
        &font_manager,
        &layout,
        &theme,
        &media,
        &mut animations,
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
        Some(&overrides),
        None,
        None,
        false,
    );
    let overridden_region = overridden
        .scroll_regions
        .iter()
        .find(|region| region.id == widget_id)
        .expect("textarea scroll region should exist");

    assert!(overridden_region.content_bounds.height > baseline_region.content_bounds.height);
    assert_eq!(
        overridden_region.content_bounds.height.get(),
        alternate_layout
            .height
            .max(overridden_region.content_viewport.height.get())
    );
}

#[test]
fn textarea_show_scrollbar_signal_only_records_scene_dependency() {
    let ctx = test_context();
    let show_scrollbar = ctx.state(false);
    let textarea: Element<()> = Textarea::new("line 0\nline 1\nline 2\nline 3")
        .height(dp(52.0))
        .show_scrollbar(show_scrollbar.signal())
        .into();
    let widget_id = textarea.id;
    let tree = WidgetTree::new(textarea);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let layout = tree.build_scene_layout(
        &font_manager,
        &Theme::default(),
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(!layout.dependencies().has_global_dependency());
    assert_eq!(layout.dependencies().dependency_count(), 0);

    let computed = tree.collect_scene_from_layout(
        &font_manager,
        &layout,
        &Theme::default(),
        &media,
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(!computed.dependencies.has_global_dependency());
    assert_eq!(computed.dependencies.dependency_count(), 2);
    assert!(computed.dependencies.contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Scene,
        property: None,
    }));
}

#[test]
fn textarea_lifecycle_snapshot_ignores_internal_text_revision() {
    let ctx = test_context();
    let controller = ctx.text_controller("hello");
    let tree = WidgetTree::new(
        Textarea::<()>::new(controller.clone()).on_update(Command::new(|_vm: &mut ()| {})),
    );

    let states_before = tree.lifecycle_event_states(&Theme::default());
    let before = states_before
        .first()
        .expect("textarea lifecycle state should exist");

    controller.set_text("hello world");

    let states_after = tree.lifecycle_event_states(&Theme::default());
    let after = states_after
        .first()
        .expect("textarea lifecycle state should still exist");

    assert!(before.snapshot == after.snapshot);
}

#[test]
fn widget_tree_detects_lifecycle_handlers_in_dynamic_children() {
    let ctx = test_context();
    let visible = ctx.state(false);
    let tree = WidgetTree::new_legacy(Stack::<()>::new().dynamic_child(
        visible.signal().map_unchecked(|visible| {
            let element: Element<()> = if visible {
                Text::new("shown")
                    .on_update(Command::new(|_vm: &mut ()| {}))
                    .into()
            } else {
                Stack::<()>::new().into()
            };
            element
        }),
    ));

    assert!(!tree.has_lifecycle_handlers());

    visible.set(true);

    assert!(tree.has_lifecycle_handlers());
}

#[test]
fn virtual_viewport_resolves_only_visible_window_children() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 220.0, 120.0);
    let source: Vec<usize> = (0..100_000).collect();
    let tree: WidgetTree<()> = WidgetTree::new(
        VirtualViewport::new(
            source,
            VirtualArrangement::Linear(VirtualDirection::Vertical),
            crate::ui::widget::ItemLayout::Fixed {
                item_extent: dp(20.0),
                spacing: Dp::ZERO,
                overscan: 1,
            },
            |index, item| {
                Text::new(format!("row-{index}-{item}"))
                    .height(dp(20.0))
                    .into()
            },
        )
        .size(dp(220.0), dp(120.0)),
    );

    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );

    let ResolvedWidgetKind::Virtual {
        children,
        window_plan,
        ..
    } = &layout.resolved_root.kind
    else {
        panic!("root should resolve to virtual widget");
    };

    assert!(
        children.len() < 16,
        "visible virtual children should stay bounded"
    );
    assert_eq!(children.len(), window_plan.placements.len());
    assert_eq!(window_plan.visible_range.start, 0);
    assert!(window_plan.visible_range.end <= 14);
}

#[test]
fn virtual_list_defaults_to_bounded_vertical_window_children() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 240.0, 160.0);
    let source: Vec<usize> = (0..100_000).collect();
    let tree: WidgetTree<()> = WidgetTree::new(
        VirtualList::new(source, |index, item| {
            Text::new(format!("row-{index}-{item}"))
                .height(dp(40.0))
                .into()
        })
        .size(dp(240.0), dp(160.0)),
    );

    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );

    let ResolvedWidgetKind::Virtual {
        children,
        window_plan,
        ..
    } = &layout.resolved_root.kind
    else {
        panic!("root should resolve to virtual widget");
    };

    assert!(
        children.len() < 24,
        "VirtualList should resolve only visible rows plus overscan"
    );
    assert_eq!(children.len(), window_plan.placements.len());
    assert_eq!(
        window_plan.visible_range.start, 0,
        "initial VirtualList window should start at the first row"
    );
}

#[test]
fn horizontal_virtual_viewport_uses_scroll_offset_for_visible_range() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 120.0, 40.0);
    let element: Element<()> = VirtualViewport::new(
        (0..1_000).collect::<Vec<_>>(),
        VirtualArrangement::Linear(VirtualDirection::Horizontal),
        crate::ui::widget::ItemLayout::Fixed {
            item_extent: dp(20.0),
            spacing: Dp::ZERO,
            overscan: 0,
        },
        |index, _| Text::new(format!("col-{index}")).width(dp(20.0)).into(),
    )
    .size(dp(120.0), dp(40.0))
    .into();
    let widget_id = element.id;
    let tree: WidgetTree<()> = WidgetTree::new(element);
    let baseline_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let baseline_range = match &baseline_layout.resolved_root.kind {
        ResolvedWidgetKind::Virtual { window_plan, .. } => window_plan.visible_range.clone(),
        _ => panic!("root should resolve to horizontal virtual widget"),
    };
    let scroll_offsets = HashMap::from([(widget_id, Point::new(dp(80.0), Dp::ZERO))]);
    let virtual_states = HashMap::from([(
        widget_id,
        VirtualCacheState {
            viewport_hint: Some(VirtualViewportHint {
                width: dp(120.0),
                height: dp(40.0),
            }),
            ..Default::default()
        },
    )]);

    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &scroll_offsets,
        &virtual_states,
        viewport,
    );

    let ResolvedWidgetKind::Virtual {
        window_plan,
        children,
        ..
    } = &layout.resolved_root.kind
    else {
        panic!("root should resolve to horizontal virtual widget");
    };

    assert_eq!(window_plan.visible_range.start, 4);
    assert!(window_plan.visible_range.end > window_plan.visible_range.start);
    assert!(window_plan.visible_range.start > baseline_range.start);
    assert!(window_plan.visible_range.len() < 32);
    assert_eq!(children.len(), window_plan.visible_range.len());
}

#[test]
fn measured_virtual_viewport_updates_total_extent_after_collect_feedback() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 200.0, 80.0);
    let element: Element<()> = VirtualViewport::new(
        (0..5).collect::<Vec<_>>(),
        VirtualArrangement::Linear(VirtualDirection::Vertical),
        crate::ui::widget::ItemLayout::Measured {
            estimate: dp(10.0),
            spacing: Dp::ZERO,
            overscan: 0,
        },
        |index, _| Text::new(format!("row-{index}")).height(dp(30.0)).into(),
    )
    .size(dp(200.0), dp(80.0))
    .into();
    let widget_id = element.id;
    let tree: WidgetTree<()> = WidgetTree::new(element);

    let first_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let first_extent = match &first_layout.resolved_root.kind {
        ResolvedWidgetKind::Virtual { window_plan, .. } => window_plan.total_main_extent,
        _ => panic!("root should resolve to virtual widget"),
    };
    assert_eq!(first_extent, dp(50.0));

    let computed = tree.collect_scene_from_layout(
        &font_manager,
        &first_layout,
        &theme,
        &media,
        &mut animations,
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
    );
    let update = computed
        .virtual_state_updates
        .iter()
        .find(|entry| entry.widget_id == widget_id)
        .expect("virtual collect should emit state update");
    assert!(update.invalidate_layout);
    assert!(!update.measured_extents.is_empty());

    let next_virtual_state = VirtualCacheState {
        viewport_hint: Some(update.viewport_hint.clone()),
        measured_extents: update.measured_extents.iter().copied().collect(),
        widget_ids_by_key: update.widget_ids_by_key.iter().cloned().collect(),
    };
    let second_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::from([(widget_id, next_virtual_state)]),
        viewport,
    );
    let second_extent = match &second_layout.resolved_root.kind {
        ResolvedWidgetKind::Virtual { window_plan, .. } => window_plan.total_main_extent,
        _ => panic!("root should resolve to virtual widget"),
    };
    assert!(second_extent > first_extent);
}

#[test]
fn measured_virtual_viewport_can_shrink_below_estimate_after_collect_feedback() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 200.0, 180.0);
    let element: Element<()> = VirtualViewport::new(
        (0..3).collect::<Vec<_>>(),
        VirtualArrangement::Linear(VirtualDirection::Vertical),
        crate::ui::widget::ItemLayout::Measured {
            estimate: dp(120.0),
            spacing: Dp::ZERO,
            overscan: 1,
        },
        |index, _| Text::new(format!("row-{index}")).height(dp(30.0)).into(),
    )
    .size(dp(200.0), dp(180.0))
    .into();
    let widget_id = element.id;
    let tree: WidgetTree<()> = WidgetTree::new(element);

    let first_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
    );
    let first_extent = match &first_layout.resolved_root.kind {
        ResolvedWidgetKind::Virtual { window_plan, .. } => window_plan.total_main_extent,
        _ => panic!("root should resolve to virtual widget"),
    };
    assert_eq!(first_extent, dp(360.0));

    let computed = tree.collect_scene_from_layout(
        &font_manager,
        &first_layout,
        &theme,
        &media,
        &mut animations,
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
    );
    let update = computed
        .virtual_state_updates
        .iter()
        .find(|entry| entry.widget_id == widget_id)
        .expect("virtual collect should emit state update");
    assert!(update.invalidate_layout);
    assert_eq!(update.measured_extents.len(), 3);

    let next_virtual_state = VirtualCacheState {
        viewport_hint: Some(update.viewport_hint.clone()),
        measured_extents: update.measured_extents.iter().copied().collect(),
        widget_ids_by_key: update.widget_ids_by_key.iter().cloned().collect(),
    };
    let second_layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::from([(widget_id, next_virtual_state)]),
        viewport,
    );
    let second_extent = match &second_layout.resolved_root.kind {
        ResolvedWidgetKind::Virtual { window_plan, .. } => window_plan.total_main_extent,
        _ => panic!("root should resolve to virtual widget"),
    };
    assert_eq!(second_extent, dp(90.0));
}

#[test]
fn measured_virtual_viewport_ignores_subpixel_extent_jitter() {
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let theme = Theme::default();
    let mut animations = AnimationEngine::default();
    let viewport = Rect::new(0.0, 0.0, 200.0, 80.0);
    let element: Element<()> = VirtualViewport::new(
        (0..3).collect::<Vec<_>>(),
        VirtualArrangement::Linear(VirtualDirection::Vertical),
        crate::ui::widget::ItemLayout::Measured {
            estimate: dp(30.0),
            spacing: Dp::ZERO,
            overscan: 0,
        },
        |index, _| Text::new(format!("row-{index}")).height(dp(30.25)).into(),
    )
    .size(dp(200.0), dp(80.0))
    .into();
    let widget_id = element.id;
    let tree: WidgetTree<()> = WidgetTree::new(element);
    let virtual_states = HashMap::from([(
        widget_id,
        VirtualCacheState {
            viewport_hint: Some(VirtualViewportHint {
                width: dp(200.0),
                height: dp(80.0),
            }),
            measured_extents: HashMap::from([(0, dp(30.0)), (1, dp(30.0)), (2, dp(30.0))]),
            ..Default::default()
        },
    )]);

    let layout = tree.build_scene_layout(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &virtual_states,
        viewport,
    );
    let computed = tree.collect_scene_from_layout(
        &font_manager,
        &layout,
        &theme,
        &media,
        &mut animations,
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
    );
    let update = computed
        .virtual_state_updates
        .iter()
        .find(|entry| entry.widget_id == widget_id)
        .expect("virtual collect should emit state update");

    assert!(
        !update.invalidate_layout,
        "subpixel measured extent jitter should not invalidate layout"
    );
}
