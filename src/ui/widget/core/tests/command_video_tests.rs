use super::*;
#[cfg(feature = "video")]
use crate::ui::widget::VideoSurfaceStyle;

#[test]
fn scoped_value_commands_cover_switch_canvas_and_media() {
    let mut vm = ScopeRootVm::default();
    let switch: Element<ScopeChildVm> = Switch::new(false)
        .on_change(ValueCommand::new(|vm: &mut ScopeChildVm, value| {
            vm.checked = value;
        }))
        .into();
    let switch = switch.scope(scope_child);
    match switch.kind {
        WidgetKind::Switch {
            on_change: Some(command),
            ..
        } => command.execute(&mut vm, true),
        _ => panic!("switch command should be scoped"),
    }
    assert!(vm.child.checked);

    vm.child.checked = false;
    let checkbox: Element<ScopeChildVm> = Checkbox::new(false)
        .on_change(ValueCommand::new(|vm: &mut ScopeChildVm, value| {
            vm.checked = value;
        }))
        .into();
    let checkbox = checkbox.scope(scope_child);
    match checkbox.kind {
        WidgetKind::Checkbox {
            on_change: Some(command),
            ..
        } => command.execute(&mut vm, true),
        _ => panic!("checkbox command should be scoped"),
    }
    assert!(vm.child.checked);

    vm.child.checked = false;
    let radio: Element<ScopeChildVm> = Radio::new(false)
        .on_change(ValueCommand::new(|vm: &mut ScopeChildVm, value| {
            vm.checked = value;
        }))
        .into();
    let radio = radio.scope(scope_child);
    match radio.kind {
        WidgetKind::Radio {
            on_change: Some(command),
            ..
        } => command.execute(&mut vm, true),
        _ => panic!("radio command should be scoped"),
    }
    assert!(vm.child.checked);

    let canvas: Element<ScopeChildVm> = Canvas::new(CanvasRecorder::build(|_| {}))
        .on_item_click(ValueCommand::new(|vm: &mut ScopeChildVm, _event| {
            vm.canvas_hits += 1;
        }))
        .into();
    let canvas = canvas.scope(scope_child);
    match canvas.kind {
        WidgetKind::Canvas {
            item_interactions, ..
        } => item_interactions
            .on_click
            .expect("canvas item command")
            .execute(
                &mut vm,
                crate::ui::widget::CanvasPointerEvent {
                    item_id: 1_u64.into(),
                    button: None,
                    canvas_position: Point::ZERO,
                    scene_position: Point::ZERO,
                    local_position: Point::ZERO,
                    text_hit: None,
                },
            ),
        _ => panic!("canvas command should be scoped"),
    }
    assert_eq!(vm.child.canvas_hits, 1);

    let image = Image::from_path("missing-test-image.png")
        .on_loading(Command::new(|vm: &mut ScopeChildVm| vm.count += 10))
        .scope(scope_child);
    let media_command = image.media_events.on_loading.expect("media command");
    media_command.execute(&mut vm);
    assert_eq!(vm.child.count, 10);
}

#[test]
fn scoped_dynamic_children_resolve_to_root_commands() {
    let context = test_context();
    let show = context.state(true);
    let child_a: Element<ScopeChildVm> = Stack::new()
        .on_click(Command::new(|vm: &mut ScopeChildVm| vm.count += 1))
        .into();
    let child_b: Element<ScopeChildVm> = Stack::new()
        .on_click(Command::new(|vm: &mut ScopeChildVm| vm.count += 10))
        .into();

    let tree = WidgetTree::new(Stack::<ScopeRootVm>::new().child(show.signal().map(
        move |visible| {
            if visible {
                vec![child_a.clone().scope(scope_child)]
            } else {
                vec![child_b.clone().scope(scope_other)]
            }
        },
    )));

    let resolved = match &tree.root.kind {
        WidgetKind::Container { children, .. } => children[0].resolve(None),
        _ => panic!("root should be a container"),
    };

    let command = resolved[0]
        .interactions
        .on_click
        .clone()
        .expect("dynamic scoped command");
    let mut vm = ScopeRootVm::default();
    command.execute(&mut vm);
    assert_eq!(vm.child.count, 1);
    assert_eq!(vm.other.count, 0);

    show.set(false);
    let resolved = match &tree.root.kind {
        WidgetKind::Container { children, .. } => children[0].resolve(None),
        _ => panic!("root should be a container"),
    };
    let command = resolved[0]
        .interactions
        .on_click
        .clone()
        .expect("dynamic scoped command");
    command.execute(&mut vm);
    assert_eq!(vm.child.count, 1);
    assert_eq!(vm.other.count, 10);
}

