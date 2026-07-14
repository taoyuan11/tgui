use std::slice;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TryRecvError};

use crate::audio::backend::shared::AudioOutput;
use crate::foundation::error::TguiError;

use super::*;

mod buffering;
mod open;
mod worker;

use self::open::{
    collect_audio_tracks, collect_subtitle_tracks, open_audio_pipeline, open_subtitle_pipeline,
    seek_input_to_start_position, select_audio_stream, select_subtitle_stream,
    validate_subtitle_selection,
};
pub(super) use self::worker::decode_main;

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
    _input_resource: Option<TemporaryMediaFile>,
    video_stream_index: usize,
    audio_stream_index: Option<usize>,
    subtitle_stream_index: Option<usize>,
    video_decoder: ffmpeg::decoder::Video,
    video_codec_id: codec::Id,
    video_decoder_name: String,
    audio_decoder: Option<ffmpeg::decoder::Audio>,
    subtitle_decoder: Option<ffmpeg::decoder::Subtitle>,
    decoded_video_frame: VideoFrame,
    video_converter: VideoFrameConverter,
    resampler: Option<Resampler>,
    audio_resample_frame: ReusableAudioFrame,
    video_time_base: ffmpeg::Rational,
    subtitle_time_base: Option<ffmpeg::Rational>,
    audio_output: Option<AudioOutput>,
    shared_queue: Arc<SharedVideoQueue>,
    playback_clock: SharedPlaybackClock,
    video_texture_id: u64,
    next_video_texture_revision: u64,
    target_raster: Option<RasterRequest>,
    pending_video_packets: VecDeque<QueuedVideoPacket>,
    pending_subtitle_cues: VecDeque<DecodedSubtitleCue>,
    pending_subtitle_bitmap_cues: VecDeque<VideoSubtitleBitmapCue>,
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
        playback_rate: f32,
        audio_track_selection: VideoAudioTrackSelection,
        subtitle_track_selection: VideoSubtitleTrackSelection,
        buffer_memory_limit_bytes: u64,
        target_raster: Option<RasterRequest>,
        shared_queue: Arc<SharedVideoQueue>,
        playback_clock: SharedPlaybackClock,
    ) -> Result<(Self, StreamOpenedEvent, Duration), TguiError> {
        let buffering_profile = buffering_profile_for_source(&source);
        let opened_input = open_input(&source)?;
        let mut input = opened_input.input;
        let input_resource = opened_input.resource;
        seek_input_to_start_position(&mut input, start_position)?;

        let video_stream = input
            .streams()
            .best(media::Type::Video)
            .ok_or_else(|| TguiError::Media("video stream not found".to_string()))?;
        let video_stream_index = video_stream.index();
        let video_time_base = video_stream.time_base();
        let opened_video_decoder = open_video_decoder(&video_stream)?;
        let video_decoder = opened_video_decoder.decoder;

        let intrinsic_size =
            IntrinsicSize::from_pixels(video_decoder.width(), video_decoder.height());
        let duration = stream_duration(video_stream.duration(), video_time_base);
        let video_frame_duration =
            stream_frame_duration(&video_stream).unwrap_or(Duration::from_millis(33));

        let audio_tracks = collect_audio_tracks(&input);
        let audio_stream = select_audio_stream(&input, audio_track_selection)?;
        let subtitle_tracks = collect_subtitle_tracks(&input);
        validate_subtitle_selection(&input, subtitle_track_selection)?;
        let subtitle_stream = select_subtitle_stream(&input, subtitle_track_selection)?;
        let (audio_stream_index, audio_decoder, resampler, audio_output, audio_clock) =
            if let Some(audio_stream) = audio_stream {
                let opened_audio =
                    open_audio_pipeline(&audio_stream, volume, muted, playback_rate)?;
                (
                    Some(opened_audio.stream_index),
                    Some(opened_audio.decoder),
                    Some(opened_audio.resampler),
                    Some(opened_audio.output),
                    Some(opened_audio.clock),
                )
            } else {
                (None, None, None, None, None)
            };
        let (subtitle_stream_index, subtitle_decoder, subtitle_time_base) =
            if let Some(subtitle_stream) = subtitle_stream {
                let opened_subtitle = open_subtitle_pipeline(&subtitle_stream)?;
                (
                    Some(opened_subtitle.stream_index),
                    Some(opened_subtitle.decoder),
                    Some(opened_subtitle.time_base),
                )
            } else {
                (None, None, None)
            };

        let mut session = Self {
            generation,
            _reason: reason,
            start_position,
            video_frame_duration,
            input,
            _input_resource: input_resource,
            video_stream_index,
            audio_stream_index,
            subtitle_stream_index,
            video_decoder,
            video_codec_id: opened_video_decoder.codec_id,
            video_decoder_name: opened_video_decoder.decoder_name,
            audio_decoder,
            subtitle_decoder,
            decoded_video_frame: VideoFrame::empty(),
            video_converter: VideoFrameConverter::new(),
            resampler,
            audio_resample_frame: ReusableAudioFrame::new(),
            video_time_base,
            subtitle_time_base,
            audio_output,
            shared_queue,
            playback_clock,
            video_texture_id: TextureFrame::allocate_id(),
            next_video_texture_revision: 1,
            target_raster,
            pending_video_packets: VecDeque::new(),
            pending_subtitle_cues: VecDeque::new(),
            pending_subtitle_bitmap_cues: VecDeque::new(),
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
            audio_tracks,
            audio_track_selection,
            subtitle_tracks,
            subtitle_track_selection,
        };

        Ok((session, opened, first_frame_position))
    }

    fn prime_first_frame(&mut self) -> Result<Duration, TguiError> {
        loop {
            if let Some(position) = self.shared_queue.front_position(self.generation) {
                return Ok(position);
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
                if let Some(position) = self.shared_queue.front_position(self.generation) {
                    return Ok(position);
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
                            &mut self.audio_resample_frame,
                            &mut self.pending_audio_compressed_bytes,
                        )?;
                        snapshot_changed = true;
                    }
                } else if Some(stream_index) == self.subtitle_stream_index {
                    self.decode_subtitle_packet(&packet)?;
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
                            &mut self.audio_resample_frame,
                            &mut self.pending_audio_compressed_bytes,
                        )?;
                        flush_audio_resampler(
                            resampler,
                            audio_output,
                            &mut self.audio_resample_frame,
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

    fn decode_subtitle_packet(&mut self, packet: &ffmpeg::Packet) -> Result<(), TguiError> {
        let (Some(decoder), Some(time_base)) =
            (self.subtitle_decoder.as_mut(), self.subtitle_time_base)
        else {
            return Ok(());
        };

        let mut subtitle = DecodedSubtitle::new();
        let decoded = decoder.decode(packet, subtitle.as_mut()).map_err(|error| {
            TguiError::Media(format!("failed to decode subtitle packet: {error}"))
        })?;
        if !decoded {
            return Ok(());
        }

        let cues = subtitle_cues_from_packet(subtitle.as_ref(), packet, time_base);
        self.pending_subtitle_cues.extend(cues);
        let bitmap_cues = subtitle_bitmap_cues_from_packet(subtitle.as_ref(), packet, time_base);
        self.pending_subtitle_bitmap_cues.extend(bitmap_cues);
        Ok(())
    }

    fn drain_subtitle_cues(&mut self) -> Vec<DecodedSubtitleCue> {
        self.pending_subtitle_cues.drain(..).collect()
    }

    fn drain_subtitle_bitmap_cues(&mut self) -> Vec<VideoSubtitleBitmapCue> {
        self.pending_subtitle_bitmap_cues.drain(..).collect()
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

    fn set_playback_rate(&mut self, rate: f32) {
        if let Some(audio_output) = self.audio_output.as_ref() {
            audio_output.set_playback_rate(rate);
        }
    }

    fn set_buffer_memory_limit_bytes(&mut self, bytes: u64) {
        self.buffer_memory_limit_bytes = bytes;
    }

    fn set_target_raster(&mut self, raster: Option<RasterRequest>) {
        if self.target_raster == raster {
            return;
        }
        self.target_raster = raster;
    }

    fn next_video_texture_revision(&mut self) -> u64 {
        let revision = self.next_video_texture_revision.max(1);
        self.next_video_texture_revision = revision.wrapping_add(1).max(1);
        revision
    }
}

struct DecodedSubtitle {
    inner: ffmpeg::Subtitle,
}

#[derive(Clone)]
struct DecodedSubtitleCue {
    cue: VideoSubtitleCue,
    placement: Option<VideoSubtitleCuePlacement>,
    style: Option<VideoSubtitleCueStyle>,
}

impl DecodedSubtitle {
    fn new() -> Self {
        Self {
            inner: ffmpeg::Subtitle::new(),
        }
    }

    fn as_ref(&self) -> &ffmpeg::Subtitle {
        &self.inner
    }

    fn as_mut(&mut self) -> &mut ffmpeg::Subtitle {
        &mut self.inner
    }
}

impl Drop for DecodedSubtitle {
    fn drop(&mut self) {
        unsafe {
            ffmpeg::ffi::avsubtitle_free(self.inner.as_mut_ptr());
        }
    }
}

fn subtitle_cues_from_packet(
    subtitle: &ffmpeg::Subtitle,
    packet: &ffmpeg::Packet,
    time_base: ffmpeg::Rational,
) -> Vec<DecodedSubtitleCue> {
    let subtitle_text = subtitle_text(subtitle);
    if subtitle_text.text.trim().is_empty() {
        return Vec::new();
    }

    let Some((start, end)) = subtitle_timing(subtitle, packet, time_base) else {
        return Vec::new();
    };

    vec![DecodedSubtitleCue {
        cue: VideoSubtitleCue {
            text: subtitle_text.text,
            start,
            end,
        },
        placement: subtitle_text.placement,
        style: subtitle_text.style,
    }]
}

fn subtitle_bitmap_cues_from_packet(
    subtitle: &ffmpeg::Subtitle,
    packet: &ffmpeg::Packet,
    time_base: ffmpeg::Rational,
) -> Vec<VideoSubtitleBitmapCue> {
    let Some((start, end)) = subtitle_timing(subtitle, packet, time_base) else {
        return Vec::new();
    };

    subtitle
        .rects()
        .filter_map(|rect| match rect {
            ffmpeg::subtitle::Rect::Bitmap(bitmap) => subtitle_bitmap_cue(&bitmap, start, end),
            _ => None,
        })
        .collect()
}

fn subtitle_bitmap_cue(
    bitmap: &ffmpeg::subtitle::Bitmap<'_>,
    start: Duration,
    end: Duration,
) -> Option<VideoSubtitleBitmapCue> {
    let ptr = unsafe { bitmap.as_ptr() };
    if ptr.is_null() {
        return None;
    }

    let rect = unsafe { &*ptr };
    let width = u32::try_from(rect.w).ok()?;
    let height = u32::try_from(rect.h).ok()?;
    let x = u32::try_from(rect.x.max(0)).ok()?;
    let y = u32::try_from(rect.y.max(0)).ok()?;
    let stride = usize::try_from(rect.linesize[0]).ok()?;
    let color_count = usize::try_from(rect.nb_colors).ok()?;
    if width == 0
        || height == 0
        || stride < width as usize
        || color_count == 0
        || rect.data[0].is_null()
        || rect.data[1].is_null()
    {
        return None;
    }

    let palette_len = color_count.checked_mul(4)?;
    let palette = unsafe { slice::from_raw_parts(rect.data[1] as *const u8, palette_len) };
    let pixels = unsafe {
        pal8_bitmap_to_rgba_from_ptr(
            rect.data[0] as *const u8,
            stride,
            palette,
            color_count,
            width,
            height,
        )?
    };
    VideoSubtitleBitmapCue::new(x, y, width, height, Arc::from(pixels), start, end)
}

unsafe fn pal8_bitmap_to_rgba_from_ptr(
    indexes: *const u8,
    stride: usize,
    palette: &[u8],
    color_count: usize,
    width: u32,
    height: u32,
) -> Option<Vec<u8>> {
    if indexes.is_null() || stride < width as usize || color_count == 0 {
        return None;
    }

    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let mut pixels = Vec::with_capacity(width.checked_mul(height)?.checked_mul(4)?);
    for row in 0..height {
        let row_ptr = indexes.add(row.checked_mul(stride)?);
        let row_indexes = slice::from_raw_parts(row_ptr, width);
        append_pal8_row_as_rgba(row_indexes, palette, color_count, &mut pixels);
    }
    Some(pixels)
}

fn append_pal8_row_as_rgba(indexes: &[u8], palette: &[u8], color_count: usize, out: &mut Vec<u8>) {
    for &index in indexes {
        let index = usize::from(index);
        if index >= color_count {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        let base = index * 4;
        if base + 3 >= palette.len() {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        out.extend_from_slice(&rgb32_palette_entry_to_rgba(&palette[base..base + 4]));
    }
}

fn rgb32_palette_entry_to_rgba(entry: &[u8]) -> [u8; 4] {
    debug_assert!(entry.len() >= 4);
    #[cfg(target_endian = "little")]
    {
        [entry[2], entry[1], entry[0], entry[3]]
    }
    #[cfg(target_endian = "big")]
    {
        [entry[1], entry[2], entry[3], entry[0]]
    }
}

#[derive(Default)]
struct DecodedSubtitleText {
    text: String,
    placement: Option<VideoSubtitleCuePlacement>,
    style: Option<VideoSubtitleCueStyle>,
}

fn subtitle_text(subtitle: &ffmpeg::Subtitle) -> DecodedSubtitleText {
    let mut text_parts = Vec::new();
    let mut placement = None;
    let mut style = None;
    for rect in subtitle.rects() {
        match rect {
            ffmpeg::subtitle::Rect::Text(text) => {
                let text = text.get().trim().to_string();
                if !text.trim().is_empty() {
                    text_parts.push(text);
                }
            }
            ffmpeg::subtitle::Rect::Ass(ass) => {
                let decoded = plain_ass_text_metadata(ass.get());
                if placement.is_none() {
                    placement = decoded.placement;
                }
                if style.is_none() && !decoded.style.unwrap_or_default().is_empty() {
                    style = decoded.style;
                }
                if !decoded.text.trim().is_empty() {
                    text_parts.push(decoded.text);
                }
            }
            _ => {}
        }
    }

    DecodedSubtitleText {
        text: text_parts.join("\n"),
        placement,
        style,
    }
}

#[cfg(test)]
fn plain_ass_text(ass: &str) -> String {
    plain_ass_text_metadata(ass).text
}

#[cfg(test)]
fn plain_ass_text_and_placement(ass: &str) -> (String, Option<VideoSubtitleCuePlacement>) {
    let decoded = plain_ass_text_metadata(ass);
    (decoded.text, decoded.placement)
}

fn plain_ass_text_metadata(ass: &str) -> DecodedSubtitleText {
    let body = ass.trim();
    let text = if let Some(dialogue) = body.strip_prefix("Dialogue:") {
        dialogue.trim().splitn(10, ',').nth(9).unwrap_or_default()
    } else {
        body
    };
    let placement = ass_placement_from_text(text);
    let style = ass_style_from_text(text);
    let text = strip_ass_override_tags_and_drawings(text)
        .replace("\\N", "\n")
        .replace("\\n", "\n")
        .replace("\\h", " ")
        .trim()
        .to_string();
    DecodedSubtitleText {
        text,
        placement,
        style,
    }
}

fn ass_placement_from_text(text: &str) -> Option<VideoSubtitleCuePlacement> {
    let mut placement = None;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '{' {
            continue;
        }
        let mut tag = String::new();
        for tag_ch in chars.by_ref() {
            if tag_ch == '}' {
                break;
            }
            tag.push(tag_ch);
        }
        if let Some(alignment) = ass_alignment_from_tag(&tag) {
            placement = Some(alignment);
        }
    }
    placement
}

fn ass_alignment_from_tag(tag: &str) -> Option<VideoSubtitleCuePlacement> {
    let mut chars = tag.chars().peekable();
    let mut placement = None;
    while let Some(ch) = chars.next() {
        if ch != '\\' || chars.next() != Some('a') || chars.next() != Some('n') {
            continue;
        }
        let Some(digit) = chars.next().and_then(|value| value.to_digit(10)) else {
            continue;
        };
        if let Some(alignment) = VideoSubtitleCuePlacement::from_ass_alignment(digit as u8) {
            placement = Some(alignment);
        }
    }
    placement
}

fn ass_style_from_text(text: &str) -> Option<VideoSubtitleCueStyle> {
    let mut style = VideoSubtitleCueStyle::default();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '{' {
            continue;
        }
        let mut tag = String::new();
        for tag_ch in chars.by_ref() {
            if tag_ch == '}' {
                break;
            }
            tag.push(tag_ch);
        }
        if let Some(color) = ass_primary_color_from_tag(&tag) {
            style.primary_color = Some(color);
        }
        if let Some(color) = ass_outline_color_from_tag(&tag) {
            style.outline_color = Some(color);
        }
        if let Some(color) = ass_shadow_color_from_tag(&tag) {
            style.shadow_color = Some(color);
        }
        if let Some(font_weight) = ass_font_weight_from_tag(&tag) {
            style.font_weight = Some(font_weight);
        }
        if let Some(font_size) =
            ass_centi_pixel_tag_from_tag(&tag, "fs", 128.0).filter(|size| *size > 0)
        {
            style.font_size_centi_px = Some(font_size);
        }
        if let Some(width) = ass_centi_pixel_tag_from_tag(&tag, "bord", 64.0) {
            style.outline_width_centi_px = Some(width);
        }
        if let Some(depth) = ass_centi_pixel_tag_from_tag(&tag, "shad", 64.0) {
            style.shadow_depth_centi_px = Some(depth);
        }
    }
    (!style.is_empty()).then_some(style)
}

fn ass_primary_color_from_tag(tag: &str) -> Option<crate::foundation::color::Color> {
    ass_color_from_tag(tag, &[b"c", b"1c"])
}

fn ass_outline_color_from_tag(tag: &str) -> Option<crate::foundation::color::Color> {
    ass_color_from_tag(tag, &[b"3c"])
}

fn ass_shadow_color_from_tag(tag: &str) -> Option<crate::foundation::color::Color> {
    ass_color_from_tag(tag, &[b"4c"])
}

fn ass_color_from_tag(tag: &str, prefixes: &[&[u8]]) -> Option<crate::foundation::color::Color> {
    let bytes = tag.as_bytes();
    let mut index = 0;
    let mut color = None;
    while index < bytes.len() {
        if bytes[index] != b'\\' {
            index += 1;
            continue;
        }

        let Some(prefix) = prefixes
            .iter()
            .find(|prefix| bytes.get(index + 1..index + 1 + prefix.len()) == Some(*prefix))
        else {
            index += 1;
            continue;
        };
        let color_start = index + 1 + prefix.len();

        if let Some((parsed, next_index)) = ass_color_from_bytes(bytes, color_start) {
            color = Some(parsed);
            index = next_index;
        } else {
            index = color_start;
        }
    }
    color
}

fn ass_color_from_bytes(
    bytes: &[u8],
    start: usize,
) -> Option<(crate::foundation::color::Color, usize)> {
    let mut index = start;
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    if bytes.get(index) != Some(&b'&') || !matches!(bytes.get(index + 1), Some(b'H' | b'h')) {
        return None;
    }

    index += 2;
    let hex_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_hexdigit) {
        index += 1;
    }
    let hex = &bytes[hex_start..index];
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }

    let value = u32::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
    let (alpha, b, g, r) = if hex.len() == 8 {
        (
            255_u8.saturating_sub(((value >> 24) & 0xFF) as u8),
            ((value >> 16) & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            (value & 0xFF) as u8,
        )
    } else {
        (
            255,
            ((value >> 16) & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            (value & 0xFF) as u8,
        )
    };
    let end = if bytes.get(index) == Some(&b'&') {
        index + 1
    } else {
        index
    };
    Some((crate::foundation::color::Color::rgba(r, g, b, alpha), end))
}

fn ass_centi_pixel_tag_from_tag(tag: &str, name: &str, max_value_px: f32) -> Option<u16> {
    let bytes = tag.as_bytes();
    let name = name.as_bytes();
    let mut index = 0;
    let mut value = None;
    while index < bytes.len() {
        if bytes[index] != b'\\' || bytes.get(index + 1..index + 1 + name.len()) != Some(name) {
            index += 1;
            continue;
        }

        let mut cursor = index + 1 + name.len();
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }

        let number_start = cursor;
        if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
            cursor += 1;
        }
        let mut has_digit = false;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            has_digit = true;
            cursor += 1;
        }
        if bytes.get(cursor) == Some(&b'.') {
            cursor += 1;
            while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
                has_digit = true;
                cursor += 1;
            }
        }
        if !has_digit {
            index = cursor.max(index + 1);
            continue;
        }

        if let Ok(raw) = std::str::from_utf8(&bytes[number_start..cursor])
            .unwrap_or_default()
            .parse::<f32>()
        {
            let clamped = raw.clamp(0.0, max_value_px);
            value = Some((clamped * 100.0).round() as u16);
        }
        index = cursor;
    }
    value
}

