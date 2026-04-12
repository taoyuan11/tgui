use super::{
    IntrinsicSize, MediaSource, TextureFrame, VideoConfig, VideoControlCommand,
    VideoPlaybackState,
};
use crate::foundation::binding::InvalidationSignal;
use crate::foundation::error::TguiError;
use bytemuck::cast_slice;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig};
use crossbeam_channel::{
    bounded, unbounded, Receiver, RecvTimeoutError, Sender, TryRecvError, TrySendError,
};
use ffmpeg_next as ffmpeg;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

const AUDIO_BUFFER_AHEAD: Duration = Duration::from_millis(900);
const AUDIO_SYNC_PREROLL: Duration = Duration::from_millis(180);
const PLAYBACK_RESUME_AUDIO_WATERMARK: Duration = Duration::from_millis(900);
const PLAYBACK_RESUME_VIDEO_WATERMARK: Duration = Duration::from_millis(250);
const PLAYBACK_WAIT_SLICE: Duration = Duration::from_millis(8);
const MAX_PENDING_VIDEO_BUFFER: Duration = Duration::from_secs(3);
const MAX_VIDEO_FRAME_QUEUE: usize = 96;
const VIDEO_STALL_GRACE: Duration = Duration::from_millis(180);
const MIN_LOADING_DURATION: Duration = Duration::from_secs(2);
const VIDEO_FREEZE_RECOVERY_THRESHOLD: Duration = Duration::from_millis(700);
const VIDEO_FREEZE_RECOVERY_COOLDOWN: Duration = Duration::from_secs(2);
const VIDEO_PACKET_CHANNEL_CAPACITY: usize = 192;
const VIDEO_FRAME_CHANNEL_CAPACITY: usize = 96;
const NETWORK_OPEN_TIMEOUT: Duration = Duration::from_secs(8);
const NETWORK_RW_TIMEOUT: Duration = Duration::from_secs(10);
const NETWORK_BUFFER_SIZE: usize = 512 * 1024;
const NETWORK_FIFO_SIZE: usize = 2 * 1024 * 1024;
const STREAM_PROBE_SIZE: usize = 512 * 1024;
const STREAM_ANALYZE_DURATION: Duration = Duration::from_millis(1500);

macro_rules! media_log {
    ($($arg:tt)*) => {
        if media_log_enabled() {
            eprintln!("[tgui-media] {}", format!($($arg)*));
        }
    };
}

struct MediaPipeline {
    stop: Arc<AtomicBool>,
    playback_window: Arc<PlaybackWindow>,
    audio: Option<AudioPlayback>,
    video_frames: Receiver<VideoFrameEvent>,
    pending_frames: VecDeque<DecodedVideoFrame>,
    video_ended: bool,
    duration: Option<Duration>,
    frame_interval: Duration,
    intrinsic_size: IntrinsicSize,
}

struct AudioPlayback {
    _stream: Stream,
    shared: Arc<Mutex<AudioOutputShared>>,
    ready_for_sync: Arc<AtomicBool>,
    output_sample_rate: u32,
    decode_channels: u16,
    base_offset: Duration,
}

struct AudioOutputShared {
    samples: VecDeque<f32>,
    queued_frames: u64,
    played_frames: u64,
    output_channels: u16,
    decode_channels: u16,
    muted: bool,
    volume: f32,
    paused: bool,
    eos_received: bool,
}

struct AudioChunk {
    samples: Vec<f32>,
    channels: u16,
}

struct AudioDecoder {
    decoder: ffmpeg::decoder::Audio,
    resampler: ffmpeg::software::resampling::Context,
    output_channels: u16,
    output_rate: u32,
    eof_sent: bool,
}

struct DecodedVideoFrame {
    timestamp: Duration,
    texture: TextureFrame,
}

struct PlaybackWindow {
    position_micros: AtomicU64,
    paused: AtomicBool,
}

enum PacketMessage {
    Packet(ffmpeg::Packet),
    EndOfStream,
}

enum VideoFrameEvent {
    Frame(DecodedVideoFrame),
    EndOfStream,
    Error(String),
}

#[derive(Clone, Copy)]
enum MediaReadyState {
    HaveMetadata,
    HaveCurrentData,
    HaveFutureData,
    HaveEnoughData,
}

impl MediaReadyState {
    fn is_future_data(self) -> bool {
        matches!(self, Self::HaveFutureData | Self::HaveEnoughData)
    }

    fn is_enough_data(self) -> bool {
        matches!(self, Self::HaveEnoughData)
    }
}

impl AudioPlayback {
    fn new(muted: bool, volume: f32, playing: bool, base_offset: Duration) -> Result<Self, TguiError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| TguiError::Media("failed to open default audio output".to_string()))?;
        let supported_config = device.default_output_config().map_err(|error| {
            TguiError::Media(format!("failed to query default audio output: {error}"))
        })?;
        let sample_format = supported_config.sample_format();
        let config = supported_config.config();
        let output_channels = config.channels.max(1);
        let decode_channels = if output_channels <= 1 { 1 } else { 2 };
        let ready_for_sync = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Mutex::new(AudioOutputShared {
            samples: VecDeque::new(),
            queued_frames: 0,
            played_frames: 0,
            output_channels,
            decode_channels,
            muted,
            volume: volume.clamp(0.0, 1.0),
            paused: !playing,
            eos_received: false,
        }));
        let stream = build_audio_output_stream(
            &device,
            &config,
            sample_format,
            shared.clone(),
            ready_for_sync.clone(),
        )?;
        stream
            .play()
            .map_err(|error| TguiError::Media(format!("failed to start audio output: {error}")))?;

        Ok(Self {
            _stream: stream,
            shared,
            ready_for_sync,
            output_sample_rate: config.sample_rate.max(1),
            decode_channels,
            base_offset,
        })
    }

    fn pause(&self) {
        let mut shared = self.shared.lock().expect("audio state lock poisoned");
        shared.paused = true;
    }

    fn resume(&self) {
        let mut shared = self.shared.lock().expect("audio state lock poisoned");
        shared.paused = false;
    }

    fn set_muted(&self, muted: bool) {
        let mut shared = self.shared.lock().expect("audio state lock poisoned");
        shared.muted = muted;
    }

    fn set_volume(&self, volume: f32) {
        let mut shared = self.shared.lock().expect("audio state lock poisoned");
        shared.volume = volume.clamp(0.0, 1.0);
    }

    fn buffered_duration(&self) -> Duration {
        let shared = self.shared.lock().expect("audio state lock poisoned");
        queued_audio_duration(shared.queued_frames, self.output_sample_rate)
    }

    fn position(&self) -> Duration {
        let played_frames = {
            let shared = self.shared.lock().expect("audio state lock poisoned");
            shared.played_frames
        };
        self.base_offset
            + Duration::from_secs_f64(played_frames as f64 / f64::from(self.output_sample_rate))
    }

    fn sync_position(&self) -> Option<Duration> {
        self.ready_for_sync
            .load(Ordering::Acquire)
            .then_some(self.position())
    }

    fn is_ready_for_sync(&self) -> bool {
        self.ready_for_sync.load(Ordering::Acquire)
    }

    fn is_drained(&self) -> bool {
        let shared = self.shared.lock().expect("audio state lock poisoned");
        shared.eos_received && shared.queued_frames == 0 && shared.samples.is_empty()
    }
}

