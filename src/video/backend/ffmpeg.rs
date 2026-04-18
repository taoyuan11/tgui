use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream, SupportedStreamConfig};
use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender, TryRecvError};
use ffmpeg::codec;
use ffmpeg::format;
use ffmpeg::media;
use ffmpeg::software::resampling::context::Context as Resampler;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags as ScalingFlags};
use ffmpeg::util::format::pixel::Pixel;
use ffmpeg::util::frame::{audio::Audio as AudioFrame, video::Video as VideoFrame};
use ffmpeg_next as ffmpeg;

use crate::media::{IntrinsicSize, TextureFrame};
use crate::TguiError;

use super::{BackendSharedState, VideoBackend};
use crate::video::{PlaybackState, VideoSize, VideoSource, VideoSurfaceSnapshot};

// 后台播放线程在非播放态下轮询控制命令的时间间隔。
// 越小，Play/Pause/Seek 响应越快；越大，空闲时更省 CPU。
const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(10);

// 本地文件模式下，视频队列的软上限。
// 使用“时间”而不是“帧数”衡量，避免 60/120fps 视频下缓存变得过浅。
const LOCAL_VIDEO_QUEUE_HIGH_WATER: Duration = Duration::from_secs(5);
// 本地文件模式下，视频队列的硬上限。
// 达到后必须暂停 demux/解码，避免继续累积过多待显示帧。
const LOCAL_VIDEO_QUEUE_HARD_WATER: Duration = Duration::from_secs(6);
// 本地文件模式下，视频队列允许保留的最大帧数保险丝。
// 这个值不是主要策略，只用于防止异常时间戳或损坏源导致无限堆帧。
const LOCAL_VIDEO_MAX_FRAME_COUNT: usize = 120;
// 本地文件模式下，音频缓冲的软上限。
// 正常播放时达到该值后，可暂缓继续读包，避免音频缓存过深。
const LOCAL_AUDIO_QUEUE_HIGH_WATER: Duration = Duration::from_millis(1500);
// 本地文件模式下，音频缓冲的硬上限。
// 无论当前状态如何，达到该值都应强制节流，防止内存持续增长。
const LOCAL_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(3000);

// 网络流模式下，视频队列的软上限。
// 使用时间水位控制，在高帧率视频上也能保持稳定的真实缓存时长。
const NETWORK_VIDEO_QUEUE_HIGH_WATER: Duration = Duration::from_secs(5);
// 网络流模式下，视频队列的硬上限。
// 用比本地更深的视频缓存吸收网络读包抖动。
const NETWORK_VIDEO_QUEUE_HARD_WATER: Duration = Duration::from_secs(6);
// 网络流模式下，视频队列允许保留的最大帧数保险丝。
// 只做安全兜底，不参与正常的时间水位调度。
const NETWORK_VIDEO_MAX_FRAME_COUNT: usize = 300;
// 网络流模式下，音频缓冲的软上限。
// 设得更深，尽量减少网络短抖动引发的反复 Buffering。
const NETWORK_AUDIO_QUEUE_HIGH_WATER: Duration = Duration::from_millis(4000);
// 网络流模式下，音频缓冲的硬上限。
// 防止网络恢复后一次性灌入过多音频样本。
const NETWORK_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(8000);

// 本地文件首次点击播放时，音频至少要攒到多深才开始真正输出。
const LOCAL_START_BUFFER_TARGET: Duration = Duration::from_millis(1000);
// 本地文件从 Buffering 恢复播放时，音频至少要恢复到多深。
const LOCAL_REBUFFER_TARGET: Duration = Duration::from_millis(800);
// 网络流首次播放时的音频启动门槛。
// 比本地更高，用更深缓冲换取更稳定的连续播放。
const NETWORK_START_BUFFER_TARGET: Duration = Duration::from_millis(2500);
// 网络流从 Buffering 恢复播放时的音频门槛。
const NETWORK_REBUFFER_TARGET: Duration = Duration::from_millis(2000);

// 本地文件首次播放时，视频队列至少要领先当前播放位置这么久。
const LOCAL_VIDEO_START_BUFFER_TARGET: Duration = Duration::from_secs(5);
// 本地文件从 Buffering 恢复时，视频队列至少要领先当前播放位置这么久。
const LOCAL_VIDEO_RESUME_BUFFER_TARGET: Duration = Duration::from_secs(5);
// 网络流首次播放时，视频侧需要的最小前置缓存。
const NETWORK_VIDEO_START_BUFFER_TARGET: Duration = Duration::from_secs(5);
// 网络流从 Buffering 恢复时，视频侧需要的最小前置缓存。
const NETWORK_VIDEO_RESUME_BUFFER_TARGET: Duration = Duration::from_secs(5);
// 播放中如果视频前置缓存低于这个值，就主动暂停进入 Buffering。
const VIDEO_REBUFFER_ENTER_THRESHOLD: Duration = Duration::from_secs(2);
// Seek 后丢弃“明显还在目标位置之前”的视频帧时允许的误差。
// 避免因为时间戳微小偏差把应该保留的首帧也丢掉。
const VIDEO_SEEK_PREROLL_TOLERANCE: Duration = Duration::from_millis(50);

// 本地文件模式下，音频缓冲低于这个值就认为快饿死了，需要进入 Buffering。
const LOCAL_AUDIO_STARVING_THRESHOLD: Duration = Duration::from_millis(120);
// 网络流模式下，音频进入“危险区”的阈值。
// 设得更高，好让播放器在真正 underflow 前提前停下来攒缓冲。
const NETWORK_AUDIO_STARVING_THRESHOLD: Duration = Duration::from_millis(250);

// 视频帧显示判定的时间容差。
// 播放位置距离帧时间戳只差一点点时，允许提前呈现，减少卡在临界点的抖动。
const VIDEO_PRESENT_TOLERANCE: Duration = Duration::from_millis(8);
// 当 demux 因缓存已满而暂时空转时，后台线程每次 sleep 的时长。
// 控制“既别忙等烧 CPU，也别睡太久导致恢复迟钝”之间的平衡。
const STEP_IDLE_SLEEP: Duration = Duration::from_millis(4);

static FFMPEG_INIT: Once = Once::new();

enum BackendCommand {
    Load(VideoSource),
    Play,
    Pause,
    Seek(Duration),
    SetVolume(f32),
    SetMuted(bool),
    Shutdown,
}

pub(crate) struct FfmpegVideoBackend {
    command_tx: Sender<BackendCommand>,
    latest_frame: Arc<Mutex<Option<Arc<TextureFrame>>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl FfmpegVideoBackend {
    pub(crate) fn new(shared: BackendSharedState) -> Self {
        let (command_tx, command_rx) = unbounded();
        let latest_frame = Arc::new(Mutex::new(None));
        let worker_frame = latest_frame.clone();
        let worker = thread::spawn(move || worker_main(command_rx, shared, worker_frame));
        Self {
            command_tx,
            latest_frame,
            worker: Mutex::new(Some(worker)),
        }
    }
}

impl Drop for FfmpegVideoBackend {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl VideoBackend for FfmpegVideoBackend {
    fn load(&self, source: VideoSource) -> Result<(), TguiError> {
        self.command_tx
            .send(BackendCommand::Load(source))
            .map_err(|_| TguiError::Media("video backend is unavailable".to_string()))
    }

    fn play(&self) {
        let _ = self.command_tx.send(BackendCommand::Play);
    }

