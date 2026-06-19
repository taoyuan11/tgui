use super::*;

#[derive(Clone, Default)]
pub(crate) struct AnimationCoordinator {
    controllers: Arc<Mutex<Vec<Weak<Mutex<AnimationControllerState>>>>>,
}

impl AnimationCoordinator {
    pub(super) fn register(&self, controller: &Arc<Mutex<AnimationControllerState>>) {
        self.controllers
            .lock()
            .expect("animation coordinator lock poisoned")
            .push(Arc::downgrade(controller));
    }

    pub(crate) fn refresh(&self, now: Instant) -> bool {
        let mut controllers = self
            .controllers
            .lock()
            .expect("animation coordinator lock poisoned");
        let mut changed = false;
        controllers.retain(|weak| {
            let Some(controller) = weak.upgrade() else {
                return false;
            };
            changed |= controller
                .lock()
                .expect("animation controller lock poisoned")
                .tick(now);
            true
        });
        changed
    }

    pub(crate) fn next_frame_deadline(&self, now: Instant) -> Option<Instant> {
        let controllers = self
            .controllers
            .lock()
            .expect("animation coordinator lock poisoned");
        controllers
            .iter()
            .filter_map(|weak| weak.upgrade())
            .any(|controller| {
                controller
                    .lock()
                    .expect("animation controller lock poisoned")
                    .is_running()
            })
            .then_some(now + FRAME_INTERVAL)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TimelineSample {
    pub(crate) active: bool,
    pub(crate) completed: bool,
    pub(crate) cycle_index: u32,
    pub(crate) local_time: Duration,
    pub(crate) reversed: bool,
}

pub(crate) fn sample_timeline(
    total_duration: Duration,
    playback: Playback,
    elapsed: Duration,
) -> Option<TimelineSample> {
    let direction = playback.direction_mode();
    let fill = playback.fill();
    let repeat = playback.repeat_mode();
    let delay = playback.delay_duration();
    let start_reversed = direction.starts_reversed();

    if total_duration.is_zero() {
        return Some(TimelineSample {
            active: true,
            completed: playback.repeat_mode().finite_cycles().is_some(),
            cycle_index: 0,
            local_time: Duration::ZERO,
            reversed: start_reversed,
        });
    }

    let speed = playback.speed_factor().max(0.0);
    let scaled_elapsed = if speed == 1.0 {
        elapsed
    } else if speed == 0.0 {
        Duration::ZERO
    } else {
        Duration::from_secs_f64(elapsed.as_secs_f64() * speed as f64)
    };

    if scaled_elapsed < delay {
        return match fill {
            FillMode::Backwards | FillMode::Both => Some(TimelineSample {
                active: false,
                completed: false,
                cycle_index: 0,
                local_time: if start_reversed {
                    total_duration
                } else {
                    Duration::ZERO
                },
                reversed: start_reversed,
            }),
            FillMode::None | FillMode::Forwards => None,
        };
    }

    let active_elapsed = scaled_elapsed.saturating_sub(delay);
    let cycles = repeat.finite_cycles();

    if let Some(cycle_count) = cycles {
        if cycle_count == 1 && active_elapsed < total_duration {
            return Some(TimelineSample {
                active: true,
                completed: false,
                cycle_index: 0,
                local_time: active_elapsed,
                reversed: is_cycle_reversed(direction, 0),
            });
        } else if elapsed_reaches_cycle_count(active_elapsed, total_duration, cycle_count) {
            return match fill {
                FillMode::Forwards | FillMode::Both => {
                    let cycle_index = cycle_count.saturating_sub(1);
                    let reversed = is_cycle_reversed(direction, cycle_index);
                    Some(TimelineSample {
                        active: false,
                        completed: true,
                        cycle_index,
                        local_time: if reversed {
                            Duration::ZERO
                        } else {
                            total_duration
                        },
                        reversed,
                    })
                }
                FillMode::None | FillMode::Backwards => None,
            };
        }
    }

    let (cycle_index, cycle_time) = cycle_position(active_elapsed, total_duration);

    Some(TimelineSample {
        active: true,
        completed: false,
        cycle_index,
        local_time: cycle_time,
        reversed: is_cycle_reversed(direction, cycle_index),
    })
}

fn elapsed_reaches_cycle_count(
    elapsed: Duration,
    total_duration: Duration,
    cycle_count: u32,
) -> bool {
    elapsed.as_nanos()
        >= total_duration
            .as_nanos()
            .saturating_mul(cycle_count as u128)
}

fn cycle_position(elapsed: Duration, total_duration: Duration) -> (u32, Duration) {
    let total_nanos = total_duration.as_nanos();
    let elapsed_nanos = elapsed.as_nanos();
    let raw_cycle_index = elapsed_nanos / total_nanos;
    let remainder = elapsed_nanos % total_nanos;

    if !elapsed.is_zero() && remainder == 0 {
        return (
            raw_cycle_index.saturating_sub(1).min(u32::MAX as u128) as u32,
            total_duration,
        );
    }

    (
        raw_cycle_index.min(u32::MAX as u128) as u32,
        duration_from_nanos(remainder),
    )
}

fn duration_from_nanos(nanos: u128) -> Duration {
    Duration::new(
        (nanos / 1_000_000_000).min(u64::MAX as u128) as u64,
        (nanos % 1_000_000_000) as u32,
    )
}

fn is_cycle_reversed(direction: PlaybackDirection, cycle_index: u32) -> bool {
    match direction {
        PlaybackDirection::Normal => false,
        PlaybackDirection::Reverse => true,
        PlaybackDirection::Alternate => cycle_index % 2 == 1,
        PlaybackDirection::AlternateReverse => cycle_index % 2 == 0,
    }
}
