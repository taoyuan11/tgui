use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::foundation::view_model::ValueCommand;

use super::*;

#[derive(Clone)]
struct CarouselAutoPlaySnapshot<VM: 'static> {
    id: WidgetId,
    frame: Rect,
    selected: usize,
    count: usize,
    interval: Duration,
    on_change: Option<ValueCommand<VM, usize>>,
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn drive_carousel_auto_play(&mut self, now: Instant) -> bool {
        let snapshots = {
            let states = &self.computed_scene().carousel_auto_play;
            if states.is_empty() {
                Vec::new()
            } else {
                states
                    .iter()
                    .map(|state| CarouselAutoPlaySnapshot {
                        id: state.id,
                        frame: state.frame,
                        selected: state.selected,
                        count: state.count,
                        interval: state.interval,
                        on_change: state.on_change.clone(),
                    })
                    .collect::<Vec<_>>()
            }
        };
        if snapshots.is_empty() {
            self.carousel_auto_play_last.clear();
            self.next_carousel_wakeup_deadline = None;
            return false;
        }

        let active_ids = snapshots
            .iter()
            .map(|state| state.id)
            .collect::<HashSet<_>>();
        self.carousel_auto_play_last
            .retain(|id, _| active_ids.contains(id));

        let mut next_deadline: Option<Instant> = None;
        let cursor_position = self.cursor_position;
        let mut due_targets = Vec::new();

        for state in &snapshots {
            if state.count < 2 || state.interval == Duration::ZERO || state.on_change.is_none() {
                continue;
            }

            let paused = cursor_position
                .map(|position| state.frame.contains(position))
                .unwrap_or(false);
            if paused {
                self.carousel_auto_play_last.insert(state.id, now);
                continue;
            }

            let last_tick = *self.carousel_auto_play_last.entry(state.id).or_insert(now);
            if now.duration_since(last_tick) >= state.interval {
                due_targets.push((state.id, (state.selected + 1) % state.count));
                self.carousel_auto_play_last.insert(state.id, now);
                next_deadline = min_deadline(next_deadline, now + state.interval);
            } else {
                next_deadline = min_deadline(next_deadline, last_tick + state.interval);
            }
        }

        self.next_carousel_wakeup_deadline = next_deadline;
        if due_targets.is_empty() {
            return false;
        }

        let mut changed = false;
        for (id, target) in due_targets {
            let command = snapshots
                .iter()
                .find(|state| state.id == id)
                .and_then(|state| state.on_change.as_ref());

            if let Some(command) = command {
                self.execute_value_command(command, target);
                changed = true;
            }
        }

        if changed {
            self.invalidate_scene_with_reason("carousel_auto_play");
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
        changed
    }
}

fn min_deadline(current: Option<Instant>, candidate: Instant) -> Option<Instant> {
    Some(
        current
            .map(|current| current.min(candidate))
            .unwrap_or(candidate),
    )
}
