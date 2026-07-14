use std::time::{Duration, Instant};

use crossbeam_channel::{after, select, Receiver, Sender};

use super::*;

mod playback;

pub(super) fn present_main(
    backend_rx: Receiver<BackendCommand>,
    decode_tx: Sender<DecodeCommand>,
    event_rx: Receiver<DecodeEvent>,
    shared: BackendSharedState,
    latest_frame: Arc<Mutex<Option<VideoRenderFrame>>>,
    shared_queue: Arc<SharedVideoQueue>,
    playback_clock: SharedPlaybackClock,
) {
    let mut worker = PresentWorker::new(
        backend_rx,
        decode_tx,
        event_rx,
        shared,
        latest_frame,
        shared_queue,
        playback_clock,
    );
    worker.run();
}

struct PresentWorker {
    backend_rx: Receiver<BackendCommand>,
    decode_tx: Sender<DecodeCommand>,
    event_rx: Receiver<DecodeEvent>,
    shared: BackendSharedState,
    latest_frame: Arc<Mutex<Option<VideoRenderFrame>>>,
    shared_queue: Arc<SharedVideoQueue>,
    playback_clock: SharedPlaybackClock,
    current_source: Option<VideoSource>,
    current_generation: u64,
    current_duration: Option<Duration>,
    current_intrinsic_size: IntrinsicSize,
    current_video_size: VideoSize,
    current_start_position: Duration,
    current_buffering_profile: BufferingProfile,
    current_audio_clock: Option<SharedAudioClock>,
    last_presented_position: Duration,
    software_paused_position: Duration,
    software_play_started_at: Option<Instant>,
    should_play: bool,
    decode_playing: bool,
    looping: bool,
    playback_rate: f32,
    audio_track_selection: VideoAudioTrackSelection,
    subtitle_track_selection: VideoSubtitleTrackSelection,
    subtitle_cues: VecDeque<QueuedSubtitleCue>,
    subtitle_bitmap_cues: VecDeque<VideoSubtitleBitmapCue>,
    active_subtitle: Option<VideoSubtitleCue>,
    active_subtitle_placement: Option<VideoSubtitleCuePlacement>,
    active_subtitle_style: Option<VideoSubtitleCueStyle>,
    active_subtitle_bitmap: Option<VideoSubtitleBitmapCue>,
    playback_ended: bool,
    buffer_snapshot: BufferSnapshot,
    pending_open_reason: Option<OpenReason>,
    stream_opened: bool,
    startup_pending: bool,
}

#[derive(Clone)]
struct QueuedSubtitleCue {
    cue: VideoSubtitleCue,
    placement: Option<VideoSubtitleCuePlacement>,
    style: Option<VideoSubtitleCueStyle>,
}

impl PresentWorker {
    fn new(
        backend_rx: Receiver<BackendCommand>,
        decode_tx: Sender<DecodeCommand>,
        event_rx: Receiver<DecodeEvent>,
        shared: BackendSharedState,
        latest_frame: Arc<Mutex<Option<VideoRenderFrame>>>,
        shared_queue: Arc<SharedVideoQueue>,
        playback_clock: SharedPlaybackClock,
    ) -> Self {
        let looping = shared.looping.get();
        let playback_rate = normalize_playback_rate(shared.playback_rate.get());
        let audio_track_selection = shared.audio_track_selection.get();
        let subtitle_track_selection = shared.subtitle_track_selection.get();
        Self {
            backend_rx,
            decode_tx,
            event_rx,
            shared,
            latest_frame,
            shared_queue,
            playback_clock,
            current_source: None,
            current_generation: 0,
            current_duration: None,
            current_intrinsic_size: IntrinsicSize::ZERO,
            current_video_size: VideoSize::default(),
            current_start_position: Duration::ZERO,
            current_buffering_profile: LOCAL_BUFFERING_PROFILE,
            current_audio_clock: None,
            last_presented_position: Duration::ZERO,
            software_paused_position: Duration::ZERO,
            software_play_started_at: None,
            should_play: false,
            decode_playing: false,
            looping,
            playback_rate,
            audio_track_selection,
            subtitle_track_selection,
            subtitle_cues: VecDeque::new(),
            subtitle_bitmap_cues: VecDeque::new(),
            active_subtitle: None,
            active_subtitle_placement: None,
            active_subtitle_style: None,
            active_subtitle_bitmap: None,
            playback_ended: false,
            buffer_snapshot: BufferSnapshot::default(),
            pending_open_reason: None,
            stream_opened: false,
            startup_pending: false,
        }
    }

