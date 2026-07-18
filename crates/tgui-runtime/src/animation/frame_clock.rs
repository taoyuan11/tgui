use std::time::{Duration, Instant};

const DEFAULT_REFRESH_RATE_MILLIHERTZ: u32 = 60_000;
const MIN_REFRESH_RATE_MILLIHERTZ: u32 = 30_000;
const MAX_REFRESH_RATE_MILLIHERTZ: u32 = 240_000;
const NANOS_PER_MILLIHERTZ_PERIOD: u64 = 1_000_000_000_000;

pub(crate) const DEFAULT_FRAME_INTERVAL: Duration = Duration::from_nanos(16_666_667);

fn normalized_refresh_rate(refresh_rate_millihertz: Option<u32>) -> u32 {
    refresh_rate_millihertz
        .filter(|rate| *rate > 0)
        .unwrap_or(DEFAULT_REFRESH_RATE_MILLIHERTZ)
        .clamp(MIN_REFRESH_RATE_MILLIHERTZ, MAX_REFRESH_RATE_MILLIHERTZ)
}

fn interval_for_refresh_rate(refresh_rate_millihertz: Option<u32>) -> Duration {
    let rate = u64::from(normalized_refresh_rate(refresh_rate_millihertz));
    Duration::from_nanos((NANOS_PER_MILLIHERTZ_PERIOD + rate / 2) / rate)
}

fn duration_mul(interval: Duration, count: u128) -> Duration {
    let nanos = interval.as_nanos().saturating_mul(count);
    Duration::from_nanos(nanos.min(u128::from(u64::MAX)) as u64)
}

fn aligned_deadline_after(origin: Instant, interval: Duration, now: Instant) -> Instant {
    if now < origin {
        return origin;
    }

    let interval_nanos = interval.as_nanos().max(1);
    let elapsed_nanos = now.saturating_duration_since(origin).as_nanos();
    let steps = elapsed_nanos / interval_nanos + 1;
    origin
        .checked_add(duration_mul(interval, steps))
        .unwrap_or_else(|| now.checked_add(interval).unwrap_or(now))
}

/// Copyable timing information propagated through retained scene collection.
///
/// Toasts and nested portal/overlay scenes use this snapshot to request the next
/// absolute-phase frame instead of restarting cadence from collection time.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameClockSnapshot {
    origin: Instant,
    interval: Duration,
}

