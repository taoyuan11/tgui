use super::*;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};

use accesskit::{Action, ActionRequest, Node, Role, TreeId};
use chrono::{NaiveDate, NaiveTime};

use crate::ui::widget::{
    ColorPicker, DatePicker, TimePicker, Upload, UploadFile, UploadFileId, UploadSelection,
    UploadStatus,
};

fn accessibility_update(handler: &mut BoundRuntimeHandler<TestVm>) -> accesskit::TreeUpdate {
    handler.accessibility_tree_update_for_test()
}

fn node_with_label<'a>(update: &'a accesskit::TreeUpdate, role: Role, label: &str) -> &'a Node {
    update
        .nodes
        .iter()
        .find_map(|(_, node)| (node.role() == role && node.label() == Some(label)).then_some(node))
        .unwrap_or_else(|| panic!("missing {role:?} node named {label:?}"))
}

fn node_center(node: &Node) -> Point {
    let bounds = node
        .bounds()
        .expect("interactive accessibility node bounds");
    Point::new(
        dp(((bounds.x0 + bounds.x1) * 0.5) as f32),
        dp(((bounds.y0 + bounds.y1) * 0.5) as f32),
    )
}

fn press_pointer(handler: &mut BoundRuntimeHandler<TestVm>, point: Point) {
    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
}

fn has_open_popover(handler: &mut BoundRuntimeHandler<TestVm>) -> bool {
    handler
        .computed_scene()
        .overlay_close_handlers
        .iter()
        .any(|handle| handle.layer == crate::runtime::overlay::OverlayLayer::Popover)
}

fn scene_contains_text(handler: &mut BoundRuntimeHandler<TestVm>, expected: &str) -> bool {
    let computed = handler.computed_scene();
    computed
        .scene
        .texts
        .iter()
        .chain(computed.scene.overlay_texts.iter())
        .any(|text| text.content.as_ref() == expected)
}

fn focused_calendar_date(handler: &mut BoundRuntimeHandler<TestVm>) -> Option<NaiveDate> {
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

#[test]
fn date_picker_mouse_selection_updates_state_and_accessibility() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::new_legacy("2026-06-06");
    let tree = WidgetTree::new(
        DatePicker::new(
            controller.clone(),
            Some(NaiveDate::from_ymd_opt(2026, 6, 6).unwrap()),
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        )
        .label("Start date"),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(520.0, 520.0),
    );
    handler.reduced_motion = true;

    let closed = accessibility_update(&mut handler);
    let input = node_with_label(&closed, Role::TextInput, "Start date");
    assert_eq!(input.value(), Some("2026-06-06"));
    let trigger = node_with_label(&closed, Role::Button, "Open date picker");
    assert!(trigger.supports_action(Action::Click));
    let trigger_center = node_center(trigger);

    press_pointer(&mut handler, trigger_center);
    assert!(has_open_popover(&mut handler));
    let open = accessibility_update(&mut handler);
    let selected = node_with_label(&open, Role::Button, "2026-06-06");
    assert_eq!(selected.is_selected(), Some(true));
    let target = node_with_label(&open, Role::Button, "2026-06-15");
    assert_eq!(target.is_selected(), Some(false));
    let target_center = node_center(target);

    press_pointer(&mut handler, target_center);
    let _ = handler.computed_scene();
    assert_eq!(controller.text(), "2026-06-15");
    assert!(!has_open_popover(&mut handler));
}

#[test]
fn date_picker_keyboard_selection_closes_and_restores_input_focus() {
    let invalidation = InvalidationSignal::new();
    let selected = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
    let controller = TextController::new_legacy("2026-06-15");
    let tree = WidgetTree::new(DatePicker::new(
        controller,
        Some(selected),
        NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
    ));
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(520.0, 520.0),
    );
    handler.reduced_motion = true;

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    let input_id = handler
        .focused_text_input_id()
        .expect("date input should be the first tab stop");
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert!(has_open_popover(&mut handler));

    let tab_stop_count = handler.focusable_widgets_in_tab_order().len();
    assert!(
        tab_stop_count > 2,
        "open calendar should add keyboard stops"
    );
    let mut reached_selected_day = false;
    for _ in 0..=tab_stop_count {
        assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
        if focused_calendar_date(&mut handler) == Some(selected) {
            reached_selected_day = true;
            break;
        }
    }
    assert!(
        reached_selected_day,
        "Tab should reach the calendar's roving day stop"
    );
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    let _ = handler.computed_scene();

    assert!(!has_open_popover(&mut handler));
    assert_eq!(handler.focused_widget_id(), Some(input_id));
}

