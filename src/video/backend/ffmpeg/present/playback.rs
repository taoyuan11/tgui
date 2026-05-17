use std::time::{Duration, Instant};

use super::*;

impl PresentWorker {
    pub(super) fn next_wait_duration(&self) -> Duration {
        if !self.decode_playing {
            return COMMAND_POLL_INTERVAL;
        }

        let Some(next_frame) = self.shared_queue.front(self.current_generation) else {
            return STEP_IDLE_SLEEP;
        };
        let playback = self.playback_position();
        let due_position = playback.saturating_add(VIDEO_PRESENT_TOLERANCE);
        if due_position >= next_frame.position {
            return Duration::ZERO;
        }
        next_frame
            .position
            .saturating_sub(due_position)
            .min(COMMAND_POLL_INTERVAL)
    }

    pub(super) fn playback_position(&self) -> Duration {
        if let Some(audio_clock) = self.current_audio_clock.as_ref() {
            return self
                .current_start_position
                .saturating_add(audio_clock.position());
        }

        match self.software_play_started_at {
            Some(started_at) => self
                .software_paused_position
                .saturating_add(started_at.elapsed()),
            None => self.software_paused_position,
        }
    }

    pub(super) fn buffered_position(&self, current_position: Duration) -> Option<Duration> {
        let audio_buffer_end = self
            .current_audio_clock
            .as_ref()
            .map(|clock| current_position.saturating_add(clock.buffered_duration()));
        let video_buffer_end = self.shared_queue.tail_end_position(self.current_generation);

        match (audio_buffer_end, video_buffer_end) {
            (Some(a), Some(v)) => Some(a.min(v)),
            (Some(a), None) => Some(a),
            (None, Some(v)) => Some(v),
            (None, None) => None,
        }
    }

    pub(super) fn present_due_frames(&mut self) {
        loop {
            let Some(next_frame) = self.shared_queue.front(self.current_generation) else {
                break;
            };
            if !self.is_frame_due(next_frame.position) {
                break;
            }
            let _ = self.present_next_frame();
        }
    }

    pub(super) fn present_next_frame(&mut self) -> Option<Duration> {
        let frame = self
            .shared_queue
            .pop_front_matching(self.current_generation)?;
        let position = frame.position;
        let texture = frame.texture;
        *self.latest_frame.lock().expect("video frame lock poisoned") = Some(texture.clone());
        self.shared.surface.set(VideoSurfaceSnapshot {
            intrinsic_size: self.current_intrinsic_size,
            texture: Some(texture),
            loading: false,
            error: None,
        });

        self.last_presented_position = position;
        if self.current_audio_clock.is_none() {
            self.software_paused_position = position;
            if self.decode_playing {
                self.software_play_started_at = Some(Instant::now());
            }
        }

        self.playback_clock.set_position(position);

        let mut metrics = self.shared.metrics.get();
        metrics.duration = self.current_duration;
        metrics.position = position;
        metrics.buffered = self.buffered_position(position);
        metrics.video_width = self.current_video_size.width;
        metrics.video_height = self.current_video_size.height;
        self.shared.metrics.set(metrics);
        Some(position)
    }

    pub(super) fn is_frame_due(&self, position: Duration) -> bool {
        if let Some(audio_clock) = self.current_audio_clock.as_ref() {
            if !audio_clock.has_started_clock() {
                return false;
            }
            let playback = self
                .current_start_position
                .saturating_add(audio_clock.position());
            return playback.saturating_add(VIDEO_PRESENT_TOLERANCE) >= position;
        }

        self.playback_position()
            .saturating_add(VIDEO_PRESENT_TOLERANCE)
            >= position
    }

