use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, Once, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{unbounded, Sender};
use ffmpeg::codec;
use ffmpeg::format;
use ffmpeg::media;
use ffmpeg::software::resampling::context::Context as Resampler;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags as ScalingFlags};
use ffmpeg::util::format::pixel::Pixel;
use ffmpeg::util::frame::{audio::Audio as AudioFrame, video::Video as VideoFrame};
use ffmpeg_next as ffmpeg;

use crate::media::{IntrinsicSize, TextureFrame};
use crate::video::{PlaybackState, VideoSize, VideoSource, VideoSurfaceSnapshot};
use crate::TguiError;

use super::{BackendSharedState, VideoBackend};

mod audio;
mod decode;
mod present;

use audio::{AudioOutput, SharedAudioClock};
use decode::decode_main;
use present::present_main;

// 后台控制线程在空闲态下轮询控制命令的时间间隔。
const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(10);

// 本地文件模式下，视频队列的软上限。
const LOCAL_VIDEO_QUEUE_HIGH_WATER: Duration = Duration::from_secs(3);
// 本地文件模式下，视频队列的硬上限。
const LOCAL_VIDEO_QUEUE_HARD_WATER: Duration = Duration::from_secs(4);
// 本地文件模式下，视频队列允许保留的最大帧数保险丝。
const LOCAL_VIDEO_MAX_PACKET_COUNT: usize = 120;
const LOCAL_READY_VIDEO_FRAME_COUNT: usize = 4;
// 本地文件模式下，音频缓冲的软上限。
const LOCAL_AUDIO_QUEUE_HIGH_WATER: Duration = Duration::from_millis(1500);
// 本地文件模式下，音频缓冲的硬上限。
const LOCAL_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(3000);

// 网络流模式下，视频队列的软上限。
const NETWORK_VIDEO_QUEUE_HIGH_WATER: Duration = Duration::from_secs(5);
// 网络流模式下，视频队列的硬上限。
const NETWORK_VIDEO_QUEUE_HARD_WATER: Duration = Duration::from_secs(6);
// 网络流模式下，视频队列允许保留的最大帧数保险丝。
const NETWORK_VIDEO_MAX_PACKET_COUNT: usize = 300;
const NETWORK_READY_VIDEO_FRAME_COUNT: usize = 8;
// 网络流模式下，音频缓冲的软上限。
const NETWORK_AUDIO_QUEUE_HIGH_WATER: Duration = Duration::from_millis(4000);
// 网络流模式下，音频缓冲的硬上限。
const NETWORK_AUDIO_QUEUE_HARD_WATER: Duration = Duration::from_millis(8000);

// 本地文件首次点击播放时，音频至少要攒到多深才开始真正输出。
const LOCAL_START_BUFFER_TARGET: Duration = Duration::from_millis(1000);
// 本地文件从 Buffering 恢复播放时，音频至少要恢复到多深。
const LOCAL_REBUFFER_TARGET: Duration = Duration::from_millis(800);
// 网络流首次播放时的音频启动门槛。
const NETWORK_START_BUFFER_TARGET: Duration = Duration::from_millis(2500);
// 网络流从 Buffering 恢复播放时的音频门槛。
const NETWORK_REBUFFER_TARGET: Duration = Duration::from_millis(2000);

// 本地文件首次播放时，视频队列至少要领先当前播放位置这么久。
const LOCAL_VIDEO_START_BUFFER_TARGET: Duration = Duration::from_millis(1500);
// 本地文件从 Buffering 恢复时，视频队列至少要领先当前播放位置这么久。
const LOCAL_VIDEO_RESUME_BUFFER_TARGET: Duration = Duration::from_millis(800);
// 网络流首次播放时，视频侧需要的最小前置缓存。
const NETWORK_VIDEO_START_BUFFER_TARGET: Duration = Duration::from_secs(5);
// 网络流从 Buffering 恢复时，视频侧需要的最小前置缓存。
const NETWORK_VIDEO_RESUME_BUFFER_TARGET: Duration = Duration::from_secs(5);
// 播放中如果视频前置缓存低于这个值，就主动暂停进入 Buffering。
const VIDEO_REBUFFER_ENTER_THRESHOLD: Duration = Duration::from_secs(2);
// Seek 后丢弃“明显还在目标位置之前”的视频帧时允许的误差。
const VIDEO_SEEK_PREROLL_TOLERANCE: Duration = Duration::from_millis(50);