fn ass_font_weight_from_tag(tag: &str) -> Option<crate::text::font::FontWeight> {
    let mut chars = tag.chars().peekable();
    let mut weight = None;
    while let Some(ch) = chars.next() {
        if ch != '\\' || chars.next() != Some('b') {
            continue;
        }

        let mut value = String::new();
        if matches!(chars.peek(), Some('-')) {
            value.push('-');
            chars.next();
        }
        while matches!(chars.peek(), Some(next) if next.is_ascii_digit()) {
            if let Some(digit) = chars.next() {
                value.push(digit);
            }
        }
        if value.is_empty() {
            continue;
        }
        if let Ok(raw) = value.parse::<i32>() {
            weight = Some(ass_font_weight(raw));
        }
    }
    weight
}

fn ass_font_weight(raw: i32) -> crate::text::font::FontWeight {
    use crate::text::font::FontWeight;

    match raw {
        value if value <= 0 => FontWeight::Regular,
        1 => FontWeight::Bold,
        value if value < 200 => FontWeight::Thin,
        value if value < 400 => FontWeight::Light,
        value if value < 500 => FontWeight::Regular,
        value if value < 600 => FontWeight::Medium,
        value if value < 700 => FontWeight::SemiBold,
        value if value < 800 => FontWeight::Bold,
        _ => FontWeight::ExtraBold,
    }
}