    fn pause(&self) {
        let _ = self.command_tx.send(BackendCommand::Pause);
    }

    fn seek(&self, position: Duration) {
        let _ = self.command_tx.send(BackendCommand::Seek(position));
    }

    fn set_volume(&self, volume: f32) {
        let _ = self.command_tx.send(BackendCommand::SetVolume(volume));
    }

    fn set_muted(&self, muted: bool) {
        let _ = self.command_tx.send(BackendCommand::SetMuted(muted));
    }

    fn current_frame(&self) -> Option<Arc<TextureFrame>> {
        self.latest_frame
            .lock()
            .expect("video frame lock poisoned")
            .clone()
    }

    fn shutdown(&self) {
        let _ = self.command_tx.send(BackendCommand::Shutdown);
        if let Some(worker) = self.worker.lock().expect("worker lock poisoned").take() {
            let _ = worker.join();
        }
    }
}

fn worker_main(
    command_rx: Receiver<BackendCommand>,
    shared: BackendSharedState,
    latest_frame: Arc<Mutex<Option<Arc<TextureFrame>>>>,
) {
    FFMPEG_INIT.call_once(|| {
        let _ = ffmpeg::init();
    });

    let mut current_source: Option<VideoSource> = None;
    let mut current_position = Duration::ZERO;
    let mut should_play = false;
    let mut current_session: Option<PlaybackSession> = None;

    loop {
        let command_result = if should_play {
            match command_rx.try_recv() {
                Ok(command) => Ok(command),
                Err(TryRecvError::Empty) => Err(RecvTimeoutError::Timeout),
                Err(TryRecvError::Disconnected) => Err(RecvTimeoutError::Disconnected),
            }
        } else {
            command_rx.recv_timeout(COMMAND_POLL_INTERVAL)
        };

        match command_result {
            Ok(command) => match command {
                BackendCommand::Shutdown => break,
                BackendCommand::Load(source) => {
                    current_position = Duration::ZERO;
                    should_play = false;
                    current_source = Some(source.clone());
                    clear_latest_frame(&latest_frame);
                    shared.reset_for_load();
                    current_session = Some(
                        match PlaybackSession::open(
                            source,
                            current_position,
                            &shared,
                            &latest_frame,
                        ) {
                            Ok(session) => session,
                            Err(error) => {
                                shared.set_error(error.to_string());
                                continue;
                            }
                        },
                    );
                }
                BackendCommand::Play => {
                    should_play = true;
                    if let Some(session) = current_session.as_mut() {
                        session.set_playing(false);
                        if session.can_start_playback() {
                            session.set_playing(true);
                            shared.playback_state.set(PlaybackState::Playing);
                        } else {
                            shared.playback_state.set(PlaybackState::Buffering);
                        }
                    }
                }
                BackendCommand::Pause => {
                    should_play = false;
                    if let Some(session) = current_session.as_mut() {
                        session.set_playing(false);
                        let mut metrics = shared.metrics.get();
                        metrics.position = session.playback_position();
                        shared.metrics.set(metrics);
                        shared.playback_state.set(PlaybackState::Paused);
                    }
                }
                BackendCommand::Seek(position) => {
                    current_position = position;
                    if let Some(source) = current_source.clone() {
                        if let Some(existing) = current_session.as_mut() {
                            existing.set_playing(false);
                            existing.clear_audio_buffer();
                        }

                        // 不要先清空最后一帧，避免视觉闪一下
                        shared.playback_state.set(PlaybackState::Loading);

                        current_session = Some(
                            match PlaybackSession::open(
                                source,
                                current_position,
                                &shared,
                                &latest_frame,
                            ) {
                                Ok(mut session) => {
                                    session.set_playing(false);

                                    if should_play {
                                        if session.can_resume_playback() {
                                            session.set_playing(true);
                                            shared.playback_state.set(PlaybackState::Playing);
                                        } else {
                                            shared.playback_state.set(PlaybackState::Buffering);
                                        }
                                    } else {
                                        shared.playback_state.set(PlaybackState::Paused);
                                    }

                                    session
                                }
                                Err(error) => {
                                    shared.set_error(error.to_string());
                                    continue;
                                }
                            },
                        );
                    }
                }
                BackendCommand::SetVolume(volume) => {
                    let volume = volume.clamp(0.0, 1.0);
                    shared.volume.set(volume);
                    if let Some(session) = current_session.as_mut() {
                        session.set_volume(volume);
                    }
                }
                BackendCommand::SetMuted(muted) => {
                    shared.muted.set(muted);
                    if let Some(session) = current_session.as_mut() {
                        session.set_muted(muted);
                    }
                }
            },
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        if !should_play {
            continue;
        }

        let step_outcome = match current_session.as_mut() {
            Some(session) => session.step(&shared, &command_rx, &mut current_position),
            None => continue,
        };

        match step_outcome {
            StepOutcome::Continue => {}
            StepOutcome::Restart(position) => {
                current_position = position;
                if let Some(source) = current_source.clone() {
                    shared.playback_state.set(PlaybackState::Loading);

                    if let Some(session) = current_session.as_mut() {
                        session.set_playing(false);
                        session.clear_audio_buffer();
                    }

                    current_session = match PlaybackSession::open(
                        source,
                        current_position,
                        &shared,
                        &latest_frame,
                    ) {
                        Ok(mut reopened) => {
                            reopened.set_playing(false);

                            if should_play {
                                if reopened.can_resume_playback() {
                                    reopened.set_playing(true);
                                    shared.playback_state.set(PlaybackState::Playing);
                                } else {
                                    shared.playback_state.set(PlaybackState::Buffering);
                                }
                            } else {
                                shared.playback_state.set(PlaybackState::Paused);
                            }

                            Some(reopened)
                        }
                        Err(error) => {
                            shared.set_error(error.to_string());
                            None
                        }
                    };
                }
            }
            StepOutcome::Reload { source, position } => {
                current_source = Some(source.clone());
                current_position = position;
                should_play = false;
                if let Some(session) = current_session.as_mut() {
                    session.set_playing(false);
                    session.clear_audio_buffer();
                }
                clear_latest_frame(&latest_frame);
                shared.reset_for_load();
                current_session = Some(
                    match PlaybackSession::open(source, current_position, &shared, &latest_frame) {
                        Ok(session) => session,
                        Err(error) => {
                            shared.set_error(error.to_string());
                            continue;
                        }
                    },
                );
            }
            StepOutcome::Paused(position) => {
                current_position = position;
                should_play = false;
                if let Some(session) = current_session.as_mut() {
                    session.set_playing(false);
                }
                shared.playback_state.set(PlaybackState::Paused);
            }
            StepOutcome::Ended(position) => {
                current_position = position;
                should_play = false;
                if let Some(session) = current_session.as_mut() {
                    session.set_playing(false);
                    session.clear_audio_buffer();
                }
                let mut metrics = shared.metrics.get();
                metrics.position = position;
                shared.metrics.set(metrics);
                shared.playback_state.set(PlaybackState::Ended);
            }
            StepOutcome::Error(error) => {
                should_play = false;
                if let Some(session) = current_session.as_mut() {
                    session.set_playing(false);
                    session.clear_audio_buffer();
                }
                shared.set_error(error);
            }
            StepOutcome::Shutdown => break,
        }
    }
}

fn clear_latest_frame(latest_frame: &Arc<Mutex<Option<Arc<TextureFrame>>>>) {
    *latest_frame.lock().expect("video frame lock poisoned") = None;
}

enum StepOutcome {
    Continue,
    Restart(Duration),
    Reload {
        source: VideoSource,
        position: Duration,
    },
    Paused(Duration),
    Ended(Duration),
    Shutdown,
    Error(String),
}

enum ReceiveVideoOutcome {
    Position(Option<Duration>),
    Command(StepOutcome),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BufferingProfile {
    video_queue_high_water: Duration,
    video_queue_hard_water: Duration,
    video_max_frame_count: usize,
    audio_queue_high_water: Duration,
    audio_queue_hard_water: Duration,
    start_buffer_target: Duration,
    rebuffer_target: Duration,
    video_start_buffer_target: Duration,
    video_resume_buffer_target: Duration,
    audio_starving_threshold: Duration,
}

const LOCAL_BUFFERING_PROFILE: BufferingProfile = BufferingProfile {
    video_queue_high_water: LOCAL_VIDEO_QUEUE_HIGH_WATER,
    video_queue_hard_water: LOCAL_VIDEO_QUEUE_HARD_WATER,
    video_max_frame_count: LOCAL_VIDEO_MAX_FRAME_COUNT,
    audio_queue_high_water: LOCAL_AUDIO_QUEUE_HIGH_WATER,
    audio_queue_hard_water: LOCAL_AUDIO_QUEUE_HARD_WATER,
    start_buffer_target: LOCAL_START_BUFFER_TARGET,
    rebuffer_target: LOCAL_REBUFFER_TARGET,
    video_start_buffer_target: LOCAL_VIDEO_START_BUFFER_TARGET,
    video_resume_buffer_target: LOCAL_VIDEO_RESUME_BUFFER_TARGET,
    audio_starving_threshold: LOCAL_AUDIO_STARVING_THRESHOLD,
};

const NETWORK_BUFFERING_PROFILE: BufferingProfile = BufferingProfile {
    video_queue_high_water: NETWORK_VIDEO_QUEUE_HIGH_WATER,
    video_queue_hard_water: NETWORK_VIDEO_QUEUE_HARD_WATER,
    video_max_frame_count: NETWORK_VIDEO_MAX_FRAME_COUNT,
    audio_queue_high_water: NETWORK_AUDIO_QUEUE_HIGH_WATER,
    audio_queue_hard_water: NETWORK_AUDIO_QUEUE_HARD_WATER,
    start_buffer_target: NETWORK_START_BUFFER_TARGET,
    rebuffer_target: NETWORK_REBUFFER_TARGET,
    video_start_buffer_target: NETWORK_VIDEO_START_BUFFER_TARGET,
    video_resume_buffer_target: NETWORK_VIDEO_RESUME_BUFFER_TARGET,
    audio_starving_threshold: NETWORK_AUDIO_STARVING_THRESHOLD,
};

struct QueuedVideoFrame {
    position: Duration,
    end_position: Duration,
    texture: Arc<TextureFrame>,
}

struct PlaybackSession {
    start_position: Duration,
    duration: Option<Duration>,
    video_frame_duration: Duration,
    input: format::context::Input,
    video_stream_index: usize,
    audio_stream_index: Option<usize>,
    video_decoder: ffmpeg::decoder::Video,
    audio_decoder: Option<ffmpeg::decoder::Audio>,
    scaler: Scaler,
    resampler: Option<Resampler>,
    video_time_base: ffmpeg::Rational,
    audio_time_base: Option<ffmpeg::Rational>,
    audio_output: Option<AudioOutput>,
    latest_frame: Arc<Mutex<Option<Arc<TextureFrame>>>>,
    pending_video_frames: VecDeque<QueuedVideoFrame>,
    buffering_profile: BufferingProfile,
    paused_position: Duration,
    last_presented_position: Duration,
    play_started_at: Option<Instant>,
    eof_sent: bool,
}

impl PlaybackSession {
    fn open(
        source: VideoSource,
        start_position: Duration,
        shared: &BackendSharedState,
        latest_frame: &Arc<Mutex<Option<Arc<TextureFrame>>>>,
    ) -> Result<Self, TguiError> {
        let source_url = match &source {
            VideoSource::File(path) => path
                .to_str()
                .ok_or_else(|| TguiError::Media("video path is not valid UTF-8".to_string()))?
                .to_string(),
            VideoSource::Url(url) => url.clone(),
        };
        let buffering_profile = buffering_profile_for_source(&source);

        let mut input = open_input(&source, &source_url)
            .map_err(|error| TguiError::Media(format!("failed to open video source: {error}")))?;

        if !start_position.is_zero() {
            let timestamp = start_position.as_micros().min(i64::MAX as u128) as i64;
            input.seek(timestamp, ..timestamp).map_err(|error| {
                TguiError::Media(format!("failed to seek video source: {error}"))
            })?;
        }

        let video_stream = input
            .streams()
            .best(media::Type::Video)
            .ok_or_else(|| TguiError::Media("video stream not found".to_string()))?;
        let video_stream_index = video_stream.index();
        let video_time_base = video_stream.time_base();
        let video_context = codec::context::Context::from_parameters(video_stream.parameters())
            .map_err(|error| TguiError::Media(format!("failed to open video codec: {error}")))?;
        let video_decoder = video_context.decoder().video().map_err(|error| {
            TguiError::Media(format!("failed to create video decoder: {error}"))
        })?;
        let scaler = Scaler::get(
            video_decoder.format(),
            video_decoder.width(),
            video_decoder.height(),
            Pixel::RGBA,
            video_decoder.width(),
            video_decoder.height(),
            ScalingFlags::BILINEAR,
        )
        .map_err(|error| TguiError::Media(format!("failed to create video scaler: {error}")))?;

        let intrinsic_size =
            IntrinsicSize::from_pixels(video_decoder.width(), video_decoder.height());
        shared.video_size.set(VideoSize {
            width: video_decoder.width(),
            height: video_decoder.height(),
        });

        let duration = stream_duration(video_stream.duration(), video_time_base);
        let video_frame_duration = stream_frame_duration(&video_stream)
            .unwrap_or(Duration::from_millis(33));
        let audio_stream = input.streams().best(media::Type::Audio);
        let (audio_stream_index, audio_decoder, audio_time_base, resampler, audio_output) =
            if let Some(audio_stream) = audio_stream {
                let audio_stream_index = audio_stream.index();
                let audio_time_base = audio_stream.time_base();
                let audio_context =
                    codec::context::Context::from_parameters(audio_stream.parameters()).map_err(
                        |error| TguiError::Media(format!("failed to open audio codec: {error}")),
                    )?;
                let mut audio_decoder = audio_context.decoder().audio().map_err(|error| {
                    TguiError::Media(format!("failed to create audio decoder: {error}"))
                })?;
                audio_decoder
                    .set_parameters(audio_stream.parameters())
                    .map_err(|error| {
                        TguiError::Media(format!("failed to configure audio decoder: {error}"))
                    })?;
                if audio_decoder.channel_layout().is_empty() {
                    audio_decoder.set_channel_layout(ffmpeg::ChannelLayout::default(
                        audio_decoder.channels().into(),
                    ));
                }
                let audio_output = AudioOutput::new(shared.volume.get(), shared.muted.get())
                    .map_err(|error| {
                        TguiError::Media(format!("failed to create audio output: {error}"))
                    })?;
                let resampler = Resampler::get(
                    audio_decoder.format(),
                    audio_decoder.channel_layout(),
                    audio_decoder.rate(),
                    ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                    ffmpeg::ChannelLayout::default(audio_output.channels().into()),
                    audio_output.sample_rate(),
                )
                .map_err(|error| {
                    TguiError::Media(format!("failed to create audio resampler: {error}"))
                })?;
                (
                    Some(audio_stream_index),
                    Some(audio_decoder),
                    Some(audio_time_base),
                    Some(resampler),
                    Some(audio_output),
                )
            } else {
                (None, None, None, None, None)
            };

        let mut session = Self {
            start_position,
            duration,
            video_frame_duration,
            input,
            video_stream_index,
            audio_stream_index,
            video_decoder,
            audio_decoder,
            scaler,
            resampler,
            video_time_base,
            audio_time_base,
            audio_output,
            latest_frame: latest_frame.clone(),
            pending_video_frames: VecDeque::new(),
            buffering_profile,
            paused_position: start_position,
            last_presented_position: start_position,
            play_started_at: None,
            eof_sent: false,
        };

        let preview_position = session.prime_first_frame(shared)?;
        session.paused_position = preview_position;
        session.last_presented_position = preview_position;
        let mut metrics = shared.metrics.get();
        metrics.duration = duration;
        metrics.position = preview_position;
        metrics.buffered = Some(preview_position);
        metrics.video_width = intrinsic_size.width as u32;
        metrics.video_height = intrinsic_size.height as u32;
        shared.metrics.set(metrics);
        shared.error.set(None);
        shared.surface.set(VideoSurfaceSnapshot {
            intrinsic_size,
            texture: session
                .latest_frame
                .lock()
                .expect("video frame lock poisoned")
                .clone(),
            loading: false,
            error: None,
        });
        shared.playback_state.set(PlaybackState::Ready);

        Ok(session)
    }

    fn update_buffered_metrics(&self, shared: &BackendSharedState) {
        let current = self.playback_position();

        let audio_buffer_end = self.audio_output.as_ref()
            .map(|output| current.saturating_add(output.buffered_duration()));

        let video_buffer_end = self.pending_video_frames.back()
            .map(|frame| frame.end_position);

        let buffered = match (audio_buffer_end, video_buffer_end) {
            (Some(a), Some(v)) => Some(a.min(v)),
            (Some(a), None) => Some(a),
            (None, Some(v)) => Some(v),
            (None, None) => None,
        };

        let mut metrics = shared.metrics.get();
        metrics.position = current;
        metrics.buffered = buffered;
        metrics.video_width = self.video_decoder.width();
        metrics.video_height = self.video_decoder.height();
        shared.metrics.set(metrics);
    }

    fn should_throttle_demux(&self, shared: &BackendSharedState) -> bool {
        let audio_soft_full = self
            .audio_output
            .as_ref()
            .map(|output| output.buffered_duration() >= self.buffering_profile.audio_queue_high_water)
            .unwrap_or(false);

        let audio_hard_full = self
            .audio_output
            .as_ref()
            .map(|output| output.buffered_duration() >= self.buffering_profile.audio_queue_hard_water)
            .unwrap_or(false);

        let video_buffered = self.video_buffered_duration();
        let video_soft_full = video_buffered >= self.buffering_profile.video_queue_high_water;
        let video_hard_full = video_buffered >= self.buffering_profile.video_queue_hard_water
            || self.pending_video_frames.len() >= self.buffering_profile.video_max_frame_count;

        match shared.playback_state.get() {
            PlaybackState::Buffering => {
                // Buffering 期间不要被软阈值卡住
                audio_hard_full && video_hard_full
            }
            _ => {
                should_throttle_demux(
                    audio_soft_full,
                    audio_hard_full,
                    video_soft_full,
                    video_hard_full,
                )
            }
        }
    }

    fn prime_first_frame(&mut self, shared: &BackendSharedState) -> Result<Duration, TguiError> {
        loop {
            let next_packet = {
                let mut packets = self.input.packets();
                packets
                    .next()
                    .map(|(stream, packet)| (stream.index(), packet))
            };
            let Some((stream_index, packet)) = next_packet else {
                break;
            };

            if stream_index != self.video_stream_index {
                continue;
            }

            self.video_decoder.send_packet(&packet).map_err(|error| {
                TguiError::Media(format!("failed to decode preview frame: {error}"))
            })?;
            match self.receive_video_frames(shared, None)? {
                ReceiveVideoOutcome::Position(Some(_)) => {
                    if let Some(position) = self.present_next_video_frame(shared) {
                        return Ok(position);
                    }
                }
                ReceiveVideoOutcome::Position(None) => {}
                ReceiveVideoOutcome::Command(_) => {}
            }
        }

        self.video_decoder.send_eof().map_err(|error| {
            TguiError::Media(format!("failed to flush preview decoder: {error}"))
        })?;
        match self.receive_video_frames(shared, None)? {
            ReceiveVideoOutcome::Position(Some(_)) => {
                if let Some(position) = self.present_next_video_frame(shared) {
                    return Ok(position);
                }
            }
            ReceiveVideoOutcome::Position(None) => {}
            ReceiveVideoOutcome::Command(_) => {}
        }

        Err(TguiError::Media(
            "video source does not contain a decodable frame".to_string(),
        ))
    }

    fn set_playing(&mut self, playing: bool) {
        if let Some(audio_output) = self.audio_output.as_ref() {
            audio_output.set_playing(playing);
        }
        if self.audio_output.is_none() {
            if playing {
                if self.play_started_at.is_none() {
                    self.play_started_at = Some(Instant::now());
                }
            } else if let Some(started_at) = self.play_started_at.take() {
                self.paused_position = self.paused_position.saturating_add(started_at.elapsed());
            }
        }
    }

    fn clear_audio_buffer(&mut self) {
        if let Some(audio_output) = self.audio_output.as_ref() {
            audio_output.clear();
        }
    }

    fn set_volume(&mut self, volume: f32) {
        if let Some(audio_output) = self.audio_output.as_ref() {
            audio_output.set_volume(volume);
        }
    }

    fn set_muted(&mut self, muted: bool) {
        if let Some(audio_output) = self.audio_output.as_ref() {
            audio_output.set_muted(muted);
        }
    }

    fn playback_position(&self) -> Duration {
        if let Some(audio_output) = self.audio_output.as_ref() {
            let elapsed = audio_output.position();
            return self.start_position.saturating_add(elapsed);
        }
        if let Some(started_at) = self.play_started_at {
            return self.paused_position.saturating_add(started_at.elapsed());
        }
        self.paused_position
    }

    fn maybe_enter_buffering(&self, shared: &BackendSharedState) {
        if self.should_keep_draining_eof() {
            return;
        }
        if !self.should_buffer() {
            return;
        }
        if !matches!(&shared.playback_state.get(), PlaybackState::Buffering) {
            shared.playback_state.set(PlaybackState::Buffering);
        }
    }

    fn should_buffer(&self) -> bool {
        let audio_starving = self.audio_output.as_ref()
            .map(|output| output.buffered_duration() < self.buffering_profile.audio_starving_threshold)
            .unwrap_or(false);

        let video_starving = should_buffer_video(
            self.video_buffered_duration(),
            VIDEO_REBUFFER_ENTER_THRESHOLD,
            self.remaining_duration(),
        );

        audio_starving || video_starving
    }

    fn present_next_video_frame(&mut self, shared: &BackendSharedState) -> Option<Duration> {
        let frame = self.pending_video_frames.pop_front()?;
        let position = frame.position;
        let texture = frame.texture;
        *self.latest_frame.lock().expect("video frame lock poisoned") = Some(texture.clone());
        shared.surface.set(VideoSurfaceSnapshot {
            intrinsic_size: IntrinsicSize::from_pixels(
                self.video_decoder.width(),
                self.video_decoder.height(),
            ),
            texture: Some(texture),
            loading: false,
            error: None,
        });

        let mut metrics = shared.metrics.get();
        metrics.position = position;
        metrics.video_width = self.video_decoder.width();
        metrics.video_height = self.video_decoder.height();
        shared.metrics.set(metrics);
        self.last_presented_position = position;
        if self.play_started_at.is_none() {
            self.paused_position = position;
        }
        Some(position)
    }

    fn present_due_video_frames(&mut self, shared: &BackendSharedState) -> Option<Duration> {
        let mut last_position = None;
        while let Some(frame) = self.pending_video_frames.front() {
            if !self.is_frame_due(frame.position) {
                break;
            }
            last_position = self.present_next_video_frame(shared);
        }
        last_position
    }

    fn is_frame_due(&self, position: Duration) -> bool {
        if let Some(audio_output) = self.audio_output.as_ref() {
            if !audio_output.has_started_clock() {
                return false;
            }
            let playback_position = self.start_position.saturating_add(audio_output.position());
            return playback_position.saturating_add(VIDEO_PRESENT_TOLERANCE) >= position;
        }

        self.playback_position()
            .saturating_add(VIDEO_PRESENT_TOLERANCE)
            >= position
    }

    fn should_drop_video_preroll_frame(&self, position: Duration) -> bool {
        !self.start_position.is_zero()
            && position.saturating_add(VIDEO_SEEK_PREROLL_TOLERANCE) < self.start_position
    }

    fn video_buffered_duration_from(&self, baseline: Duration) -> Duration {
        self.pending_video_frames
            .back()
            .map(|last| last.end_position.saturating_sub(baseline))
            .unwrap_or(Duration::ZERO)
    }

    fn remaining_duration(&self) -> Option<Duration> {
        self.duration
            .map(|duration| duration.saturating_sub(self.playback_position()))
    }

    fn video_buffer_target_satisfied(&self, target: Duration) -> bool {
        video_buffer_target_satisfied(
            self.video_buffered_duration(),
            target,
            self.remaining_duration(),
            self.pending_video_frames.len() >= self.buffering_profile.video_max_frame_count,
        )
    }

    fn has_pending_media(&self) -> bool {
        !self.pending_video_frames.is_empty()
            || self
                .audio_output
                .as_ref()
                .map(|output| !output.buffered_duration().is_zero())
                .unwrap_or(false)
    }

    fn should_keep_draining_eof(&self) -> bool {
        self.eof_sent && self.has_pending_media()
    }

    fn step(
        &mut self,
        shared: &BackendSharedState,
        command_rx: &Receiver<BackendCommand>,
        current_position: &mut Duration,
    ) -> StepOutcome {

        eprintln!(
            "[video] state={:?} audio_buf={:?} video_q={} underflow={} can_resume={} pos={:?}",
            shared.playback_state.get(),
            self.audio_output
                .as_ref()
                .map(|o| o.buffered_duration())
                .unwrap_or(Duration::ZERO),
            self.pending_video_frames.len(),
            self.audio_output
                .as_ref()
                .map(|o| o.is_underflowing())
                .unwrap_or(false),
            self.can_resume_playback(),
            self.playback_position(),
        );

        self.update_buffered_metrics(shared);

        let draining_eof = self.should_keep_draining_eof();

        if self.should_buffer() && !draining_eof {
            self.set_playing(false);
            if !matches!(shared.playback_state.get(), PlaybackState::Buffering) {
                shared.playback_state.set(PlaybackState::Buffering);
            }
        } else if matches!(shared.playback_state.get(), PlaybackState::Buffering | PlaybackState::Ready)
            && (self.can_resume_playback() || draining_eof)
        {
            self.set_playing(true);
            shared.playback_state.set(PlaybackState::Playing);
        }

        if let Some(position) = self.present_due_video_frames(shared) {
            *current_position = position;
        }

        if self.should_throttle_demux(shared) {
            if let Some(outcome) = self.process_commands(shared, command_rx, current_position) {
                return outcome;
            }
            thread::sleep(STEP_IDLE_SLEEP);
            return StepOutcome::Continue;
        }

        let next_packet = {
            let mut packets = self.input.packets();
            packets.next().map(|(stream, packet)| (stream.index(), packet))
        };


        match next_packet {
            Some((stream_index, packet)) => {
                if let Some(outcome) = self.process_commands(shared, command_rx, current_position) {
                    return outcome;
                }

                if stream_index == self.video_stream_index {
                    if let Err(error) = self.video_decoder.send_packet(&packet) {
                        return StepOutcome::Error(format!("failed to send video packet: {error}"));
                    }
                    match self.receive_video_frames(shared, Some(command_rx)) {
                        Ok(ReceiveVideoOutcome::Position(Some(_))) => {}
                        Ok(ReceiveVideoOutcome::Position(None)) => {}
                        Ok(ReceiveVideoOutcome::Command(outcome)) => return outcome,
                        Err(error) => return StepOutcome::Error(error.to_string()),
                    }
                } else if Some(stream_index) == self.audio_stream_index {
                    if let (
                        Some(audio_decoder),
                        Some(resampler),
                        Some(audio_time_base),
                        Some(audio_output),
                    ) = (
                        self.audio_decoder.as_mut(),
                        self.resampler.as_mut(),
                        self.audio_time_base,
                        self.audio_output.as_ref(),
                    ) {
                        if let Err(error) = audio_decoder.send_packet(&packet) {
                            return StepOutcome::Error(format!(
                                "failed to send audio packet: {error}"
                            ));
                        }
                        if let Err(error) = receive_audio_frames(
                            audio_decoder,
                            resampler,
                            audio_time_base,
                            audio_output,
                        ) {
                            return StepOutcome::Error(error.to_string());
                        }
                    }
                }

                if let Some(position) = self.present_due_video_frames(shared) {
                    *current_position = position;
                    if !matches!(&shared.playback_state.get(), PlaybackState::Playing) {
                        shared.playback_state.set(PlaybackState::Playing);
                    }
                }
                self.maybe_enter_buffering(shared);

                StepOutcome::Continue
            }
            None => {
                if self.eof_sent {
                    if self.pending_video_frames.is_empty()
                        && self
                            .audio_output
                            .as_ref()
                            .map(|output| output.buffered_duration().is_zero())
                            .unwrap_or(true)
                    {
                        return StepOutcome::Ended(self.playback_position());
                    }
                    if let Some(position) = self.present_due_video_frames(shared) {
                        *current_position = position;
                    }
                    self.maybe_enter_buffering(shared);
                    thread::sleep(Duration::from_millis(4));
                    return StepOutcome::Continue;
                }

                self.eof_sent = true;
                if let Err(error) = self.video_decoder.send_eof() {
                    return StepOutcome::Error(format!("failed to flush video decoder: {error}"));
                }
                match self.receive_video_frames(shared, Some(command_rx)) {
                    Ok(ReceiveVideoOutcome::Position(Some(_))) => {}
                    Ok(ReceiveVideoOutcome::Position(None)) => {}
                    Ok(ReceiveVideoOutcome::Command(outcome)) => return outcome,
                    Err(error) => return StepOutcome::Error(error.to_string()),
                }

                if let (
                    Some(audio_decoder),
                    Some(resampler),
                    Some(audio_time_base),
                    Some(audio_output),
                ) = (
                    self.audio_decoder.as_mut(),
                    self.resampler.as_mut(),
                    self.audio_time_base,
                    self.audio_output.as_ref(),
                ) {
                    let _ = audio_decoder.send_eof();
                    if let Err(error) = receive_audio_frames(
                        audio_decoder,
                        resampler,
                        audio_time_base,
                        audio_output,
                    ) {
                        return StepOutcome::Error(error.to_string());
                    }
                    if let Err(error) = flush_audio_resampler(resampler, audio_output) {
                        return StepOutcome::Error(error.to_string());
                    }
                }

                if let Some(position) = self.present_due_video_frames(shared) {
                    *current_position = position;
                }
                self.maybe_enter_buffering(shared);
                StepOutcome::Continue
            }
        }
    }

    fn process_commands(
        &mut self,
        shared: &BackendSharedState,
        command_rx: &Receiver<BackendCommand>,
        current_position: &mut Duration,
    ) -> Option<StepOutcome> {
        while let Ok(command) = command_rx.try_recv() {
            match command {
                BackendCommand::Play => {
                    if self.can_resume_playback() || self.should_keep_draining_eof() {
                        self.set_playing(true);
                        shared.playback_state.set(PlaybackState::Playing);
                    } else {
                        self.set_playing(false);
                        shared.playback_state.set(PlaybackState::Buffering);
                    }
                }
                BackendCommand::Pause => {
                    let position = self.playback_position();
                    *current_position = position;
                    return Some(StepOutcome::Paused(position));
                }
                BackendCommand::Seek(position) => return Some(StepOutcome::Restart(position)),
                BackendCommand::SetVolume(volume) => self.set_volume(volume.clamp(0.0, 1.0)),
                BackendCommand::SetMuted(muted) => self.set_muted(muted),
                BackendCommand::Load(source) => {
                    return Some(StepOutcome::Reload {
                        source,
                        position: Duration::ZERO,
                    });
                }
                BackendCommand::Shutdown => return Some(StepOutcome::Shutdown),
            }
        }
        None
    }

    fn receive_video_frames(
        &mut self,
        shared: &BackendSharedState,
        command_rx: Option<&Receiver<BackendCommand>>,
    ) -> Result<ReceiveVideoOutcome, TguiError> {
        let mut decoded = VideoFrame::empty();
        let mut last_position = None;

        while self.pending_video_frames.len() < self.buffering_profile.video_max_frame_count
            && self.video_buffered_duration() < self.buffering_profile.video_queue_hard_water
            && self.video_decoder.receive_frame(&mut decoded).is_ok()
        {
            let position = pts_to_duration(decoded.timestamp(), self.video_time_base)
                .unwrap_or(self.start_position);

            if let Some(receiver) = command_rx {
                let mut command_position = position;
                if let Some(outcome) =
                    self.process_commands(shared, receiver, &mut command_position)
                {
                    match outcome {
                        StepOutcome::Restart(_)
                        | StepOutcome::Reload { .. }
                        | StepOutcome::Paused(_)
                        | StepOutcome::Ended(_)
                        | StepOutcome::Shutdown => {
                            return Ok(ReceiveVideoOutcome::Command(outcome));
                        }
                        StepOutcome::Continue => {}
                        StepOutcome::Error(error) => return Err(TguiError::Media(error)),
                    }
                }
            }

            if self.should_drop_video_preroll_frame(position) {
                continue;
            }

            if let Some(previous) = self.pending_video_frames.back_mut() {
                if position > previous.position {
                    previous.end_position = position;
                    self.video_frame_duration = position.saturating_sub(previous.position);
                }
            }

            let texture = Arc::new(video_frame_to_texture(&mut self.scaler, &decoded)?);
            self.pending_video_frames
                .push_back(QueuedVideoFrame {
                    position,
                    end_position: position.saturating_add(self.video_frame_duration),
                    texture,
                });

            last_position = Some(position);
        }

        Ok(ReceiveVideoOutcome::Position(last_position))
    }

    fn audio_buffered_duration(&self) -> Duration {
        self.audio_output
            .as_ref()
            .map(|output| output.buffered_duration())
            .unwrap_or(Duration::ZERO)
    }

    fn video_buffered_duration(&self) -> Duration {
        let baseline = std::cmp::max(self.last_presented_position, self.playback_position());
        self.video_buffered_duration_from(baseline)
    }

    fn can_start_playback(&self) -> bool {
        let audio_ok = self.audio_output.is_none()
            || self.audio_buffered_duration() >= self.buffering_profile.start_buffer_target;
        let video_ok = self.video_buffer_target_satisfied(
            self.buffering_profile.video_start_buffer_target,
        );
        audio_ok && video_ok
    }

    fn can_resume_playback(&self) -> bool {
        let audio_ok = self.audio_output.is_none()
            || self.audio_buffered_duration() >= self.buffering_profile.rebuffer_target;
        let video_ok = self.video_buffer_target_satisfied(
            self.buffering_profile.video_resume_buffer_target,
        );
        audio_ok && video_ok
    }
}

fn buffering_profile_for_source(source: &VideoSource) -> BufferingProfile {
    match source {
        VideoSource::File(_) => LOCAL_BUFFERING_PROFILE,
        VideoSource::Url(_) => NETWORK_BUFFERING_PROFILE,
    }
}

fn stream_frame_duration(stream: &format::stream::Stream<'_>) -> Option<Duration> {
    rational_frame_duration(stream.avg_frame_rate())
        .or_else(|| rational_frame_duration(stream.rate()))
}

fn rational_frame_duration(rate: ffmpeg::Rational) -> Option<Duration> {
    let numerator = rate.numerator();
    let denominator = rate.denominator();
    if numerator <= 0 || denominator <= 0 {
        return None;
    }

    Some(Duration::from_secs_f64(denominator as f64 / numerator as f64))
}

fn http_input_options() -> ffmpeg::Dictionary<'static> {
    let mut options = ffmpeg::Dictionary::new();
    options.set("user_agent", concat!("tgui/", env!("CARGO_PKG_VERSION")));
    options.set("multiple_requests", "1");
    options.set("short_seek_size", "65536");
    options.set("reconnect", "1");
    options.set("reconnect_streamed", "1");
    options.set("reconnect_on_network_error", "1");
    options.set("reconnect_on_http_error", "4xx,5xx");
    options.set("reconnect_delay_max", "2");
    options.set("rw_timeout", "15000000");
    options
}

fn open_input(source: &VideoSource, source_url: &str) -> Result<format::context::Input, ffmpeg::Error> {
    match source {
        VideoSource::File(_) => format::input(&source_url),
        VideoSource::Url(_) => format::input_with_dictionary(&source_url, http_input_options()),
    }
}

fn receive_audio_frames(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut Resampler,
    _time_base: ffmpeg::Rational,
    audio_output: &AudioOutput,
) -> Result<(), TguiError> {
    let mut decoded = AudioFrame::empty();
    while decoder.receive_frame(&mut decoded).is_ok() {
        let mut resampled = allocate_resampled_audio_frame(resampler, &decoded);
        resampler.run(&decoded, &mut resampled).map_err(|error| {
            TguiError::Media(format!("failed to resample audio frame: {error}"))
        })?;
        queue_audio_frame(audio_output, &resampled);
    }
    Ok(())
}

fn flush_audio_resampler(
    resampler: &mut Resampler,
    audio_output: &AudioOutput,
) -> Result<(), TguiError> {
    loop {
        let mut resampled = allocate_flush_audio_frame(resampler);
        match resampler
            .flush(&mut resampled)
            .map_err(|error| TguiError::Media(format!("failed to flush resampler: {error}")))?
        {
            Some(_) => queue_audio_frame(audio_output, &resampled),
            None => break,
        }
    }
    Ok(())
}

fn allocate_resampled_audio_frame(resampler: &Resampler, decoded: &AudioFrame) -> AudioFrame {
    let delay = resampler
        .delay()
        .map(|delay| delay.output.max(0) as usize)
        .unwrap_or(0);
    let input_rate = decoded.rate().max(1) as u64;
    let output_rate = resampler.output().rate.max(1) as u64;
    let scaled_samples =
        ((decoded.samples() as u64 * output_rate) + input_rate.saturating_sub(1)) / input_rate;
    let samples = delay
        .saturating_add(scaled_samples as usize)
        .saturating_add(32)
        .max(1);
    let mut frame = AudioFrame::empty();
    unsafe {
        frame.alloc(
            resampler.output().format,
            samples,
            resampler.output().channel_layout,
        );
    }
    frame
}

fn allocate_flush_audio_frame(resampler: &Resampler) -> AudioFrame {
    let samples = resampler
        .delay()
        .map(|delay| delay.output.max(0) as usize)
        .unwrap_or(0)
        .saturating_add(32)
        .max(1);
    let mut frame = AudioFrame::empty();
    unsafe {
        frame.alloc(
            resampler.output().format,
            samples,
            resampler.output().channel_layout,
        );
    }
    frame
}

fn queue_audio_frame(audio_output: &AudioOutput, frame: &AudioFrame) {
    if frame.samples() == 0 {
        return;
    }
    let samples = audio_frame_to_f32(frame);
    if !samples.is_empty() {
        audio_output.push_samples(&samples);
    }
}

fn audio_frame_to_f32(frame: &AudioFrame) -> Vec<f32> {
    if !frame.is_packed() {
        return Vec::new();
    }

    unsafe {
        let len = frame.samples() * frame.channels() as usize;
        let slice = std::slice::from_raw_parts((*frame.as_ptr()).data[0] as *const f32, len);
        slice.to_vec()
    }
}

fn video_frame_to_texture(
    scaler: &mut Scaler,
    decoded: &VideoFrame,
) -> Result<TextureFrame, TguiError> {
    let mut rgba_frame = VideoFrame::empty();
    scaler
        .run(decoded, &mut rgba_frame)
        .map_err(|error| TguiError::Media(format!("failed to convert video frame: {error}")))?;

    let width = rgba_frame.width();
    let height = rgba_frame.height();
    let stride = rgba_frame.stride(0);
    let data = rgba_frame.data(0);
    let row_len = width as usize * 4;
    let mut pixels = vec![0u8; row_len * height as usize];
    for row in 0..height as usize {
        let src_offset = row * stride;
        let dst_offset = row * row_len;
        pixels[dst_offset..dst_offset + row_len]
            .copy_from_slice(&data[src_offset..src_offset + row_len]);
    }

    Ok(TextureFrame::new(width, height, pixels))
}

fn pts_to_duration(timestamp: Option<i64>, time_base: ffmpeg::Rational) -> Option<Duration> {
    let timestamp = timestamp?;
    let numerator = time_base.numerator() as f64;
    let denominator = time_base.denominator() as f64;
    if denominator <= 0.0 {
        return None;
    }
    let seconds = timestamp as f64 * numerator / denominator;
    Some(Duration::from_secs_f64(seconds.max(0.0)))
}

fn stream_duration(duration: i64, time_base: ffmpeg::Rational) -> Option<Duration> {
    (duration > 0)
        .then_some(duration)
        .and_then(|duration| pts_to_duration(Some(duration), time_base))
}

struct AudioOutput {
    shared: Arc<SharedAudioOutput>,
    _stream: Stream,
    channels: u16,
    sample_rate: u32,
}

struct SharedAudioOutput {
    queue: Mutex<VecDeque<f32>>,
    playing: AtomicBool,
    muted: AtomicBool,
    volume_bits: AtomicU32,
    played_frames: AtomicU64,
    channels: u16,
    underflowing: AtomicBool,
}

impl AudioOutput {
    fn new(volume: f32, muted: bool) -> Result<Self, TguiError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| TguiError::Media("audio output device not found".to_string()))?;
        let config = device.default_output_config().map_err(|error| {
            TguiError::Media(format!("failed to query audio output config: {error}"))
        })?;

        let shared = Arc::new(SharedAudioOutput {
            queue: Mutex::new(VecDeque::new()),
            playing: AtomicBool::new(false),
            muted: AtomicBool::new(muted),
            volume_bits: AtomicU32::new(volume.to_bits()),
            played_frames: AtomicU64::new(0),
            channels: config.channels(),
            underflowing: AtomicBool::new(false),
        });

        let stream = build_output_stream(&device, &config, shared.clone())?;
        stream.play().map_err(|error| {
            TguiError::Media(format!("failed to start audio output stream: {error}"))
        })?;

        Ok(Self {
            shared,
            _stream: stream,
            channels: config.channels(),
            sample_rate: config.sample_rate(),
        })
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn set_playing(&self, playing: bool) {
        self.shared.playing.store(playing, Ordering::SeqCst);
        if !playing {
            self.shared.underflowing.store(false, Ordering::SeqCst);
        }
    }

    fn set_volume(&self, volume: f32) {
        self.shared
            .volume_bits
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::SeqCst);
    }

