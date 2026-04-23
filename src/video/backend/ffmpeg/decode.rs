use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TryRecvError};

use super::*;

pub(super) fn decode_main(
    command_rx: Receiver<DecodeCommand>,
    event_tx: Sender<DecodeEvent>,
    shared_queue: Arc<SharedVideoQueue>,
    playback_clock: SharedPlaybackClock,
) {
    let mut worker = DecodeWorker::new(command_rx, event_tx, shared_queue, playback_clock);
    worker.run();
}

struct DecodeWorker {
    command_rx: Receiver<DecodeCommand>,
    event_tx: Sender<DecodeEvent>,
    shared_queue: Arc<SharedVideoQueue>,
    playback_clock: SharedPlaybackClock,
    volume: f32,
    muted: bool,
    buffer_memory_limit_bytes: u64,
    session: Option<DecodeSession>,
}

impl DecodeWorker {
    fn new(
        command_rx: Receiver<DecodeCommand>,
        event_tx: Sender<DecodeEvent>,
        shared_queue: Arc<SharedVideoQueue>,
        playback_clock: SharedPlaybackClock,
    ) -> Self {
        Self {
            command_rx,
            event_tx,
            shared_queue,
            playback_clock,
            volume: 1.0,
            muted: false,
            buffer_memory_limit_bytes: super::super::DEFAULT_VIDEO_BUFFER_MEMORY_LIMIT_BYTES,
            session: None,
        }
    }