fn strip_ass_override_tags_and_drawings(text: &str) -> String {
    let mut stripped = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut drawing_mode = false;
    let mut removed_drawing_payload = false;

    while let Some(ch) = chars.next() {
        if ch == '{' {
            let mut tag = String::new();
            let was_drawing = drawing_mode;
            for tag_ch in chars.by_ref() {
                if tag_ch == '}' {
                    break;
                }
                tag.push(tag_ch);
            }
            if let Some(enabled) = ass_drawing_mode_from_tag(&tag) {
                drawing_mode = enabled;
                if was_drawing && !drawing_mode {
                    removed_drawing_payload = true;
                }
            }
        } else if !drawing_mode {
            if removed_drawing_payload
                && ch.is_whitespace()
                && stripped
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace)
            {
                removed_drawing_payload = false;
                continue;
            }
            removed_drawing_payload = false;
            stripped.push(ch);
        } else {
            removed_drawing_payload = true;
        }
    }

    stripped
}

fn ass_drawing_mode_from_tag(tag: &str) -> Option<bool> {
    let mut chars = tag.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            continue;
        }

        if chars.next() != Some('p') {
            continue;
        }

        let mut value = String::new();
        if matches!(chars.peek(), Some('-')) {
            value.push('-');
            chars.next();
        }
        while matches!(chars.peek(), Some(next) if next.is_ascii_digit()) {
            if let Some(digit) = chars.next() {
                value.push(digit);
            }
        }
        if !value.is_empty() {
            return value.parse::<i32>().ok().map(|mode| mode > 0);
        }
    }

    None
}