fn build_audio_output_stream(
    device: &cpal::Device,
    config: &StreamConfig,
    sample_format: SampleFormat,
    shared: Arc<Mutex<AudioOutputShared>>,
    ready_for_sync: Arc<AtomicBool>,
) -> Result<Stream, TguiError> {
    let err_fn = move |error| {
        ready_for_sync.store(false, Ordering::Release);
        eprintln!("tgui media audio output error: {error}");
    };

    match sample_format {
        SampleFormat::F32 => build_audio_stream_typed::<f32>(device, config, shared, err_fn),
        SampleFormat::I16 => build_audio_stream_typed::<i16>(device, config, shared, err_fn),
        SampleFormat::U16 => build_audio_stream_typed::<u16>(device, config, shared, err_fn),
        other => Err(TguiError::Media(format!(
            "unsupported audio sample format: {other:?}"
        ))),
    }
}

fn build_audio_stream_typed<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    shared: Arc<Mutex<AudioOutputShared>>,
    err_fn: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, TguiError>
where
    T: SizedSample + FromSample<f32>,
{
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| write_audio_output::<T>(data, &shared),
            err_fn,
            None,
        )
        .map_err(|error| TguiError::Media(format!("failed to build audio output stream: {error}")))
}

fn write_audio_output<T>(data: &mut [T], shared: &Arc<Mutex<AudioOutputShared>>)
where
    T: Sample + FromSample<f32>,
{
    let mut shared = shared.lock().expect("audio state lock poisoned");
    let output_channels = shared.output_channels.max(1) as usize;
    let decode_channels = shared.decode_channels.max(1) as usize;
    let equilibrium = T::EQUILIBRIUM;
    let volume = if shared.muted { 0.0 } else { shared.volume };

    for frame in data.chunks_mut(output_channels) {
        if shared.paused || shared.samples.len() < decode_channels {
            frame.fill(equilibrium);
            continue;
        }

        let source_left = shared.samples.pop_front().unwrap_or(0.0);
        let source_right = if decode_channels > 1 {
            shared.samples.pop_front().unwrap_or(source_left)
        } else {
            source_left
        };

        let mono = (source_left + source_right) * 0.5 * volume;
        for (channel_index, output) in frame.iter_mut().enumerate() {
            let sample = if shared.output_channels == 1 {
                mono
            } else if decode_channels == 1 {
                source_left * volume
            } else {
                match channel_index {
                    0 => source_left * volume,
                    1 => source_right * volume,
                    _ => 0.0,
                }
            };
            *output = T::from_sample(sample);
        }

        shared.queued_frames = shared.queued_frames.saturating_sub(1);
        shared.played_frames = shared.played_frames.saturating_add(1);
    }
}

fn queued_audio_duration(frames: u64, sample_rate: u32) -> Duration {
    if sample_rate == 0 {
        Duration::ZERO
    } else {
        Duration::from_secs_f64(frames as f64 / f64::from(sample_rate))
    }
}

impl AudioDecoder {
    fn new(
        parameters: ffmpeg::codec::Parameters,
        target_sample_rate: u32,
        target_channels: u16,
    ) -> Result<Self, TguiError> {
        let context = ffmpeg::codec::context::Context::from_parameters(parameters).map_err(
            |error| TguiError::Media(format!("failed to create audio decoder: {error}")),
        )?;
        let decoder = context
            .decoder()
            .audio()
            .map_err(|error| TguiError::Media(format!("failed to open audio decoder: {error}")))?;

        let input_layout = normalized_channel_layout(decoder.channel_layout(), decoder.channels());
        let output_layout = ffmpeg::ChannelLayout::default(i32::from(target_channels.max(1)));
        let output_channels = output_layout.channels().max(1) as u16;
        let output_rate = target_sample_rate.max(1);
        let output_format = ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed);
        let resampler = ffmpeg::software::resampling::Context::get(
            decoder.format(),
            input_layout,
            decoder.rate().max(1),
            output_format,
            output_layout,
            output_rate,
        )
        .map_err(|error| TguiError::Media(format!("failed to create audio resampler: {error}")))?;

        Ok(Self {
            decoder,
            resampler,
            output_channels,
            output_rate,
            eof_sent: false,
        })
    }

    fn send_packet(&mut self, packet: &ffmpeg::Packet) -> Result<(), TguiError> {
        self.decoder
            .send_packet(packet)
            .map_err(|error| TguiError::Media(format!("failed to send audio packet: {error}")))
    }

    fn send_eof(&mut self) -> Result<(), TguiError> {
        if self.eof_sent {
            return Ok(());
        }
        self.decoder.send_eof().map_err(|error| {
            TguiError::Media(format!("failed to finalize audio stream: {error}"))
        })?;
        self.eof_sent = true;
        Ok(())
    }

    fn receive_chunk(&mut self) -> Result<Option<AudioChunk>, TguiError> {
        let mut decoded = ffmpeg::util::frame::audio::Audio::empty();
        let mut converted = ffmpeg::util::frame::audio::Audio::empty();
        if self.decoder.receive_frame(&mut decoded).is_err() {
            return Ok(None);
        }

        let samples = resample_audio_frame(
            &mut self.resampler,
            &decoded,
            &mut converted,
            self.output_channels,
        )?;
        if samples.is_empty() {
            return Ok(None);
        }

        Ok(Some(AudioChunk {
            samples,
            channels: self.output_channels,
        }))
    }
}

impl PlaybackWindow {
    fn new(position: Duration, paused: bool) -> Self {
        Self {
            position_micros: AtomicU64::new(duration_to_micros_u64(position)),
            paused: AtomicBool::new(paused),
        }
    }

    fn position(&self) -> Duration {
        Duration::from_micros(self.position_micros.load(Ordering::Acquire))
    }

    fn set_position(&self, position: Duration) {
        self.position_micros
            .store(duration_to_micros_u64(position), Ordering::Release);
    }

    fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Acquire)
    }

    fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Release);
    }
}