    fn set_muted(&self, muted: bool) {
        self.shared.muted.store(muted, Ordering::SeqCst);
    }

    fn clear(&self) {
        self.shared
            .queue
            .lock()
            .expect("audio queue lock poisoned")
            .clear();
        self.shared.played_frames.store(0, Ordering::SeqCst);
        self.shared.underflowing.store(false, Ordering::SeqCst);
    }

    fn push_samples(&self, samples: &[f32]) {
        let mut queue = self.shared.queue.lock().expect("audio queue lock poisoned");
        queue.extend(samples.iter().copied());
        self.shared.underflowing.store(false, Ordering::SeqCst);
    }

    fn position(&self) -> Duration {
        let played_frames = self.shared.played_frames.load(Ordering::SeqCst);
        Duration::from_secs_f64(played_frames as f64 / self.sample_rate as f64)
    }

    fn buffered_duration(&self) -> Duration {
        let buffered_samples = self
            .shared
            .queue
            .lock()
            .expect("audio queue lock poisoned")
            .len();
        let buffered_frames = buffered_samples / self.channels as usize;
        Duration::from_secs_f64(buffered_frames as f64 / self.sample_rate as f64)
    }

    fn has_started_clock(&self) -> bool {
        self.shared.played_frames.load(Ordering::SeqCst) > 0
    }