// 本地文件模式下，音频缓冲低于这个值就认为快饿死了，需要进入 Buffering。
const LOCAL_AUDIO_STARVING_THRESHOLD: Duration = Duration::from_millis(120);
// 网络流模式下，音频进入“危险区”的阈值。
const NETWORK_AUDIO_STARVING_THRESHOLD: Duration = Duration::from_millis(250);

// 视频帧显示判定的时间容差。
const VIDEO_PRESENT_TOLERANCE: Duration = Duration::from_millis(8);
// 当 demux 因缓存已满而暂时空转时，后台线程每次 sleep 的时长。
const STEP_IDLE_SLEEP: Duration = Duration::from_millis(4);

static FFMPEG_INIT: Once = Once::new();
static VIDEO_DEBUG_ENABLED: OnceLock<bool> = OnceLock::new();

macro_rules! video_debug {
    ($($arg:tt)*) => {
        if crate::video::backend::ffmpeg::video_debug_enabled() {
            crate::Log::with_tag("tgui-video").debug(format_args!($($arg)*));
        }
    };
}

pub(super) fn video_debug_enabled() -> bool {
    *VIDEO_DEBUG_ENABLED.get_or_init(|| {
        std::env::var("TGUI_VIDEO_DEBUG")
            .map(|value| {
                let value = value.trim();
                value == "1"
                    || value.eq_ignore_ascii_case("true")
                    || value.eq_ignore_ascii_case("yes")
                    || value.eq_ignore_ascii_case("on")
            })
            .unwrap_or(false)
    })
}

pub(crate) struct FfmpegVideoBackend {
    command_tx: Sender<BackendCommand>,
    latest_frame: Arc<Mutex<Option<Arc<TextureFrame>>>>,
    present_worker: Mutex<Option<JoinHandle<()>>>,
    decode_worker: Mutex<Option<JoinHandle<()>>>,
}