impl MediaPipeline {
    fn open(
        source: &MediaSource,
        start_offset: Duration,
        muted: bool,
        volume: f32,
        playing: bool,
        outer_stop: Arc<AtomicBool>,
    ) -> Result<Self, TguiError> {
        let mut input = open_media_input(source, "media source")?;
        let (video_index, video_time_base, duration, frame_interval, video_parameters) = {
            let video_stream = input
                .streams()
                .best(ffmpeg::media::Type::Video)
                .ok_or_else(|| TguiError::Media("video source has no video stream".to_string()))?;
            let time_base = video_stream.time_base();
            let duration = (video_stream.duration() > 0)
                .then_some(duration_from_pts(video_stream.duration(), time_base));
            let frame_interval = frame_interval_from_rate(video_stream.avg_frame_rate())
                .or_else(|| frame_interval_from_rate(video_stream.rate()))
                .unwrap_or(super::DEFAULT_VIDEO_FRAME_INTERVAL);
            (
                video_stream.index(),
                time_base,
                duration,
                frame_interval,
                video_stream.parameters(),
            )
        };
        let audio_stream = input.streams().best(ffmpeg::media::Type::Audio).map(|stream| {
            (
                stream.index(),
                stream.parameters(),
            )
        });

        if !start_offset.is_zero() {
            let timestamp = av_time_from_duration(start_offset);
            input
                .seek(timestamp, ..)
                .map_err(|error| TguiError::Media(format!("failed to seek media stream: {error}")))?;
        }

        let (video_decoder, _) = create_video_decoder(&video_parameters)?;
        let intrinsic_size =
            IntrinsicSize::from_pixels(video_decoder.width(), video_decoder.height());

        let stop = Arc::new(AtomicBool::new(false));
        let playback_window = Arc::new(PlaybackWindow::new(start_offset, !playing));
        let (video_packet_tx, video_packet_rx) = bounded(VIDEO_PACKET_CHANNEL_CAPACITY);
        let (video_frame_tx, video_frame_rx) = bounded(VIDEO_FRAME_CHANNEL_CAPACITY);

        spawn_video_decoder_worker(
            video_parameters,
            video_packet_rx,
            video_frame_tx,
            video_time_base,
            playback_window.clone(),
            stop.clone(),
            outer_stop.clone(),
        );

        let audio = if let Some((audio_index, audio_parameters)) = audio_stream {
            let playback = AudioPlayback::new(muted, volume, playing, start_offset)?;
            let (audio_packet_tx, audio_packet_rx) = unbounded();
            spawn_audio_decoder_worker(
                audio_parameters,
                audio_packet_rx,
                playback.shared.clone(),
                playback.ready_for_sync.clone(),
                playback.output_sample_rate,
                playback.decode_channels,
                stop.clone(),
                outer_stop.clone(),
            );
            spawn_demux_worker(
                input,
                Some((audio_index, audio_packet_tx)),
                (video_index, video_packet_tx),
                stop.clone(),
                outer_stop,
            );
            Some(playback)
        } else {
            spawn_demux_worker(
                input,
                None,
                (video_index, video_packet_tx),
                stop.clone(),
                outer_stop,
            );
            None
        };

        media_log!(
            "open source={} start={:.3}s audio={} duration={:?} frame_interval={:.3}ms",
            source_path(source),
            start_offset.as_secs_f64(),
            audio.is_some(),
            duration,
            frame_interval.as_secs_f64() * 1000.0
        );

        Ok(Self {
            stop,
            playback_window,
            audio,
            video_frames: video_frame_rx,
            pending_frames: VecDeque::with_capacity(MAX_VIDEO_FRAME_QUEUE),
            video_ended: false,
            duration,
            frame_interval,
            intrinsic_size,
        })
    }

    fn pause_audio(&self) {
        if let Some(audio) = self.audio.as_ref() {
            audio.pause();
        }
    }

    fn resume_audio(&self) {
        if let Some(audio) = self.audio.as_ref() {
            audio.resume();
        }
    }

    fn set_muted(&self, muted: bool) {
        if let Some(audio) = self.audio.as_ref() {
            audio.set_muted(muted);
        }
    }

    fn set_volume(&self, volume: f32) {
        if let Some(audio) = self.audio.as_ref() {
            audio.set_volume(volume);
        }
    }

    fn sync_position(&self) -> Option<Duration> {
        self.audio.as_ref().and_then(AudioPlayback::sync_position)
    }

    fn position(&self) -> Option<Duration> {
        self.audio.as_ref().map(AudioPlayback::position)
    }

    fn buffered_video_duration(&self) -> Duration {
        buffered_video_duration(&self.pending_frames, self.frame_interval)
    }

    fn pending_frame_count(&self) -> usize {
        self.pending_frames.len()
    }

    fn next_video_timestamp(&self) -> Option<Duration> {
        self.pending_frames.front().map(|frame| frame.timestamp)
    }

    fn audio_buffered_duration(&self) -> Option<Duration> {
        self.audio.as_ref().map(AudioPlayback::buffered_duration)
    }

    fn drain_video_events(&mut self) -> Result<(), TguiError> {
        self.pump_video_events(None)
    }

    fn prime_video_frame(&mut self, wait_for: Duration) -> Result<Option<DecodedVideoFrame>, TguiError> {
        let deadline = Instant::now() + wait_for;
        loop {
            self.drain_video_events()?;
            if let Some(frame) = self.pending_frames.pop_front() {
                return Ok(Some(frame));
            }
            if self.video_ended || Instant::now() >= deadline {
                return Ok(None);
            }
            self.pump_video_events(Some(PLAYBACK_WAIT_SLICE))?;
        }
    }

    fn pump_video_events(&mut self, wait_for: Option<Duration>) -> Result<(), TguiError> {
        if let Some(timeout) = wait_for {
            match self.video_frames.recv_timeout(timeout) {
                Ok(event) => self.handle_video_event(event)?,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => self.video_ended = true,
            }
        }

        loop {
            if self.pending_frames.len() >= MAX_VIDEO_FRAME_QUEUE
                || self.buffered_video_duration() >= MAX_PENDING_VIDEO_BUFFER
            {
                return Ok(());
            }
            match self.video_frames.try_recv() {
                Ok(event) => self.handle_video_event(event)?,
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => {
                    self.video_ended = true;
                    return Ok(());
                }
            }
        }
    }

    fn handle_video_event(&mut self, event: VideoFrameEvent) -> Result<(), TguiError> {
        match event {
            VideoFrameEvent::Frame(frame) => {
                self.pending_frames.push_back(frame);
                trim_pending_video_frames(&mut self.pending_frames, self.frame_interval);
                Ok(())
            }
            VideoFrameEvent::EndOfStream => {
                self.video_ended = true;
                Ok(())
            }
            VideoFrameEvent::Error(error) => Err(TguiError::Media(error)),
        }
    }
}

impl Drop for MediaPipeline {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn media_log_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("TGUI_MEDIA_LOG")
            .map(|value| {
                let value = value.trim();
                value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("debug")
            })
            .unwrap_or(false)
    })
}

fn spawn_audio_decoder_worker(
    parameters: ffmpeg::codec::Parameters,
    packet_rx: Receiver<PacketMessage>,
    shared: Arc<Mutex<AudioOutputShared>>,
    ready_for_sync: Arc<AtomicBool>,
    target_sample_rate: u32,
    target_channels: u16,
    stop: Arc<AtomicBool>,
    outer_stop: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        media_log!("audio decoder worker started");
        let mut decoder = match AudioDecoder::new(parameters, target_sample_rate, target_channels) {
            Ok(decoder) => decoder,
            Err(error) => {
                ready_for_sync.store(false, Ordering::Release);
                eprintln!("tgui media audio disabled: {error}");
                return;
            }
        };
        let mut decoded_chunks: u64 = 0;

        loop {
            if stop.load(Ordering::Relaxed) || outer_stop.load(Ordering::Relaxed) {
                media_log!("audio decoder worker stopping");
                return;
            }

            let buffered = {
                let shared = shared.lock().expect("audio state lock poisoned");
                queued_audio_duration(shared.queued_frames, decoder.output_rate)
            };
            ready_for_sync.store(buffered >= AUDIO_SYNC_PREROLL, Ordering::Release);

            if buffered >= AUDIO_BUFFER_AHEAD {
                thread::sleep(Duration::from_millis(6));
                continue;
            }

            match decoder.receive_chunk() {
                Ok(Some(chunk)) => {
                    decoded_chunks = decoded_chunks.saturating_add(1);
                    if decoded_chunks % 120 == 0 {
                        media_log!(
                            "audio decoded chunks={} buffered={:.3}s",
                            decoded_chunks,
                            buffered.as_secs_f64()
                        );
                    }
                    push_audio_chunk(&shared, chunk);
                    continue;
                }
                Ok(None) => {}
                Err(error) => {
                    ready_for_sync.store(false, Ordering::Release);
                    eprintln!("tgui media audio disabled: {error}");
                    return;
                }
            }

            if decoder.eof_sent {
                let mut shared = shared.lock().expect("audio state lock poisoned");
                shared.eos_received = true;
                ready_for_sync.store(shared.queued_frames >= 1, Ordering::Release);
                media_log!("audio decoder worker reached eof");
                return;
            }

            match packet_rx.recv_timeout(PLAYBACK_WAIT_SLICE) {
                Ok(PacketMessage::Packet(packet)) => {
                    if let Err(error) = decoder.send_packet(&packet) {
                        ready_for_sync.store(false, Ordering::Release);
                        eprintln!("tgui media audio disabled: {error}");
                        return;
                    }
                }
                Ok(PacketMessage::EndOfStream) => {
                    if let Err(error) = decoder.send_eof() {
                        ready_for_sync.store(false, Ordering::Release);
                        eprintln!("tgui media audio disabled: {error}");
                        return;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    if let Err(error) = decoder.send_eof() {
                        ready_for_sync.store(false, Ordering::Release);
                        eprintln!("tgui media audio disabled: {error}");
                    }
                }
            }
        }
    });
}

