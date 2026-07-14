use super::*;
#[cfg(feature = "video")]
use crate::ui::layout::{LayoutStyle, Value};
#[cfg(feature = "video")]
use crate::ui::widget::icon::SvgIconId;
#[cfg(feature = "video")]
use crate::ui::widget::VideoSurfaceStyle;
#[cfg(feature = "video")]
use crate::ui::widget::{ResolvedElement, VideoStyle};

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
    loopings: Vec<bool>,
    playback_rates: Vec<f32>,
    audio_track_selections: Vec<crate::video::VideoAudioTrackSelection>,
    subtitle_track_selections: Vec<crate::video::VideoSubtitleTrackSelection>,
    target_rasters: Vec<Option<crate::media::RasterRequest>>,
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

    fn stop(&self) {}

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

    fn set_looping(&self, looping: bool) {
        self.commands
            .lock()
            .expect("commands lock")
            .loopings
            .push(looping);
    }

    fn set_playback_rate(&self, rate: f32) {
        self.commands
            .lock()
            .expect("commands lock")
            .playback_rates
            .push(rate);
    }

    fn set_audio_track_selection(&self, selection: crate::video::VideoAudioTrackSelection) {
        self.commands
            .lock()
            .expect("commands lock")
            .audio_track_selections
            .push(selection);
    }

    fn set_subtitle_track_selection(&self, selection: crate::video::VideoSubtitleTrackSelection) {
        self.commands
            .lock()
            .expect("commands lock")
            .subtitle_track_selections
            .push(selection);
    }

    fn set_buffer_memory_limit_bytes(&self, _bytes: u64) {}

    fn set_target_raster(&self, raster: Option<crate::media::RasterRequest>) {
        self.commands
            .lock()
            .expect("commands lock")
            .target_rasters
            .push(raster);
    }

    fn current_render_frame(&self) -> Option<VideoRenderFrame> {
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
    recorded_video_controller_with_audio_tracks(
        state,
        duration,
        muted,
        Vec::new(),
        VideoAudioTrackSelection::Auto,
    )
}

#[cfg(feature = "video")]
fn recorded_video_controller_with_audio_tracks(
    state: VideoPlaybackState,
    duration: Option<std::time::Duration>,
    muted: bool,
    audio_tracks: Vec<VideoAudioTrack>,
    audio_track_selection: VideoAudioTrackSelection,
) -> (
    VideoController,
    std::sync::Arc<std::sync::Mutex<RecordedVideoCommands>>,
) {
    recorded_video_controller_with_tracks_and_subtitle(
        state,
        duration,
        muted,
        audio_tracks,
        audio_track_selection,
        Vec::new(),
        crate::video::VideoSubtitleTrackSelection::Disabled,
        None,
    )
}

#[cfg(feature = "video")]
fn recorded_video_controller_with_audio_tracks_and_subtitle(
    state: VideoPlaybackState,
    duration: Option<std::time::Duration>,
    muted: bool,
    audio_tracks: Vec<VideoAudioTrack>,
    audio_track_selection: VideoAudioTrackSelection,
    current_subtitle: Option<VideoSubtitleCue>,
) -> (
    VideoController,
    std::sync::Arc<std::sync::Mutex<RecordedVideoCommands>>,
) {
    recorded_video_controller_with_tracks_and_subtitle(
        state,
        duration,
        muted,
        audio_tracks,
        audio_track_selection,
        Vec::new(),
        crate::video::VideoSubtitleTrackSelection::Disabled,
        current_subtitle,
    )
}

#[cfg(feature = "video")]
fn recorded_video_controller_with_subtitle_placement(
    state: VideoPlaybackState,
    duration: Option<std::time::Duration>,
    muted: bool,
    current_subtitle: Option<VideoSubtitleCue>,
    current_subtitle_placement: Option<crate::video::VideoSubtitleCuePlacement>,
) -> (
    VideoController,
    std::sync::Arc<std::sync::Mutex<RecordedVideoCommands>>,
) {
    recorded_video_controller_with_subtitle_metadata(
        state,
        duration,
        muted,
        current_subtitle,
        current_subtitle_placement,
        None,
    )
}

#[cfg(feature = "video")]
fn recorded_video_controller_with_subtitle_metadata(
    state: VideoPlaybackState,
    duration: Option<std::time::Duration>,
    muted: bool,
    current_subtitle: Option<VideoSubtitleCue>,
    current_subtitle_placement: Option<crate::video::VideoSubtitleCuePlacement>,
    current_subtitle_style: Option<crate::video::VideoSubtitleCueStyle>,
) -> (
    VideoController,
    std::sync::Arc<std::sync::Mutex<RecordedVideoCommands>>,
) {
    recorded_video_controller_with_tracks_subtitle_and_placement(
        state,
        duration,
        muted,
        Vec::new(),
        VideoAudioTrackSelection::Auto,
        Vec::new(),
        crate::video::VideoSubtitleTrackSelection::Disabled,
        current_subtitle,
        current_subtitle_placement,
        current_subtitle_style,
    )
}