#[cfg(feature = "video")]
#[test]
fn video_surface_renders_placeholder_without_frame() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let controller = test_video_controller(crate::video::VideoSurfaceSnapshot {
        intrinsic_size: crate::media::IntrinsicSize::from_pixels(16, 9),
        texture: None,
        loading: true,
        error: None,
    });
    let tree: WidgetTree<()> =
        WidgetTree::new(VideoSurface::new(controller).size(dp(160.0), dp(90.0)));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 90.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered.primitives.textures.is_empty());
    assert!(rendered
        .primitives
        .texts
        .iter()
        .any(|text| text.content.contains("loading video")));
}

#[cfg(feature = "video")]
#[test]
fn video_surface_idle_placeholder_uses_surface_background() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let background = Color::hexa(0x123456FF);
    let radius = dp(12.0);
    let controller = test_video_controller(crate::video::VideoSurfaceSnapshot {
        intrinsic_size: crate::media::IntrinsicSize::ZERO,
        texture: None,
        loading: false,
        error: None,
    });
    let tree: WidgetTree<()> = WidgetTree::new(
        VideoSurface::new(controller)
            .size(dp(160.0), dp(90.0))
            .style(move |mode| {
                let mut style = VideoSurfaceStyle::default_for(mode);
                style.surface.background = Some(background.into());
                style.surface.border_radius = Some(radius.into());
                style
            }),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 90.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert!(rendered.primitives.textures.is_empty());
    assert!(rendered
        .primitives
        .shapes
        .iter()
        .any(|shape| shape.color == background && shape.corner_radius == radius.get()));
    assert!(rendered
        .primitives
        .texts
        .iter()
        .any(|text| text.content.contains("video unavailable")));
}

#[cfg(feature = "video")]
#[test]
fn video_surface_renders_texture_when_frame_exists() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let texture = std::sync::Arc::new(crate::media::TextureFrame::new(
        32,
        18,
        vec![255; 32 * 18 * 4],
    ));
    let controller = test_video_controller(crate::video::VideoSurfaceSnapshot {
        intrinsic_size: crate::media::IntrinsicSize::from_pixels(32, 18),
        texture: Some(texture),
        loading: false,
        error: None,
    });
    let tree: WidgetTree<()> = WidgetTree::new(
        VideoSurface::new(controller)
            .width(dp(160.0))
            .aspect_ratio(32.0 / 18.0),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 90.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.textures.len(), 1);
    assert_eq!(rendered.primitives.textures[0].frame.width, 160.0);
    assert_eq!(rendered.primitives.textures[0].frame.height, 90.0);
}

#[test]
fn binding_driven_children_can_switch_component_types() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let show_button = context.state(false);
    let tree = WidgetTree::new(Stack::<()>::new().child(show_button.signal().map(|value| {
        if value {
            vec![super::Element::from(crate::ui::widget::Button::new(
                "toggle button",
            ))]
        } else {
            vec![Element::from(Text::new("toggle text"))]
        }
    })));

    let mut animations = AnimationEngine::default();
    let text_render = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert_eq!(text_render.primitives.shapes.len(), 0);

    show_button.set(true);
    let button_render = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 220.0, 120.0),
        None,
        None,
        None,
        None,
        false,
    );
    assert!(!button_render.primitives.shapes.is_empty());
}