fn spawn_video_decoder_worker(
    parameters: ffmpeg::codec::Parameters,
    packet_rx: Receiver<PacketMessage>,
    frame_tx: Sender<VideoFrameEvent>,
    time_base: ffmpeg::Rational,
    playback_window: Arc<PlaybackWindow>,
    stop: Arc<AtomicBool>,
    outer_stop: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        media_log!("video decoder worker started");
        let (mut decoder, mut scaler) = match create_video_decoder(&parameters) {
            Ok(parts) => parts,
            Err(error) => {
                let _ = frame_tx.send(VideoFrameEvent::Error(error.to_string()));
                return;
            }
        };

        let mut decoded = ffmpeg::util::frame::video::Video::empty();
        let mut rgba = ffmpeg::util::frame::video::Video::empty();
        let mut eof_sent = false;
        let mut decoded_frames: u64 = 0;

        loop {
            if stop.load(Ordering::Relaxed) || outer_stop.load(Ordering::Relaxed) {
                media_log!("video decoder worker stopping");
                return;
            }

            while decoder.receive_frame(&mut decoded).is_ok() {
                if let Err(error) = scaler.run(&decoded, &mut rgba) {
                    let _ = frame_tx.send(VideoFrameEvent::Error(format!(
                        "failed to scale video frame: {error}"
                    )));
                    return;
                }
                let timestamp = decoded
                    .timestamp()
                    .map(|value| duration_from_pts(value, time_base))
                    .unwrap_or(Duration::ZERO);
                throttle_video_decode(
                    timestamp,
                    &playback_window,
                    &stop,
                    &outer_stop,
                );
                if stop.load(Ordering::Relaxed) || outer_stop.load(Ordering::Relaxed) {
                    return;
                }
                decoded_frames = decoded_frames.saturating_add(1);
                if decoded_frames % 120 == 0 {
                    media_log!(
                        "video decoded frames={} pts={:.3}s",
                        decoded_frames,
                        timestamp.as_secs_f64()
                    );
                }
                if !send_with_backpressure(
                    &frame_tx,
                    VideoFrameEvent::Frame(DecodedVideoFrame {
                        timestamp,
                        texture: rgba_frame_to_texture(&rgba),
                    }),
                    &stop,
                    &outer_stop,
                ) {
                    return;
                }
            }

            if eof_sent {
                media_log!("video decoder worker reached eof");
                let _ = frame_tx.send(VideoFrameEvent::EndOfStream);
                return;
            }

            match packet_rx.recv_timeout(PLAYBACK_WAIT_SLICE) {
                Ok(PacketMessage::Packet(packet)) => {
                    if let Err(error) = decoder.send_packet(&packet) {
                        let _ = frame_tx.send(VideoFrameEvent::Error(format!(
                            "failed to send video packet: {error}"
                        )));
                        return;
                    }
                }
                Ok(PacketMessage::EndOfStream) => {
                    if let Err(error) = decoder.send_eof() {
                        let _ = frame_tx.send(VideoFrameEvent::Error(format!(
                            "failed to finalize video stream: {error}"
                        )));
                        return;
                    }
                    eof_sent = true;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    if decoder.send_eof().is_ok() {
                        eof_sent = true;
                    } else {
                        let _ = frame_tx.send(VideoFrameEvent::EndOfStream);
                        return;
                    }
                }
            }
        }
    });
}

fn spawn_demux_worker(
    mut input: ffmpeg::format::context::Input,
    audio: Option<(usize, Sender<PacketMessage>)>,
    video: (usize, Sender<PacketMessage>),
    stop: Arc<AtomicBool>,
    outer_stop: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        media_log!(
            "demux worker started audio={} video_stream={}",
            audio.as_ref().map(|(index, _)| *index).map(|value| value.to_string()).unwrap_or_else(|| "none".to_string()),
            video.0
        );
        let mut packets = input.packets();
        let mut audio_packets: u64 = 0;
        let mut video_packets: u64 = 0;
        loop {
            if stop.load(Ordering::Relaxed) || outer_stop.load(Ordering::Relaxed) {
                media_log!("demux worker stopping");
                return;
            }

            match packets.next() {
                Some((stream, packet)) => {
                    let stream_index = stream.index();
                    if let Some((audio_index, audio_tx)) = audio.as_ref() {
                        if stream_index == *audio_index {
                            audio_packets = audio_packets.saturating_add(1);
                            if audio_packets % 240 == 0 {
                                media_log!("demux audio packets={}", audio_packets);
                            }
                            if !send_with_backpressure(
                                audio_tx,
                                PacketMessage::Packet(packet),
                                &stop,
                                &outer_stop,
                            ) {
                                return;
                            }
                            continue;
                        }
                    }
                    if stream_index == video.0
                    {
                        video_packets = video_packets.saturating_add(1);
                        if video_packets % 240 == 0 {
                            media_log!("demux video packets={}", video_packets);
                        }
                        if !send_with_backpressure(
                            &video.1,
                            PacketMessage::Packet(packet),
                            &stop,
                            &outer_stop,
                        ) {
                            return;
                        }
                    }
                }
                None => {
                    media_log!(
                        "demux worker reached eof audio_packets={} video_packets={}",
                        audio_packets,
                        video_packets
                    );
                    if let Some((_, audio_tx)) = audio.as_ref() {
                        let _ = send_with_backpressure(
                            audio_tx,
                            PacketMessage::EndOfStream,
                            &stop,
                            &outer_stop,
                        );
                    }
                    let _ = send_with_backpressure(
                        &video.1,
                        PacketMessage::EndOfStream,
                        &stop,
                        &outer_stop,
                    );
                    return;
                }
            }
        }
    });
}

fn push_audio_chunk(shared: &Arc<Mutex<AudioOutputShared>>, chunk: AudioChunk) {
    let mut shared = shared.lock().expect("audio state lock poisoned");
    let frame_count = chunk.samples.len() / usize::from(chunk.channels.max(1));
    shared.samples.extend(chunk.samples);
    shared.queued_frames = shared.queued_frames.saturating_add(frame_count as u64);
}

fn send_with_backpressure<T>(
    sender: &Sender<T>,
    mut value: T,
    stop: &Arc<AtomicBool>,
    outer_stop: &Arc<AtomicBool>,
) -> bool {
    loop {
        if stop.load(Ordering::Relaxed) || outer_stop.load(Ordering::Relaxed) {
            return false;
        }

        match sender.try_send(value) {
            Ok(_) => return true,
            Err(TrySendError::Full(returned)) => {
                value = returned;
                thread::sleep(Duration::from_millis(4));
            }
            Err(TrySendError::Disconnected(_)) => return false,
        }
    }
}

