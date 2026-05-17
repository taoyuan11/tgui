use super::*;

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
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(layout.dependencies().contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Layout,
    }));

    let computed = tree.collect_scene_from_layout(
        &font_manager,
        &layout,
        &Theme::default(),
        &media,
        &mut animations,
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
    }));
}

#[test]
fn dynamic_children_signal_records_structure_dependency() {
    let ctx = test_context();
    let show = ctx.state(true);
    let container: Element<()> = Stack::new()
        .child(show.signal().map(|show| {
            if show {
                Text::new("shown")
            } else {
                Text::new("hidden")
            }
        }))
        .into();
    let widget_id = container.id;
    let tree = WidgetTree::new(container);
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();

    let layout = tree.build_scene_layout(
        &font_manager,
        &Theme::default(),
        &media,
        &mut animations,
        UnitContext::default(),
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(layout.dependencies().contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Structure,
    }));
}

#[test]
fn keyed_dynamic_children_reuse_widget_ids_across_reorder_patch() {
    let ctx = test_context();
    let reversed = ctx.state(false);
    let container: Element<()> = Stack::<()>::new()
        .child(reversed.signal().map(|reversed| {
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
    let tree = WidgetTree::new(container);
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
        Rect::new(0.0, 0.0, 200.0, 120.0),
    );

    assert!(!layout.dependencies().has_global_dependency());
    assert!(layout.dependencies().contains_owner(DependencyOwner {
        widget_id: widget_id.raw(),
        phase: DependencyPhase::Layout,
    }));

    let computed = tree.collect_scene_from_layout(
        &font_manager,
        &layout,
        &Theme::default(),
        &media,
        &mut animations,
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
        viewport,
    );

    let baseline = tree.collect_scene_from_layout_with_focus_value(
        &font_manager,
        &layout,
        &theme,
        &media,
        &mut animations,
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

    let style = TextareaStyle::default_for(infer_theme_mode(&theme));
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
    let tree = WidgetTree::new(Stack::<()>::new().child(visible.signal().map(|visible| {
        let element: Element<()> = if visible {
            Text::new("shown")
                .on_update(Command::new(|_vm: &mut ()| {}))
                .into()
        } else {
            Stack::<()>::new().into()
        };
        element
    })));

    assert!(!tree.has_lifecycle_handlers());

    visible.set(true);

    assert!(tree.has_lifecycle_handlers());
}
