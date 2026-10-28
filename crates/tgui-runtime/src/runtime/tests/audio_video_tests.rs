use super::*;
#[cfg(feature = "video")]
use crate::media::RasterRequest;
#[cfg(feature = "video")]
use crate::video::backend::VideoRenderFrame;

#[test]
fn clicking_disabled_checkbox_does_not_dispatch_toggled_value() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Checkbox::new(false)
            .disable(true)
            .on_change(ValueCommand::new(|vm: &mut SwitchVm, value| {
                vm.checked = value
            }))
            .size(dp(120.0), dp(30.0)),
    );
    let mut handler = test_handler_with_vm(SwitchVm::default(), Some(tree), invalidation);
    let viewport = handler.viewport_rect();

    let frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::Disabled { .. } => Some(region.rect),
                _ => None,
            })
            .expect("disabled hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + (frame.width * 0.5),
        y: frame.y + (frame.height * 0.5),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    let checked = handler.with_view_model(|vm| vm.checked);
    assert!(!checked);
}

#[cfg(feature = "audio")]
#[derive(Default)]
struct AudioEventVm {
    loading: usize,
    success: usize,
    errors: Vec<String>,
}

#[cfg(feature = "audio")]
impl crate::foundation::view_model::ViewModel for AudioEventVm {
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

#[cfg(feature = "audio")]
#[derive(Default)]
struct RecordedAudioCommands {
    loads: Vec<AudioSource>,
    commands: Vec<&'static str>,
    loopings: Vec<bool>,
}

#[cfg(feature = "audio")]
struct MockAudioBackend {
    recorded: Arc<Mutex<RecordedAudioCommands>>,
}

#[cfg(feature = "audio")]
impl MockAudioBackend {
    fn new() -> Self {
        Self {
            recorded: Arc::new(Mutex::new(RecordedAudioCommands::default())),
        }
    }
}

#[cfg(feature = "audio")]
impl AudioBackend for MockAudioBackend {
    fn load(&self, source: AudioSource) -> Result<(), crate::foundation::error::TguiError> {
        self.recorded
            .lock()
            .expect("audio commands lock poisoned")
            .loads
            .push(source);
        Ok(())
    }

    fn play(&self) {
        self.recorded
            .lock()
            .expect("audio commands lock poisoned")
            .commands
            .push("play");
    }

    fn pause(&self) {
        self.recorded
            .lock()
            .expect("audio commands lock poisoned")
            .commands
            .push("pause");
    }

    fn stop(&self) {
        self.recorded
            .lock()
            .expect("audio commands lock poisoned")
            .commands
            .push("stop");
    }

    fn seek(&self, _position: Duration) {
        self.recorded
            .lock()
            .expect("audio commands lock poisoned")
            .commands
            .push("seek");
    }

    fn set_volume(&self, _volume: f32) {}

    fn set_muted(&self, _muted: bool) {}

    fn set_looping(&self, looping: bool) {
        self.recorded
            .lock()
            .expect("audio commands lock poisoned")
            .loopings
            .push(looping);
    }

    fn set_playback_rate(&self, _rate: f32) {}

    fn set_buffer_memory_limit_bytes(&self, _bytes: u64) {}