impl FrameClockSnapshot {
    pub(crate) fn fallback(origin: Instant) -> Self {
        Self {
            origin,
            interval: DEFAULT_FRAME_INTERVAL,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_refresh_rate(origin: Instant, refresh_rate_millihertz: Option<u32>) -> Self {
        Self {
            origin,
            interval: interval_for_refresh_rate(refresh_rate_millihertz),
        }
    }

    pub(crate) fn interval(self) -> Duration {
        self.interval
    }

    pub(crate) fn next_deadline_after(self, now: Instant) -> Instant {
        aligned_deadline_after(self.origin, self.interval, now)
    }
}

/// Per-window animation clock tied to the current monitor refresh rate.
///
/// The clock advances from an absolute phase. A late render therefore skips
/// missed ticks and returns to the original cadence instead of accumulating
/// `render_cost + interval` drift on every frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AdaptiveFrameClock {
    refresh_rate_millihertz: u32,
    snapshot: FrameClockSnapshot,
    next_tick: Option<Instant>,
}

impl AdaptiveFrameClock {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            refresh_rate_millihertz: DEFAULT_REFRESH_RATE_MILLIHERTZ,
            snapshot: FrameClockSnapshot::fallback(now),
            next_tick: None,
        }
    }

    pub(crate) fn snapshot(self) -> FrameClockSnapshot {
        self.snapshot
    }

    pub(crate) fn interval(self) -> Duration {
        self.snapshot.interval()
    }

    pub(crate) fn is_armed(self) -> bool {
        self.next_tick.is_some()
    }

    /// Update cadence from a monitor-reported rate. Unknown and zero rates use
    /// 60Hz; implausible values are clamped to 30–240Hz.
    pub(crate) fn update_refresh_rate(
        &mut self,
        refresh_rate_millihertz: Option<u32>,
        now: Instant,
    ) -> bool {
        let normalized = normalized_refresh_rate(refresh_rate_millihertz);
        if normalized == self.refresh_rate_millihertz {
            return false;
        }

        let interval = interval_for_refresh_rate(Some(normalized));
        let candidate = now.checked_add(interval).unwrap_or(now);
        let next_tick = self.next_tick.map(|deadline| {
            if deadline <= now {
                deadline
            } else {
                deadline.min(candidate)
            }
        });
        let next_phase = next_tick.unwrap_or(candidate);
        self.refresh_rate_millihertz = normalized;
        self.snapshot = FrameClockSnapshot {
            origin: next_phase.checked_sub(interval).unwrap_or(now),
            interval,
        };
        self.next_tick = next_tick;
        true
    }

    /// Arm or disarm the window clock for the current set of frame-driven
    /// sources and return the next absolute deadline.
    pub(crate) fn set_active(&mut self, active: bool, now: Instant) -> Option<Instant> {
        if !active {
            self.next_tick = None;
            return None;
        }

        if self.next_tick.is_none() {
            self.next_tick = Some(self.snapshot.next_deadline_after(now));
        }
        self.next_tick
    }

    /// Consume at most one logical frame. Missed periods are skipped in O(1),
    /// and the following deadline remains on the original phase grid.
    pub(crate) fn consume_due_tick(&mut self, now: Instant) -> bool {
        let Some(deadline) = self.next_tick else {
            return false;
        };
        if deadline > now {
            return false;
        }

        self.next_tick = Some(aligned_deadline_after(
            deadline,
            self.snapshot.interval,
            now,
        ));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_interval(rate: Option<u32>, expected_nanos: u64) {
        assert_eq!(
            interval_for_refresh_rate(rate),
            Duration::from_nanos(expected_nanos)
        );
    }

    #[test]
    fn refresh_rates_resolve_60_120_and_144_hz_periods() {
        assert_interval(None, 16_666_667);
        assert_interval(Some(60_000), 16_666_667);
        assert_interval(Some(120_000), 8_333_333);
        assert_interval(Some(144_000), 6_944_444);
        let snapshot = FrameClockSnapshot::for_refresh_rate(Instant::now(), Some(144_000));
        assert_eq!(snapshot.interval(), Duration::from_nanos(6_944_444));
    }

    #[test]
    fn refresh_rate_is_clamped_and_zero_falls_back_to_60_hz() {
        assert_interval(Some(0), 16_666_667);
        assert_interval(Some(10_000), 33_333_333);
        assert_interval(Some(500_000), 4_166_667);
    }

    #[test]
    fn late_frames_keep_absolute_phase_without_render_cost_drift() {
        let start = Instant::now();
        let mut clock = AdaptiveFrameClock::new(start);
        assert!(clock.update_refresh_rate(Some(120_000), start));
        let interval = clock.interval();
        let first = clock.set_active(true, start).unwrap();
        assert_eq!(first, start + interval);

        let render_late = first + Duration::from_millis(3);
        assert!(clock.consume_due_tick(render_late));
        assert_eq!(clock.set_active(true, render_late), Some(first + interval));

        let several_frames_late = first + interval * 4 + Duration::from_millis(2);
        assert!(clock.consume_due_tick(several_frames_late));
        assert_eq!(
            clock.set_active(true, several_frames_late),
            Some(first + interval * 5)
        );
    }

    #[test]
    fn refresh_change_rephases_without_delaying_an_already_armed_frame() {
        let start = Instant::now();
        let mut clock = AdaptiveFrameClock::new(start);
        let original = clock.set_active(true, start).unwrap();
        let changed_at = start + Duration::from_millis(2);
        assert!(clock.update_refresh_rate(Some(144_000), changed_at));
        let faster = clock.set_active(true, changed_at).unwrap();
        assert!(faster <= original);
        assert_eq!(clock.interval(), Duration::from_nanos(6_944_444));
    }

    #[test]
    fn refresh_change_does_not_drop_a_tick_that_is_already_due() {
        let start = Instant::now();
        let mut clock = AdaptiveFrameClock::new(start);
        let due = clock.set_active(true, start).unwrap();
        assert!(clock.update_refresh_rate(Some(120_000), due));
        assert!(clock.consume_due_tick(due));
        assert!(clock.set_active(true, due).is_some_and(|next| next > due));
    }

    #[test]
    fn inactive_clock_is_fully_disarmed() {
        let start = Instant::now();
        let mut clock = AdaptiveFrameClock::new(start);
        assert!(clock.set_active(true, start).is_some());
        assert!(clock.is_armed());
        assert_eq!(clock.set_active(false, start), None);
        assert!(!clock.is_armed());
        assert!(!clock.consume_due_tick(start + Duration::from_secs(1)));
    }

    #[test]
    fn windows_keep_independent_refresh_cadences() {
        let start = Instant::now();
        let mut sixty = AdaptiveFrameClock::new(start);
        let mut one_forty_four = AdaptiveFrameClock::new(start);
        assert!(one_forty_four.update_refresh_rate(Some(144_000), start));

        assert_eq!(
            sixty.set_active(true, start),
            Some(start + Duration::from_nanos(16_666_667))
        );
        assert_eq!(
            one_forty_four.set_active(true, start),
            Some(start + Duration::from_nanos(6_944_444))
        );

        assert!(sixty.update_refresh_rate(Some(120_000), start));
        assert_eq!(one_forty_four.interval(), Duration::from_nanos(6_944_444));
    }
}
