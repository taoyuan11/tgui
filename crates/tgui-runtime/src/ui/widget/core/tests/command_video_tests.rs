use super::*;
#[cfg(feature = "video")]
use crate::ui::widget::icon::SvgIconId;
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

    let tree = WidgetTree::new_legacy(Stack::<ScopeRootVm>::new().dynamic_child(
        show.signal().map_unchecked(move |visible| {
            if visible {
                vec![child_a.clone().scope(scope_child)]
            } else {
                vec![child_b.clone().scope(scope_other)]
            }
        }),
    ));

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
#[derive(Default)]
struct RecordedVideoCommands {
    plays: usize,
    pauses: usize,
    seeks: Vec<std::time::Duration>,
    volumes: Vec<f32>,
    muteds: Vec<bool>,
}

#[cfg(feature = "video")]
struct RecordedVideoBackend {
    commands: std::sync::Arc<std::sync::Mutex<RecordedVideoCommands>>,
}

#[cfg(feature = "video")]
impl VideoBackend for RecordedVideoBackend {
    fn load(&self, _source: crate::video::VideoSource) -> Result<(), crate::core::TguiError> {
        Ok(())
    }

    fn play(&self) {
        self.commands.lock().expect("commands lock").plays += 1;
    }

    fn pause(&self) {
        self.commands.lock().expect("commands lock").pauses += 1;
    }

    fn seek(&self, position: std::time::Duration) {
        self.commands
            .lock()
            .expect("commands lock")
            .seeks
            .push(position);
    }

    fn set_volume(&self, volume: f32) {
        self.commands
            .lock()
            .expect("commands lock")
            .volumes
            .push(volume);
    }

    fn set_muted(&self, muted: bool) {
        self.commands
            .lock()
            .expect("commands lock")
            .muteds
            .push(muted);
    }

    fn set_buffer_memory_limit_bytes(&self, _bytes: u64) {}

    fn set_target_raster(&self, _raster: Option<crate::media::RasterRequest>) {}

    fn current_frame(&self) -> Option<std::sync::Arc<crate::media::TextureFrame>> {
        None
    }

    fn shutdown(&self) {}
}

#[cfg(feature = "video")]
fn recorded_video_controller(
    state: VideoPlaybackState,
    duration: Option<std::time::Duration>,
    muted: bool,
) -> (
    VideoController,
    std::sync::Arc<std::sync::Mutex<RecordedVideoCommands>>,
) {
    let ctx = test_context();
    let commands = std::sync::Arc::new(std::sync::Mutex::new(RecordedVideoCommands::default()));
    let shared = BackendSharedState {
        playback_state: ctx.state(state),
        metrics: ctx.state(VideoMetrics {
            duration,
            position: std::time::Duration::ZERO,
            buffered: duration,
            video_width: 16,
            video_height: 9,
        }),
        volume: ctx.state(1.0),
        muted: ctx.state(muted),
        metrics_observed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        buffer_memory_limit_bytes: ctx.state(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
        video_size: ctx.state(VideoSize {
            width: 16,
            height: 9,
        }),
        error: ctx.state(None),
        surface: ctx.state(crate::video::VideoSurfaceSnapshot {
            intrinsic_size: crate::media::IntrinsicSize::from_pixels(16, 9),
            texture: None,
            loading: false,
            error: None,
        }),
    };
    let backend = RecordedVideoBackend {
        commands: commands.clone(),
    };
    (
        VideoController::from_parts(shared, std::sync::Arc::new(backend)),
        commands,
    )
}

#[cfg(feature = "video")]
fn collect_clickable_elements<VM>(element: &Element<VM>, out: &mut Vec<Element<VM>>) {
    if element.interactions.on_click.is_some() {
        out.push(element.clone());
    }
    if let WidgetKind::Container { children, .. } = &element.kind {
        for child in children {
            for resolved in child.resolve(None) {
                collect_clickable_elements(&resolved, out);
            }
        }
    }
}

#[cfg(feature = "video")]
fn collect_icon_sources<VM>(element: &Element<VM>, out: &mut Vec<SvgIconId>) {
    if let WidgetKind::Icon { icon } = &element.kind {
        out.push(icon.source);
    }
    if let WidgetKind::Container { children, .. } = &element.kind {
        for child in children {
            for resolved in child.resolve(None) {
                collect_icon_sources(&resolved, out);
            }
        }
    }
}

#[cfg(feature = "video")]
fn collect_slider_elements<VM>(element: &Element<VM>, out: &mut Vec<Element<VM>>) {
    if matches!(&element.kind, WidgetKind::Slider { .. }) {
        out.push(element.clone());
    }
    if let WidgetKind::Container { children, .. } = &element.kind {
        for child in children {
            for resolved in child.resolve(None) {
                collect_slider_elements(&resolved, out);
            }
        }
    }
}

#[cfg(feature = "video")]
#[test]
fn video_player_renders_placeholder_controls_and_status() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let (controller, _) = recorded_video_controller(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(30)),
        false,
    );
    let tree: WidgetTree<()> = WidgetTree::new(Video::new(controller));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 640.0, 420.0),
        None,
        None,
        None,
        None,
        false,
    );

    let text = rendered
        .primitives
        .texts
        .iter()
        .map(|primitive| primitive.content.as_ref())
        .collect::<Vec<_>>();
    assert!(text
        .iter()
        .any(|content| content.contains("video unavailable")));
    assert!(text.iter().any(|content| content.contains("00:00 / 00:30")));
    assert!(text.iter().any(|content| content.contains("Ready")));

    let mut icons = Vec::new();
    collect_icon_sources(&tree.root, &mut icons);
    assert!(icons.contains(&SvgIconId::PlayArrow));
    assert!(icons.contains(&SvgIconId::VolumeUp));
}