    fn run(&mut self) {
        loop {
            self.present_due_frames();
            self.sync_subtitle_cue();
            self.sync_subtitle_bitmap_cue();
            self.sync_metrics();
            self.evaluate_playback_state();

            let wait = self.next_wait_duration();
            let timeout = after(wait);

            select! {
                recv(self.backend_rx) -> message => {
                    match message {
                        Ok(command) => {
                            if !self.handle_backend_command(command) {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                recv(self.event_rx) -> message => {
                    match message {
                        Ok(event) => self.handle_decode_event(event),
                        Err(_) => break,
                    }
                }
                recv(timeout) -> _ => {}
            }
        }

        let _ = self.decode_tx.send(DecodeCommand::Shutdown);
    }

    fn handle_backend_command(&mut self, command: BackendCommand) -> bool {
        match command {
            BackendCommand::Load(source) => {
                self.set_decode_playing(false);
                self.current_source = Some(source.clone());
                self.current_generation = self.current_generation.saturating_add(1);
                self.pending_open_reason = Some(OpenReason::Load);
                self.stream_opened = false;
                self.startup_pending = true;
                self.current_start_position = Duration::ZERO;
                self.current_duration = None;
                self.current_audio_clock = None;
                self.last_presented_position = Duration::ZERO;
                self.software_paused_position = Duration::ZERO;
                self.software_play_started_at = None;
                self.playback_ended = false;
                self.playback_clock.set_position(Duration::ZERO);
                self.shared_queue
                    .replace_generation(self.current_generation);
                clear_latest_frame(&self.latest_frame);
                self.clear_subtitles();
                self.shared.reset_for_load();
                let _ = self.decode_tx.send(DecodeCommand::Load {
                    generation: self.current_generation,
                    source,
                });
            }
            BackendCommand::Play => {
                if self.current_source.is_none() && !self.stream_opened {
                    return true;
                }
                if self.playback_ended {
                    self.shared.playback_state.set(VideoPlaybackState::Ended);
                    return true;
                }
                self.should_play = true;
                self.evaluate_playback_state();
            }
            BackendCommand::Pause => {
                self.should_play = false;
                let position = self.playback_position();
                self.pause_software_clock(position);
                self.set_decode_playing(false);
                if self.stream_opened {
                    if self.shared.metrics_enabled() {
                        let mut metrics = self.shared.metrics.get();
                        metrics.position = position;
                        self.shared.metrics.set(metrics);
                    }
                    self.shared.playback_state.set(VideoPlaybackState::Paused);
                }
            }
            BackendCommand::Stop { completed } => {
                self.stop_current_session();
                let _ = completed.send(());
            }
            BackendCommand::Seek(position) => {
                let Some(source) = self.current_source.clone() else {
                    return true;
                };
                self.set_decode_playing(false);
                self.current_generation = self.current_generation.saturating_add(1);
                self.pending_open_reason = Some(OpenReason::Seek);
                self.stream_opened = false;
                self.startup_pending = true;
                self.current_start_position = position;
                self.current_duration = None;
                self.current_audio_clock = None;
                self.software_paused_position = position;
                self.software_play_started_at = None;
                self.playback_ended = false;
                self.playback_clock.set_position(position);
                self.shared_queue
                    .replace_generation(self.current_generation);
                self.clear_subtitles();
                self.shared.playback_state.set(VideoPlaybackState::Loading);
                self.shared.error.set(None);
                self.shared.surface.set(VideoSurfaceSnapshot {
                    intrinsic_size: self.current_intrinsic_size,
                    texture: None,
                    loading: true,
                    error: None,
                });
                let _ = self.decode_tx.send(DecodeCommand::Seek {
                    generation: self.current_generation,
                    source,
                    position,
                });
            }
            BackendCommand::SetVolume(volume) => {
                let volume = volume.clamp(0.0, 1.0);
                self.shared.volume.set(volume);
                let _ = self.decode_tx.send(DecodeCommand::SetVolume(volume));
            }
            BackendCommand::SetMuted(muted) => {
                self.shared.muted.set(muted);
                let _ = self.decode_tx.send(DecodeCommand::SetMuted(muted));
            }
            BackendCommand::SetLooping(looping) => {
                self.looping = looping;
                self.shared.looping.set(looping);
            }
            BackendCommand::SetPlaybackRate(rate) => {
                self.set_playback_rate(rate);
            }
            BackendCommand::SetAudioTrackSelection(selection) => {
                self.set_audio_track_selection(selection);
            }
            BackendCommand::SetSubtitleTrackSelection(selection) => {
                self.set_subtitle_track_selection(selection);
            }
            BackendCommand::SetBufferMemoryLimitBytes(bytes) => {
                self.shared.buffer_memory_limit_bytes.set(bytes);
                let _ = self
                    .decode_tx
                    .send(DecodeCommand::SetBufferMemoryLimitBytes(bytes));
            }
            BackendCommand::SetTargetRaster(raster) => {
                let _ = self.decode_tx.send(DecodeCommand::SetTargetRaster(raster));
            }
            BackendCommand::Shutdown => return false,
        }

        true
    }

    fn stop_current_session(&mut self) {
        self.set_decode_playing(false);
        self.current_source = None;
        self.current_generation = self.current_generation.saturating_add(1);
        self.current_duration = None;
        self.current_intrinsic_size = IntrinsicSize::ZERO;
        self.current_video_size = VideoSize::default();
        self.current_start_position = Duration::ZERO;
        self.current_buffering_profile = LOCAL_BUFFERING_PROFILE;
        self.current_audio_clock = None;
        self.last_presented_position = Duration::ZERO;
        self.software_paused_position = Duration::ZERO;
        self.software_play_started_at = None;
        self.should_play = false;
        self.playback_ended = false;
        self.buffer_snapshot = BufferSnapshot::default();
        self.pending_open_reason = None;
        self.stream_opened = false;
        self.startup_pending = false;
        self.playback_clock.set_position(Duration::ZERO);
        self.shared_queue
            .replace_generation(self.current_generation);
        clear_latest_frame(&self.latest_frame);
        self.clear_subtitles();
        self.shared.reset_for_stop();
        let _ = self.decode_tx.send(DecodeCommand::Stop);
    }

    fn handle_decode_event(&mut self, event: DecodeEvent) {
        match event {
            DecodeEvent::StreamOpened(opened) => {
                if opened.generation != self.current_generation {
                    return;
                }
                self.stream_opened = true;
                self.current_start_position = opened.start_position;
                self.current_duration = opened.duration;
                self.current_intrinsic_size = opened.intrinsic_size;
                self.current_video_size = opened.video_size;
                self.current_buffering_profile = opened.buffering_profile;
                self.current_audio_clock = opened.audio_clock;
                self.clear_subtitles();
                self.shared.audio_tracks.set(opened.audio_tracks);
                self.shared
                    .audio_track_selection
                    .set(opened.audio_track_selection);
                self.shared.subtitle_tracks.set(opened.subtitle_tracks);
                self.shared
                    .subtitle_track_selection
                    .set(opened.subtitle_track_selection);
                self.last_presented_position = opened.start_position;
                self.software_paused_position = opened.start_position;
                self.software_play_started_at = None;
                self.playback_ended = false;
                self.playback_clock.set_position(opened.start_position);
                self.shared.video_size.set(opened.video_size);
                self.shared.error.set(None);
                self.shared.surface.set(VideoSurfaceSnapshot {
                    intrinsic_size: opened.intrinsic_size,
                    texture: None,
                    loading: true,
                    error: None,
                });
            }
            DecodeEvent::SubtitleCue(event) => {
                if event.generation != self.current_generation {
                    return;
                }
                self.insert_subtitle_cue(event.cue, event.placement, event.style);
                self.sync_subtitle_cue();
            }
            DecodeEvent::SubtitleBitmapCue(event) => {
                if event.generation != self.current_generation {
                    return;
                }
                self.insert_subtitle_bitmap_cue(event.cue);
                self.sync_subtitle_bitmap_cue();
            }
            DecodeEvent::FirstFrameReady { generation, .. } => {
                if generation != self.current_generation {
                    return;
                }
                let _ = self.present_next_frame();
                if self.should_play {
                    self.evaluate_playback_state();
                } else {
                    match self.pending_open_reason.take().unwrap_or(OpenReason::Load) {
                        OpenReason::Load => {
                            self.shared.playback_state.set(VideoPlaybackState::Ready)
                        }
                        OpenReason::Seek => {
                            self.shared.playback_state.set(VideoPlaybackState::Paused)
                        }
                    }
                }
            }
            DecodeEvent::BufferSnapshot(snapshot) => {
                if snapshot.generation != self.current_generation {
                    return;
                }
                self.buffer_snapshot = snapshot;
            }
            DecodeEvent::EofDrained { generation } => {
                if generation != self.current_generation {
                    return;
                }
                self.buffer_snapshot.eof_sent = true;
                if self.shared_queue.ready_frame_count(generation) == 0
                    && self.audio_buffered_duration().is_zero()
                {
                    if self.looping {
                        self.restart_loop_from_start();
                        return;
                    }

                    let position = self.playback_position();
                    self.set_decode_playing(false);
                    self.playback_ended = true;
                    self.should_play = false;
                    self.startup_pending = false;
                    if self.shared.metrics_enabled() {
                        let mut metrics = self.shared.metrics.get();
                        metrics.position = position;
                        self.shared.metrics.set(metrics);
                    }
                    self.shared.playback_state.set(VideoPlaybackState::Ended);
                }
            }
            DecodeEvent::FatalError {
                generation,
                message,
            } => {
                if generation != self.current_generation {
                    return;
                }
                self.set_decode_playing(false);
                self.shared.set_error(message);
            }
        }
    }

    fn restart_loop_from_start(&mut self) {
        let Some(source) = self.current_source.clone() else {
            return;
        };

        self.set_decode_playing(false);
        self.current_generation = self.current_generation.saturating_add(1);
        self.pending_open_reason = Some(OpenReason::Seek);
        self.stream_opened = false;
        self.startup_pending = true;
        self.current_start_position = Duration::ZERO;
        self.current_duration = None;
        self.current_audio_clock = None;
        self.last_presented_position = Duration::ZERO;
        self.software_paused_position = Duration::ZERO;
        self.software_play_started_at = None;
        self.playback_ended = false;
        self.buffer_snapshot = BufferSnapshot::default();
        self.playback_clock.set_position(Duration::ZERO);
        self.shared_queue
            .replace_generation(self.current_generation);
        self.clear_subtitles();

        if self.shared.metrics_enabled() {
            let mut metrics = self.shared.metrics.get();
            metrics.position = Duration::ZERO;
            metrics.buffered = None;
            self.shared.metrics.set(metrics);
        }
        self.shared
            .playback_state
            .set(VideoPlaybackState::Buffering);
        let _ = self.decode_tx.send(DecodeCommand::Seek {
            generation: self.current_generation,
            source,
            position: Duration::ZERO,
        });
    }

    fn sync_metrics(&mut self) {
        if !self.shared.metrics_enabled() {
            return;
        }

        if !self.stream_opened {
            return;
        }

        let position = self.playback_position();
        self.playback_clock.set_position(position);

        let previous = self.shared.metrics.get();
        let mut metrics = previous.clone();
        metrics.duration = self.current_duration;
        metrics.position = position;
        metrics.buffered = self.buffered_position(position);
        metrics.video_width = self.current_video_size.width;
        metrics.video_height = self.current_video_size.height;
        if metrics != previous {
            self.shared.metrics.set(metrics);
            if self.decode_playing {
                self.shared.request_redraw();
            }
        }
    }

    fn insert_subtitle_cue(
        &mut self,
        cue: VideoSubtitleCue,
        placement: Option<VideoSubtitleCuePlacement>,
        style: Option<VideoSubtitleCueStyle>,
    ) {
        if cue.text.trim().is_empty() || cue.end <= cue.start {
            return;
        }

        let queued = QueuedSubtitleCue {
            cue,
            placement,
            style,
        };
        let index = self
            .subtitle_cues
            .iter()
            .position(|existing| queued.cue.start < existing.cue.start)
            .unwrap_or(self.subtitle_cues.len());
        self.subtitle_cues.insert(index, queued);
    }

    fn insert_subtitle_bitmap_cue(&mut self, cue: VideoSubtitleBitmapCue) {
        if cue.end <= cue.start || cue.width == 0 || cue.height == 0 {
            return;
        }

        let index = self
            .subtitle_bitmap_cues
            .iter()
            .position(|existing| cue.start < existing.start)
            .unwrap_or(self.subtitle_bitmap_cues.len());
        self.subtitle_bitmap_cues.insert(index, cue);
    }

    fn sync_subtitle_cue(&mut self) {
        if !self.stream_opened {
            self.publish_subtitle(None, None, None);
            return;
        }

        let position = self.playback_position();
        while self
            .subtitle_cues
            .front()
            .map(|cue| cue.cue.end <= position)
            .unwrap_or(false)
        {
            self.subtitle_cues.pop_front();
        }

        let active = self
            .subtitle_cues
            .iter()
            .find(|cue| cue.cue.start <= position && position < cue.cue.end)
            .cloned();
        if let Some(active) = active {
            self.publish_subtitle(Some(active.cue), active.placement, active.style);
        } else {
            self.publish_subtitle(None, None, None);
        }
    }

    fn sync_subtitle_bitmap_cue(&mut self) {
        if !self.stream_opened {
            self.publish_subtitle_bitmap(None);
            return;
        }

        let position = self.playback_position();
        while self
            .subtitle_bitmap_cues
            .front()
            .map(|cue| cue.end <= position)
            .unwrap_or(false)
        {
            self.subtitle_bitmap_cues.pop_front();
        }

        let active = self
            .subtitle_bitmap_cues
            .iter()
            .find(|cue| cue.start <= position && position < cue.end)
            .cloned();
        self.publish_subtitle_bitmap(active);
    }

    fn publish_subtitle(
        &mut self,
        cue: Option<VideoSubtitleCue>,
        placement: Option<VideoSubtitleCuePlacement>,
        style: Option<VideoSubtitleCueStyle>,
    ) {
        if self.active_subtitle == cue
            && self.active_subtitle_placement == placement
            && self.active_subtitle_style == style
        {
            return;
        }
        self.active_subtitle = cue.clone();
        self.active_subtitle_placement = placement;
        self.active_subtitle_style = style;
        self.shared.current_subtitle.set(cue);
        self.shared.current_subtitle_placement.set(placement);
        self.shared.current_subtitle_style.set(style);
    }

    fn publish_subtitle_bitmap(&mut self, cue: Option<VideoSubtitleBitmapCue>) {
        if self.active_subtitle_bitmap == cue {
            return;
        }
        self.active_subtitle_bitmap = cue.clone();
        self.shared.current_subtitle_bitmap.set(cue);
    }

    fn clear_subtitles(&mut self) {
        self.subtitle_cues.clear();
        self.subtitle_bitmap_cues.clear();
        self.publish_subtitle(None, None, None);
        self.publish_subtitle_bitmap(None);
    }

    fn set_decode_playing(&mut self, playing: bool) {
        if self.decode_playing == playing {
            return;
        }

        self.decode_playing = playing;
        if self.current_audio_clock.is_none() {
            if playing {
                self.software_play_started_at = Some(Instant::now());
            } else {
                let position = self.playback_position();
                self.software_play_started_at = None;
                self.software_paused_position = position;
            }
        }

        let _ = self.decode_tx.send(DecodeCommand::SetPlaying {
            generation: self.current_generation,
            playing,
        });
    }

    fn set_playback_rate(&mut self, rate: f32) {
        let rate = normalize_playback_rate(rate);
        if (self.playback_rate - rate).abs() <= f32::EPSILON {
            self.shared.playback_rate.set(rate);
            let _ = self.decode_tx.send(DecodeCommand::SetPlaybackRate(rate));
            return;
        }

        let position = self.playback_position();
        self.playback_rate = rate;
        self.shared.playback_rate.set(rate);
        self.playback_clock.set_position(position);
        if self.current_audio_clock.is_none() {
            self.software_paused_position = position;
            self.software_play_started_at = self.decode_playing.then(Instant::now);
        }
        let _ = self.decode_tx.send(DecodeCommand::SetPlaybackRate(rate));
    }

    fn set_audio_track_selection(&mut self, selection: VideoAudioTrackSelection) {
        if self.audio_track_selection == selection {
            self.shared.audio_track_selection.set(selection);
            let _ = self
                .decode_tx
                .send(DecodeCommand::SetAudioTrackSelection(selection));
            return;
        }

        self.audio_track_selection = selection;
        self.shared.audio_track_selection.set(selection);
        let _ = self
            .decode_tx
            .send(DecodeCommand::SetAudioTrackSelection(selection));

        let Some(source) = self.current_source.clone() else {
            return;
        };

        let position = self.playback_position();
        self.set_decode_playing(false);
        self.current_generation = self.current_generation.saturating_add(1);
        self.pending_open_reason = Some(OpenReason::Seek);
        self.stream_opened = false;
        self.startup_pending = true;
        self.current_start_position = position;
        self.current_duration = None;
        self.current_audio_clock = None;
        self.software_paused_position = position;
        self.software_play_started_at = None;
        self.playback_ended = false;
        self.buffer_snapshot = BufferSnapshot::default();
        self.playback_clock.set_position(position);
        self.shared_queue
            .replace_generation(self.current_generation);
        self.shared.playback_state.set(VideoPlaybackState::Loading);
        self.shared.error.set(None);
        self.shared.surface.set(VideoSurfaceSnapshot {
            intrinsic_size: self.current_intrinsic_size,
            texture: None,
            loading: true,
            error: None,
        });
        let _ = self.decode_tx.send(DecodeCommand::Seek {
            generation: self.current_generation,
            source,
            position,
        });
    }

    fn set_subtitle_track_selection(&mut self, selection: VideoSubtitleTrackSelection) {
        if self.subtitle_track_selection == selection {
            self.shared.subtitle_track_selection.set(selection);
            let _ = self
                .decode_tx
                .send(DecodeCommand::SetSubtitleTrackSelection(selection));
            return;
        }

        self.subtitle_track_selection = selection;
        self.shared.subtitle_track_selection.set(selection);
        let _ = self
            .decode_tx
            .send(DecodeCommand::SetSubtitleTrackSelection(selection));
        self.clear_subtitles();

        let Some(source) = self.current_source.clone() else {
            return;
        };

        let position = self.playback_position();
        self.set_decode_playing(false);
        self.current_generation = self.current_generation.saturating_add(1);
        self.pending_open_reason = Some(OpenReason::Seek);
        self.stream_opened = false;
        self.startup_pending = true;
        self.current_start_position = position;
        self.current_duration = None;
        self.current_audio_clock = None;
        self.software_paused_position = position;
        self.software_play_started_at = None;
        self.playback_ended = false;
        self.buffer_snapshot = BufferSnapshot::default();
        self.playback_clock.set_position(position);
        self.shared_queue
            .replace_generation(self.current_generation);
        self.shared.playback_state.set(VideoPlaybackState::Loading);
        self.shared.error.set(None);
        self.shared.surface.set(VideoSurfaceSnapshot {
            intrinsic_size: self.current_intrinsic_size,
            texture: None,
            loading: true,
            error: None,
        });
        let _ = self.decode_tx.send(DecodeCommand::Seek {
            generation: self.current_generation,
            source,
            position,
        });
    }

    fn pause_software_clock(&mut self, position: Duration) {
        self.software_play_started_at = None;
        self.software_paused_position = position;
        self.playback_clock.set_position(position);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::Duration;

    use parking_lot::Mutex;

    use crossbeam_channel::{bounded, unbounded, TryRecvError};

    use crate::animation::AnimationCoordinator;
    use crate::foundation::binding::{InvalidationSignal, ViewModelContext};
    use crate::video::VideoMetrics;

    use super::super::super::{BackendSharedState, DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES};
    use super::*;

    fn test_context() -> ViewModelContext {
        ViewModelContext::new(InvalidationSignal::new(), AnimationCoordinator::default())
    }

    fn test_shared(ctx: &ViewModelContext) -> BackendSharedState {
        BackendSharedState {
            playback_state: ctx.state(VideoPlaybackState::Idle),
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
            video_size: ctx.state(VideoSize::default()),
            error: ctx.state(None),
            surface: ctx.state(VideoSurfaceSnapshot::default()),
        }
    }

    fn test_worker() -> (PresentWorker, crossbeam_channel::Receiver<DecodeCommand>) {
        let ctx = test_context();
        let shared = test_shared(&ctx);
        let (backend_tx, backend_rx) = unbounded();
        drop(backend_tx);
        let (decode_tx, decode_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();
        drop(event_tx);

        (
            PresentWorker::new(
                backend_rx,
                decode_tx,
                event_rx,
                shared,
                Arc::new(Mutex::new(None)),
                Arc::new(SharedVideoQueue::new()),
                SharedPlaybackClock::default(),
            ),
            decode_rx,
        )
    }

    #[test]
    fn sync_metrics_requests_redraw_while_playing_without_new_frame() {
        let (mut worker, _decode_rx) = test_worker();
        worker.shared.enable_metrics();
        worker.stream_opened = true;
        worker.decode_playing = true;
        worker.current_duration = Some(Duration::from_secs(30));
        worker.current_video_size = VideoSize {
            width: 1920,
            height: 1080,
        };
        worker.software_paused_position = Duration::from_secs(5);

        assert!(!worker.shared.surface.invalidation().take_redraw_request());

        worker.sync_metrics();

        assert_eq!(worker.shared.metrics.get().position, Duration::from_secs(5));
        assert_eq!(
            worker.shared.metrics.get().duration,
            Some(Duration::from_secs(30))
        );
        assert!(
            worker.shared.surface.invalidation().take_redraw_request(),
            "playing timeline changes should request redraw even when no frame is presented"
        );

        worker.sync_metrics();

        assert!(
            !worker.shared.surface.invalidation().take_redraw_request(),
            "unchanged metrics should not keep requesting redraws"
        );
    }

    #[test]
    fn seek_pauses_previous_generation_before_reopening() {
        let (mut worker, decode_rx) = test_worker();
        worker.current_source = Some(VideoSource::File("demo.mp4".into()));
        worker.current_generation = 7;
        worker.decode_playing = true;

        assert!(worker.handle_backend_command(BackendCommand::Seek(Duration::from_secs(5))));

        assert!(matches!(
            decode_rx
                .recv()
                .expect("pause command should be sent first"),
            DecodeCommand::SetPlaying {
                generation: 7,
                playing: false,
            }
        ));
        assert!(matches!(
            decode_rx.recv().expect("seek command should follow pause"),
            DecodeCommand::Seek {
                generation: 8,
                position,
                ..
            } if position == Duration::from_secs(5)
        ));
    }

    #[test]
    fn load_pauses_previous_generation_before_reopening() {
        let (mut worker, decode_rx) = test_worker();
        worker.current_generation = 3;
        worker.decode_playing = true;
        let source = VideoSource::File("demo.mp4".into());

        assert!(worker.handle_backend_command(BackendCommand::Load(source.clone())));

        assert!(matches!(
            decode_rx
                .recv()
                .expect("pause command should be sent first"),
            DecodeCommand::SetPlaying {
                generation: 3,
                playing: false,
            }
        ));
        assert!(matches!(
            decode_rx.recv().expect("load command should follow pause"),
            DecodeCommand::Load {
                generation: 4,
                source: queued_source,
            } if queued_source == source
        ));
    }

    #[test]
    fn stop_clears_session_without_shutting_down_decode_worker() {
        let (mut worker, decode_rx) = test_worker();
        worker.current_source = Some(VideoSource::File("demo.mp4".into()));
        worker.current_generation = 7;
        worker.current_duration = Some(Duration::from_secs(30));
        worker.current_intrinsic_size = IntrinsicSize::from_pixels(16, 9);
        worker.current_video_size = VideoSize {
            width: 16,
            height: 9,
        };
        worker.stream_opened = true;
        worker.should_play = true;
        worker.decode_playing = true;
        worker.playback_clock.set_position(Duration::from_secs(3));
        *worker.latest_frame.lock() = Some(VideoRenderFrame::rgba(Arc::new(TextureFrame::new(
            1,
            1,
            vec![255; 4],
        ))));
        worker.shared_queue.replace_generation(7);
        worker.shared_queue.push_frames(vec![QueuedVideoFrame {
            generation: 7,
            position: Duration::from_millis(33),
            end_position: Duration::from_millis(66),
            frame: VideoRenderFrame::rgba(Arc::new(TextureFrame::new(1, 1, vec![255; 4]))),
            compressed_bytes: 4,
            decoded_bytes: 4,
        }]);
        worker
            .shared
            .playback_state
            .set(VideoPlaybackState::Playing);
        worker.shared.video_size.set(worker.current_video_size);
        worker.shared.surface.set(VideoSurfaceSnapshot {
            intrinsic_size: worker.current_intrinsic_size,
            texture: None,
            loading: false,
            error: None,
        });

        let (completed_tx, completed_rx) = bounded(1);
        assert!(worker.handle_backend_command(BackendCommand::Stop {
            completed: completed_tx,
        }));

        completed_rx
            .try_recv()
            .expect("stop should acknowledge completion");
        assert!(worker.current_source.is_none());
        assert_eq!(worker.current_generation, 8);
        assert!(!worker.stream_opened);
        assert!(!worker.should_play);
        assert!(!worker.decode_playing);
        assert_eq!(worker.shared.playback_state.get(), VideoPlaybackState::Idle);
        assert_eq!(worker.shared.video_size.get(), VideoSize::default());
        assert!(worker.latest_frame.lock().is_none());
        assert_eq!(worker.shared_queue.ready_frame_count(7), 0);
        assert_eq!(worker.shared_queue.ready_frame_count(8), 0);
        assert_eq!(worker.playback_clock.position(), Duration::ZERO);

        assert!(matches!(
            decode_rx
                .recv()
                .expect("old generation should be paused first"),
            DecodeCommand::SetPlaying {
                generation: 7,
                playing: false,
            }
        ));
        assert!(matches!(
            decode_rx.recv().expect("decode session should be stopped"),
            DecodeCommand::Stop
        ));

        assert!(worker.handle_backend_command(BackendCommand::Play));
        assert!(matches!(decode_rx.try_recv(), Err(TryRecvError::Empty)));
        assert!(!worker.should_play);
    }

    #[test]
    fn looping_reopens_current_source_at_eof_without_ending() {
        let (mut worker, decode_rx) = test_worker();
        let source = VideoSource::File("demo.mp4".into());
        worker.current_source = Some(source.clone());
        worker.current_generation = 7;
        worker.current_duration = Some(Duration::from_secs(30));
        worker.stream_opened = true;
        worker.should_play = true;
        worker.decode_playing = true;
        worker.looping = true;
        worker.shared.looping.set(true);
        worker
            .shared
            .playback_state
            .set(VideoPlaybackState::Playing);
        worker.shared_queue.replace_generation(7);

        worker.handle_decode_event(DecodeEvent::EofDrained { generation: 7 });

        assert_eq!(worker.current_generation, 8);
        assert!(worker.should_play);
        assert!(!worker.playback_ended);
        assert!(!worker.stream_opened);
        assert_eq!(
            worker.shared.playback_state.get(),
            VideoPlaybackState::Buffering
        );
        assert!(matches!(
            decode_rx
                .recv()
                .expect("old generation should be paused first"),
            DecodeCommand::SetPlaying {
                generation: 7,
                playing: false,
            }
        ));
        assert!(matches!(
            decode_rx.recv().expect("loop should reopen from start"),
            DecodeCommand::Seek {
                generation: 8,
                source: queued_source,
                position,
            } if queued_source == source && position == Duration::ZERO
        ));
    }

    #[test]
    fn set_looping_updates_worker_and_shared_state() {
        let (mut worker, decode_rx) = test_worker();

        assert!(worker.handle_backend_command(BackendCommand::SetLooping(true)));
        assert!(worker.looping);
        assert!(worker.shared.looping.get());
        assert!(matches!(decode_rx.try_recv(), Err(TryRecvError::Empty)));

        assert!(worker.handle_backend_command(BackendCommand::SetLooping(false)));
        assert!(!worker.looping);
        assert!(!worker.shared.looping.get());
    }

    #[test]
    fn set_playback_rate_updates_worker_shared_state_and_decode_worker() {
        let (mut worker, decode_rx) = test_worker();

        assert!(worker.handle_backend_command(BackendCommand::SetPlaybackRate(2.0)));

        assert_eq!(worker.playback_rate, 2.0);
        assert_eq!(worker.shared.playback_rate.get(), 2.0);
        assert!(matches!(
            decode_rx.recv().expect("rate should be forwarded"),
            DecodeCommand::SetPlaybackRate(rate) if (rate - 2.0).abs() <= f32::EPSILON
        ));

        assert!(worker.handle_backend_command(BackendCommand::SetPlaybackRate(99.0)));
        assert_eq!(worker.playback_rate, 4.0);
        assert_eq!(worker.shared.playback_rate.get(), 4.0);
    }

    #[test]
    fn set_audio_track_selection_reopens_current_source_at_current_position() {
        let (mut worker, decode_rx) = test_worker();
        let source = VideoSource::File("demo.mp4".into());
        worker.current_source = Some(source.clone());
        worker.current_generation = 7;
        worker.stream_opened = true;
        worker.decode_playing = true;
        worker.should_play = true;
        worker.software_paused_position = Duration::from_secs(5);
        worker.playback_clock.set_position(Duration::from_secs(5));

        assert!(
            worker.handle_backend_command(BackendCommand::SetAudioTrackSelection(
                VideoAudioTrackSelection::Disabled,
            ))
        );

        assert_eq!(
            worker.audio_track_selection,
            VideoAudioTrackSelection::Disabled
        );
        assert_eq!(
            worker.shared.audio_track_selection.get(),
            VideoAudioTrackSelection::Disabled
        );
        assert_eq!(worker.current_generation, 8);
        assert!(!worker.stream_opened);
        assert!(worker.startup_pending);
        assert!(matches!(
            decode_rx
                .recv()
                .expect("selection should be forwarded first"),
            DecodeCommand::SetAudioTrackSelection(VideoAudioTrackSelection::Disabled)
        ));
        assert!(matches!(
            decode_rx.recv().expect("old generation should be paused"),
            DecodeCommand::SetPlaying {
                generation: 7,
                playing: false,
            }
        ));
        assert!(matches!(
            decode_rx.recv().expect("source should reopen"),
            DecodeCommand::Seek {
                generation: 8,
                source: queued_source,
                position,
            } if queued_source == source && position == Duration::from_secs(5)
        ));
    }

    #[test]
    fn set_audio_track_selection_without_source_only_updates_decode_worker() {
        let (mut worker, decode_rx) = test_worker();

        assert!(
            worker.handle_backend_command(BackendCommand::SetAudioTrackSelection(
                VideoAudioTrackSelection::Stream(4),
            ))
        );

        assert_eq!(
            worker.audio_track_selection,
            VideoAudioTrackSelection::Stream(4)
        );
        assert!(matches!(
            decode_rx.recv().expect("selection should be forwarded"),
            DecodeCommand::SetAudioTrackSelection(VideoAudioTrackSelection::Stream(4))
        ));
        assert!(matches!(decode_rx.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn set_subtitle_track_selection_updates_worker_shared_state_and_decode_worker() {
        let (mut worker, decode_rx) = test_worker();
        let source = VideoSource::File("demo.mp4".into());
        worker.current_source = Some(source.clone());
        worker.current_generation = 7;
        worker.stream_opened = true;
        worker.decode_playing = true;
        worker.should_play = true;
        worker.software_paused_position = Duration::from_secs(5);
        worker.playback_clock.set_position(Duration::from_secs(5));

        assert!(
            worker.handle_backend_command(BackendCommand::SetSubtitleTrackSelection(
                VideoSubtitleTrackSelection::Stream(6),
            ))
        );

        assert_eq!(
            worker.subtitle_track_selection,
            VideoSubtitleTrackSelection::Stream(6)
        );
        assert_eq!(
            worker.shared.subtitle_track_selection.get(),
            VideoSubtitleTrackSelection::Stream(6)
        );
        assert_eq!(worker.current_generation, 8);
        assert!(!worker.stream_opened);
        assert!(worker.startup_pending);
        assert!(matches!(
            decode_rx
                .recv()
                .expect("selection should be forwarded first"),
            DecodeCommand::SetSubtitleTrackSelection(VideoSubtitleTrackSelection::Stream(6))
        ));
        assert!(matches!(
            decode_rx.recv().expect("old generation should be paused"),
            DecodeCommand::SetPlaying {
                generation: 7,
                playing: false,
            }
        ));
        assert!(matches!(
            decode_rx.recv().expect("source should reopen"),
            DecodeCommand::Seek {
                generation: 8,
                source: queued_source,
                position,
            } if queued_source == source && position == Duration::from_secs(5)
        ));
    }

    #[test]
    fn stream_opened_publishes_subtitle_tracks_and_selection() {
        let (mut worker, _decode_rx) = test_worker();
        worker.current_generation = 4;

        let tracks = vec![VideoSubtitleTrack {
            stream_index: 6,
            title: Some("English CC".to_string()),
            language: Some("en".to_string()),
            codec: Some("subrip".to_string()),
        }];

        worker.handle_decode_event(DecodeEvent::StreamOpened(StreamOpenedEvent {
            generation: 4,
            start_position: Duration::from_secs(2),
            duration: Some(Duration::from_secs(60)),
            intrinsic_size: IntrinsicSize::from_pixels(1920, 1080),
            video_size: VideoSize {
                width: 1920,
                height: 1080,
            },
            buffering_profile: LOCAL_BUFFERING_PROFILE,
            audio_clock: None,
            audio_tracks: Vec::new(),
            audio_track_selection: VideoAudioTrackSelection::Auto,
            subtitle_tracks: tracks.clone(),
            subtitle_track_selection: VideoSubtitleTrackSelection::Stream(6),
        }));

        assert!(worker.stream_opened);
        assert_eq!(worker.shared.subtitle_tracks.get(), tracks);
        assert_eq!(
            worker.shared.subtitle_track_selection.get(),
            VideoSubtitleTrackSelection::Stream(6)
        );
    }

    #[test]
    fn subtitle_cue_event_publishes_active_text_and_clears_after_end() {
        let (mut worker, _decode_rx) = test_worker();
        worker.current_generation = 4;
        worker.stream_opened = true;
        worker.software_paused_position = Duration::from_secs(5);

        let cue = VideoSubtitleCue {
            text: "hello".to_string(),
            start: Duration::from_secs(4),
            end: Duration::from_secs(6),
        };

        worker.handle_decode_event(DecodeEvent::SubtitleCue(SubtitleCueEvent {
            generation: 4,
            cue: cue.clone(),
            placement: Some(VideoSubtitleCuePlacement::from_ass_alignment(9).unwrap()),
            style: Some(VideoSubtitleCueStyle {
                primary_color: Some(crate::foundation::color::Color::RED),
                font_weight: None,
                ..Default::default()
            }),
        }));

        assert_eq!(worker.shared.current_subtitle.get(), Some(cue));
        assert_eq!(
            worker.shared.current_subtitle_placement.get(),
            Some(VideoSubtitleCuePlacement::from_ass_alignment(9).unwrap())
        );
        assert_eq!(
            worker.shared.current_subtitle_style.get(),
            Some(VideoSubtitleCueStyle {
                primary_color: Some(crate::foundation::color::Color::RED),
                font_weight: None,
                ..Default::default()
            })
        );

        worker.software_paused_position = Duration::from_secs(7);
        worker.sync_subtitle_cue();

        assert_eq!(worker.shared.current_subtitle.get(), None);
        assert_eq!(worker.shared.current_subtitle_placement.get(), None);
        assert_eq!(worker.shared.current_subtitle_style.get(), None);
    }

    #[test]
    fn subtitle_bitmap_cue_event_publishes_active_bitmap_and_clears_after_end() {
        let (mut worker, _decode_rx) = test_worker();
        worker.current_generation = 4;
        worker.stream_opened = true;
        worker.software_paused_position = Duration::from_secs(5);

        let cue = VideoSubtitleBitmapCue::new(
            8,
            12,
            2,
            1,
            Arc::from(vec![255, 0, 0, 255, 0, 255, 0, 255]),
            Duration::from_secs(4),
            Duration::from_secs(6),
        )
        .expect("valid bitmap subtitle cue");

        worker.handle_decode_event(DecodeEvent::SubtitleBitmapCue(SubtitleBitmapCueEvent {
            generation: 4,
            cue: cue.clone(),
        }));

        assert_eq!(worker.shared.current_subtitle_bitmap.get(), Some(cue));

        worker.software_paused_position = Duration::from_secs(7);
        worker.sync_subtitle_bitmap_cue();

        assert_eq!(worker.shared.current_subtitle_bitmap.get(), None);
    }

    #[test]
    fn playback_rate_rescales_software_clock_without_jump() {
        let (mut worker, _decode_rx) = test_worker();
        worker.stream_opened = true;
        worker.decode_playing = true;
        worker.current_audio_clock = None;
        worker.software_paused_position = Duration::from_secs(3);
        worker.software_play_started_at = Some(std::time::Instant::now());

        assert!(worker.handle_backend_command(BackendCommand::SetPlaybackRate(2.0)));

        assert_eq!(worker.playback_rate, 2.0);
        assert!(worker.software_paused_position >= Duration::from_secs(3));
        assert!(worker.software_play_started_at.is_some());
    }
}