fn throttle_video_decode(
    timestamp: Duration,
    playback_window: &Arc<PlaybackWindow>,
    stop: &Arc<AtomicBool>,
    outer_stop: &Arc<AtomicBool>,
) {
    let mut logged = false;
    loop {
        if stop.load(Ordering::Relaxed) || outer_stop.load(Ordering::Relaxed) {
            return;
        }

        let playhead = playback_window.position();
        let decode_limit = playhead.saturating_add(MAX_PENDING_VIDEO_BUFFER);
        if timestamp <= decode_limit {
            return;
        }

        if !logged {
            media_log!(
                "video decode throttle playhead={:.3}s frame={:.3}s limit={:.3}s paused={}",
                playhead.as_secs_f64(),
                timestamp.as_secs_f64(),
                decode_limit.as_secs_f64(),
                playback_window.is_paused()
            );
            logged = true;
        }

        thread::sleep(Duration::from_millis(4));
    }
}

pub(super) fn run_video_session(
    source: &MediaSource,
    state: Arc<Mutex<VideoPlaybackState>>,
    invalidation: InvalidationSignal,
    stop: Arc<AtomicBool>,
    config: VideoConfig,
) -> Result<(), TguiError> {
    ffmpeg::init()
        .map_err(|error| TguiError::Media(format!("failed to init ffmpeg: {error}")))?;

    let mut pipeline = MediaPipeline::open(
        source,
        Duration::ZERO,
        config.muted,
        config.volume,
        config.autoplay,
        stop.clone(),
    )?;
    let initial_frame = pipeline.prime_video_frame(Duration::from_millis(250))?;
    let initial_position = initial_frame
        .as_ref()
        .map(|frame| frame.timestamp)
        .unwrap_or(Duration::ZERO);

    let initial_loading = config.autoplay
        && !pipeline
            .ready_state(initial_frame.is_some(), initial_position)
            .is_enough_data();

    {
        let mut guard = state.lock().expect("video state lock poisoned");
        guard.loading = initial_loading;
        guard.error = None;
        guard.intrinsic_size = pipeline.intrinsic_size;
        guard.duration = pipeline.duration;
        guard.frame_interval = pipeline.frame_interval;
        guard.paused = !config.autoplay;
        guard.looping = config.looping;
        guard.muted = config.muted;
        guard.volume = config.volume;
        guard.ended = false;
        guard.position = initial_position;
        guard.texture = initial_frame.map(|frame| Arc::new(frame.texture));
        guard.publish_controller_snapshot();
    }
    invalidation.mark_dirty();

    let mut anchor_position = initial_position;
    let mut anchor_instant = Instant::now();
    let mut last_presented_position = initial_position;
    let mut last_presented_instant = Instant::now();
    let mut last_recovery_instant = Instant::now()
        .checked_sub(VIDEO_FREEZE_RECOVERY_COOLDOWN)
        .unwrap_or_else(Instant::now);
    let mut waiting = initial_loading;
    let mut loading_since = initial_loading.then_some(Instant::now());

    loop {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }

        pipeline.drain_video_events()?;
        let commands = drain_controller_commands(&state);
        if !commands.is_empty() {
            apply_controller_commands(
                source,
                &state,
                &invalidation,
                &mut pipeline,
                &mut anchor_position,
                &mut anchor_instant,
                &stop,
                commands,
            )?;
        }

        let state_loading = {
            state
                .lock()
                .expect("video state lock poisoned")
                .loading
        };
        if state_loading {
            loading_since.get_or_insert_with(Instant::now);
        } else {
            loading_since = None;
        }

        let (paused, looping, has_texture) = {
            let guard = state.lock().expect("video state lock poisoned");
            (guard.paused, guard.looping, guard.texture.is_some())
        };

        if paused {
            pipeline.playback_window.set_paused(true);
            pipeline.playback_window.set_position(anchor_position);
            pipeline.pause_audio();
            if !has_texture {
                if let Some(frame) = pipeline.take_first_frame() {
                    {
                        let mut guard = state.lock().expect("video state lock poisoned");
                        guard.texture = Some(Arc::new(frame.texture));
                        guard.position = frame.timestamp;
                        guard.loading = false;
                        guard.publish_controller_snapshot();
                    }
                    invalidation.mark_dirty();
                    anchor_position = {
                        let guard = state.lock().expect("video state lock poisoned");
                        guard.position
                    };
                    anchor_instant = Instant::now();
                }
            }
            thread::sleep(Duration::from_millis(12));
            continue;
        }

        let media_position = pipeline
            .sync_position()
            .or_else(|| pipeline.position())
            .unwrap_or_else(|| {
                anchor_position.saturating_add(Instant::now().saturating_duration_since(anchor_instant))
            });
        let ready_state = pipeline.ready_state(has_texture, media_position);
        if !ready_state.is_future_data() && !pipeline.is_finished() {
            pipeline.playback_window.set_paused(true);
            pipeline.pause_audio();
            anchor_position = {
                let guard = state.lock().expect("video state lock poisoned");
                guard.position
            };
            anchor_instant = Instant::now();
            pipeline.playback_window.set_position(anchor_position);
            if !waiting {
                waiting = true;
                loading_since.get_or_insert_with(Instant::now);
                media_log!(
                    "enter waiting media={:.3}s next_video={:?} video_buffer={:.3}s audio_buffer={:?} pending_frames={}",
                    media_position.as_secs_f64(),
                    pipeline.next_video_timestamp().map(|value| value.as_secs_f64()),
                    pipeline.buffered_video_duration().as_secs_f64(),
                    pipeline.audio_buffered_duration().map(|value| value.as_secs_f64()),
                    pipeline.pending_frame_count()
                );
            }
            set_video_loading_state(&state, &invalidation, true);
            thread::sleep(Duration::from_millis(12));
            continue;
        }

        let loading_elapsed = loading_since
            .map(|since| Instant::now().saturating_duration_since(since))
            .unwrap_or(MIN_LOADING_DURATION);
        if (ready_state.is_enough_data() || pipeline.is_finished())
            && (pipeline.is_finished() || loading_elapsed >= MIN_LOADING_DURATION)
        {
            if waiting {
                media_log!(
                    "resume playback media={:.3}s next_video={:?} video_buffer={:.3}s audio_buffer={:?} pending_frames={}",
                    media_position.as_secs_f64(),
                    pipeline.next_video_timestamp().map(|value| value.as_secs_f64()),
                    pipeline.buffered_video_duration().as_secs_f64(),
                    pipeline.audio_buffered_duration().map(|value| value.as_secs_f64()),
                    pipeline.pending_frame_count()
                );
            }
            waiting = false;
            loading_since = None;
            set_video_loading_state(&state, &invalidation, false);
        } else if waiting {
            pipeline.playback_window.set_paused(true);
            pipeline.pause_audio();
            anchor_position = {
                let guard = state.lock().expect("video state lock poisoned");
                guard.position
            };
            anchor_instant = Instant::now();
            pipeline.playback_window.set_position(anchor_position);
            set_video_loading_state(&state, &invalidation, true);
            thread::sleep(Duration::from_millis(12));
            continue;
        }
        pipeline.playback_window.set_paused(false);
        pipeline.playback_window.set_position(media_position);
        pipeline.resume_audio();

        if let Some(frame) = pipeline.take_frame_for(media_position) {
            {
                let mut guard = state.lock().expect("video state lock poisoned");
                if !guard.paused {
                    guard.texture = Some(Arc::new(frame.texture));
                    guard.position = frame.timestamp;
                    guard.ended = false;
                    guard.error = None;
                    guard.loading = false;
                    guard.publish_controller_snapshot();
                }
            }
            invalidation.mark_dirty();
            last_presented_position = frame.timestamp;
            last_presented_instant = Instant::now();
            pipeline.playback_window.set_position(frame.timestamp);
            waiting = false;
        } else {
            if pipeline.should_wait_for_video(media_position) && !pipeline.is_finished() {
                pipeline.playback_window.set_paused(true);
                pipeline.pause_audio();
                anchor_position = {
                    let guard = state.lock().expect("video state lock poisoned");
                    guard.position
                };
                anchor_instant = Instant::now();
                pipeline.playback_window.set_position(anchor_position);
                if !waiting {
                    waiting = true;
                    loading_since.get_or_insert_with(Instant::now);
                    media_log!(
                        "wait for video frame media={:.3}s next_video={:?} video_buffer={:.3}s audio_buffer={:?} pending_frames={}",
                        media_position.as_secs_f64(),
                        pipeline.next_video_timestamp().map(|value| value.as_secs_f64()),
                        pipeline.buffered_video_duration().as_secs_f64(),
                        pipeline.audio_buffered_duration().map(|value| value.as_secs_f64()),
                        pipeline.pending_frame_count()
                    );
                }
                set_video_loading_state(&state, &invalidation, true);
                if Instant::now().saturating_duration_since(last_recovery_instant)
                    >= VIDEO_FREEZE_RECOVERY_COOLDOWN
                    && loading_since
                        .map(|since| Instant::now().saturating_duration_since(since))
                        .unwrap_or(Duration::ZERO)
                        >= MIN_LOADING_DURATION
                {
                    media_log!(
                        "waiting recovery media={:.3}s last_presented={:.3}s next_video={:?} video_buffer={:.3}s audio_buffer={:?} pending_frames={}",
                        media_position.as_secs_f64(),
                        last_presented_position.as_secs_f64(),
                        pipeline.next_video_timestamp().map(|value| value.as_secs_f64()),
                        pipeline.buffered_video_duration().as_secs_f64(),
                        pipeline.audio_buffered_duration().map(|value| value.as_secs_f64()),
                        pipeline.pending_frame_count()
                    );
                    let recovered_position = restart_from_position(
                        source,
                        media_position,
                        true,
                        false,
                        &state,
                        &invalidation,
                        &mut pipeline,
                        &stop,
                    )?;
                    anchor_position = recovered_position;
                    anchor_instant = Instant::now();
                    last_presented_position = recovered_position;
                    last_presented_instant = Instant::now();
                    last_recovery_instant = Instant::now();
                    waiting = true;
                    loading_since = Some(Instant::now());
                    continue;
                }
                thread::sleep(Duration::from_millis(12));
                continue;
            }

            if !pipeline.is_finished()
                && media_position
                    > last_presented_position.saturating_add(video_stall_threshold(pipeline.frame_interval))
                && Instant::now().saturating_duration_since(last_presented_instant)
                    >= VIDEO_FREEZE_RECOVERY_THRESHOLD
                && Instant::now().saturating_duration_since(last_recovery_instant)
                    >= VIDEO_FREEZE_RECOVERY_COOLDOWN
            {
                media_log!(
                    "video freeze recovery media={:.3}s last_presented={:.3}s next_video={:?} video_buffer={:.3}s audio_buffer={:?} pending_frames={}",
                    media_position.as_secs_f64(),
                    last_presented_position.as_secs_f64(),
                    pipeline.next_video_timestamp().map(|value| value.as_secs_f64()),
                    pipeline.buffered_video_duration().as_secs_f64(),
                    pipeline.audio_buffered_duration().map(|value| value.as_secs_f64()),
                    pipeline.pending_frame_count()
                );
                let recovered_position = restart_from_position(
                    source,
                    media_position,
                    true,
                    false,
                    &state,
                    &invalidation,
                    &mut pipeline,
                    &stop,
                )?;
                anchor_position = recovered_position;
                anchor_instant = Instant::now();
                last_presented_position = recovered_position;
                last_presented_instant = Instant::now();
                last_recovery_instant = Instant::now();
                waiting = true;
                continue;
            }

            let clamped = pipeline
                .duration
                .map(|duration| media_position.min(duration))
                .unwrap_or(media_position);
            {
                let mut guard = state.lock().expect("video state lock poisoned");
                if !guard.paused {
                    guard.position = clamped;
                    guard.publish_controller_snapshot();
                }
            }
            pipeline.playback_window.set_position(clamped);
        }

        if pipeline.is_finished() {
            if looping {
                let loop_position = restart_from_position(
                    source,
                    Duration::ZERO,
                    true,
                    false,
                    &state,
                    &invalidation,
                    &mut pipeline,
                    &stop,
                )?;
                anchor_position = loop_position;
                anchor_instant = Instant::now();
                continue;
            }

            let mut guard = state.lock().expect("video state lock poisoned");
            guard.paused = true;
            guard.ended = true;
            if let Some(duration) = pipeline.duration {
                guard.position = duration;
            }
            guard.publish_controller_snapshot();
            invalidation.mark_dirty();
            return Ok(());
        }

        thread::sleep(PLAYBACK_WAIT_SLICE.min(pipeline.frame_interval / 2));
    }
}