    fn shutdown(&self) {}
}

#[cfg(feature = "audio")]
fn test_audio_controller() -> (
    AudioController,
    AudioBackendSharedState,
    Arc<Mutex<RecordedAudioCommands>>,
) {
    let invalidation = InvalidationSignal::new();
    let ctx = ViewModelContext::new(invalidation, AnimationCoordinator::default());
    let shared = AudioBackendSharedState {
        playback_state: ctx.state(AudioPlaybackState::Idle),
        metrics: ctx.state(AudioMetrics::default()),
        volume: ctx.state(1.0),
        muted: ctx.state(false),
        looping: ctx.state(false),
        playback_rate: ctx.state(1.0),
        metrics_observed: Arc::new(AtomicBool::new(false)),
        buffer_memory_limit_bytes: ctx.state(DEFAULT_AUDIO_BUFFER_MEMORY_LIMIT_BYTES),
        error: ctx.state(None),
        snapshot: ctx.state(crate::audio::AudioSnapshot::default()),
    };
    let backend = Arc::new(MockAudioBackend::new());
    let recorded = backend.recorded.clone();
    let controller = AudioController::from_parts(shared.clone(), backend);
    (controller, shared, recorded)
}

#[cfg(feature = "audio")]
#[test]
fn audio_widget_mount_autoplay_and_looping_do_not_reload_controller_source() {
    let invalidation = InvalidationSignal::new();
    let (controller, _shared, recorded) = test_audio_controller();
    controller
        .load(AudioSource::File("demo.mp3".into()))
        .expect("audio source should load");
    let tree = WidgetTree::new(Audio::new(controller).autoplay(true).looping(true));
    let mut handler = test_handler_with_vm(AudioEventVm::default(), Some(tree), invalidation);

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);

    let recorded = recorded.lock().expect("audio commands lock poisoned");
    assert_eq!(recorded.loads, vec![AudioSource::File("demo.mp3".into())]);
    assert_eq!(recorded.loopings, vec![true]);
    assert_eq!(recorded.commands, vec!["play"]);
}

#[cfg(feature = "audio")]
#[test]
fn audio_widget_autoplay_and_looping_signals_update_the_existing_tree() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let autoplay = context.state(false);
    let looping = context.state(false);
    let (controller, _shared, recorded) = test_audio_controller();
    let tree = WidgetTree::new(
        Audio::new(controller)
            .autoplay(autoplay.signal())
            .looping(looping.signal()),
    );
    let mut handler = test_handler_with_vm(AudioEventVm::default(), Some(tree), invalidation);

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);
    looping.set(true);
    dispatch_lifecycle_if_dirty(&mut handler);
    autoplay.set(true);
    dispatch_lifecycle_if_dirty(&mut handler);
    autoplay.set(false);
    looping.set(false);
    dispatch_lifecycle_if_dirty(&mut handler);

    let recorded = recorded.lock().expect("audio commands lock poisoned");
    assert_eq!(recorded.loopings, vec![false, true, false]);
    assert_eq!(recorded.commands, vec!["play"]);
}

#[cfg(feature = "audio")]
#[test]
fn audio_widget_controller_change_stops_previous_and_autoplays_new_controller() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let use_second = context.state(false);
    let (first_controller, _first_shared, first_recorded) = test_audio_controller();
    first_controller
        .load(AudioSource::File("first.mp3".into()))
        .expect("first audio source should load");
    let (second_controller, _second_shared, second_recorded) = test_audio_controller();
    second_controller
        .load(AudioSource::File("second.mp3".into()))
        .expect("second audio source should load");
    let tree = WidgetTree::new_legacy(Stack::<AudioEventVm>::new().dynamic_child(
        use_second.signal().map_unchecked(move |use_second| {
            if use_second {
                let element: Element<AudioEventVm> =
                    Audio::new(second_controller.clone()).autoplay(true).into();
                element
            } else {
                let element: Element<AudioEventVm> =
                    Audio::new(first_controller.clone()).autoplay(true).into();
                element
            }
        }),
    ));
    let mut handler = test_handler_with_vm(AudioEventVm::default(), Some(tree), invalidation);

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);
    use_second.set(true);
    dispatch_lifecycle_if_dirty(&mut handler);

    assert_eq!(
        first_recorded
            .lock()
            .expect("first audio commands lock poisoned")
            .loads,
        vec![AudioSource::File("first.mp3".into())]
    );
    assert_eq!(
        first_recorded
            .lock()
            .expect("first audio commands lock poisoned")
            .commands,
        vec!["play", "stop"]
    );
    assert_eq!(
        second_recorded
            .lock()
            .expect("second audio commands lock poisoned")
            .loads,
        vec![AudioSource::File("second.mp3".into())]
    );
    assert_eq!(
        second_recorded
            .lock()
            .expect("second audio commands lock poisoned")
            .commands,
        vec!["play"]
    );
}