    fn is_underflowing(&self) -> bool {
        self.shared.underflowing.load(Ordering::SeqCst)
    }
}

fn build_output_stream(
    device: &cpal::Device,
    config: &SupportedStreamConfig,
    shared: Arc<SharedAudioOutput>,
) -> Result<Stream, TguiError> {
    let error_callback = |error| eprintln!("tgui video audio stream error: {error}");
    let stream_config = config.config();

    match config.sample_format() {
        SampleFormat::I16 => {
            let shared = shared.clone();
            device
                .build_output_stream(
                    &stream_config,
                    move |buffer: &mut [i16], _| write_audio_samples(buffer, &shared),
                    error_callback,
                    None,
                )
                .map_err(|error| {
                    TguiError::Media(format!("failed to build i16 audio stream: {error}"))
                })
        }
        SampleFormat::U16 => {
            let shared = shared.clone();
            device
                .build_output_stream(
                    &stream_config,
                    move |buffer: &mut [u16], _| write_audio_samples(buffer, &shared),
                    error_callback,
                    None,
                )
                .map_err(|error| {
                    TguiError::Media(format!("failed to build u16 audio stream: {error}"))
                })
        }
        SampleFormat::F32 => device
            .build_output_stream(
                &stream_config,
                move |buffer: &mut [f32], _| write_audio_samples(buffer, &shared),
                error_callback,
                None,
            )
            .map_err(|error| {
                TguiError::Media(format!("failed to build f32 audio stream: {error}"))
            }),
        other => Err(TguiError::Media(format!(
            "unsupported audio sample format: {other:?}"
        ))),
    }
}

fn write_audio_samples<T>(buffer: &mut [T], shared: &Arc<SharedAudioOutput>)
where
    T: Sample + FromSample<f32>,
{
    let playing = shared.playing.load(Ordering::SeqCst);
    let muted = shared.muted.load(Ordering::SeqCst);
    let volume = f32::from_bits(shared.volume_bits.load(Ordering::SeqCst));
    let mut queue = shared.queue.lock().expect("audio queue lock poisoned");
    let mut consumed_samples = 0usize;

    for sample in buffer.iter_mut() {
        let next = if playing {
            match queue.pop_front() {
                Some(sample) => {
                    consumed_samples += 1;
                    sample
                }
                None => {
                    shared.underflowing.store(true, Ordering::SeqCst);
                    0.0
                }
            }
        } else {
            0.0
        };
        let next = if muted { 0.0 } else { next * volume };
        *sample = T::from_sample(next);
    }

    drop(queue);

    if playing && consumed_samples > 0 {
        let consumed_frames = (consumed_samples / shared.channels as usize) as u64;
        shared
            .played_frames
            .fetch_add(consumed_frames, Ordering::SeqCst);
        if consumed_samples == buffer.len() {
            shared.underflowing.store(false, Ordering::SeqCst);
        }
    }
}

fn should_throttle_demux(
    audio_soft_full: bool,
    audio_hard_full: bool,
    video_soft_full: bool,
    video_hard_full: bool,
) -> bool {
    audio_hard_full || video_hard_full || (audio_soft_full && video_soft_full)
}

fn video_buffer_target_satisfied(
    buffered: Duration,
    target: Duration,
    remaining: Option<Duration>,
    frame_cap_reached: bool,
) -> bool {
    buffered >= target
        || frame_cap_reached
        || remaining
            .map(|remaining| buffered.saturating_add(VIDEO_PRESENT_TOLERANCE) >= remaining)
            .unwrap_or(false)
}

fn should_buffer_video(
    buffered: Duration,
    threshold: Duration,
    remaining: Option<Duration>,
) -> bool {
    buffered < threshold
        && !remaining
            .map(|remaining| buffered.saturating_add(VIDEO_PRESENT_TOLERANCE) >= remaining)
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muted_audio_still_advances_clock() {
        let shared = Arc::new(SharedAudioOutput {
            queue: Mutex::new(VecDeque::from(vec![0.25, -0.25, 0.5, -0.5])),
            playing: AtomicBool::new(true),
            muted: AtomicBool::new(true),
            volume_bits: AtomicU32::new(1.0f32.to_bits()),
            played_frames: AtomicU64::new(0),
            channels: 2,
            underflowing: AtomicBool::new(false),
        });

        let mut buffer = [1.0f32; 4];
        write_audio_samples(&mut buffer, &shared);

        assert_eq!(buffer, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(shared.played_frames.load(Ordering::SeqCst), 2);
        assert!(!shared.underflowing.load(Ordering::SeqCst));
    }

    #[test]
    fn demux_keeps_running_when_only_video_reaches_soft_limit() {
        assert!(!should_throttle_demux(false, false, true, false));
    }

    #[test]
    fn demux_throttles_once_both_soft_limits_are_full() {
        assert!(should_throttle_demux(true, false, true, false));
    }

    #[test]
    fn demux_throttles_immediately_on_any_hard_limit() {
        assert!(should_throttle_demux(false, true, false, false));
        assert!(should_throttle_demux(false, false, false, true));
    }

    #[test]
    fn video_buffer_target_accepts_remaining_tail() {
        assert!(video_buffer_target_satisfied(
            Duration::from_millis(900),
            Duration::from_secs(5),
            Some(Duration::from_millis(850)),
            false,
        ));
    }

    #[test]
    fn video_buffer_target_accepts_frame_cap_fallback() {
        assert!(video_buffer_target_satisfied(
            Duration::from_secs(2),
            Duration::from_secs(5),
            Some(Duration::from_secs(20)),
            true,
        ));
    }

    #[test]
    fn should_buffer_video_ignores_tail_section() {
        assert!(!should_buffer_video(
            Duration::from_millis(500),
            Duration::from_secs(2),
            Some(Duration::from_millis(450)),
        ));
        assert!(should_buffer_video(
            Duration::from_millis(500),
            Duration::from_secs(2),
            Some(Duration::from_secs(3)),
        ));
    }

    #[test]
    fn rational_frame_duration_converts_fps_to_frame_span() {
        assert_eq!(
            rational_frame_duration(ffmpeg::Rational(24, 1)),
            Some(Duration::from_secs_f64(1.0 / 24.0))
        );
        assert_eq!(
            rational_frame_duration(ffmpeg::Rational(24000, 1001)),
            Some(Duration::from_secs_f64(1001.0 / 24000.0))
        );
    }

    #[test]
    fn url_sources_use_deeper_buffer_profile() {
        let profile = buffering_profile_for_source(&VideoSource::Url("https://example.com/demo.mp4".to_string()));
        assert_eq!(profile, NETWORK_BUFFERING_PROFILE);
        assert_eq!(profile.video_start_buffer_target, Duration::from_secs(5));
        assert_eq!(profile.video_resume_buffer_target, Duration::from_secs(5));
        assert_eq!(profile.video_queue_high_water, Duration::from_secs(5));
        assert_eq!(profile.video_queue_hard_water, Duration::from_secs(6));
        assert!(profile.audio_queue_hard_water > LOCAL_BUFFERING_PROFILE.audio_queue_hard_water);
        assert!(profile.audio_starving_threshold > LOCAL_BUFFERING_PROFILE.audio_starving_threshold);
    }

    #[test]
    fn http_sources_enable_recovery_and_connection_reuse_options() {
        let options = http_input_options();
        assert_eq!(options.get("multiple_requests"), Some("1"));
        assert_eq!(options.get("short_seek_size"), Some("65536"));
        assert_eq!(options.get("reconnect"), Some("1"));
        assert_eq!(options.get("reconnect_streamed"), Some("1"));
        assert_eq!(options.get("reconnect_on_network_error"), Some("1"));
        assert_eq!(options.get("reconnect_on_http_error"), Some("4xx,5xx"));
        assert_eq!(options.get("rw_timeout"), Some("15000000"));
    }
}