fn drain_controller_commands(state: &Arc<Mutex<VideoPlaybackState>>) -> Vec<VideoControlCommand> {
    let controller = state
        .lock()
        .expect("video state lock poisoned")
        .controller
        .clone();
    controller
        .map(|controller| controller.take_commands())
        .unwrap_or_default()
}

fn apply_controller_commands(
    source: &MediaSource,
    state: &Arc<Mutex<VideoPlaybackState>>,
    invalidation: &InvalidationSignal,
    pipeline: &mut MediaPipeline,
    anchor_position: &mut Duration,
    anchor_instant: &mut Instant,
    stop: &Arc<AtomicBool>,
    commands: Vec<VideoControlCommand>,
) -> Result<(), TguiError> {
    for command in commands {
        match command {
            VideoControlCommand::Play => {
                let should_restart = {
                    let guard = state.lock().expect("video state lock poisoned");
                    guard.ended
                };
                if should_restart {
                    *anchor_position = restart_from_position(
                        source,
                        Duration::ZERO,
                        true,
                        false,
                        state,
                        invalidation,
                        pipeline,
                        stop,
                    )?;
                    *anchor_instant = Instant::now();
                } else {
                    let mut guard = state.lock().expect("video state lock poisoned");
                    if guard.error.is_some() {
                        continue;
                    }
                    guard.paused = false;
                    guard.ended = false;
                    *anchor_position = guard.position;
                    *anchor_instant = Instant::now();
                    guard.publish_controller_snapshot();
                    invalidation.mark_dirty();
                }
            }
            VideoControlCommand::Pause => {
                pipeline.playback_window.set_paused(true);
                pipeline.pause_audio();
                let mut guard = state.lock().expect("video state lock poisoned");
                guard.paused = true;
                guard.publish_controller_snapshot();
                invalidation.mark_dirty();
            }
            VideoControlCommand::Resume => {
                let mut guard = state.lock().expect("video state lock poisoned");
                if guard.error.is_some() || guard.ended {
                    continue;
                }
                guard.paused = false;
                *anchor_position = guard.position;
                *anchor_instant = Instant::now();
                pipeline.playback_window.set_paused(false);
                pipeline.playback_window.set_position(*anchor_position);
                guard.publish_controller_snapshot();
                invalidation.mark_dirty();
            }
            VideoControlCommand::Replay => {
                *anchor_position = restart_from_position(
                    source,
                    Duration::ZERO,
                    true,
                    false,
                    state,
                    invalidation,
                    pipeline,
                    stop,
                )?;
                *anchor_instant = Instant::now();
            }
            VideoControlCommand::SeekTime(target) => {
                let should_play = {
                    let guard = state.lock().expect("video state lock poisoned");
                    !guard.paused && guard.error.is_none()
                };
                *anchor_position = restart_from_position(
                    source,
                    target,
                    should_play,
                    true,
                    state,
                    invalidation,
                    pipeline,
                    stop,
                )?;
                *anchor_instant = Instant::now();
            }
            VideoControlCommand::SetMuted(muted) => {
                let mut guard = state.lock().expect("video state lock poisoned");
                guard.muted = muted;
                guard.publish_controller_snapshot();
                invalidation.mark_dirty();
                pipeline.set_muted(muted);
            }
            VideoControlCommand::SetVolume(volume) => {
                let mut guard = state.lock().expect("video state lock poisoned");
                guard.volume = volume;
                guard.publish_controller_snapshot();
                invalidation.mark_dirty();
                pipeline.set_volume(volume);
            }
            VideoControlCommand::SetLooping(looping) => {
                let mut guard = state.lock().expect("video state lock poisoned");
                guard.looping = looping;
                guard.publish_controller_snapshot();
                invalidation.mark_dirty();
            }
        }
    }
    Ok(())
}

