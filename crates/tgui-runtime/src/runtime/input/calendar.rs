use chrono::{Datelike, Duration, NaiveDate};

use super::*;

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn focused_calendar_day(
        &mut self,
    ) -> Option<crate::ui::widget::CalendarDayInteraction<VM>> {
        let focused_id = self.focused_widget_id()?;
        let computed = self.computed_scene();
        computed
            .hit_regions
            .iter()
            .chain(computed.overlay_hit_regions.iter())
            .find_map(|region| {
                let focus = region.focus.as_ref()?;
                if focus.widget_id != focused_id {
                    return None;
                }
                region
                    .interaction
                    .interactions()
                    .and_then(|interactions| interactions.calendar_day.clone())
            })
    }

    fn move_focused_calendar_day_to(&mut self, target: Option<NaiveDate>) -> bool {
        let Some(day) = self.focused_calendar_day() else {
            return false;
        };
        let Some(target) = target else {
            return true;
        };
        if target == day.date {
            return true;
        }
        self.pending_calendar_focus = Some((day.owner_id, target));
        self.execute_value_command(&day.on_focus_move, target);
        true
    }

    pub(super) fn move_focused_calendar_day_by_days(&mut self, days: i64) -> bool {
        let target = self
            .focused_calendar_day()
            .and_then(|day| day.date.checked_add_signed(Duration::days(days)));
        if target.is_none() && self.focused_calendar_day().is_none() {
            return false;
        }
        self.move_focused_calendar_day_to(target)
    }

    pub(super) fn move_focused_calendar_day_to_week_edge(&mut self, end: bool) -> bool {
        let Some(day) = self.focused_calendar_day() else {
            return false;
        };
        let weekday = i64::from(day.date.weekday().num_days_from_monday());
        let delta = if end { 6 - weekday } else { -weekday };
        self.move_focused_calendar_day_to(day.date.checked_add_signed(Duration::days(delta)))
    }

    pub(super) fn move_focused_calendar_day_by_months(&mut self, months: i32) -> bool {
        let Some(day) = self.focused_calendar_day() else {
            return false;
        };
        self.move_focused_calendar_day_to(shift_month_clamped(day.date, months))
    }

    pub(in crate::runtime) fn reconcile_calendar_focus_after_scene_update(&mut self) -> bool {
        let Some((owner_id, date)) = self.pending_calendar_focus else {
            return false;
        };
        let target = self.cached_scene.as_ref().and_then(|cached| {
            cached
                .computed
                .hit_regions
                .iter()
                .chain(cached.computed.overlay_hit_regions.iter())
                .find_map(|region| {
                    let calendar_day = region
                        .interaction
                        .interactions()
                        .and_then(|interactions| interactions.calendar_day.as_ref())?;
                    if calendar_day.owner_id != owner_id || calendar_day.date != date {
                        return None;
                    }
                    let focus = region.focus.as_ref()?;
                    Some((
                        FocusedWidget {
                            widget_id: focus.widget_id,
                            scope_path: focus.scope_path.clone(),
                            on_blur: focus.on_blur.clone(),
                        },
                        focus.on_focus.clone(),
                    ))
                })
        });
        let Some((target, on_focus)) = target else {
            return false;
        };
        self.pending_calendar_focus = None;
        self.update_focus(Some(target), on_focus, true);
        true
    }
}

fn shift_month_clamped(date: NaiveDate, months: i32) -> Option<NaiveDate> {
    let total = i64::from(date.year())
        .checked_mul(12)?
        .checked_add(i64::from(date.month0()))?
        .checked_add(i64::from(months))?;
    let year = i32::try_from(total.div_euclid(12)).ok()?;
    let month = u32::try_from(total.rem_euclid(12) + 1).ok()?;
    let mut day = date.day();
    loop {
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
            return Some(date);
        }
        day = day.checked_sub(1)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_shift_clamps_to_the_last_valid_day() {
        let january_31 = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();
        assert_eq!(
            shift_month_clamped(january_31, 1),
            NaiveDate::from_ymd_opt(2024, 2, 29)
        );
        assert_eq!(
            shift_month_clamped(january_31, -1),
            NaiveDate::from_ymd_opt(2023, 12, 31)
        );
    }
}