#[cfg(feature = "audio")]
#[test]
fn audio_widget_unmount_stops_controller() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let visible = context.state(true);
    let (controller, _shared, recorded) = test_audio_controller();
    let tree = WidgetTree::new_legacy(Stack::<AudioEventVm>::new().dynamic_child(
        visible.signal().map_unchecked(move |visible| {
            let element: Element<AudioEventVm> = if visible {
                Audio::new(controller.clone()).key("tracked").into()
            } else {
                Stack::<AudioEventVm>::new().into()
            };
            element
        }),
    ));
    let mut handler = test_handler_with_vm(AudioEventVm::default(), Some(tree), invalidation);

    handler.invalidation.mark_dirty();
    dispatch_lifecycle_if_dirty(&mut handler);
    visible.set(false);
    dispatch_lifecycle_if_dirty(&mut handler);

    let recorded = recorded.lock().expect("audio commands lock poisoned");
    assert!(recorded.commands.contains(&"stop"));
}

#[cfg(feature = "audio")]
#[test]
fn audio_media_events_only_dispatch_on_phase_change() {
    let invalidation = InvalidationSignal::new();
    let (controller, shared, _recorded) = test_audio_controller();
    let tree = WidgetTree::new(
        Audio::new(controller)
            .on_loading(Command::new(|vm: &mut AudioEventVm| vm.loading += 1))
            .on_success(Command::new(|vm: &mut AudioEventVm| vm.success += 1))
            .on_error(ValueCommand::new(|vm: &mut AudioEventVm, error| {
                vm.errors.push(error);
            })),
    );
    let mut handler = test_handler_with_vm(AudioEventVm::default(), Some(tree), invalidation);

    shared.snapshot.set(crate::audio::AudioSnapshot {
        loading: true,
        error: None,
    });
    handler.dispatch_media_events();
    handler.dispatch_media_events();

    shared.snapshot.set(crate::audio::AudioSnapshot {
        loading: false,
        error: None,
    });
    handler.dispatch_media_events();
    handler.dispatch_media_events();

    shared.snapshot.set(crate::audio::AudioSnapshot {
        loading: false,
        error: Some("boom".to_string()),
    });
    handler.dispatch_media_events();
    handler.dispatch_media_events();

    let vm = handler
        .view_model
        .lock()
        .expect("view model lock should not be poisoned");
    assert_eq!(vm.loading, 1);
    assert_eq!(vm.success, 1);
    assert_eq!(vm.errors, vec!["boom".to_string()]);
}

#[cfg(feature = "video")]
struct MockVideoBackend;

#[cfg(feature = "video")]
impl VideoBackend for MockVideoBackend {
    fn load(&self, _source: VideoSource) -> Result<(), crate::foundation::error::TguiError> {
        Ok(())
    }

    fn play(&self) {}

    fn pause(&self) {}

    fn stop(&self) {}

    fn seek(&self, _position: Duration) {}

    fn set_volume(&self, _volume: f32) {}

    fn set_muted(&self, _muted: bool) {}

    fn set_looping(&self, _looping: bool) {}

    fn set_playback_rate(&self, _rate: f32) {}

    fn set_audio_track_selection(&self, _selection: VideoAudioTrackSelection) {}

    fn set_subtitle_track_selection(&self, _selection: VideoSubtitleTrackSelection) {}

    fn set_buffer_memory_limit_bytes(&self, _bytes: u64) {}

    fn set_target_raster(&self, _raster: Option<RasterRequest>) {}

    fn current_render_frame(&self) -> Option<VideoRenderFrame> {
        None
    }

    fn shutdown(&self) {}
}

#[cfg(feature = "video")]
struct RecordingVideoBackend {
    events: Arc<Mutex<Vec<&'static str>>>,
}