fn restart_from_position(
    source: &MediaSource,
    target: Duration,
    playing: bool,
    emit_seek: bool,
    state: &Arc<Mutex<VideoPlaybackState>>,
    invalidation: &InvalidationSignal,
    pipeline: &mut MediaPipeline,
    stop: &Arc<AtomicBool>,
) -> Result<Duration, TguiError> {
    let (duration, muted, volume) = {
        let guard = state.lock().expect("video state lock poisoned");
        (guard.duration, guard.muted, guard.volume)
    };
    let target = duration.map(|value| target.min(value)).unwrap_or(target);

    *pipeline = MediaPipeline::open(source, target, muted, volume, playing, stop.clone())?;
    let preview = pipeline.prime_video_frame(Duration::from_millis(250))?;
    let resolved_position = preview
        .as_ref()
        .map(|frame| frame.timestamp)
        .unwrap_or(target);
    let loading = playing
        && !pipeline
            .ready_state(preview.is_some(), resolved_position)
            .is_enough_data();

    {
        let mut guard = state.lock().expect("video state lock poisoned");
        guard.loading = loading;
        guard.error = None;
        guard.intrinsic_size = pipeline.intrinsic_size;
        guard.duration = pipeline.duration;
        guard.frame_interval = pipeline.frame_interval;
        guard.paused = !playing;
        guard.ended = false;
        if emit_seek {
            guard.seek_generation = guard.seek_generation.saturating_add(1);
        }
        guard.position = resolved_position;
        guard.texture = preview.map(|frame| Arc::new(frame.texture));
        guard.publish_controller_snapshot();
    }
    invalidation.mark_dirty();
    Ok(resolved_position)
}

fn resample_audio_frame(
    resampler: &mut ffmpeg::software::resampling::Context,
    decoded: &ffmpeg::util::frame::audio::Audio,
    converted: &mut ffmpeg::util::frame::audio::Audio,
    output_channels: u16,
) -> Result<Vec<f32>, TguiError> {
    *converted = ffmpeg::util::frame::audio::Audio::empty();
    resampler
        .run(decoded, converted)
        .map_err(|error| TguiError::Media(format!("failed to resample audio frame: {error}")))?;
    Ok(samples_from_audio_frame(converted, output_channels))
}

fn samples_from_audio_frame(
    frame: &ffmpeg::util::frame::audio::Audio,
    output_channels: u16,
) -> Vec<f32> {
    let frame_samples = frame.samples();
    if frame_samples == 0 {
        return Vec::new();
    }

    let sample_count = frame_samples * output_channels as usize;
    let byte_count = sample_count * std::mem::size_of::<f32>();
    let bytes = frame.data(0);
    if bytes.len() < byte_count {
        return Vec::new();
    }
    cast_slice::<u8, f32>(&bytes[..byte_count]).to_vec()
}

fn normalized_channel_layout(
    layout: ffmpeg::ChannelLayout,
    channels: u16,
) -> ffmpeg::ChannelLayout {
    if layout.is_empty() {
        ffmpeg::ChannelLayout::default(i32::from(channels.max(1)))
    } else {
        layout
    }
}

fn rgba_frame_to_texture(frame: &ffmpeg::util::frame::video::Video) -> TextureFrame {
    let width = frame.width();
    let height = frame.height();
    let stride = frame.stride(0);
    let data = frame.data(0);
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    for row in 0..height as usize {
        let src_start = row * stride;
        let src_end = src_start + width as usize * 4;
        let dst_start = row * width as usize * 4;
        let dst_end = dst_start + width as usize * 4;
        pixels[dst_start..dst_end].copy_from_slice(&data[src_start..src_end]);
    }
    TextureFrame::new(width, height, pixels)
}

fn buffered_video_duration(
    frames: &VecDeque<DecodedVideoFrame>,
    frame_interval: Duration,
) -> Duration {
    let Some(front) = frames.front() else {
        return Duration::ZERO;
    };
    let Some(back) = frames.back() else {
        return frame_interval;
    };
    back.timestamp
        .checked_sub(front.timestamp)
        .unwrap_or(Duration::ZERO)
        .saturating_add(frame_interval)
}

fn trim_pending_video_frames(frames: &mut VecDeque<DecodedVideoFrame>, frame_interval: Duration) {
    while frames.len() > MAX_VIDEO_FRAME_QUEUE
        || buffered_video_duration(frames, frame_interval) > MAX_PENDING_VIDEO_BUFFER
    {
        frames.pop_back();
    }
}

fn video_stall_threshold(frame_interval: Duration) -> Duration {
    VIDEO_STALL_GRACE.max(frame_interval.saturating_mul(3))
}

fn duration_from_pts(value: i64, time_base: ffmpeg::Rational) -> Duration {
    let seconds = (value as f64)
        * (f64::from(time_base.numerator()) / f64::from(time_base.denominator()));
    Duration::from_secs_f64(seconds.max(0.0))
}

fn frame_interval_from_rate(rate: ffmpeg::Rational) -> Option<Duration> {
    let numerator = rate.numerator();
    let denominator = rate.denominator();
    if numerator <= 0 || denominator <= 0 {
        return None;
    }

    let fps = f64::from(numerator) / f64::from(denominator);
    if !fps.is_finite() || fps <= 0.0 {
        return None;
    }

    Some(Duration::from_secs_f64(
        (1.0 / fps).clamp(1.0 / 240.0, 0.25),
    ))
}

fn av_time_from_duration(value: Duration) -> i64 {
    (value.as_secs_f64() * f64::from(ffmpeg::ffi::AV_TIME_BASE)).round() as i64
}

fn set_video_loading_state(
    state: &Arc<Mutex<VideoPlaybackState>>,
    invalidation: &InvalidationSignal,
    loading: bool,
) {
    let mut guard = state.lock().expect("video state lock poisoned");
    if guard.loading == loading {
        return;
    }
    guard.loading = loading;
    guard.publish_controller_snapshot();
    invalidation.mark_dirty();
}

fn source_path(source: &MediaSource) -> String {
    match source {
        MediaSource::Path(path) => path.to_string_lossy().into_owned(),
        MediaSource::Url(url) => url.clone(),
    }
}