#[cfg(feature = "video")]
#[allow(clippy::too_many_arguments)]
fn recorded_video_controller_with_tracks_and_subtitle(
    state: VideoPlaybackState,
    duration: Option<std::time::Duration>,
    muted: bool,
    audio_tracks: Vec<VideoAudioTrack>,
    audio_track_selection: VideoAudioTrackSelection,
    subtitle_tracks: Vec<VideoSubtitleTrack>,
    subtitle_track_selection: crate::video::VideoSubtitleTrackSelection,
    current_subtitle: Option<VideoSubtitleCue>,
) -> (
    VideoController,
    std::sync::Arc<std::sync::Mutex<RecordedVideoCommands>>,
) {
    recorded_video_controller_with_tracks_subtitle_and_placement(
        state,
        duration,
        muted,
        audio_tracks,
        audio_track_selection,
        subtitle_tracks,
        subtitle_track_selection,
        current_subtitle,
        None,
        None,
    )
}

#[cfg(feature = "video")]
#[allow(clippy::too_many_arguments)]
fn recorded_video_controller_with_tracks_subtitle_and_placement(
    state: VideoPlaybackState,
    duration: Option<std::time::Duration>,
    muted: bool,
    audio_tracks: Vec<VideoAudioTrack>,
    audio_track_selection: VideoAudioTrackSelection,
    subtitle_tracks: Vec<VideoSubtitleTrack>,
    subtitle_track_selection: crate::video::VideoSubtitleTrackSelection,
    current_subtitle: Option<VideoSubtitleCue>,
    current_subtitle_placement: Option<crate::video::VideoSubtitleCuePlacement>,
    current_subtitle_style: Option<crate::video::VideoSubtitleCueStyle>,
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
        looping: ctx.state(false),
        playback_rate: ctx.state(1.0),
        audio_tracks: ctx.state(audio_tracks),
        audio_track_selection: ctx.state(audio_track_selection),
        subtitle_tracks: ctx.state(subtitle_tracks),
        subtitle_track_selection: ctx.state(subtitle_track_selection),
        current_subtitle: ctx.state(current_subtitle),
        current_subtitle_placement: ctx.state(current_subtitle_placement),
        current_subtitle_style: ctx.state(current_subtitle_style),
        current_subtitle_bitmap: ctx.state(None),
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
#[test]
fn video_explicit_height_removes_default_aspect_ratio() {
    let (controller, _) = recorded_video_controller(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(30)),
        false,
    );
    let element: Element<()> = Video::new(controller)
        .width(pct(100.0))
        .height(dp(240.0))
        .show_controls(false)
        .show_status(false)
        .into();

    assert!(element.layout.aspect_ratio.is_none());
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
fn collect_visible_icon_sources<VM>(element: &Element<VM>, out: &mut Vec<SvgIconId>) {
    if let WidgetKind::Icon { icon } = &element.kind {
        if element.visual.opacity.resolve() > 0.5 {
            out.push(icon.source);
        }
    }
    if let WidgetKind::Container { children, .. } = &element.kind {
        for child in children {
            for resolved in child.resolve(None) {
                collect_visible_icon_sources(&resolved, out);
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
fn collect_select_elements<VM>(element: &Element<VM>, out: &mut Vec<Element<VM>>) {
    if matches!(&element.kind, WidgetKind::Select { .. }) {
        out.push(element.clone());
    }
    if let WidgetKind::Container { children, .. } = &element.kind {
        for child in children {
            for resolved in child.resolve(None) {
                collect_select_elements(&resolved, out);
            }
        }
    }
}

#[cfg(feature = "video")]
fn collect_text_contents<VM>(element: &Element<VM>, out: &mut Vec<String>) {
    if let WidgetKind::Text { text } = &element.kind {
        out.push(text.content.resolve());
    }
    if let WidgetKind::Container { children, .. } = &element.kind {
        for child in children {
            for resolved in child.resolve(None) {
                collect_text_contents(&resolved, out);
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
    let time_text = rendered
        .primitives
        .texts
        .iter()
        .find(|primitive| primitive.content.contains("00:00 / 00:30"))
        .expect("time text should render");
    let clip = time_text
        .clip_rect
        .expect("time text should be clipped to the video controls overlay");
    assert!(
        time_text.frame.y >= clip.y
            && time_text.frame.y + time_text.frame.height <= clip.y + clip.height,
        "time text should fit inside controls clip: text={:?}, clip={:?}",
        time_text.frame,
        clip
    );

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
        collect_visible_icon_sources(&tree.root, &mut icons);
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
fn video_player_looping_toggle_is_opt_in() {
    let (controller, _) = recorded_video_controller(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
    );
    let tree: WidgetTree<()> = WidgetTree::new(Video::new(controller).show_volume(false));
    let mut icons = Vec::new();
    collect_icon_sources(&tree.root, &mut icons);

    assert!(!icons.contains(&SvgIconId::Repeat));
}

#[cfg(feature = "video")]
#[test]
fn video_player_looping_toggle_forwards_changes() {
    let (controller, commands) = recorded_video_controller(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
    );
    let tree: WidgetTree<()> =
        WidgetTree::new(Video::new(controller).show_volume(false).show_looping(true));
    let mut icons = Vec::new();
    collect_icon_sources(&tree.root, &mut icons);
    assert!(icons.contains(&SvgIconId::Repeat));

    let mut clickables = Vec::new();
    collect_clickable_elements(&tree.root, &mut clickables);
    assert_eq!(clickables.len(), 2);
    let command = clickables[1]
        .interactions
        .on_click
        .clone()
        .expect("looping button command");

    command.execute(&mut ());
    command.execute(&mut ());

    assert_eq!(
        commands.lock().expect("commands lock").loopings,
        vec![true, false]
    );
}

#[cfg(feature = "video")]
#[test]
fn video_player_playback_rate_selector_is_opt_in() {
    let (controller, _) = recorded_video_controller(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
    );
    let tree: WidgetTree<()> = WidgetTree::new(Video::new(controller).show_volume(false));
    let mut selects = Vec::new();
    collect_select_elements(&tree.root, &mut selects);

    assert!(selects.is_empty());
}

#[cfg(feature = "video")]
#[test]
fn video_player_playback_rate_selector_forwards_selection() {
    let (controller, commands) = recorded_video_controller(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
    );
    let tree: WidgetTree<()> = WidgetTree::new(
        Video::new(controller)
            .show_volume(false)
            .show_playback_rate(true),
    );
    let mut selects = Vec::new();
    collect_select_elements(&tree.root, &mut selects);
    assert_eq!(selects.len(), 1);

    let WidgetKind::Select {
        selected_label,
        options,
        ..
    } = &selects[0].kind
    else {
        panic!("playback rate selector should be a Select");
    };
    assert_eq!(selected_label.resolve(), Some("Speed: 1x".to_string()));
    assert_eq!(options.len(), 9);
    assert_eq!(options[0].label.resolve(), "Speed: 0.25x");
    assert_eq!(options[5].label.resolve(), "Speed: 1.5x");

    options[5]
        .on_select
        .clone()
        .expect("playback rate option should dispatch")
        .execute(&mut ());

    assert_eq!(
        commands.lock().expect("commands lock").playback_rates,
        vec![1.5]
    );
}

#[cfg(feature = "video")]
#[test]
fn video_player_audio_track_selector_forwards_selection() {
    let (controller, commands) = recorded_video_controller_with_audio_tracks(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
        vec![
            VideoAudioTrack {
                stream_index: 2,
                title: Some("Main".to_string()),
                language: Some("en".to_string()),
                channels: 2,
                sample_rate: 48_000,
            },
            VideoAudioTrack {
                stream_index: 5,
                title: Some("Commentary".to_string()),
                language: Some("fr".to_string()),
                channels: 6,
                sample_rate: 48_000,
            },
        ],
        VideoAudioTrackSelection::Auto,
    );
    let tree: WidgetTree<()> = WidgetTree::new(Video::new(controller).show_volume(false));
    let mut selects = Vec::new();
    collect_select_elements(&tree.root, &mut selects);
    assert_eq!(selects.len(), 1);

    let WidgetKind::Select {
        selected_label,
        options,
        ..
    } = &selects[0].kind
    else {
        panic!("audio track selector should be a Select");
    };
    assert_eq!(selected_label.resolve(), Some("Audio: Auto".to_string()));
    assert_eq!(options.len(), 4);
    assert_eq!(options[0].label.resolve(), "Audio: Auto");
    assert_eq!(options[1].label.resolve(), "Audio: Off");
    assert_eq!(options[2].label.resolve(), "Main (EN, 2ch, 48kHz)");
    assert_eq!(options[3].label.resolve(), "Commentary (FR, 6ch, 48kHz)");

    options[3]
        .on_select
        .clone()
        .expect("track option should dispatch")
        .execute(&mut ());

    assert_eq!(
        commands
            .lock()
            .expect("commands lock")
            .audio_track_selections,
        vec![VideoAudioTrackSelection::Stream(5)]
    );
}

#[cfg(feature = "video")]
#[test]
fn video_player_audio_track_selector_can_be_hidden() {
    let (controller, _) = recorded_video_controller_with_audio_tracks(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
        vec![VideoAudioTrack {
            stream_index: 2,
            title: Some("Main".to_string()),
            language: Some("en".to_string()),
            channels: 2,
            sample_rate: 48_000,
        }],
        VideoAudioTrackSelection::Auto,
    );
    let tree: WidgetTree<()> = WidgetTree::new(
        Video::new(controller)
            .show_volume(false)
            .show_audio_tracks(false),
    );
    let mut selects = Vec::new();
    collect_select_elements(&tree.root, &mut selects);

    assert!(selects.is_empty());
}

#[cfg(feature = "video")]
#[test]
fn video_player_subtitle_track_selector_forwards_selection() {
    let (controller, commands) = recorded_video_controller_with_tracks_and_subtitle(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
        Vec::new(),
        VideoAudioTrackSelection::Auto,
        vec![
            VideoSubtitleTrack {
                stream_index: 4,
                title: Some("English CC".to_string()),
                language: Some("en".to_string()),
                codec: Some("subrip".to_string()),
            },
            VideoSubtitleTrack {
                stream_index: 7,
                title: None,
                language: Some("fr".to_string()),
                codec: Some("ass".to_string()),
            },
        ],
        crate::video::VideoSubtitleTrackSelection::Disabled,
        None,
    );
    let tree: WidgetTree<()> = WidgetTree::new(Video::new(controller).show_volume(false));
    let mut selects = Vec::new();
    collect_select_elements(&tree.root, &mut selects);
    assert_eq!(selects.len(), 1);

    let WidgetKind::Select {
        selected_label,
        options,
        ..
    } = &selects[0].kind
    else {
        panic!("subtitle track selector should be a Select");
    };
    assert_eq!(selected_label.resolve(), Some("Subs: Off".to_string()));
    assert_eq!(options.len(), 3);
    assert_eq!(options[0].label.resolve(), "Subs: Off");
    assert_eq!(options[1].label.resolve(), "Subs: English CC (EN, subrip)");
    assert_eq!(options[2].label.resolve(), "Subs: FR (ass)");

    options[2]
        .on_select
        .clone()
        .expect("subtitle option should dispatch")
        .execute(&mut ());

    assert_eq!(
        commands
            .lock()
            .expect("commands lock")
            .subtitle_track_selections,
        vec![crate::video::VideoSubtitleTrackSelection::Stream(7)]
    );
}

#[cfg(feature = "video")]
#[test]
fn video_player_subtitle_track_selector_can_be_hidden() {
    let (controller, _) = recorded_video_controller_with_tracks_and_subtitle(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
        Vec::new(),
        VideoAudioTrackSelection::Auto,
        vec![VideoSubtitleTrack {
            stream_index: 4,
            title: Some("English CC".to_string()),
            language: Some("en".to_string()),
            codec: Some("subrip".to_string()),
        }],
        crate::video::VideoSubtitleTrackSelection::Disabled,
        None,
    );
    let tree: WidgetTree<()> = WidgetTree::new(
        Video::new(controller)
            .show_volume(false)
            .show_subtitle_tracks(false),
    );
    let mut selects = Vec::new();
    collect_select_elements(&tree.root, &mut selects);

    assert!(selects.is_empty());
}

#[cfg(feature = "video")]
#[test]
fn video_player_renders_active_subtitle_overlay() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let (controller, _) = recorded_video_controller_with_audio_tracks_and_subtitle(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
        Vec::new(),
        VideoAudioTrackSelection::Auto,
        Some(VideoSubtitleCue {
            text: "Caption line".to_string(),
            start: std::time::Duration::from_secs(1),
            end: std::time::Duration::from_secs(3),
        }),
    );
    assert_eq!(
        controller.current_subtitle().get().map(|cue| cue.text),
        Some("Caption line".to_string())
    );
    let tree: WidgetTree<()> = WidgetTree::new(
        Video::new(controller)
            .size(dp(160.0), dp(90.0))
            .show_controls(false)
            .show_status(false),
    );
    let mut tree_texts = Vec::new();
    collect_text_contents(&tree.root, &mut tree_texts);
    assert!(
        tree_texts.iter().any(|text| text == "Caption line"),
        "tree texts: {tree_texts:?}"
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

    let text_contents: Vec<_> = rendered
        .primitives
        .texts
        .iter()
        .chain(rendered.primitives.overlay_texts.iter())
        .map(|text| text.content.to_string())
        .collect();
    assert!(
        text_contents.iter().any(|text| text == "Caption line"),
        "rendered texts: {text_contents:?}"
    );
}

#[cfg(feature = "video")]
#[test]
fn video_player_positions_subtitle_overlay_from_ass_alignment() {
    let render_subtitle = |placement| {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let (controller, _) = recorded_video_controller_with_subtitle_placement(
            VideoPlaybackState::Ready,
            Some(std::time::Duration::from_secs(40)),
            false,
            Some(VideoSubtitleCue {
                text: "Aligned caption".to_string(),
                start: std::time::Duration::from_secs(1),
                end: std::time::Duration::from_secs(3),
            }),
            placement,
        );
        let tree: WidgetTree<()> = WidgetTree::new(
            Video::new(controller)
                .size(dp(240.0), dp(160.0))
                .show_controls(false)
                .show_status(false),
        );

        let rendered = tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            Rect::new(0.0, 0.0, 240.0, 160.0),
            None,
            None,
            None,
            None,
            false,
        );

        rendered
            .primitives
            .texts
            .iter()
            .chain(rendered.primitives.overlay_texts.iter())
            .find(|text| text.content.as_ref() == "Aligned caption")
            .expect("subtitle text should render")
            .frame
    };

    let default_frame = render_subtitle(None);
    let top_right_frame = render_subtitle(VideoSubtitleCuePlacement::from_ass_alignment(9));

    assert!(
        top_right_frame.y < default_frame.y,
        "top-right ASS placement should render above default bottom placement: {top_right_frame:?} vs {default_frame:?}"
    );
    assert!(
        top_right_frame.x > default_frame.x,
        "top-right ASS placement should render to the right of default center placement: {top_right_frame:?} vs {default_frame:?}"
    );
}

#[cfg(feature = "video")]
#[test]
fn video_player_applies_subtitle_primary_color_style() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let subtitle_color = Color::rgb(0x33, 0x22, 0x11);
    let (controller, _) = recorded_video_controller_with_subtitle_metadata(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
        Some(VideoSubtitleCue {
            text: "Colored caption".to_string(),
            start: std::time::Duration::from_secs(1),
            end: std::time::Duration::from_secs(3),
        }),
        None,
        Some(crate::video::VideoSubtitleCueStyle {
            primary_color: Some(subtitle_color),
            font_weight: None,
            ..Default::default()
        }),
    );
    let tree: WidgetTree<()> = WidgetTree::new(
        Video::new(controller)
            .size(dp(240.0), dp(160.0))
            .show_controls(false)
            .show_status(false),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 160.0),
        None,
        None,
        None,
        None,
        false,
    );

    let subtitle = rendered
        .primitives
        .texts
        .iter()
        .chain(rendered.primitives.overlay_texts.iter())
        .find(|text| text.content.as_ref() == "Colored caption")
        .expect("subtitle text should render");
    assert_eq!(subtitle.color, subtitle_color);
}

#[cfg(feature = "video")]
#[test]
fn video_player_applies_subtitle_font_weight_style() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let (controller, _) = recorded_video_controller_with_subtitle_metadata(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
        Some(VideoSubtitleCue {
            text: "Bold caption".to_string(),
            start: std::time::Duration::from_secs(1),
            end: std::time::Duration::from_secs(3),
        }),
        None,
        Some(crate::video::VideoSubtitleCueStyle {
            primary_color: None,
            font_weight: Some(crate::text::font::FontWeight::Bold),
            ..Default::default()
        }),
    );
    let tree: WidgetTree<()> = WidgetTree::new(
        Video::new(controller)
            .size(dp(240.0), dp(160.0))
            .show_controls(false)
            .show_status(false),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 160.0),
        None,
        None,
        None,
        None,
        false,
    );

    let subtitle = rendered
        .primitives
        .texts
        .iter()
        .chain(rendered.primitives.overlay_texts.iter())
        .find(|text| text.content.as_ref() == "Bold caption")
        .expect("subtitle text should render");
    assert_eq!(subtitle.font_weight, crate::text::font::FontWeight::Bold);
}

#[cfg(feature = "video")]
#[test]
fn video_player_applies_subtitle_font_size_style() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let (controller, _) = recorded_video_controller_with_subtitle_metadata(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
        Some(VideoSubtitleCue {
            text: "Sized caption".to_string(),
            start: std::time::Duration::from_secs(1),
            end: std::time::Duration::from_secs(3),
        }),
        None,
        Some(crate::video::VideoSubtitleCueStyle {
            font_size_centi_px: Some(2450),
            ..Default::default()
        }),
    );
    let tree: WidgetTree<()> = WidgetTree::new(
        Video::new(controller)
            .size(dp(240.0), dp(160.0))
            .show_controls(false)
            .show_status(false),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 160.0),
        None,
        None,
        None,
        None,
        false,
    );

    let subtitle = rendered
        .primitives
        .texts
        .iter()
        .chain(rendered.primitives.overlay_texts.iter())
        .find(|text| text.content.as_ref() == "Sized caption")
        .expect("subtitle text should render");
    assert_eq!(subtitle.font_size, 24.5);
}

#[cfg(feature = "video")]
#[test]
fn video_player_applies_subtitle_outline_and_shadow_style() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let outline_color = Color::rgb(0x10, 0x20, 0x30);
    let shadow_color = Color::rgb(0x40, 0x50, 0x60);
    let (controller, _) = recorded_video_controller_with_subtitle_metadata(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
        Some(VideoSubtitleCue {
            text: "Styled caption".to_string(),
            start: std::time::Duration::from_secs(1),
            end: std::time::Duration::from_secs(3),
        }),
        None,
        Some(crate::video::VideoSubtitleCueStyle {
            primary_color: Some(Color::WHITE),
            outline_color: Some(outline_color),
            shadow_color: Some(shadow_color),
            outline_width_centi_px: Some(200),
            shadow_depth_centi_px: Some(300),
            ..Default::default()
        }),
    );
    let tree: WidgetTree<()> = WidgetTree::new(
        Video::new(controller)
            .size(dp(240.0), dp(160.0))
            .show_controls(false)
            .show_status(false),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 240.0, 160.0),
        None,
        None,
        None,
        None,
        false,
    );

    let styled_texts: Vec<_> = rendered
        .primitives
        .texts
        .iter()
        .chain(rendered.primitives.overlay_texts.iter())
        .filter(|text| text.content.as_ref() == "Styled caption")
        .collect();
    let foreground = styled_texts
        .iter()
        .find(|text| text.color == Color::WHITE)
        .expect("foreground subtitle text should render");

    assert!(
        styled_texts
            .iter()
            .any(|text| text.color == outline_color && text.frame.x != foreground.frame.x),
        "outline subtitle layers should render around the foreground: {styled_texts:?}"
    );
    assert!(
        styled_texts.iter().any(|text| {
            text.color == shadow_color
                && text.frame.x > foreground.frame.x
                && text.frame.y > foreground.frame.y
        }),
        "shadow subtitle layer should render below and to the right: {styled_texts:?}"
    );
}

