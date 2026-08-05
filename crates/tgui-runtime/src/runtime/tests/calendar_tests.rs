use super::*;

use chrono::{Datelike, NaiveDate, Weekday};

use crate::ui::widget::{Calendar, CalendarChangeTrigger, CalendarSelectionChange};

fn focused_calendar_date(handler: &mut BoundRuntimeHandler<TestVm>) -> Option<NaiveDate> {
    let _ = handler.computed_scene();
    let focused = handler.focused_widget_id()?;
    let computed = handler.computed_scene();
    computed
        .hit_regions
        .iter()
        .chain(computed.overlay_hit_regions.iter())
        .find_map(|region| {
            let focus = region.focus.as_ref()?;
            if focus.widget_id != focused {
                return None;
            }
            region
                .interaction
                .interactions()
                .and_then(|interactions| interactions.calendar_day.as_ref())
                .map(|day| day.date)
        })
}

fn press(handler: &mut BoundRuntimeHandler<TestVm>, key: KeyCode) {
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(key))));
}

#[test]
fn calendar_uses_roving_tab_focus_and_standard_date_navigation() {
    let invalidation = InvalidationSignal::new();
    let changes = Arc::new(Mutex::new(Vec::<CalendarSelectionChange>::new()));
    let changes_for_command = Arc::clone(&changes);
    let selected = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
    let tree = WidgetTree::new(
        Calendar::new(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(), Some(selected))
            .today(None)
            .on_change(ValueCommand::new(move |_: &mut TestVm, change| {
                changes_for_command.lock().unwrap().push(change);
            })),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(480.0, 480.0),
    );

    press(&mut handler, KeyCode::Tab);
    press(&mut handler, KeyCode::Tab);
    press(&mut handler, KeyCode::Tab);
    assert_eq!(focused_calendar_date(&mut handler), Some(selected));

    let calendar_tab_stops = handler
        .computed_scene()
        .hit_regions
        .iter()
        .filter(|region| {
            region
                .interaction
                .interactions()
                .is_some_and(|interactions| interactions.calendar_day.is_some())
                && region
                    .focus
                    .as_ref()
                    .is_some_and(|focus| focus.tab_index.unwrap_or(0) >= 0)
        })
        .count();
    assert_eq!(calendar_tab_stops, 1);

    press(&mut handler, KeyCode::ArrowRight);
    assert_eq!(
        focused_calendar_date(&mut handler),
        NaiveDate::from_ymd_opt(2026, 6, 16)
    );
    press(&mut handler, KeyCode::ArrowDown);
    assert_eq!(
        focused_calendar_date(&mut handler),
        NaiveDate::from_ymd_opt(2026, 6, 23)
    );
    press(&mut handler, KeyCode::Home);
    let monday = focused_calendar_date(&mut handler).unwrap();
    assert_eq!(monday, NaiveDate::from_ymd_opt(2026, 6, 22).unwrap());
    assert_eq!(monday.weekday(), Weekday::Mon);
    press(&mut handler, KeyCode::End);
    let sunday = focused_calendar_date(&mut handler).unwrap();
    assert_eq!(sunday, NaiveDate::from_ymd_opt(2026, 6, 28).unwrap());
    assert_eq!(sunday.weekday(), Weekday::Sun);
    assert!(changes.lock().unwrap().is_empty());

    press(&mut handler, KeyCode::PageDown);
    assert_eq!(
        focused_calendar_date(&mut handler),
        NaiveDate::from_ymd_opt(2026, 7, 28)
    );
    press(&mut handler, KeyCode::PageUp);
    assert_eq!(
        focused_calendar_date(&mut handler),
        NaiveDate::from_ymd_opt(2026, 6, 28)
    );
    let month_changes = changes.lock().unwrap().clone();
    assert_eq!(month_changes.len(), 2);
    assert_eq!(month_changes[0].trigger, CalendarChangeTrigger::NextMonth);
    assert_eq!(
        month_changes[1].trigger,
        CalendarChangeTrigger::PreviousMonth
    );

    press(&mut handler, KeyCode::Enter);
    let changes = changes.lock().unwrap();
    assert_eq!(changes.len(), 3);
    assert_eq!(changes[2].trigger, CalendarChangeTrigger::Day);
    assert_eq!(changes[2].date, sunday);
}

#[test]
fn calendar_page_navigation_clamps_end_of_month() {
    let invalidation = InvalidationSignal::new();
    let january_31 = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();
    let tree = WidgetTree::new(Calendar::<TestVm>::new(january_31, Some(january_31)).today(None));
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(480.0, 480.0),
    );

    press(&mut handler, KeyCode::Tab);
    press(&mut handler, KeyCode::Tab);
    press(&mut handler, KeyCode::Tab);
    assert_eq!(focused_calendar_date(&mut handler), Some(january_31));
    press(&mut handler, KeyCode::PageDown);
    assert_eq!(
        focused_calendar_date(&mut handler),
        NaiveDate::from_ymd_opt(2024, 2, 29)
    );
}

#[test]
fn controlled_calendar_without_change_callback_is_read_only_and_skips_tab_order() {
    let invalidation = InvalidationSignal::new();
    let display_month = State::new(
        NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        invalidation.clone(),
    );
    let selected = State::new(
        Some(NaiveDate::from_ymd_opt(2026, 6, 15).unwrap()),
        invalidation.clone(),
    );
    let tree = WidgetTree::new(
        Calendar::<TestVm>::new(display_month.signal(), selected.signal()).today(None),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(480.0, 480.0),
    );

    let computed = handler.computed_scene();
    assert!(computed.hit_regions.iter().all(|region| {
        region.focus.is_none()
            && region
                .interaction
                .interactions()
                .is_none_or(|interactions| interactions.calendar_day.is_none())
    }));
    assert!(!handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab,))));
    assert_eq!(handler.focused_widget_id(), None);
}