#[cfg(feature = "video")]
impl VideoBackend for RecordingVideoBackend {
    fn load(&self, _source: VideoSource) -> Result<(), crate::foundation::error::TguiError> {
        Ok(())
    }

    fn play(&self) {}

    fn pause(&self) {}

    fn stop(&self) {}

    fn seek(&self, _position: Duration) {}

    fn set_volume(&self, _volume: f32) {}

    fn set_muted(&self, _muted: bool) {}

    fn set_looping(&self, _looping: bool) {}

    fn set_playback_rate(&self, _rate: f32) {}

    fn set_audio_track_selection(&self, _selection: VideoAudioTrackSelection) {}

    fn set_subtitle_track_selection(&self, _selection: VideoSubtitleTrackSelection) {}

    fn set_buffer_memory_limit_bytes(&self, _bytes: u64) {}

    fn set_target_raster(&self, _raster: Option<RasterRequest>) {}

    fn current_render_frame(&self) -> Option<VideoRenderFrame> {
        None
    }

    fn shutdown(&self) {}

    fn on_surface_lost(&self) {
        self.events
            .lock()
            .expect("events lock poisoned")
            .push("surface_lost");
    }

    fn on_surface_restored(&self) {
        self.events
            .lock()
            .expect("events lock poisoned")
            .push("surface_restored");
    }

    fn on_app_background(&self) {
        self.events
            .lock()
            .expect("events lock poisoned")
            .push("app_background");
    }

    fn on_app_foreground(&self) {
        self.events
            .lock()
            .expect("events lock poisoned")
            .push("app_foreground");
    }
}