#[cfg(feature = "video")]
#[test]
fn video_player_subtitle_overlay_can_be_hidden() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let (controller, _) = recorded_video_controller_with_audio_tracks_and_subtitle(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(40)),
        false,
        Vec::new(),
        VideoAudioTrackSelection::Auto,
        Some(VideoSubtitleCue {
            text: "Hidden caption".to_string(),
            start: std::time::Duration::from_secs(1),
            end: std::time::Duration::from_secs(3),
        }),
    );
    let tree: WidgetTree<()> = WidgetTree::new(
        Video::new(controller)
            .size(dp(160.0), dp(90.0))
            .show_controls(false)
            .show_status(false)
            .show_subtitles(false),
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

    assert!(!rendered
        .primitives
        .texts
        .iter()
        .chain(rendered.primitives.overlay_texts.iter())
        .any(|text| text.content.as_ref() == "Hidden caption"));
}

#[cfg(feature = "video")]
#[test]
fn video_style_public_entries_are_available() {
    let theme = Theme::default();
    let _: crate::video::VideoStyle = crate::ui::widget::VideoStyle::default_for_theme(&theme);
    let _sheet = crate::ui::widget::StyleSheet::new().video_class("player", |style, _| {
        style.controls_gap = dp(4.0);
        style.playback_rate_width = dp(80.0);
        style.audio_track_width = dp(128.0);
        style.subtitle_track_width = dp(128.0);
        style.subtitle_bottom_offset = dp(18.0);
    });
    let _components = crate::theme::ComponentThemes::default().video(|style, _| {
        style.gap = dp(6.0);
    });
}