#[test]
fn color_picker_trigger_toggles_with_pointer_keyboard_and_accessibility_click() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(ColorPicker::new(Color::hexa(0x3366CCFF)));
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(520.0, 520.0),
    );
    handler.reduced_motion = true;

    let closed = accessibility_update(&mut handler);
    let (trigger_node_id, trigger) = closed
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Button && node.label() == Some("#3366CCFF"))
        .expect("color picker trigger");
    assert!(trigger.supports_action(Action::Click));
    let trigger_node_id = *trigger_node_id;
    let trigger_center = node_center(trigger);

    press_pointer(&mut handler, trigger_center);
    assert!(has_open_popover(&mut handler));
    press_pointer(&mut handler, trigger_center);
    assert!(!has_open_popover(&mut handler));

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert!(has_open_popover(&mut handler));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert!(!has_open_popover(&mut handler));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Space))));
    assert!(has_open_popover(&mut handler));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Space))));
    assert!(!has_open_popover(&mut handler));

    handler
        .accessibility_action_sender
        .send(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: trigger_node_id,
            data: None,
        })
        .unwrap();
    assert!(handler.drain_accessibility_actions());
    assert!(has_open_popover(&mut handler));
}

#[test]
fn controlled_read_only_picker_triggers_are_inert_but_text_fields_remain_editable() {
    for kind in 0..3 {
        let invalidation = InvalidationSignal::new();
        let open = State::new(false, invalidation.clone());
        let (root, trigger_label, input_label): (Element<TestVm>, &str, Option<&str>) = match kind {
            0 => (
                DatePicker::new(
                    "2026-06-06",
                    Some(NaiveDate::from_ymd_opt(2026, 6, 6).unwrap()),
                    NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
                )
                .open(open.signal())
                .label("Date")
                .into(),
                "Open date picker",
                Some("Date"),
            ),
            1 => (
                TimePicker::new("09:30", Some(NaiveTime::from_hms_opt(9, 30, 0).unwrap()))
                    .open(open.signal())
                    .label("Time")
                    .into(),
                "Open time picker",
                Some("Time"),
            ),
            _ => (
                ColorPicker::new(Color::hexa(0x3366CCFF))
                    .open(open.signal())
                    .into(),
                "#3366CCFF",
                None,
            ),
        };
        let mut handler = test_handler_with_config(
            TestVm,
            Some(WidgetTree::new(root)),
            invalidation,
            test_config_with_size(520.0, 520.0),
        );

        let update = accessibility_update(&mut handler);
        let (trigger_id, trigger) = update
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Button && node.label() == Some(trigger_label))
            .unwrap_or_else(|| panic!("missing controlled picker trigger {trigger_label:?}"));
        assert!(!trigger.supports_action(Action::Click));
        assert!(!trigger.supports_action(Action::Focus));
        let trigger_id = *trigger_id;
        handler
            .accessibility_action_sender
            .send(ActionRequest {
                action: Action::Click,
                target_tree: TreeId::ROOT,
                target_node: trigger_id,
                data: None,
            })
            .unwrap();
        assert!(!handler.drain_accessibility_actions());
        assert!(!has_open_popover(&mut handler));

        if let Some(input_label) = input_label {
            let update = accessibility_update(&mut handler);
            let input = node_with_label(&update, Role::TextInput, input_label);
            assert!(input.supports_action(Action::SetValue));
            assert_eq!(handler.focusable_widgets_in_tab_order().len(), 1);
            assert!(
                handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab,)))
            );
            let _ = handler
                .handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter)));
        } else {
            assert!(handler.focusable_widgets_in_tab_order().is_empty());
            assert!(!handler
                .handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter,))));
        }
        assert!(!has_open_popover(&mut handler));
    }
}

