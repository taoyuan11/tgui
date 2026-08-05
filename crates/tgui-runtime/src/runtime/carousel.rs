use std::time::{Duration, Instant};

use smallvec::SmallVec;

use super::*;

#[derive(Clone)]
struct CarouselAutoPlaySnapshot {
    id: WidgetId,
    frame: Rect,
    selected: usize,
    count: usize,
    interval: Duration,
    disabled: bool,
    has_on_change: bool,
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(in crate::runtime) fn drive_carousel_auto_play(&mut self, now: Instant) -> bool {
        let snapshots = {
            let states = &self.computed_scene().carousel_auto_play;
            if states.is_empty() {
                SmallVec::new()
            } else {
                states
                    .iter()
                    .map(|state| CarouselAutoPlaySnapshot {
                        id: state.id,
                        frame: state.frame,
                        selected: state.selected.resolve_untracked(),
                        count: state.count,
                        interval: state.interval,
                        disabled: state.disabled.resolve_untracked(),
                        has_on_change: state.on_change.is_some(),
                    })
                    .collect::<SmallVec<[_; 2]>>()
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
            .collect::<SmallVec<[_; 2]>>();
        self.carousel_auto_play_last
            .retain(|id, _| active_ids.contains(id));

        let mut next_deadline: Option<Instant> = None;
        let cursor_position = self.cursor_position;
        let focused_ancestors = self
            .focused_widget_id()
            .and_then(|focused_id| {
                let layout = self.cached_scene.as_ref()?.layout.as_ref()?;
                let mut ancestors = SmallVec::<[WidgetId; 8]>::new();
                let mut current = Some(focused_id);
                while let Some(widget_id) = current {
                    ancestors.push(widget_id);
                    current = layout.parent_of(widget_id);
                }
                Some(ancestors)
            })
            .unwrap_or_default();
        let mut due_targets = SmallVec::<[(WidgetId, usize); 2]>::new();

        for state in &snapshots {
            if state.disabled
                || state.count < 2
                || state.interval == Duration::ZERO
                || !state.has_on_change
            {
                self.carousel_auto_play_last.remove(&state.id);
                continue;
            }

            let paused = cursor_position
                .map(|position| state.frame.contains(position))
                .unwrap_or(false)
                || focused_ancestors.contains(&state.id);
            if paused {
                // Removing the clock makes resume start a fresh interval. Keeping the previous
                // timestamp would make a long hover/disabled pause advance immediately on exit.
                self.carousel_auto_play_last.remove(&state.id);
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

        let commands = {
            let states = &self.computed_scene().carousel_auto_play;
            due_targets
                .iter()
                .filter_map(|(id, target)| {
                    states
                        .iter()
                        .find(|state| state.id == *id)
                        .and_then(|state| state.on_change.clone())
                        .map(|command| (command, *target))
                })
                .collect::<SmallVec<[_; 2]>>()
        };

        let changed = !commands.is_empty();
        for (command, target) in commands {
            self.execute_value_command(&command, target);
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
