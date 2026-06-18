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
    let start_reversed = playback.direction_mode().starts_reversed();

    if total_duration.is_zero() {
        return Some(TimelineSample {
            active: true,
            completed: playback.repeat_mode().finite_cycles().is_some(),
            cycle_index: 0,
            local_time: Duration::ZERO,
            reversed: start_reversed,
        });
    }

    let scaled_elapsed =
        Duration::from_secs_f64(elapsed.as_secs_f64() * playback.speed_factor().max(0.0) as f64);

    if scaled_elapsed < playback.delay_duration() {
        return match playback.fill() {
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

    let active_elapsed = scaled_elapsed.saturating_sub(playback.delay_duration());
    let cycle_secs = total_duration.as_secs_f64();
    let elapsed_secs = active_elapsed.as_secs_f64();
    let cycles = playback.repeat_mode().finite_cycles();

    if let Some(cycle_count) = cycles {
        let total_secs = cycle_secs * cycle_count as f64;
        if elapsed_secs >= total_secs {
            return match playback.fill() {
                FillMode::Forwards | FillMode::Both => {
                    let cycle_index = cycle_count.saturating_sub(1);
                    let reversed = is_cycle_reversed(playback.direction_mode(), cycle_index);
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

    let mut cycle_index = (elapsed_secs / cycle_secs).floor() as u32;
    let mut cycle_time = Duration::from_secs_f64(elapsed_secs % cycle_secs);
    if !active_elapsed.is_zero() && cycle_time.is_zero() {
        cycle_index = cycle_index.saturating_sub(1);
        cycle_time = total_duration;
    }

    Some(TimelineSample {
        active: true,
        completed: false,
        cycle_index,
        local_time: cycle_time,
        reversed: is_cycle_reversed(playback.direction_mode(), cycle_index),
    })
}

fn is_cycle_reversed(direction: PlaybackDirection, cycle_index: u32) -> bool {
    match direction {
        PlaybackDirection::Normal => false,
        PlaybackDirection::Reverse => true,
        PlaybackDirection::Alternate => cycle_index % 2 == 1,
        PlaybackDirection::AlternateReverse => cycle_index % 2 == 0,
    }
}