#[cfg(feature = "video")]
#[test]
fn video_runtime_geometry_tracks_custom_style_on_the_same_tree() {
    fn collect<VM>(
        element: &ResolvedElement<VM>,
        layouts: &mut Vec<LayoutStyle>,
        gaps: &mut Vec<Value<crate::ui::layout::Length>>,
    ) {
        layouts.push(element.layout.clone());
        if let ResolvedWidgetKind::Container {
            layout, children, ..
        } = &element.kind
        {
            gaps.push(layout.gap.clone());
            for child in children {
                collect(child, layouts, gaps);
            }
        }
    }
    let (controller, _) = recorded_video_controller(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(30)),
        false,
    );
    let tree: WidgetTree<()> = WidgetTree::new(
        Video::new(controller)
            .style_full(|context| {
                let mut style = VideoStyle::default_for_theme(context.theme);
                let spacious = context.theme.density == crate::ui::theme::Density::Spacious;
                style.progress_hit_height = dp(if spacious { 31.0 } else { 17.0 });
                style.progress_height = dp(if spacious { 7.0 } else { 3.0 });
                style.volume_width = dp(if spacious { 131.0 } else { 79.0 });
                style.control_button_size = dp(if spacious { 43.0 } else { 29.0 });
                style.control_icon_size = dp(if spacious { 27.0 } else { 15.0 });
                style.controls_gap = dp(if spacious { 13.0 } else { 5.0 });
                style.overlay_gap = dp(if spacious { 11.0 } else { 4.0 });
                style.overlay_padding = Insets::all(dp(if spacious { 14.0 } else { 6.0 }));
                style.status_padding = Insets::all(dp(if spacious { 9.0 } else { 3.0 }));
                style
            })
            .size(dp(333.0), dp(187.0)),
    );
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    for (density, hit, volume, button, controls_gap) in [
        (
            crate::ui::theme::Density::Compact,
            dp(17.0),
            dp(79.0),
            dp(29.0),
            dp(5.0),
        ),
        (
            crate::ui::theme::Density::Spacious,
            dp(31.0),
            dp(131.0),
            dp(43.0),
            dp(13.0),
        ),
    ] {
        let mut theme = Theme::dark();
        theme.density = density;
        let mut animations = AnimationEngine::default();
        let layout = tree.build_scene_layout(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            UnitContext::default(),
            &HashMap::new(),
            &HashMap::new(),
            Rect::new(0.0, 0.0, 400.0, 240.0),
        );
        assert_eq!(
            layout.resolved_root.layout.width,
            Some(Value::Static(crate::ui::layout::Length::Px(dp(333.0))))
        );
        assert_eq!(
            layout.resolved_root.layout.height,
            Some(Value::Static(crate::ui::layout::Length::Px(dp(187.0))))
        );
        let mut layouts = Vec::new();
        let mut gaps = Vec::new();
        collect(&layout.resolved_root, &mut layouts, &mut gaps);
        assert!(layouts
            .iter()
            .any(|item| item.height == Some(Value::Static(crate::ui::layout::Length::Px(hit)))));
        assert!(layouts
            .iter()
            .any(|item| item.width == Some(Value::Static(crate::ui::layout::Length::Px(volume)))));
        assert!(layouts.iter().any(|item| item.width
            == Some(Value::Static(crate::ui::layout::Length::Px(button)))
            && item.height == Some(Value::Static(crate::ui::layout::Length::Px(button)))));
        assert!(gaps.contains(&Value::Static(crate::ui::layout::Length::Px(controls_gap))));
    }
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
fn video_surface_target_raster_updates_only_when_layout_size_changes() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let (controller, commands) = recorded_video_controller(
        VideoPlaybackState::Ready,
        Some(std::time::Duration::from_secs(30)),
        false,
    );
    let tree: WidgetTree<()> = WidgetTree::new(
        VideoSurface::new(controller)
            .width(pct(100.0))
            .aspect_ratio(16.0 / 9.0),
    );

    let mut render = |viewport| {
        tree.render_output(
            &font_manager,
            &theme,
            &media,
            &mut animations,
            None,
            None,
            &HashMap::new(),
            viewport,
            None,
            None,
            None,
            None,
            false,
        )
    };
    render(Rect::new(0.0, 0.0, 160.0, 90.0));
    render(Rect::new(0.0, 0.0, 160.0, 90.0));
    render(Rect::new(0.0, 0.0, 320.0, 180.0));

    assert_eq!(
        commands.lock().expect("commands lock").target_rasters,
        vec![
            Some(crate::media::RasterRequest::new_clamped(160, 90)),
            Some(crate::media::RasterRequest::new_clamped(320, 180)),
        ]
    );
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