#[cfg(feature = "video")]
#[test]
fn video_lifecycle_notifications_deduplicate_active_controllers() {
    let invalidation = InvalidationSignal::new();
    let animations = AnimationCoordinator::default();
    let ctx = ViewModelContext::new(invalidation.clone(), animations);
    let events = Arc::new(Mutex::new(Vec::new()));
    let shared = BackendSharedState {
        playback_state: ctx.state(VideoPlaybackState::Ready),
        metrics: ctx.state(VideoMetrics::default()),
        volume: ctx.state(1.0),
        muted: ctx.state(false),
        looping: ctx.state(false),
        playback_rate: ctx.state(1.0),
        audio_tracks: ctx.state(Vec::new()),
        audio_track_selection: ctx.state(VideoAudioTrackSelection::Auto),
        subtitle_tracks: ctx.state(Vec::new()),
        subtitle_track_selection: ctx.state(VideoSubtitleTrackSelection::Disabled),
        current_subtitle: ctx.state(None),
        current_subtitle_placement: ctx.state(None),
        current_subtitle_style: ctx.state(None),
        current_subtitle_bitmap: ctx.state(None),
        metrics_observed: Arc::new(AtomicBool::new(false)),
        buffer_memory_limit_bytes: ctx.state(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
        video_size: ctx.state(VideoSize {
            width: 160,
            height: 90,
        }),
        error: ctx.state(None),
        surface: ctx.state(VideoSurfaceSnapshot::default()),
    };
    let controller = VideoController::from_parts(
        shared,
        Arc::new(RecordingVideoBackend {
            events: events.clone(),
        }),
    );
    let tree: WidgetTree<TestVm> = WidgetTree::new(
        Stack::new()
            .child(VideoSurface::new(controller.clone()).size(dp(160.0), dp(90.0)))
            .child(VideoSurface::new(controller).size(dp(160.0), dp(90.0))),
    );
    let handler = test_handler(Some(tree), invalidation);

    handler.notify_video_app_background();
    handler.notify_video_surface_lost();
    handler.notify_video_surface_restored();
    handler.notify_video_app_foreground();

    assert_eq!(
        *events.lock().expect("events lock poisoned"),
        vec![
            "app_background",
            "surface_lost",
            "surface_restored",
            "app_foreground"
        ]
    );
}

#[cfg(feature = "video")]
#[test]
fn video_lifecycle_notifications_follow_suspend_order() {
    let invalidation = InvalidationSignal::new();
    let animations = AnimationCoordinator::default();
    let ctx = ViewModelContext::new(invalidation.clone(), animations);
    let events = Arc::new(Mutex::new(Vec::new()));
    let shared = BackendSharedState {
        playback_state: ctx.state(VideoPlaybackState::Ready),
        metrics: ctx.state(VideoMetrics::default()),
        volume: ctx.state(1.0),
        muted: ctx.state(false),
        looping: ctx.state(false),
        playback_rate: ctx.state(1.0),
        audio_tracks: ctx.state(Vec::new()),
        audio_track_selection: ctx.state(VideoAudioTrackSelection::Auto),
        subtitle_tracks: ctx.state(Vec::new()),
        subtitle_track_selection: ctx.state(VideoSubtitleTrackSelection::Disabled),
        current_subtitle: ctx.state(None),
        current_subtitle_placement: ctx.state(None),
        current_subtitle_style: ctx.state(None),
        current_subtitle_bitmap: ctx.state(None),
        metrics_observed: Arc::new(AtomicBool::new(false)),
        buffer_memory_limit_bytes: ctx.state(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
        video_size: ctx.state(VideoSize {
            width: 160,
            height: 90,
        }),
        error: ctx.state(None),
        surface: ctx.state(VideoSurfaceSnapshot::default()),
    };
    let controller = VideoController::from_parts(
        shared,
        Arc::new(RecordingVideoBackend {
            events: events.clone(),
        }),
    );
    let tree: WidgetTree<TestVm> = WidgetTree::new(
        Stack::new()
            .child(VideoSurface::new(controller.clone()).size(dp(160.0), dp(90.0)))
            .child(VideoSurface::new(controller).size(dp(160.0), dp(90.0))),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    handler.suspend();

    assert_eq!(
        *events.lock().expect("events lock poisoned"),
        vec!["app_background", "surface_lost"]
    );
}

#[cfg(feature = "video")]
#[test]
fn hover_path_keeps_video_surface_hit_testing_when_scene_is_cached() {
    let invalidation = InvalidationSignal::new();
    let animations = AnimationCoordinator::default();
    let ctx = ViewModelContext::new(invalidation.clone(), animations.clone());
    let shared = BackendSharedState {
        playback_state: ctx.state(VideoPlaybackState::Ready),
        metrics: ctx.state(VideoMetrics::default()),
        volume: ctx.state(1.0),
        muted: ctx.state(false),
        looping: ctx.state(false),
        playback_rate: ctx.state(1.0),
        audio_tracks: ctx.state(Vec::new()),
        audio_track_selection: ctx.state(VideoAudioTrackSelection::Auto),
        subtitle_tracks: ctx.state(Vec::new()),
        subtitle_track_selection: ctx.state(VideoSubtitleTrackSelection::Disabled),
        current_subtitle: ctx.state(None),
        current_subtitle_placement: ctx.state(None),
        current_subtitle_style: ctx.state(None),
        current_subtitle_bitmap: ctx.state(None),
        metrics_observed: Arc::new(AtomicBool::new(false)),
        buffer_memory_limit_bytes: ctx.state(DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES),
        video_size: ctx.state(VideoSize {
            width: 160,
            height: 90,
        }),
        error: ctx.state(None),
        surface: ctx.state(VideoSurfaceSnapshot::default()),
    };
    let controller = VideoController::from_parts(shared, Arc::new(MockVideoBackend));
    let tree = WidgetTree::new(
        VideoSurface::new(controller)
            .size(dp(160.0), dp(90.0))
            .cursor(CursorStyle::Pointer),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(10.0), dp(10.0)));

    let viewport = handler.viewport_rect();
    assert_eq!(handler.hover_path(viewport).len(), 1);
    assert_eq!(handler.hover_path(viewport).len(), 1);
}

#[cfg(feature = "video")]
#[test]
fn video_settings_inside_scrolled_content_accept_pointer_input() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let controller = VideoController::new(&context);
    let player: Element<TestVm> = crate::video::Video::new(controller.clone())
        .size(dp(400.0), dp(270.0))
        .shrink(0.0)
        .show_looping(true)
        .show_playback_rate(true)
        .into();
    let root: Element<TestVm> = Flex::vertical()
        .size(dp(640.0), dp(360.0))
        .overflow(Overflow::Scroll)
        .child(Stack::<TestVm>::new().height(dp(600.0)).shrink(0.0))
        .child(player)
        .into();
    let scroll_id = root.id;
    let mut handler = test_handler_with_config(
        TestVm,
        Some(WidgetTree::new(root)),
        invalidation,
        test_config_with_size(640.0, 360.0),
    );
    handler.reduced_motion = true;
    let max_offset = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == scroll_id)
        .unwrap()
        .max_offset();
    handler.set_scroll_offset(scroll_id, max_offset);
    let _ = handler.computed_scene();
    let layout = handler
        .cached_scene
        .as_ref()
        .unwrap()
        .layout
        .as_ref()
        .unwrap();
    let trigger_id = layout
        .all_widget_ids()
        .find(|id| {
            layout.resolved_widget(*id).is_some_and(|widget| {
                widget
                    .visual
                    .accessibility_label
                    .as_ref()
                    .is_some_and(|label| label.resolve() == "Playback settings")
            })
        })
        .expect("settings trigger");
    let rect = handler
        .computed_scene()
        .hit_regions
        .iter()
        .find(|hit| hit.interaction.widget_id() == trigger_id)
        .unwrap()
        .rect;
    let click = |handler: &mut BoundRuntimeHandler<TestVm>, rect: Rect| {
        handler.cursor_position = Some(Point::new(
            rect.x + rect.width * 0.5,
            rect.y + rect.height * 0.5,
        ));
        let viewport = handler.viewport_rect();
        handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    };
    assert!(
        rect.y >= 0.0 && rect.y + rect.height <= 360.0,
        "trigger must be visible: {rect:?}"
    );
    click(&mut handler, rect);
    assert!(
        handler
            .computed_scene()
            .overlay_close_handlers
            .iter()
            .any(|handle| handle.close_on_escape),
        "settings must open after pointer activation"
    );
    let loop_rect = handler
        .computed_scene()
        .overlay_hit_regions
        .iter()
        .find(|hit| hit.interaction.keyboard_activation().is_some())
        .expect("loop button")
        .rect;
    click(&mut handler, loop_rect);
    assert!(controller.looping().get());
    let (rate_id, rate_rect) = handler
        .computed_scene()
        .overlay_hit_regions
        .iter()
        .find_map(|hit| match hit.interaction {
            HitInteraction::SelectTrigger { id, .. } => Some((id, hit.rect)),
            _ => None,
        })
        .expect("speed selector");
    click(&mut handler, rate_rect);
    // Recollection must preserve the actual Select ID, not just its wrapper.
    assert_eq!(handler.resolved_select_open_state(rate_id), Some(true));
    let option = handler
        .computed_scene()
        .overlay_hit_regions
        .iter()
        .find_map(|hit| match hit.interaction {
            HitInteraction::SelectOption {
                id,
                option_index: 1,
                ..
            } if id == rate_id => Some(hit.rect),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "0.5x option must be interactive; overlay targets: {:?}",
                handler
                    .computed_scene()
                    .overlay_hit_regions
                    .iter()
                    .map(|hit| hit.interaction.target_id())
                    .collect::<Vec<_>>()
            )
        });
    click(&mut handler, option);
    assert_eq!(controller.playback_rate().get(), 0.5);
    assert_eq!(handler.focused_widget_id(), Some(rate_id));
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)))
    );
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)))
    );
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)))
    );
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert_eq!(controller.playback_rate().get(), 0.75);
    assert_eq!(handler.focused_widget_id(), Some(rate_id));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Escape))));
    assert_eq!(handler.focused_widget_id(), Some(trigger_id));
}