    pub(super) fn evaluate_playback_state(&mut self) {
        if !self.stream_opened {
            return;
        }

        if self.playback_ended {
            self.set_decode_playing(false);
            self.shared.playback_state.set(PlaybackState::Ended);
            return;
        }

        if !self.should_play {
            return;
        }

        if self.should_buffer() && !self.should_keep_draining_eof() {
            self.set_decode_playing(false);
            self.shared.playback_state.set(PlaybackState::Buffering);
            return;
        }

        let can_start = if self.startup_pending {
            self.can_start_playback()
        } else {
            self.can_resume_playback()
        };
        if can_start || self.should_keep_draining_eof() {
            self.startup_pending = false;
            self.pending_open_reason = None;
            self.set_decode_playing(true);
            self.shared.playback_state.set(PlaybackState::Playing);
        } else {
            self.set_decode_playing(false);
            self.shared.playback_state.set(PlaybackState::Buffering);
        }
    }

    pub(super) fn remaining_duration(&self) -> Option<Duration> {
        self.current_duration
            .map(|duration| duration.saturating_sub(self.playback_position()))
    }

    pub(super) fn audio_buffered_duration(&self) -> Duration {
        self.current_audio_clock
            .as_ref()
            .map(|clock| clock.buffered_duration())
            .unwrap_or(Duration::ZERO)
    }

    pub(super) fn video_buffered_duration(&self) -> Duration {
        let baseline = self.last_presented_position.max(self.playback_position());
        self.shared_queue
            .tail_end_position(self.current_generation)
            .map(|end| end.saturating_sub(baseline))
            .unwrap_or(Duration::ZERO)
    }

    pub(super) fn can_start_playback(&self) -> bool {
        let audio_ok = self.current_audio_clock.is_none()
            || self.audio_buffered_duration() >= self.current_buffering_profile.start_buffer_target;
        let video_ok = video_buffer_target_satisfied(
            self.video_buffered_duration(),
            self.current_buffering_profile.video_start_buffer_target,
            self.remaining_duration(),
            self.shared_queue.ready_frame_count(self.current_generation)
                >= self.current_buffering_profile.video_max_packet_count,
        );
        (audio_ok && video_ok)
            || startup_playback_blocked_by_memory_limit(
                self.buffer_snapshot.buffering_constrained_by_memory_limit,
                self.shared_queue.has_frames(self.current_generation),
                self.current_audio_clock.is_some(),
                self.audio_buffered_duration(),
            )
    }

    pub(super) fn can_resume_playback(&self) -> bool {
        let audio_ok = self.current_audio_clock.is_none()
            || self.audio_buffered_duration() >= self.current_buffering_profile.rebuffer_target;
        let video_ok = video_buffer_target_satisfied(
            self.video_buffered_duration(),
            self.current_buffering_profile.video_resume_buffer_target,
            self.remaining_duration(),
            self.shared_queue.ready_frame_count(self.current_generation)
                >= self.current_buffering_profile.video_max_packet_count,
        );
        (audio_ok && video_ok)
            || startup_playback_blocked_by_memory_limit(
                self.buffer_snapshot.buffering_constrained_by_memory_limit,
                self.shared_queue.has_frames(self.current_generation),
                self.current_audio_clock.is_some(),
                self.audio_buffered_duration(),
            )
    }

    pub(super) fn should_buffer(&self) -> bool {
        let audio_starving = self
            .current_audio_clock
            .as_ref()
            .map(|clock| {
                clock.buffered_duration() < self.current_buffering_profile.audio_starving_threshold
            })
            .unwrap_or(false);
        let video_starving = should_buffer_video(
            self.video_buffered_duration(),
            VIDEO_REBUFFER_ENTER_THRESHOLD,
            self.remaining_duration(),
        );
        should_buffer_for_rebuffer(
            audio_starving,
            video_starving,
            self.buffer_snapshot.buffering_constrained_by_memory_limit,
        )
    }

    pub(super) fn should_keep_draining_eof(&self) -> bool {
        self.buffer_snapshot.eof_sent
            && (self.shared_queue.has_frames(self.current_generation)
                || !self.audio_buffered_duration().is_zero())
    }
}