#[cfg(feature = "video")]
#[test]
fn video_surface_fit_fill_uses_full_surface_bounds() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let texture = std::sync::Arc::new(crate::media::TextureFrame::new(
        16,
        9,
        vec![255; 16 * 9 * 4],
    ));
    let controller = test_video_controller(crate::video::VideoSurfaceSnapshot {
        intrinsic_size: crate::media::IntrinsicSize::from_pixels(16, 9),
        texture: Some(texture),
        loading: false,
        error: None,
    });
    let tree: WidgetTree<()> = WidgetTree::new(
        VideoSurface::new(controller)
            .fit(crate::media::ContentFit::Fill)
            .size(dp(160.0), dp(160.0)),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 160.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.video_textures.len(), 1);
    assert_eq!(rendered.primitives.video_textures[0].frame.width, 160.0);
    assert_eq!(rendered.primitives.video_textures[0].frame.height, 160.0);
}

#[cfg(feature = "video")]
#[test]
fn video_surface_style_fit_applies_without_builder_override() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let texture = std::sync::Arc::new(crate::media::TextureFrame::new(
        16,
        9,
        vec![255; 16 * 9 * 4],
    ));
    let controller = test_video_controller(crate::video::VideoSurfaceSnapshot {
        intrinsic_size: crate::media::IntrinsicSize::from_pixels(16, 9),
        texture: Some(texture),
        loading: false,
        error: None,
    });
    let tree: WidgetTree<()> = WidgetTree::new(
        VideoSurface::new(controller)
            .size(dp(160.0), dp(160.0))
            .style(|style, _| style.fit = crate::media::ContentFit::Fill),
    );

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 160.0, 160.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.video_textures.len(), 1);
    assert_eq!(rendered.primitives.video_textures[0].frame.width, 160.0);
    assert_eq!(rendered.primitives.video_textures[0].frame.height, 160.0);
}