#[cfg(feature = "video")]
#[test]
fn video_player_play_button_forwards_by_state() {
    let cases = [
        (
            VideoPlaybackState::Idle,
            (1, 0, 0),
            SvgIconId::PlayArrow,
            false,
        ),
        (
            VideoPlaybackState::Ready,
            (1, 0, 0),
            SvgIconId::PlayArrow,
            false,
        ),
        (
            VideoPlaybackState::Paused,
            (1, 0, 0),
            SvgIconId::PlayArrow,
            false,
        ),
        (
            VideoPlaybackState::Playing,
            (0, 1, 0),
            SvgIconId::Pause,
            false,
        ),
        (
            VideoPlaybackState::Ended,
            (1, 0, 1),
            SvgIconId::PlayArrow,
            false,
        ),
        (
            VideoPlaybackState::Loading,
            (0, 0, 0),
            SvgIconId::PlayArrow,
            true,
        ),
        (
            VideoPlaybackState::Buffering,
            (0, 0, 0),
            SvgIconId::PlayArrow,
            true,
        ),
        (
            VideoPlaybackState::Error("boom".to_string()),
            (0, 0, 0),
            SvgIconId::PlayArrow,
            true,
        ),
    ];

    for (state, expected, expected_icon, disabled) in cases {
        let (controller, commands) =
            recorded_video_controller(state, Some(std::time::Duration::from_secs(30)), false);
        let tree: WidgetTree<()> = WidgetTree::new(Video::new(controller));
        let mut clickables = Vec::new();
        collect_clickable_elements(&tree.root, &mut clickables);
        let command = clickables[0]
            .interactions
            .on_click
            .clone()
            .expect("play button command");
        assert_eq!(clickables[0].visual.opacity.resolve() < 1.0, disabled);

        let mut icons = Vec::new();
        collect_icon_sources(&tree.root, &mut icons);
        assert_eq!(icons[0], expected_icon);

        command.execute(&mut ());

        let commands = commands.lock().expect("commands lock");
        assert_eq!(commands.plays, expected.0);
        assert_eq!(commands.pauses, expected.1);
        assert_eq!(commands.seeks.len(), expected.2);
    }
}

#[cfg(feature = "video")]
#[test]
fn video_player_seek_slider_maps_fraction_to_duration() {
    let (controller, commands) = recorded_video_controller(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
    );
    let tree: WidgetTree<()> = WidgetTree::new(Video::new(controller).show_volume(false));
    let mut sliders = Vec::new();
    collect_slider_elements(&tree.root, &mut sliders);
    let WidgetKind::Slider {
        on_change: Some(command),
        disabled,
        ..
    } = &sliders[0].kind
    else {
        panic!("seek slider should be present");
    };

    assert!(!disabled.resolve());
    command.execute(&mut (), 0.25);
    assert_eq!(
        commands.lock().expect("commands lock").seeks,
        vec![std::time::Duration::from_secs(10)]
    );

    let (controller, _) = recorded_video_controller(VideoPlaybackState::Ready, None, false);
    let tree: WidgetTree<()> = WidgetTree::new(Video::new(controller).show_volume(false));
    let mut sliders = Vec::new();
    collect_slider_elements(&tree.root, &mut sliders);
    let WidgetKind::Slider { disabled, .. } = &sliders[0].kind else {
        panic!("seek slider should be present");
    };
    assert!(disabled.resolve());
}

#[cfg(feature = "video")]
#[test]
fn video_player_mute_and_volume_controls_forward() {
    let (controller, commands) = recorded_video_controller(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
    );
    let tree: WidgetTree<()> = WidgetTree::new(Video::new(controller));
    let mut clickables = Vec::new();
    collect_clickable_elements(&tree.root, &mut clickables);
    let mut sliders = Vec::new();
    collect_slider_elements(&tree.root, &mut sliders);

    clickables[1]
        .interactions
        .on_click
        .clone()
        .expect("mute button command")
        .execute(&mut ());
    let WidgetKind::Slider {
        on_change: Some(command),
        ..
    } = &sliders[1].kind
    else {
        panic!("volume slider should be present");
    };
    command.execute(&mut (), 0.42);

    let commands = commands.lock().expect("commands lock");
    assert_eq!(commands.muteds, vec![true]);
    assert_eq!(commands.volumes, vec![0.42]);
}

#[cfg(feature = "video")]
#[test]
fn video_style_public_entries_are_available() {
    let theme = Theme::default();
    let _: crate::video::VideoStyle = crate::ui::widget::VideoStyle::default_for_theme(&theme);
    let _sheet = crate::ui::widget::StyleSheet::new().video_class("player", |style, _| {
        style.controls_gap = dp(4.0);
    });
    let _components = crate::theme::ComponentThemes::default().video(|style, _| {
        style.gap = dp(6.0);
    });
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
            .style_full(move |ctx| {
                let mut style = VideoSurfaceStyle::default_for_theme(ctx.theme);
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

    assert_eq!(rendered.primitives.video_textures.len(), 1);
    assert_eq!(rendered.primitives.video_textures[0].frame.width, 160.0);
    assert_eq!(rendered.primitives.video_textures[0].frame.height, 90.0);
}

#[test]
fn binding_driven_children_can_switch_component_types() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let context = test_context();
    let show_button = context.state(false);
    let tree = WidgetTree::new_legacy(Stack::<()>::new().dynamic_child(
        show_button.signal().map_unchecked(|value| {
            if value {
                vec![super::Element::from(crate::ui::widget::Button::new(
                    "toggle button",
                ))]
            } else {
                vec![Element::from(Text::new("toggle text"))]
            }
        }),
    ));

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