#[test]
fn time_picker_mouse_controls_are_named_selected_and_deduplicated() {
    let invalidation = InvalidationSignal::new();
    let controller = TextController::new_legacy("09:00");
    let tree = WidgetTree::new(
        TimePicker::new(
            controller.clone(),
            Some(NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
        )
        .label("Appointment time")
        .minute_step(30),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(520.0, 520.0),
    );
    handler.reduced_motion = true;

    let closed = accessibility_update(&mut handler);
    assert_eq!(
        node_with_label(&closed, Role::TextInput, "Appointment time").value(),
        Some("09:00")
    );
    let trigger_center = node_center(node_with_label(&closed, Role::Button, "Open time picker"));
    press_pointer(&mut handler, trigger_center);

    let open = accessibility_update(&mut handler);
    assert_eq!(
        node_with_label(&open, Role::Button, "Hour 09").is_selected(),
        Some(true)
    );
    assert_eq!(
        node_with_label(&open, Role::Button, "Minute 00").is_selected(),
        Some(true)
    );
    let minute_values = open
        .nodes
        .iter()
        .filter_map(|(_, node)| {
            (node.role() == Role::Button)
                .then(|| node.label())
                .flatten()
                .filter(|label| label.starts_with("Minute "))
        })
        .collect::<Vec<_>>();
    assert_eq!(minute_values, vec!["Minute 30", "Minute 00"]);

    let hour_ten = node_center(node_with_label(&open, Role::Button, "Hour 10"));
    press_pointer(&mut handler, hour_ten);
    assert_eq!(controller.text(), "10:00");
    assert!(has_open_popover(&mut handler));

    let updated = accessibility_update(&mut handler);
    assert_eq!(
        node_with_label(&updated, Role::Button, "Hour 10").is_selected(),
        Some(true)
    );
    let done = node_center(node_with_label(&updated, Role::Button, "Done"));
    press_pointer(&mut handler, done);
    let _ = handler.computed_scene();
    assert!(!has_open_popover(&mut handler));
}

#[test]
fn picker_disabled_signals_block_stale_pointer_and_keyboard_activation() {
    for is_date in [true, false] {
        let invalidation = InvalidationSignal::new();
        let disabled = State::new(false, invalidation.clone());
        let tree = if is_date {
            WidgetTree::new(
                DatePicker::new(
                    "2026-06-06",
                    Some(NaiveDate::from_ymd_opt(2026, 6, 6).unwrap()),
                    NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
                )
                .disable(disabled.signal()),
            )
        } else {
            WidgetTree::new(
                TimePicker::new("09:30", Some(NaiveTime::from_hms_opt(9, 30, 0).unwrap()))
                    .disable(disabled.signal()),
            )
        };
        let mut handler = test_handler_with_config(
            TestVm,
            Some(tree),
            invalidation,
            test_config_with_size(520.0, 520.0),
        );
        handler.reduced_motion = true;

        let enabled = accessibility_update(&mut handler);
        let trigger_label = if is_date {
            "Open date picker"
        } else {
            "Open time picker"
        };
        let stale_center = node_center(node_with_label(&enabled, Role::Button, trigger_label));
        disabled.set(true);

        press_pointer(&mut handler, stale_center);
        assert!(!has_open_popover(&mut handler));
        let update = accessibility_update(&mut handler);
        let trigger = node_with_label(&update, Role::Button, trigger_label);
        assert!(trigger.is_disabled());
        assert!(!trigger.supports_action(Action::Click));
        let input = update
            .nodes
            .iter()
            .find_map(|(_, node)| (node.role() == Role::TextInput).then_some(node))
            .expect("picker input node");
        assert!(input.is_disabled());

        assert!(!handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
        assert_eq!(handler.focused_widget_id(), None);
    }
}

fn upload_file(id: &str, name: &str, progress: f32) -> UploadFile {
    UploadFile {
        id: UploadFileId::new(id),
        path: PathBuf::from(name),
        name: name.to_string(),
        size_bytes: Some(1024),
        status: UploadStatus::Uploading { progress },
    }
}

#[test]
fn upload_remove_supports_pointer_keyboard_and_file_specific_semantics() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(Upload::new(vec![
        upload_file("a", "report-a.pdf", 0.25),
        upload_file("b", "report-b.pdf", 0.75),
    ]));
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(600.0, 520.0),
    );

    let update = accessibility_update(&mut handler);
    assert!(node_with_label(&update, Role::Button, "Choose files").supports_action(Action::Click));
    assert_eq!(
        node_with_label(
            &update,
            Role::ProgressIndicator,
            "Upload progress for report-a.pdf"
        )
        .numeric_value(),
        Some(0.25)
    );
    let remove_a = node_with_label(&update, Role::Button, "Remove report-a.pdf");
    assert!(remove_a.supports_action(Action::Click));
    let remove_a_center = node_center(remove_a);

    press_pointer(&mut handler, remove_a_center);
    assert!(!scene_contains_text(&mut handler, "report-a.pdf"));
    assert!(scene_contains_text(&mut handler, "report-b.pdf"));

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert!(!scene_contains_text(&mut handler, "report-b.pdf"));
}