    fn run(&mut self) {
        loop {
            let command_result = if self.session.is_some() {
                match self.command_rx.try_recv() {
                    Ok(command) => Ok(command),
                    Err(TryRecvError::Empty) => Err(RecvTimeoutError::Timeout),
                    Err(TryRecvError::Disconnected) => Err(RecvTimeoutError::Disconnected),
                }
            } else {
                self.command_rx.recv_timeout(COMMAND_POLL_INTERVAL)
            };

            match command_result {
                Ok(command) => {
                    if !self.handle_command(command) {
                        break;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }

            let Some(session) = self.session.as_mut() else {
                continue;
            };

            match session.step() {
                Ok(DecodeStepOutcome::Continue { snapshot_changed }) => {
                    if snapshot_changed {
                        let _ = self
                            .event_tx
                            .send(DecodeEvent::BufferSnapshot(session.snapshot()));
                    }
                }
                Ok(DecodeStepOutcome::Idle { snapshot_changed }) => {
                    if snapshot_changed {
                        let _ = self
                            .event_tx
                            .send(DecodeEvent::BufferSnapshot(session.snapshot()));
                    }
                    thread::sleep(STEP_IDLE_SLEEP);
                }
                Ok(DecodeStepOutcome::EofDrained) => {
                    let generation = session.generation;
                    let _ = self
                        .event_tx
                        .send(DecodeEvent::BufferSnapshot(session.snapshot()));
                    let _ = self.event_tx.send(DecodeEvent::EofDrained { generation });
                    thread::sleep(STEP_IDLE_SLEEP);
                }
                Err(error) => {
                    let generation = session.generation;
                    let _ = self.event_tx.send(DecodeEvent::FatalError {
                        generation,
                        message: error.to_string(),
                    });
                    self.session = None;
                }
            }
        }
    }

    fn handle_command(&mut self, command: DecodeCommand) -> bool {
        match command {
            DecodeCommand::Load { generation, source } => {
                self.open_session(OpenReason::Load, generation, source, Duration::ZERO);
            }
            DecodeCommand::Seek {
                generation,
                source,
                position,
            } => {
                self.open_session(OpenReason::Seek, generation, source, position);
            }
            DecodeCommand::SetPlaying {
                generation,
                playing,
            } => {
                if let Some(session) = self.session.as_mut() {
                    if session.generation == generation {
                        session.set_playing(playing);
                    }
                }
            }
            DecodeCommand::SetVolume(volume) => {
                self.volume = volume.clamp(0.0, 1.0);
                if let Some(session) = self.session.as_mut() {
                    session.set_volume(self.volume);
                }
            }
            DecodeCommand::SetMuted(muted) => {
                self.muted = muted;
                if let Some(session) = self.session.as_mut() {
                    session.set_muted(muted);
                }
            }
            DecodeCommand::SetBufferMemoryLimitBytes(bytes) => {
                self.buffer_memory_limit_bytes = bytes;
                if let Some(session) = self.session.as_mut() {
                    session.set_buffer_memory_limit_bytes(bytes);
                }
            }
            DecodeCommand::Shutdown => return false,
        }

        true
    }

    fn open_session(
        &mut self,
        reason: OpenReason,
        generation: u64,
        source: VideoSource,
        position: Duration,
    ) {
        self.shared_queue.replace_generation(generation);

        match DecodeSession::open(
            reason,
            generation,
            source,
            position,
            self.volume,
            self.muted,
            self.buffer_memory_limit_bytes,
            self.shared_queue.clone(),
            self.playback_clock.clone(),
        ) {
            Ok((session, stream_opened, first_frame_position)) => {
                let _ = self
                    .event_tx
                    .send(DecodeEvent::StreamOpened(stream_opened.clone()));
                let _ = self.event_tx.send(DecodeEvent::FirstFrameReady {
                    generation,
                    _position: first_frame_position,
                });
                let _ = self
                    .event_tx
                    .send(DecodeEvent::BufferSnapshot(session.snapshot()));
                self.session = Some(session);
            }
            Err(error) => {
                let _ = self.event_tx.send(DecodeEvent::FatalError {
                    generation,
                    message: error.to_string(),
                });
                self.session = None;
            }
        }
    }
}

enum DecodeStepOutcome {
    Continue { snapshot_changed: bool },
    Idle { snapshot_changed: bool },
    EofDrained,
}

struct DecodeSession {
    generation: u64,
    _reason: OpenReason,
    start_position: Duration,
    video_frame_duration: Duration,
    input: format::context::Input,
    video_stream_index: usize,
    audio_stream_index: Option<usize>,
    video_decoder: ffmpeg::decoder::Video,
    video_codec_id: codec::Id,
    video_decoder_name: String,
    audio_decoder: Option<ffmpeg::decoder::Audio>,
    scaler: Scaler,
    resampler: Option<Resampler>,
    video_time_base: ffmpeg::Rational,
    audio_output: Option<AudioOutput>,
    shared_queue: Arc<SharedVideoQueue>,
    playback_clock: SharedPlaybackClock,
    pending_video_packets: VecDeque<QueuedVideoPacket>,
    buffering_profile: BufferingProfile,
    buffer_memory_limit_bytes: u64,
    pending_video_compressed_bytes: u64,
    pending_audio_compressed_bytes: u64,
    last_video_position: Duration,
    eof_sent: bool,
    eof_notified: bool,
    last_snapshot: Option<BufferSnapshot>,
}

impl DecodeSession {
    #[allow(clippy::too_many_arguments)]
    fn open(
        reason: OpenReason,
        generation: u64,
        source: VideoSource,
        start_position: Duration,
        volume: f32,
        muted: bool,
        buffer_memory_limit_bytes: u64,
        shared_queue: Arc<SharedVideoQueue>,
        playback_clock: SharedPlaybackClock,
    ) -> Result<(Self, StreamOpenedEvent, Duration), TguiError> {
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
        let opened_video_decoder = open_video_decoder(&video_stream)?;
        let video_decoder = opened_video_decoder.decoder;
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
        let duration = stream_duration(video_stream.duration(), video_time_base);
        let video_frame_duration =
            stream_frame_duration(&video_stream).unwrap_or(Duration::from_millis(33));

        let audio_stream = input.streams().best(media::Type::Audio);
        let (audio_stream_index, audio_decoder, resampler, audio_output, audio_clock) =
            if let Some(audio_stream) = audio_stream {
                let audio_stream_index = audio_stream.index();
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

                let audio_output = AudioOutput::new(volume, muted).map_err(|error| {
                    TguiError::Media(format!("failed to create audio output: {error}"))
                })?;
                let audio_clock = audio_output.clock_handle();
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
                    Some(resampler),
                    Some(audio_output),
                    Some(audio_clock),
                )
            } else {
                (None, None, None, None, None)
            };

        let mut session = Self {
            generation,
            _reason: reason,
            start_position,
            video_frame_duration,
            input,
            video_stream_index,
            audio_stream_index,
            video_decoder,
            video_codec_id: opened_video_decoder.codec_id,
            video_decoder_name: opened_video_decoder.decoder_name,
            audio_decoder,
            scaler,
            resampler,
            video_time_base,
            audio_output,
            shared_queue,
            playback_clock,
            pending_video_packets: VecDeque::new(),
            buffering_profile,
            buffer_memory_limit_bytes,
            pending_video_compressed_bytes: 0,
            pending_audio_compressed_bytes: 0,
            last_video_position: start_position,
            eof_sent: false,
            eof_notified: false,
            last_snapshot: None,
        };

        session.playback_clock.set_position(start_position);
        let first_frame_position = session.prime_first_frame()?;

        let opened = StreamOpenedEvent {
            generation,
            start_position,
            duration,
            intrinsic_size,
            video_size: VideoSize {
                width: session.video_decoder.width(),
                height: session.video_decoder.height(),
            },
            buffering_profile,
            audio_clock,
        };

        Ok((session, opened, first_frame_position))
    }

    fn prime_first_frame(&mut self) -> Result<Duration, TguiError> {
        loop {
            if self.shared_queue.has_frames(self.generation) {
                return Ok(self
                    .shared_queue
                    .front(self.generation)
                    .map(|frame| frame.position)
                    .unwrap_or(self.start_position));
            }

            let next_packet = {
                let mut packets = self.input.packets();
                packets
                    .next()
                    .map(|(stream, packet)| (stream.index(), packet))
            };
            let Some((stream_index, packet)) = next_packet else {
                self.eof_sent = true;
                self.video_decoder.send_eof().map_err(|error| {
                    TguiError::Media(format!("failed to flush preview decoder: {error}"))
                })?;
                self.fill_ready_video_frames(false)?;
                if let Some(frame) = self.shared_queue.front(self.generation) {
                    return Ok(frame.position);
                }
                break;
            };

            if stream_index != self.video_stream_index {
                continue;
            }

            self.queue_video_packet(packet);
            self.fill_ready_video_frames(false)?;
        }

        Err(TguiError::Media(
            "video source does not contain a decodable frame".to_string(),
        ))
    }

    fn step(&mut self) -> Result<DecodeStepOutcome, TguiError> {
        let mut snapshot_changed = false;

        if self.fill_ready_video_frames(true)? {
            snapshot_changed = true;
        }

        if self.should_throttle_demux() {
            snapshot_changed |= self.update_snapshot_cache();
            return Ok(DecodeStepOutcome::Idle { snapshot_changed });
        }

        let next_packet = {
            let mut packets = self.input.packets();
            packets
                .next()
                .map(|(stream, packet)| (stream.index(), packet))
        };

        match next_packet {
            Some((stream_index, packet)) => {
                if stream_index == self.video_stream_index {
                    self.queue_video_packet(packet);
                    snapshot_changed = true;
                    if self.fill_ready_video_frames(true)? {
                        snapshot_changed = true;
                    }
                } else if Some(stream_index) == self.audio_stream_index {
                    if let (Some(audio_decoder), Some(resampler), Some(audio_output)) = (
                        self.audio_decoder.as_mut(),
                        self.resampler.as_mut(),
                        self.audio_output.as_ref(),
                    ) {
                        audio_decoder.send_packet(&packet).map_err(|error| {
                            TguiError::Media(format!("failed to send audio packet: {error}"))
                        })?;
                        self.pending_audio_compressed_bytes = self
                            .pending_audio_compressed_bytes
                            .saturating_add(packet.size() as u64);
                        receive_audio_frames(
                            audio_decoder,
                            resampler,
                            audio_output,
                            &mut self.pending_audio_compressed_bytes,
                        )?;
                        snapshot_changed = true;
                    }
                }
            }
            None => {
                if !self.eof_sent {
                    self.eof_sent = true;
                    self.video_decoder.send_eof().map_err(|error| {
                        TguiError::Media(format!("failed to flush video decoder: {error}"))
                    })?;

                    if let (Some(audio_decoder), Some(resampler), Some(audio_output)) = (
                        self.audio_decoder.as_mut(),
                        self.resampler.as_mut(),
                        self.audio_output.as_ref(),
                    ) {
                        let _ = audio_decoder.send_eof();
                        receive_audio_frames(
                            audio_decoder,
                            resampler,
                            audio_output,
                            &mut self.pending_audio_compressed_bytes,
                        )?;
                        flush_audio_resampler(
                            resampler,
                            audio_output,
                            &mut self.pending_audio_compressed_bytes,
                        )?;
                    }
                    snapshot_changed = true;
                }

                if self.fill_ready_video_frames(true)? {
                    snapshot_changed = true;
                }

                if self.eof_sent
                    && self.pending_video_packets.is_empty()
                    && !self.shared_queue.has_frames(self.generation)
                    && self.audio_buffered_duration().is_zero()
                {
                    if !self.eof_notified {
                        self.eof_notified = true;
                        self.update_snapshot_cache();
                        return Ok(DecodeStepOutcome::EofDrained);
                    }
                    return Ok(DecodeStepOutcome::Idle {
                        snapshot_changed: false,
                    });
                }
            }
        }

        snapshot_changed |= self.update_snapshot_cache();
        Ok(DecodeStepOutcome::Continue { snapshot_changed })
    }

    fn snapshot(&self) -> BufferSnapshot {
        BufferSnapshot {
            generation: self.generation,
            eof_sent: self.eof_sent,
            total_buffered_memory_bytes: self.total_buffered_memory_bytes(),
            buffering_constrained_by_memory_limit: self.buffering_constrained_by_memory_limit(),
        }
    }

    fn update_snapshot_cache(&mut self) -> bool {
        let snapshot = self.snapshot();
        let changed = self
            .last_snapshot
            .as_ref()
            .map(|previous| {
                previous.eof_sent != snapshot.eof_sent
                    || previous.total_buffered_memory_bytes != snapshot.total_buffered_memory_bytes
                    || previous.buffering_constrained_by_memory_limit
                        != snapshot.buffering_constrained_by_memory_limit
            })
            .unwrap_or(true);
        if changed {
            self.last_snapshot = Some(snapshot);
        }
        changed
    }

    fn set_playing(&mut self, playing: bool) {
        if let Some(audio_output) = self.audio_output.as_ref() {
            audio_output.set_playing(playing);
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

    fn set_buffer_memory_limit_bytes(&mut self, bytes: u64) {
        self.buffer_memory_limit_bytes = bytes;
    }

    fn playback_position(&self) -> Duration {
        self.playback_clock.position()
    }

    fn audio_buffered_duration(&self) -> Duration {
        self.audio_output
            .as_ref()
            .map(|output| output.buffered_duration())
            .unwrap_or(Duration::ZERO)
    }

    fn ready_video_buffered_duration(&self) -> Duration {
        let baseline = self.playback_position();
        self.shared_queue
            .tail_end_position(self.generation)
            .map(|end| end.saturating_sub(baseline))
            .unwrap_or(Duration::ZERO)
    }

    fn pending_video_packet_memory_bytes(&self) -> u64 {
        self.pending_video_packets
            .iter()
            .map(|packet| packet.packet.size() as u64)
            .sum()
    }

    fn ready_video_frame_memory_bytes(&self) -> u64 {
        self.shared_queue.ready_memory_bytes(self.generation)
    }

    fn audio_buffered_memory_bytes(&self) -> u64 {
        self.audio_output
            .as_ref()
            .map(|output| output.buffered_memory_bytes())
            .unwrap_or(0)
    }

    fn total_buffered_memory_bytes(&self) -> u64 {
        total_buffered_memory_bytes(
            self.pending_video_packet_memory_bytes(),
            self.ready_video_frame_memory_bytes(),
            self.audio_buffered_memory_bytes(),
        )
    }

    fn estimated_next_video_frame_memory_bytes(&self) -> u64 {
        self.shared_queue
            .head_frame_memory_bytes(self.generation)
            .or_else(|| {
                let frame_bytes = self
                    .shared_queue
                    .state
                    .lock()
                    .expect("video queue lock poisoned")
                    .frames
                    .iter()
                    .filter(|frame| frame.generation == self.generation)
                    .map(|frame| frame.compressed_bytes)
                    .collect::<Vec<_>>();
                average_non_zero_bytes(&frame_bytes)
            })
            .or_else(|| {
                self.pending_video_packets
                    .front()
                    .map(|packet| packet.packet.size() as u64)
            })
            .unwrap_or(self.pending_video_compressed_bytes)
    }

    fn buffering_constrained_by_memory_limit(&self) -> bool {
        buffering_constrained_by_memory_limit(
            self.total_buffered_memory_bytes(),
            self.buffer_memory_limit_bytes,
            self.estimated_next_video_frame_memory_bytes(),
        )
    }

    fn should_throttle_demux(&self) -> bool {
        should_throttle_demux(
            self.total_buffered_memory_bytes() >= self.buffer_memory_limit_bytes,
            self.audio_buffered_duration() >= self.buffering_profile.audio_queue_hard_water,
            self.ready_video_buffered_duration() >= self.buffering_profile.video_queue_hard_water,
            self.pending_video_packets.len() >= self.buffering_profile.video_max_packet_count,
        )
    }

    fn queued_video_tail_position(&self) -> Option<Duration> {
        self.pending_video_packets
            .back()
            .map(|packet| packet.end_position)
            .or_else(|| self.shared_queue.tail_end_position(self.generation))
            .or(Some(self.last_video_position))
    }

    fn queue_video_packet(&mut self, packet: ffmpeg::Packet) {
        let position = packet
            .pts()
            .or_else(|| packet.dts())
            .and_then(|timestamp| pts_to_duration(Some(timestamp), self.video_time_base))
            .unwrap_or_else(|| {
                self.queued_video_tail_position()
                    .unwrap_or(self.start_position)
            });
        let duration = packet_duration(packet.duration(), self.video_time_base)
            .unwrap_or(self.video_frame_duration);
        self.pending_video_packets.push_back(QueuedVideoPacket {
            packet,
            end_position: position.saturating_add(duration),
        });
    }

    fn fill_ready_video_frames(
        &mut self,
        respect_buffer_memory_limit: bool,
    ) -> Result<bool, TguiError> {
        let mut decoded_any = false;
        let mut decode_budget = self.buffering_profile.ready_video_frame_count;

        while decode_budget > 0
            && (!respect_buffer_memory_limit || !self.buffering_constrained_by_memory_limit())
        {
            let Some(queued_packet) = self.pending_video_packets.pop_front() else {
                break;
            };

            self.video_decoder
                .send_packet(&queued_packet.packet)
                .map_err(|error| self.video_packet_send_error(error))?;
            self.pending_video_compressed_bytes = self
                .pending_video_compressed_bytes
                .saturating_add(queued_packet.packet.size() as u64);

            let mut decoded = VideoFrame::empty();
            let mut newly_decoded = Vec::new();

            while decode_budget > 0
                && (!respect_buffer_memory_limit || !self.buffering_constrained_by_memory_limit())
                && self.video_decoder.receive_frame(&mut decoded).is_ok()
            {
                let position = pts_to_duration(decoded.timestamp(), self.video_time_base)
                    .unwrap_or_else(|| {
                        self.queued_video_tail_position()
                            .unwrap_or(self.start_position)
                    });

                if self.should_drop_video_preroll_frame(position) {
                    continue;
                }

                let texture = Arc::new(video_frame_to_texture(&mut self.scaler, &decoded)?);
                let frame = QueuedVideoFrame {
                    generation: self.generation,
                    position,
                    end_position: position.saturating_add(self.video_frame_duration),
                    texture,
                    compressed_bytes: 0,
                };
                self.last_video_position = position;
                newly_decoded.push(frame);
                decode_budget = decode_budget.saturating_sub(1);
            }

            if !newly_decoded.is_empty() {
                let compressed_bytes = std::mem::take(&mut self.pending_video_compressed_bytes);
                distribute_video_compressed_bytes(&mut newly_decoded, compressed_bytes);
                self.shared_queue.push_frames(newly_decoded);
                decoded_any = true;
            }
        }

        if decode_budget > 0
            && self.pending_video_packets.is_empty()
            && self.eof_sent
            && (!respect_buffer_memory_limit || !self.buffering_constrained_by_memory_limit())
        {
            let mut decoded = VideoFrame::empty();
            let mut flushed_frames = Vec::new();
            while decode_budget > 0
                && self.video_decoder.receive_frame(&mut decoded).is_ok()
                && (!respect_buffer_memory_limit || !self.buffering_constrained_by_memory_limit())
            {
                let position = pts_to_duration(decoded.timestamp(), self.video_time_base)
                    .unwrap_or_else(|| {
                        self.queued_video_tail_position()
                            .unwrap_or(self.start_position)
                    });
                if self.should_drop_video_preroll_frame(position) {
                    continue;
                }

                let texture = Arc::new(video_frame_to_texture(&mut self.scaler, &decoded)?);
                flushed_frames.push(QueuedVideoFrame {
                    generation: self.generation,
                    position,
                    end_position: position.saturating_add(self.video_frame_duration),
                    texture,
                    compressed_bytes: 0,
                });
                self.last_video_position = position;
                decode_budget = decode_budget.saturating_sub(1);
            }

            if !flushed_frames.is_empty() {
                let compressed_bytes = std::mem::take(&mut self.pending_video_compressed_bytes);
                distribute_video_compressed_bytes(&mut flushed_frames, compressed_bytes);
                self.shared_queue.push_frames(flushed_frames);
                decoded_any = true;
            }
        }

        Ok(decoded_any)
    }

    fn should_drop_video_preroll_frame(&self, position: Duration) -> bool {
        !self.start_position.is_zero()
            && position.saturating_add(VIDEO_SEEK_PREROLL_TOLERANCE) < self.start_position
    }

    fn video_packet_send_error(&self, error: ffmpeg::Error) -> TguiError {
        if self.video_codec_id == codec::Id::AV1
            && matches!(
                error,
                ffmpeg::Error::Other {
                    errno: ffmpeg::error::ENOSYS
                }
            )
        {
            return TguiError::Media(format!(
                "AV1 is not supported by the linked FFmpeg build. The current decoder `{}` cannot decode this file on this platform. Rebuild/install FFmpeg with the `dav1d` or `aom` feature enabled in vcpkg.",
                self.video_decoder_name
            ));
        }

        TguiError::Media(format!("failed to send video packet: {error}"))
    }
}

fn average_non_zero_bytes(bytes: &[u64]) -> Option<u64> {
    let (sum, count) = bytes
        .iter()
        .copied()
        .filter(|bytes| *bytes > 0)
        .fold((0u64, 0u64), |(sum, count), bytes| {
            (sum.saturating_add(bytes), count + 1)
        });
    (count > 0).then(|| sum / count)
}

#[cfg(test)]
mod tests {
    use super::average_non_zero_bytes;

    #[test]
    fn average_non_zero_bytes_returns_none_for_empty_or_zero_only_input() {
        assert_eq!(average_non_zero_bytes(&[]), None);
        assert_eq!(average_non_zero_bytes(&[0, 0, 0]), None);
    }

    #[test]
    fn average_non_zero_bytes_ignores_zero_entries() {
        assert_eq!(average_non_zero_bytes(&[0, 10, 20]), Some(15));
    }
}