fn subtitle_base_position(
    subtitle: &ffmpeg::Subtitle,
    packet: &ffmpeg::Packet,
    time_base: ffmpeg::Rational,
) -> Duration {
    subtitle
        .pts()
        .map(|pts| Duration::from_micros(pts.max(0) as u64))
        .or_else(|| pts_to_duration(packet.pts().or_else(|| packet.dts()), time_base))
        .unwrap_or_default()
}

fn subtitle_timing(
    subtitle: &ffmpeg::Subtitle,
    packet: &ffmpeg::Packet,
    time_base: ffmpeg::Rational,
) -> Option<(Duration, Duration)> {
    let base = subtitle_base_position(subtitle, packet, time_base);
    let start = base + Duration::from_millis(subtitle.start() as u64);
    let end = subtitle_end_position(subtitle, packet, time_base, start);
    (end > start).then_some((start, end))
}

fn subtitle_end_position(
    subtitle: &ffmpeg::Subtitle,
    packet: &ffmpeg::Packet,
    time_base: ffmpeg::Rational,
    start: Duration,
) -> Duration {
    if subtitle.end() > subtitle.start() {
        return subtitle_base_position(subtitle, packet, time_base)
            + Duration::from_millis(subtitle.end() as u64);
    }

    packet_duration(packet.duration(), time_base)
        .map(|duration| start + duration)
        .unwrap_or_else(|| start + Duration::from_secs(3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video::{VideoSubtitleHorizontalAlign, VideoSubtitleVerticalAlign};

    #[test]
    fn plain_ass_text_extracts_dialogue_text_and_line_breaks() {
        let text =
            plain_ass_text("Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,{\\i1}Hello\\Nworld");

        assert_eq!(text, "Hello\nworld");
    }

    #[test]
    fn plain_ass_text_keeps_non_dialogue_payload() {
        let text = plain_ass_text("{\\b1}Plain\\hcaption");

        assert_eq!(text, "Plain caption");
    }

    #[test]
    fn plain_ass_text_keeps_commas_in_dialogue_text() {
        let text = plain_ass_text("Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Hello, world");

        assert_eq!(text, "Hello, world");
    }

    #[test]
    fn plain_ass_text_keeps_empty_dialogue_empty() {
        let text = plain_ass_text("Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,");

        assert_eq!(text, "");
    }

    #[test]
    fn plain_ass_text_strips_drawing_payloads() {
        let text = plain_ass_text(
            "Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Caption {\\p1}m 0 0 l 20 0 20 20 0 20{\\p0} visible",
        );

        assert_eq!(text, "Caption visible");
    }

    #[test]
    fn plain_ass_text_preserves_override_alignment_as_placement() {
        let (text, placement) = plain_ass_text_and_placement(
            "Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,{\\an7\\b1}Top left",
        );

        assert_eq!(text, "Top left");
        assert_eq!(
            placement,
            Some(VideoSubtitleCuePlacement {
                horizontal: VideoSubtitleHorizontalAlign::Left,
                vertical: VideoSubtitleVerticalAlign::Top,
            })
        );
    }

    #[test]
    fn plain_ass_text_uses_last_override_alignment() {
        let (_text, placement) = plain_ass_text_and_placement("{\\an1}bottom{\\an9}top right");

        assert_eq!(
            placement,
            Some(VideoSubtitleCuePlacement {
                horizontal: VideoSubtitleHorizontalAlign::Right,
                vertical: VideoSubtitleVerticalAlign::Top,
            })
        );
    }

    #[test]
    fn plain_ass_text_preserves_primary_color_as_style() {
        let decoded = plain_ass_text_metadata(
            "Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,{\\1c&H112233&}Color",
        );

        assert_eq!(decoded.text, "Color");
        assert_eq!(
            decoded.style,
            Some(VideoSubtitleCueStyle {
                primary_color: Some(crate::foundation::color::Color::rgb(0x33, 0x22, 0x11)),
                font_weight: None,
                ..Default::default()
            })
        );
    }

    #[test]
    fn plain_ass_text_preserves_primary_color_alpha() {
        let decoded = plain_ass_text_metadata("{\\c&H80223344&}Transparent color");

        assert_eq!(
            decoded.style,
            Some(VideoSubtitleCueStyle {
                primary_color: Some(crate::foundation::color::Color::rgba(
                    0x44, 0x33, 0x22, 0x7F,
                )),
                font_weight: None,
                ..Default::default()
            })
        );
    }

    #[test]
    fn plain_ass_text_uses_last_primary_color() {
        let decoded = plain_ass_text_metadata("{\\c&H0000FF&}red{\\1c&H00FF00&}green");

        assert_eq!(
            decoded.style,
            Some(VideoSubtitleCueStyle {
                primary_color: Some(crate::foundation::color::Color::GREEN),
                font_weight: None,
                ..Default::default()
            })
        );
    }

    #[test]
    fn plain_ass_text_preserves_outline_color_as_style() {
        let decoded = plain_ass_text_metadata("{\\3c&H112233&}Outlined");

        assert_eq!(
            decoded.style,
            Some(VideoSubtitleCueStyle {
                outline_color: Some(crate::foundation::color::Color::rgb(0x33, 0x22, 0x11)),
                ..Default::default()
            })
        );
    }

    #[test]
    fn plain_ass_text_preserves_shadow_color_alpha_as_style() {
        let decoded = plain_ass_text_metadata("{\\4c&H80223344&}Shadowed");

        assert_eq!(
            decoded.style,
            Some(VideoSubtitleCueStyle {
                shadow_color: Some(crate::foundation::color::Color::rgba(
                    0x44, 0x33, 0x22, 0x7F,
                )),
                ..Default::default()
            })
        );
    }

    #[test]
    fn plain_ass_text_preserves_outline_and_shadow_depths_as_style() {
        let decoded = plain_ass_text_metadata("{\\bord2.5\\shad3}Effects");

        assert_eq!(
            decoded.style,
            Some(VideoSubtitleCueStyle {
                outline_width_centi_px: Some(250),
                shadow_depth_centi_px: Some(300),
                ..Default::default()
            })
        );
    }

    #[test]
    fn plain_ass_text_preserves_font_size_as_style() {
        let decoded = plain_ass_text_metadata("{\\fs24.5}Sized");

        assert_eq!(
            decoded.style,
            Some(VideoSubtitleCueStyle {
                font_size_centi_px: Some(2450),
                ..Default::default()
            })
        );
    }

    #[test]
    fn plain_ass_text_preserves_bold_as_font_weight() {
        let decoded =
            plain_ass_text_metadata("Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,{\\b1}Bold");

        assert_eq!(decoded.text, "Bold");
        assert_eq!(
            decoded.style,
            Some(VideoSubtitleCueStyle {
                primary_color: None,
                font_weight: Some(crate::text::font::FontWeight::Bold),
                ..Default::default()
            })
        );
    }

    #[test]
    fn plain_ass_text_maps_numeric_bold_weight() {
        let decoded = plain_ass_text_metadata("{\\b600}Semi bold");

        assert_eq!(
            decoded.style,
            Some(VideoSubtitleCueStyle {
                primary_color: None,
                font_weight: Some(crate::text::font::FontWeight::SemiBold),
                ..Default::default()
            })
        );
    }

    #[test]
    fn plain_ass_text_allows_bold_to_be_disabled() {
        let decoded = plain_ass_text_metadata("{\\b1}bold{\\b0}regular");

        assert_eq!(
            decoded.style,
            Some(VideoSubtitleCueStyle {
                primary_color: None,
                font_weight: Some(crate::text::font::FontWeight::Regular),
                ..Default::default()
            })
        );
    }

    #[test]
    fn plain_ass_text_does_not_treat_border_as_bold() {
        let decoded = plain_ass_text_metadata("{\\bord4}Border");

        assert_eq!(
            decoded.style,
            Some(VideoSubtitleCueStyle {
                outline_width_centi_px: Some(400),
                font_weight: None,
                ..Default::default()
            })
        );
    }

    #[test]
    fn pal8_bitmap_to_rgba_converts_ffmpeg_rgb32_palette() {
        let indexes = [0_u8, 1, 1, 0];
        #[cfg(target_endian = "little")]
        let palette = [
            30, 20, 10, 40, //
            70, 60, 50, 80,
        ];
        #[cfg(target_endian = "big")]
        let palette = [
            40, 10, 20, 30, //
            80, 50, 60, 70,
        ];

        let pixels = unsafe {
            pal8_bitmap_to_rgba_from_ptr(indexes.as_ptr(), 2, &palette, 2, 2, 2)
                .expect("valid bitmap")
        };

        assert_eq!(
            pixels,
            vec![
                10, 20, 30, 40, //
                50, 60, 70, 80, //
                50, 60, 70, 80, //
                10, 20, 30, 40,
            ]
        );
    }

    #[test]
    fn pal8_bitmap_to_rgba_makes_out_of_range_indexes_transparent() {
        let indexes = [0_u8, 7];
        #[cfg(target_endian = "little")]
        let palette = [3, 2, 1, 4];
        #[cfg(target_endian = "big")]
        let palette = [4, 1, 2, 3];

        let pixels = unsafe {
            pal8_bitmap_to_rgba_from_ptr(indexes.as_ptr(), 2, &palette, 1, 2, 1)
                .expect("valid bitmap")
        };

        assert_eq!(pixels, vec![1, 2, 3, 4, 0, 0, 0, 0]);
    }
}