#[cfg(feature = "video")]
#[test]
fn video_surface_renders_yuv_texture_when_render_frame_exists() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let y_plane = crate::video::backend::VideoYuvPlane::new(
        crate::video::backend::VideoYuvPlaneFormat::R8,
        32,
        18,
        32,
        Arc::from(vec![16_u8; 32 * 18]),
    )
    .expect("test Y plane should be valid");
    let uv_plane = crate::video::backend::VideoYuvPlane::new(
        crate::video::backend::VideoYuvPlaneFormat::Rg8,
        16,
        9,
        32,
        Arc::from(vec![128_u8; 32 * 9]),
    )
    .expect("test UV plane should be valid");
    let yuv_frame = crate::video::backend::VideoYuvFrame::with_id_revision_and_planes(
        77,
        1,
        32,
        18,
        crate::video::backend::VideoYuvFormat::Nv12,
        crate::video::backend::VideoYuvColorSpace::default(),
        Arc::from(vec![y_plane, uv_plane]),
    )
    .expect("test YUV frame should be valid");
    let controller = test_video_controller_with_render_frame(
        crate::video::VideoSurfaceSnapshot {
            intrinsic_size: crate::media::IntrinsicSize::from_pixels(32, 18),
            texture: None,
            loading: false,
            error: None,
        },
        Some(VideoRenderFrame::yuv(Arc::new(yuv_frame))),
    );
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
    assert!(rendered.primitives.texts.is_empty());
    assert_eq!(rendered.primitives.video_textures[0].frame.width, 160.0);
    assert_eq!(rendered.primitives.video_textures[0].frame.height, 90.0);
}