fn open_media_input(
    source: &MediaSource,
    label: &str,
) -> Result<ffmpeg::format::context::Input, TguiError> {
    let source_path = source_path(source);
    ffmpeg::format::input_with_dictionary(&source_path, input_options(source)).map_err(|error| {
        TguiError::Media(format!("failed to open {label} {source_path}: {error}"))
    })
}

fn input_options(source: &MediaSource) -> ffmpeg::Dictionary<'static> {
    let mut options = ffmpeg::Dictionary::new();
    options.set("probesize", &STREAM_PROBE_SIZE.to_string());
    options.set(
        "analyzeduration",
        &duration_to_micros(STREAM_ANALYZE_DURATION),
    );
    options.set("fflags", "genpts+discardcorrupt");

    if matches!(source, MediaSource::Url(_)) {
        options.set("rw_timeout", &duration_to_micros(NETWORK_RW_TIMEOUT));
        options.set("timeout", &duration_to_micros(NETWORK_OPEN_TIMEOUT));
        options.set("buffer_size", &NETWORK_BUFFER_SIZE.to_string());
        options.set("fifo_size", &NETWORK_FIFO_SIZE.to_string());
        options.set("reconnect", "1");
        options.set("reconnect_streamed", "1");
        options.set("reconnect_on_network_error", "1");
        options.set("reconnect_on_http_error", "4xx,5xx");
        options.set("reconnect_delay_max", "2");
    }

    options
}

fn duration_to_micros(value: Duration) -> String {
    value.as_micros().max(1).to_string()
}

fn duration_to_micros_u64(value: Duration) -> u64 {
    value.as_micros().min(u128::from(u64::MAX)) as u64
}

fn create_video_decoder(
    parameters: &ffmpeg::codec::Parameters,
) -> Result<
    (
        ffmpeg::decoder::Video,
        ffmpeg::software::scaling::Context,
    ),
    TguiError,
> {
    if should_try_cuda_decoder() {
        for decoder_name in preferred_cuda_decoders(parameters.id()) {
            let Some(codec) = ffmpeg::codec::decoder::find_by_name(decoder_name) else {
                continue;
            };
            let context = ffmpeg::codec::context::Context::from_parameters(parameters.clone())
                .map_err(|error| {
                    TguiError::Media(format!("failed to create video decoder: {error}"))
                })?;
            let mut decoder_builder = context.decoder();
            configure_decoder_tolerance(&mut decoder_builder);
            if let Ok(opened) = decoder_builder.open_as(codec).and_then(|opened| opened.video()) {
                if let Ok(scaler) = create_video_scaler(&opened) {
                    return Ok((opened, scaler));
                }
            }
        }
    }

    let context = ffmpeg::codec::context::Context::from_parameters(parameters.clone()).map_err(
        |error| TguiError::Media(format!("failed to create video decoder: {error}")),
    )?;
    let mut decoder_builder = context.decoder();
    configure_decoder_tolerance(&mut decoder_builder);
    let decoder = decoder_builder
        .video()
        .map_err(|error| TguiError::Media(format!("failed to open video decoder: {error}")))?;
    let scaler = create_video_scaler(&decoder)?;
    Ok((decoder, scaler))
}

fn configure_decoder_tolerance(decoder: &mut ffmpeg::decoder::Decoder) {
    decoder.conceal(
        ffmpeg::decoder::Conceal::GUESS_MVS
            | ffmpeg::decoder::Conceal::DEBLOCK
            | ffmpeg::decoder::Conceal::FAVOR_INTER,
    );
    decoder.check(ffmpeg::decoder::Check::IGNORE_ERROR);
}

fn create_video_scaler(
    decoder: &ffmpeg::decoder::Video,
) -> Result<ffmpeg::software::scaling::Context, TguiError> {
    ffmpeg::software::scaling::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        ffmpeg::format::Pixel::RGBA,
        decoder.width(),
        decoder.height(),
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )
    .map_err(|error| TguiError::Media(format!("failed to create video scaler: {error}")))
}

fn should_try_cuda_decoder() -> bool {
    std::env::var("TGUI_VIDEO_DISABLE_HWACCEL")
        .map(|value| {
            let value = value.trim();
            !(value == "1" || value.eq_ignore_ascii_case("true"))
        })
        .unwrap_or(true)
}

fn preferred_cuda_decoders(codec_id: ffmpeg::codec::Id) -> &'static [&'static str] {
    match codec_id {
        ffmpeg::codec::Id::H264 => &["h264_cuvid"],
        ffmpeg::codec::Id::HEVC => &["hevc_cuvid"],
        ffmpeg::codec::Id::MPEG2VIDEO => &["mpeg2_cuvid"],
        ffmpeg::codec::Id::VP8 => &["vp8_cuvid"],
        ffmpeg::codec::Id::VP9 => &["vp9_cuvid"],
        ffmpeg::codec::Id::AV1 => &["av1_cuvid"],
        _ => &[],
    }
}

impl MediaPipeline {
    fn ready_state(&self, has_texture: bool, media_position: Duration) -> MediaReadyState {
        let has_current = has_texture || !self.pending_frames.is_empty();
        if !has_current {
            return MediaReadyState::HaveMetadata;
        }

        let video_buffer = buffered_video_duration(&self.pending_frames, self.frame_interval);
        let video_future = !self.should_wait_for_video(media_position);
        let audio_future = self
            .audio
            .as_ref()
            .map(|audio| audio.is_ready_for_sync() || audio.buffered_duration() >= AUDIO_SYNC_PREROLL)
            .unwrap_or(true);
        let audio_enough = self
            .audio
            .as_ref()
            .map(|audio| {
                audio.buffered_duration() >= PLAYBACK_RESUME_AUDIO_WATERMARK
                    || (self.video_ended && audio.is_drained())
            })
            .unwrap_or(true);

        if video_future
            && (video_buffer >= PLAYBACK_RESUME_VIDEO_WATERMARK || self.video_ended)
            && audio_enough
        {
            MediaReadyState::HaveEnoughData
        } else if video_future
            && ((video_buffer >= self.frame_interval) || self.video_ended)
            && audio_future
        {
            MediaReadyState::HaveFutureData
        } else {
            MediaReadyState::HaveCurrentData
        }
    }

    fn take_first_frame(&mut self) -> Option<DecodedVideoFrame> {
        self.pending_frames.pop_front()
    }

    fn take_frame_for(&mut self, position: Duration) -> Option<DecodedVideoFrame> {
        let late_threshold = self.frame_interval.max(Duration::from_millis(90));
        while self.pending_frames.len() > 1 {
            let next_timestamp = self.pending_frames.get(1)?.timestamp;
            if next_timestamp.saturating_add(late_threshold) <= position {
                self.pending_frames.pop_front();
            } else {
                break;
            }
        }

        let front = self.pending_frames.front()?;
        if front.timestamp <= position.saturating_add(late_threshold) || self.video_ended {
            self.pending_frames.pop_front()
        } else {
            None
        }
    }

    fn should_wait_for_video(&self, position: Duration) -> bool {
        if self.video_ended {
            return false;
        }

        let Some(front) = self.pending_frames.front() else {
            return true;
        };

        front.timestamp > position.saturating_add(video_stall_threshold(self.frame_interval))
    }

    fn is_finished(&self) -> bool {
        let video_done = self.video_ended && self.pending_frames.is_empty();
        let audio_done = self
            .audio
            .as_ref()
            .map(AudioPlayback::is_drained)
            .unwrap_or(true);
        video_done && audio_done
    }
}