impl FfmpegVideoBackend {
    pub(crate) fn new(shared: BackendSharedState) -> Self {
        FFMPEG_INIT.call_once(|| {
            let _ = ffmpeg::init();
        });

        let (backend_tx, backend_rx) = unbounded();
        let (decode_tx, decode_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();
        let latest_frame = Arc::new(Mutex::new(None));
        let shared_queue = Arc::new(SharedVideoQueue::new());
        let playback_clock = SharedPlaybackClock::default();

        let decode_queue = shared_queue.clone();
        let decode_clock = playback_clock.clone();
        let decode_worker = thread::spawn(move || {
            decode_main(decode_rx, event_tx, decode_queue, decode_clock);
        });

        let present_latest = latest_frame.clone();
        let present_queue = shared_queue;
        let present_clock = playback_clock;
        let present_worker = thread::spawn(move || {
            present_main(
                backend_rx,
                decode_tx,
                event_rx,
                shared,
                present_latest,
                present_queue,
                present_clock,
            );
        });

        Self {
            command_tx: backend_tx,
            latest_frame,
            present_worker: Mutex::new(Some(present_worker)),
            decode_worker: Mutex::new(Some(decode_worker)),
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

    fn set_buffer_memory_limit_bytes(&self, bytes: u64) {
        let _ = self
            .command_tx
            .send(BackendCommand::SetBufferMemoryLimitBytes(bytes));
    }

    fn current_frame(&self) -> Option<Arc<TextureFrame>> {
        self.latest_frame
            .lock()
            .expect("video frame lock poisoned")
            .clone()
    }

    fn shutdown(&self) {
        let _ = self.command_tx.send(BackendCommand::Shutdown);

        if let Some(worker) = self
            .present_worker
            .lock()
            .expect("present worker lock poisoned")
            .take()
        {
            let _ = worker.join();
        }

        if let Some(worker) = self
            .decode_worker
            .lock()
            .expect("decode worker lock poisoned")
            .take()
        {
            let _ = worker.join();
        }
    }
}

#[derive(Clone, Debug)]
enum BackendCommand {
    Load(VideoSource),
    Play,
    Pause,
    Seek(Duration),
    SetVolume(f32),
    SetMuted(bool),
    SetBufferMemoryLimitBytes(u64),
    Shutdown,
}

#[derive(Clone, Debug)]
enum DecodeCommand {
    Load {
        generation: u64,
        source: VideoSource,
    },
    Seek {
        generation: u64,
        source: VideoSource,
        position: Duration,
    },
    SetPlaying {
        generation: u64,
        playing: bool,
    },
    SetVolume(f32),
    SetMuted(bool),
    SetBufferMemoryLimitBytes(u64),
    Shutdown,
}

#[derive(Clone)]
enum DecodeEvent {
    StreamOpened(StreamOpenedEvent),
    FirstFrameReady {
        generation: u64,
        _position: Duration,
    },
    BufferSnapshot(BufferSnapshot),
    EofDrained {
        generation: u64,
    },
    FatalError {
        generation: u64,
        message: String,
    },
}

#[derive(Clone)]
struct StreamOpenedEvent {
    generation: u64,
    start_position: Duration,
    duration: Option<Duration>,
    intrinsic_size: IntrinsicSize,
    video_size: VideoSize,
    buffering_profile: BufferingProfile,
    audio_clock: Option<SharedAudioClock>,
}

#[derive(Clone, Debug, Default)]
struct BufferSnapshot {
    generation: u64,
    eof_sent: bool,
    total_buffered_memory_bytes: u64,
    buffering_constrained_by_memory_limit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenReason {
    Load,
    Seek,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BufferingProfile {
    video_queue_high_water: Duration,
    video_queue_hard_water: Duration,
    video_max_packet_count: usize,
    ready_video_frame_count: usize,
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
    video_max_packet_count: LOCAL_VIDEO_MAX_PACKET_COUNT,
    ready_video_frame_count: LOCAL_READY_VIDEO_FRAME_COUNT,
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
    video_max_packet_count: NETWORK_VIDEO_MAX_PACKET_COUNT,
    ready_video_frame_count: NETWORK_READY_VIDEO_FRAME_COUNT,
    audio_queue_high_water: NETWORK_AUDIO_QUEUE_HIGH_WATER,
    audio_queue_hard_water: NETWORK_AUDIO_QUEUE_HARD_WATER,
    start_buffer_target: NETWORK_START_BUFFER_TARGET,
    rebuffer_target: NETWORK_REBUFFER_TARGET,
    video_start_buffer_target: NETWORK_VIDEO_START_BUFFER_TARGET,
    video_resume_buffer_target: NETWORK_VIDEO_RESUME_BUFFER_TARGET,
    audio_starving_threshold: NETWORK_AUDIO_STARVING_THRESHOLD,
};

#[derive(Clone)]
struct QueuedVideoFrame {
    generation: u64,
    position: Duration,
    end_position: Duration,
    texture: Arc<TextureFrame>,
    compressed_bytes: u64,
}

struct QueuedVideoPacket {
    packet: ffmpeg::Packet,
    end_position: Duration,
}

struct OpenedVideoDecoder {
    decoder: ffmpeg::decoder::Video,
    codec_id: codec::Id,
    decoder_name: String,
}

#[derive(Default)]
struct VideoQueueState {
    frames: VecDeque<QueuedVideoFrame>,
}

struct SharedVideoQueue {
    accepted_generation: AtomicU64,
    state: Mutex<VideoQueueState>,
    condvar: Condvar,
}

impl SharedVideoQueue {
    fn new() -> Self {
        Self {
            accepted_generation: AtomicU64::new(0),
            state: Mutex::new(VideoQueueState::default()),
            condvar: Condvar::new(),
        }
    }

    fn replace_generation(&self, generation: u64) {
        self.accepted_generation.store(generation, Ordering::SeqCst);
        self.clear_all();
    }

    fn accepted_generation(&self) -> u64 {
        self.accepted_generation.load(Ordering::SeqCst)
    }

    fn clear_all(&self) {
        self.state
            .lock()
            .expect("video queue lock poisoned")
            .frames
            .clear();
        self.condvar.notify_all();
    }

    fn push_frames(&self, mut frames: Vec<QueuedVideoFrame>) {
        if frames.is_empty() {
            return;
        }

        let accepted_generation = self.accepted_generation();
        frames.retain(|frame| frame.generation == accepted_generation);
        if frames.is_empty() {
            return;
        }

        let mut state = self.state.lock().expect("video queue lock poisoned");
        let accepted_generation = self.accepted_generation();
        state.frames.extend(
            frames
                .into_iter()
                .filter(|frame| frame.generation == accepted_generation),
        );
        drop(state);
        self.condvar.notify_all();
    }

    fn pop_front_matching(&self, generation: u64) -> Option<QueuedVideoFrame> {
        let mut state = self.state.lock().expect("video queue lock poisoned");
        match state.frames.front() {
            Some(frame) if frame.generation == generation => state.frames.pop_front(),
            _ => None,
        }
    }

    fn front(&self, generation: u64) -> Option<QueuedVideoFrame> {
        self.state
            .lock()
            .expect("video queue lock poisoned")
            .frames
            .iter()
            .find(|frame| frame.generation == generation)
            .cloned()
    }

    fn has_frames(&self, generation: u64) -> bool {
        self.front(generation).is_some()
    }

    fn ready_frame_count(&self, generation: u64) -> usize {
        self.state
            .lock()
            .expect("video queue lock poisoned")
            .frames
            .iter()
            .filter(|frame| frame.generation == generation)
            .count()
    }

    fn ready_memory_bytes(&self, generation: u64) -> u64 {
        self.state
            .lock()
            .expect("video queue lock poisoned")
            .frames
            .iter()
            .filter(|frame| frame.generation == generation)
            .map(|frame| frame.compressed_bytes)
            .sum()
    }

    fn tail_end_position(&self, generation: u64) -> Option<Duration> {
        self.state
            .lock()
            .expect("video queue lock poisoned")
            .frames
            .iter()
            .rev()
            .find(|frame| frame.generation == generation)
            .map(|frame| frame.end_position)
    }

    fn head_frame_memory_bytes(&self, generation: u64) -> Option<u64> {
        self.state
            .lock()
            .expect("video queue lock poisoned")
            .frames
            .iter()
            .find(|frame| frame.generation == generation)
            .map(|frame| frame.compressed_bytes)
            .filter(|bytes| *bytes > 0)
    }
}

#[derive(Clone, Default)]
struct SharedPlaybackClock {
    position_ns: Arc<AtomicU64>,
}

impl SharedPlaybackClock {
    fn set_position(&self, position: Duration) {
        let nanos = position.as_nanos().min(u64::MAX as u128) as u64;
        self.position_ns.store(nanos, Ordering::SeqCst);
    }

    fn position(&self) -> Duration {
        Duration::from_nanos(self.position_ns.load(Ordering::SeqCst))
    }
}

fn clear_latest_frame(latest_frame: &Arc<Mutex<Option<Arc<TextureFrame>>>>) {
    *latest_frame.lock().expect("video frame lock poisoned") = None;
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

    Some(Duration::from_secs_f64(
        denominator as f64 / numerator as f64,
    ))
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

fn open_input(
    source: &VideoSource,
    source_url: &str,
) -> Result<format::context::Input, ffmpeg::Error> {
    match source {
        VideoSource::File(_) => format::input(source_url),
        VideoSource::Url(_) => format::input_with_dictionary(source_url, http_input_options()),
    }
}

fn open_video_decoder(
    stream: &format::stream::Stream<'_>,
) -> Result<OpenedVideoDecoder, TguiError> {
    let parameters = stream.parameters();
    let codec_id = parameters.id();

    if codec_id == codec::Id::AV1 {
        for decoder_name in ["libdav1d", "libaom-av1", "av1"] {
            let Some(codec) = codec::decoder::find_by_name(decoder_name) else {
                continue;
            };
            if !codec.is_video() || codec.id() != codec_id {
                continue;
            }

            match codec::context::Context::from_parameters(parameters.clone())
                .and_then(|context| context.decoder().open_as(codec))
                .and_then(|opened| opened.video())
            {
                Ok(decoder) => {
                    video_debug!(
                        "selected AV1 decoder name={} description={}",
                        codec.name(),
                        codec.description()
                    );
                    return Ok(OpenedVideoDecoder {
                        decoder,
                        codec_id,
                        decoder_name: codec.name().to_string(),
                    });
                }
                Err(error) => {
                    video_debug!(
                        "failed to open AV1 decoder name={} error={}",
                        codec.name(),
                        error
                    );
                }
            }
        }
    }

    let video_context = codec::context::Context::from_parameters(parameters)
        .map_err(|error| TguiError::Media(format!("failed to open video codec: {error}")))?;
    let video_decoder = video_context
        .decoder()
        .video()
        .map_err(|error| TguiError::Media(format!("failed to create video decoder: {error}")))?;

    if let Some(codec) = video_decoder.codec() {
        video_debug!(
            "selected video decoder name={} description={}",
            codec.name(),
            codec.description()
        );
        return Ok(OpenedVideoDecoder {
            decoder: video_decoder,
            codec_id,
            decoder_name: codec.name().to_string(),
        });
    }

    Ok(OpenedVideoDecoder {
        decoder: video_decoder,
        codec_id,
        decoder_name: codec_id.name().to_string(),
    })
}

fn receive_audio_frames(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut Resampler,
    audio_output: &AudioOutput,
    pending_compressed_bytes: &mut u64,
) -> Result<(), TguiError> {
    let mut decoded = AudioFrame::empty();
    let mut chunks = Vec::new();
    while decoder.receive_frame(&mut decoded).is_ok() {
        let mut resampled = allocate_resampled_audio_frame(resampler, &decoded);
        resampler.run(&decoded, &mut resampled).map_err(|error| {
            TguiError::Media(format!("failed to resample audio frame: {error}"))
        })?;
        if let Some(samples) = audio_frame_to_f32_if_any(&resampled) {
            chunks.push(samples);
        }
    }
    queue_audio_chunks(audio_output, chunks, pending_compressed_bytes);
    Ok(())
}

fn flush_audio_resampler(
    resampler: &mut Resampler,
    audio_output: &AudioOutput,
    pending_compressed_bytes: &mut u64,
) -> Result<(), TguiError> {
    let mut chunks = Vec::new();
    loop {
        let mut resampled = allocate_flush_audio_frame(resampler);
        match resampler
            .flush(&mut resampled)
            .map_err(|error| TguiError::Media(format!("failed to flush resampler: {error}")))?
        {
            Some(_) => {
                if let Some(samples) = audio_frame_to_f32_if_any(&resampled) {
                    chunks.push(samples);
                }
            }
            None => break,
        }
    }
    queue_audio_chunks(audio_output, chunks, pending_compressed_bytes);
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

fn queue_audio_chunks(
    audio_output: &AudioOutput,
    chunks: Vec<Vec<f32>>,
    pending_compressed_bytes: &mut u64,
) {
    if chunks.is_empty() {
        return;
    }

    let total_compressed_bytes = std::mem::take(pending_compressed_bytes);
    let total_samples = chunks
        .iter()
        .map(|samples| samples.len() as u64)
        .sum::<u64>()
        .max(1);
    let mut remaining_bytes = total_compressed_bytes;
    let mut remaining_samples = total_samples;

    for samples in chunks {
        let sample_count = samples.len() as u64;
        let chunk_bytes = if remaining_samples == sample_count {
            remaining_bytes
        } else {
            total_compressed_bytes.saturating_mul(sample_count) / total_samples
        };
        remaining_bytes = remaining_bytes.saturating_sub(chunk_bytes);
        remaining_samples = remaining_samples.saturating_sub(sample_count);
        audio_output.push_samples(&samples, chunk_bytes);
    }
}

fn audio_frame_to_f32_if_any(frame: &AudioFrame) -> Option<Vec<f32>> {
    if frame.samples() == 0 || !frame.is_packed() {
        return None;
    }

    unsafe {
        let len = frame.samples() * frame.channels() as usize;
        let slice = std::slice::from_raw_parts((*frame.as_ptr()).data[0] as *const f32, len);
        Some(slice.to_vec())
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

fn packet_duration(duration: i64, time_base: ffmpeg::Rational) -> Option<Duration> {
    (duration > 0)
        .then_some(duration)
        .and_then(|duration| pts_to_duration(Some(duration), time_base))
}

fn stream_duration(duration: i64, time_base: ffmpeg::Rational) -> Option<Duration> {
    (duration > 0)
        .then_some(duration)
        .and_then(|duration| pts_to_duration(Some(duration), time_base))
}

fn should_throttle_demux(
    compressed_buffer_limit_reached: bool,
    audio_hard_full: bool,
    decoded_video_hard_full: bool,
    video_packet_fuse_tripped: bool,
) -> bool {
    compressed_buffer_limit_reached
        || audio_hard_full
        || decoded_video_hard_full
        || video_packet_fuse_tripped
}

fn total_buffered_memory_bytes(
    pending_video_packet_bytes: u64,
    ready_video_frame_bytes: u64,
    audio_buffered_bytes: u64,
) -> u64 {
    pending_video_packet_bytes
        .saturating_add(ready_video_frame_bytes)
        .saturating_add(audio_buffered_bytes)
}

fn startup_playback_blocked_by_memory_limit(
    buffering_constrained_by_memory_limit: bool,
    has_ready_video_frames: bool,
    has_audio_output: bool,
    audio_buffered_duration: Duration,
) -> bool {
    buffering_constrained_by_memory_limit
        && has_ready_video_frames
        && (!has_audio_output || !audio_buffered_duration.is_zero())
}

fn should_buffer_for_rebuffer(
    audio_starving: bool,
    video_starving: bool,
    buffering_constrained_by_memory_limit: bool,
) -> bool {
    audio_starving || (video_starving && !buffering_constrained_by_memory_limit)
}

fn buffering_constrained_by_memory_limit(
    total_buffered_memory_bytes: u64,
    buffer_memory_limit_bytes: u64,
    next_video_frame_memory_bytes: u64,
) -> bool {
    total_buffered_memory_bytes.saturating_add(next_video_frame_memory_bytes)
        > buffer_memory_limit_bytes
}

fn distribute_video_compressed_bytes(frames: &mut [QueuedVideoFrame], compressed_bytes: u64) {
    if frames.is_empty() {
        return;
    }
    let base = compressed_bytes / frames.len() as u64;
    let remainder = compressed_bytes % frames.len() as u64;
    for (index, frame) in frames.iter_mut().enumerate() {
        frame.compressed_bytes = base + u64::from(index < remainder as usize);
    }
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
        let profile = buffering_profile_for_source(&VideoSource::Url(
            "https://example.com/demo.mp4".to_string(),
        ));
        assert_eq!(profile, NETWORK_BUFFERING_PROFILE);
        assert_eq!(profile.video_start_buffer_target, Duration::from_secs(5));
    }

    #[test]
    fn local_sources_use_shallower_video_startup_targets() {
        let profile = buffering_profile_for_source(&VideoSource::File("demo.mp4".into()));
        assert_eq!(profile, LOCAL_BUFFERING_PROFILE);
        assert_eq!(
            profile.video_start_buffer_target,
            Duration::from_millis(1500)
        );
        assert_eq!(profile.ready_video_frame_count, 4);
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

    #[test]
    fn queue_drops_stale_generation_frames() {
        let queue = SharedVideoQueue::new();
        queue.replace_generation(2);
        queue.push_frames(vec![
            QueuedVideoFrame {
                generation: 1,
                position: Duration::ZERO,
                end_position: Duration::from_millis(33),
                texture: Arc::new(TextureFrame::new(1, 1, vec![255; 4])),
                compressed_bytes: 4,
            },
            QueuedVideoFrame {
                generation: 2,
                position: Duration::from_millis(33),
                end_position: Duration::from_millis(66),
                texture: Arc::new(TextureFrame::new(1, 1, vec![255; 4])),
                compressed_bytes: 4,
            },
        ]);

        assert_eq!(queue.ready_frame_count(1), 0);
        assert_eq!(queue.ready_frame_count(2), 1);
    }
}