#[test]
fn controlled_read_only_upload_omits_selection_actions_but_static_upload_keeps_them() {
    let invalidation = InvalidationSignal::new();
    let files = State::new(Vec::<UploadFile>::new(), invalidation.clone());
    let mut controlled = test_handler_with_config(
        TestVm,
        Some(WidgetTree::new(Upload::new(files.signal()))),
        invalidation,
        test_config_with_size(600.0, 520.0),
    );

    let update = accessibility_update(&mut controlled);
    let (choose_id, choose) = update
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Button && node.label() == Some("Choose files"))
        .expect("controlled upload chooser");
    assert!(choose.is_disabled());
    assert!(!choose.supports_action(Action::Click));
    assert!(!choose.supports_action(Action::Focus));
    let choose_id = *choose_id;
    assert!(controlled
        .computed_scene()
        .hit_regions
        .iter()
        .all(|region| region
            .interaction
            .interactions()
            .is_none_or(|interactions| interactions.on_file_drop.is_none())));
    assert!(controlled.focusable_widgets_in_tab_order().is_empty());
    controlled
        .accessibility_action_sender
        .send(ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: choose_id,
            data: None,
        })
        .unwrap();
    assert!(!controlled.drain_accessibility_actions());

    let invalidation = InvalidationSignal::new();
    let mut uncontrolled = test_handler_with_config(
        TestVm,
        Some(WidgetTree::new(Upload::new(Vec::<UploadFile>::new()))),
        invalidation,
        test_config_with_size(600.0, 520.0),
    );
    let update = accessibility_update(&mut uncontrolled);
    assert!(node_with_label(&update, Role::Button, "Choose files").supports_action(Action::Click));
    assert!(uncontrolled
        .computed_scene()
        .hit_regions
        .iter()
        .any(|region| region
            .interaction
            .interactions()
            .is_some_and(|interactions| interactions.on_file_drop.is_some())));
}

struct TemporaryUploadFile {
    path: PathBuf,
}

impl TemporaryUploadFile {
    fn new(name: &str, contents: &[u8]) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "tgui-runtime-upload-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).expect("create upload test directory");
        let path = directory.join(name);
        std::fs::write(&path, contents).expect("write upload test file");
        Self { path }
    }
}

impl Drop for TemporaryUploadFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        if let Some(directory) = self.path.parent() {
            let _ = std::fs::remove_dir(directory);
        }
    }
}

#[test]
fn upload_drop_respects_live_disabled_state_and_updates_static_files() {
    let invalidation = InvalidationSignal::new();
    let disabled = State::new(true, invalidation.clone());
    let selections = Arc::new(Mutex::new(Vec::<UploadSelection>::new()));
    let selections_for_command = Arc::clone(&selections);
    let tree = WidgetTree::new(
        Upload::new(Vec::new())
            .disable(disabled.signal())
            .accept_extensions(&["txt"])
            .on_select(ValueCommand::new(move |_: &mut TestVm, selection| {
                selections_for_command.lock().unwrap().push(selection);
            })),
    );
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(600.0, 520.0),
    );
    let event_loop = TestEventLoop;
    let file = TemporaryUploadFile::new("notes.txt", b"notes");
    let drop_event = || WindowEvent::DragDropped {
        paths: vec![file.path.clone()],
        position: PhysicalPosition::new(120.0, 80.0),
    };

    let disabled_update = accessibility_update(&mut handler);
    let choose = node_with_label(&disabled_update, Role::Button, "Choose files");
    assert!(choose.is_disabled());
    assert!(!choose.supports_action(Action::Click));
    let _ = handler.handle_bound_window_event(&event_loop, drop_event());
    assert!(selections.lock().unwrap().is_empty());
    assert!(!scene_contains_text(&mut handler, "notes.txt"));

    disabled.set(false);
    let _ = handler.handle_bound_window_event(&event_loop, drop_event());
    let selections = selections.lock().unwrap();
    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0].files.len(), 1);
    drop(selections);
    assert!(scene_contains_text(&mut handler, "notes.txt"));
}