#[cfg(feature = "video")]
#[test]
fn video_surface_renders_bitmap_subtitle_overlay() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let texture = Arc::new(crate::media::TextureFrame::new(
        64,
        36,
        vec![255; 64 * 36 * 4],
    ));
    let subtitle_pixels: Arc<[u8]> = Arc::from([10, 20, 30, 255].repeat(8 * 6));
    let subtitle = crate::video::VideoSubtitleBitmapCue::new(
        16,
        18,
        8,
        6,
        Arc::clone(&subtitle_pixels),
        Duration::from_millis(100),
        Duration::from_millis(400),
    )
    .expect("valid bitmap subtitle cue");
    let subtitle_texture_id = subtitle.texture_id;
    let controller = test_video_controller_with_subtitle_bitmap(
        crate::video::VideoSurfaceSnapshot {
            intrinsic_size: crate::media::IntrinsicSize::from_pixels(64, 36),
            texture: Some(texture),
            loading: false,
            error: None,
        },
        Some(subtitle),
    );
    let tree: WidgetTree<()> =
        WidgetTree::new(VideoSurface::new(controller).size(dp(320.0), dp(180.0)));

    let rendered = tree.render_output(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        None,
        None,
        &HashMap::new(),
        Rect::new(0.0, 0.0, 320.0, 180.0),
        None,
        None,
        None,
        None,
        false,
    );

    assert_eq!(rendered.primitives.video_textures.len(), 1);
    assert_eq!(rendered.primitives.textures.len(), 1);
    let subtitle_texture = &rendered.primitives.textures[0];
    assert_eq!(subtitle_texture.frame.x, 80.0);
    assert_eq!(subtitle_texture.frame.y, 90.0);
    assert_eq!(subtitle_texture.frame.width, 40.0);
    assert_eq!(subtitle_texture.frame.height, 30.0);
    assert_eq!(subtitle_texture.texture.id(), subtitle_texture_id);
    assert_eq!(subtitle_texture.texture.size(), (8, 6));
    assert_eq!(subtitle_texture.texture.pixels(), &*subtitle_pixels);
}

#[cfg(feature = "video")]
#[test]
fn video_player_contains_portrait_frame_in_fixed_box() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let texture = std::sync::Arc::new(crate::media::TextureFrame::new(
        9,
        16,
        vec![255; 9 * 16 * 4],
    ));
    let controller = test_video_controller(crate::video::VideoSurfaceSnapshot {
        intrinsic_size: crate::media::IntrinsicSize::from_pixels(9, 16),
        texture: Some(texture),
        loading: false,
        error: None,
    });
    let tree: WidgetTree<()> = WidgetTree::new(
        Video::new(controller)
            .width(dp(160.0))
            .height(dp(90.0))
            .show_controls(false)
            .show_status(false),
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
    let frame = rendered.primitives.video_textures[0].frame;
    assert!(
        (frame.width.get() - 50.625).abs() < 0.01,
        "unexpected portrait video frame: {frame:?}"
    );
    assert_eq!(frame.height, 90.0);
    assert!((frame.x.get() - 54.6875).abs() < 0.01);
    assert_eq!(frame.y, 0.0);
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
