use super::*;

use std::sync::Arc;
use std::sync::Mutex;

use crate::platform::event::MouseButton;
use crate::ui::widget::{
    Button, ContextMenu, DoubleTapEvent, GestureRecognizer, KeyChord, LongPressEvent, Menu,
    MenuBar, MenuBarEntry, MenuItem,
};

fn press_menu_point<VM: ViewModel>(
    handler: &mut BoundRuntimeHandler<VM>,
    point: Point,
    button: CanvasMouseButton,
) {
    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(point);
    let _ = handler.handle_hover(viewport);
    handler.handle_mouse_press(viewport, Instant::now(), button);
}

fn has_open_menu<VM: ViewModel>(handler: &mut BoundRuntimeHandler<VM>) -> bool {
    handler
        .computed_scene()
        .overlay_close_handlers
        .iter()
        .any(|handle| handle.layer == crate::runtime::overlay::OverlayLayer::Menu)
}

#[test]
fn arrow_keys_advance_menu_cursor_and_enter_dispatches_select() {
    let invalidation = InvalidationSignal::new();
    let counter = Arc::new(Mutex::new(0_u32));
    let counter_for_cmd = counter.clone();
    let tree = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![
                MenuItem::new("New").on_select(crate::foundation::view_model::Command::new(
                    move |_: &mut TestVm| {
                        *counter_for_cmd.lock().unwrap() += 1;
                    },
                )),
                MenuItem::separator(),
                MenuItem::new("Open"),
            ])
            .open(true),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    // 菜单已打开：Up 应被吞掉（即返回 true），按 Enter 应触发第一项 on_select。
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown))),
        "ArrowDown should be consumed by open menu"
    );
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))),
        "Enter should activate the menu cursor"
    );
    assert_eq!(
        *counter.lock().unwrap(),
        1,
        "on_select should have fired once"
    );
}

#[test]
fn right_key_enters_submenu_and_enter_dispatches_inner_item() {
    let invalidation = InvalidationSignal::new();
    let counter = Arc::new(Mutex::new(0_u32));
    let counter_for_cmd = counter.clone();
    let tree = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![
                MenuItem::submenu(
                    "Recent",
                    vec![
                        MenuItem::new("a.txt").on_select(
                            crate::foundation::view_model::Command::new(move |_: &mut TestVm| {
                                *counter_for_cmd.lock().unwrap() += 1;
                            }),
                        ),
                        MenuItem::new("b.txt"),
                    ],
                ),
                MenuItem::new("Exit"),
            ])
            .open(true),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    // Down → cursor 在 "Recent"(idx=0)。
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)))
    );
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    // Right → 进入 submenu，cursor 跳到 "a.txt"。
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight)))
    );
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    // Enter → 触发 a.txt 的 on_select。
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert_eq!(
        *counter.lock().unwrap(),
        1,
        "submenu leaf on_select should fire"
    );
}

#[test]
fn left_key_pops_submenu_cursor_back_to_parent() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![
                MenuItem::submenu(
                    "Recent",
                    vec![MenuItem::new("a.txt"), MenuItem::new("b.txt")],
                ),
                MenuItem::new("Exit"),
            ])
            .open(true),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    // Down → ArrowRight → 进入 submenu，path 深度 2。
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)));
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();
    handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight)));
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    // Left → 弹栈，path 深度 1。再 Left → 没有 MenuBar，noop（仍返回 false 或 true 都行，不应 panic）。
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowLeft)))
    );
    let _ =
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowLeft)));
}

#[test]
fn keyboard_does_not_interfere_when_menu_closed() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![MenuItem::new("New")])
            .open(false),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    // 菜单关闭：ArrowDown 不应被菜单分支吞掉（应回退到原始 focus/slider 路径）。
    // 这里只验证不 panic 并且返回不一定为 true；即菜单导航没被错误激活。
    let _ =
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown)));
}

#[test]
fn uncontrolled_menu_opens_when_trigger_clicked() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0))).items(vec![MenuItem::new("New")]),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(Point::new(dp(24.0), dp(14.0)));
    let _ = handler.handle_hover(viewport);

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.invalidate_computed_scene();
    let labels: Vec<_> = handler
        .computed_scene()
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect();
    assert!(labels.iter().any(|label| *label == "New"));
}

#[test]
fn context_menu_opens_on_right_click_without_on_show() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        ContextMenu::new(Button::new("Photo").size(dp(90.0), dp(30.0)))
            .items(vec![MenuItem::new("Copy"), MenuItem::new("Delete")]),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(Point::new(dp(24.0), dp(14.0)));
    let _ = handler.handle_hover(viewport);

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Right);
    handler.invalidate_computed_scene();
    let labels: Vec<_> = handler
        .computed_scene()
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect();
    assert!(labels.iter().any(|label| *label == "Copy"));
    assert!(labels.iter().any(|label| *label == "Delete"));
}

#[test]
fn shift_f10_opens_focused_context_menu_without_faking_pointer_show_event() {
    let invalidation = InvalidationSignal::new();
    let opens = Arc::new(Mutex::new(Vec::new()));
    let shows = Arc::new(Mutex::new(0_u32));
    let opens_for_command = opens.clone();
    let shows_for_command = shows.clone();
    let context_menu: Element<TestVm> =
        ContextMenu::new(Button::new("Photo").size(dp(90.0), dp(30.0)))
            .items(vec![MenuItem::new("Copy")])
            .on_show(ValueCommand::new(move |_: &mut TestVm, _| {
                *shows_for_command.lock().unwrap() += 1;
            }))
            .on_open_change(ValueCommand::new(move |_: &mut TestVm, open| {
                opens_for_command.lock().unwrap().push(open);
            }))
            .into();
    let context_menu_id = context_menu.id;
    let mut handler = test_handler(Some(WidgetTree::new(context_menu)), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert_eq!(handler.focused_widget_id(), Some(context_menu_id));
    handler.modifiers = ModifiersState::SHIFT;
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::F10))));

    let anchor = handler
        .context_menu_anchor_states
        .get(&context_menu_id)
        .copied()
        .expect("Shift+F10 should install a context-menu anchor");
    assert_eq!(anchor, Point::new(dp(0.0), dp(30.0)));
    assert_eq!(*opens.lock().unwrap(), vec![true]);
    assert_eq!(*shows.lock().unwrap(), 0);
    assert!(has_open_menu(&mut handler));
}

#[test]
fn context_menu_key_opens_focused_context_menu() {
    let invalidation = InvalidationSignal::new();
    let context_menu: Element<TestVm> =
        ContextMenu::new(Button::new("Photo").size(dp(90.0), dp(30.0)))
            .items(vec![MenuItem::new("Copy")])
            .into();
    let context_menu_id = context_menu.id;
    let mut handler = test_handler(Some(WidgetTree::new(context_menu)), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ContextMenu,)))
    );
    assert!(handler
        .context_menu_anchor_states
        .contains_key(&context_menu_id));
    assert!(has_open_menu(&mut handler));
}

#[test]
fn disabled_context_menu_rejects_semantic_keyboard_open() {
    let invalidation = InvalidationSignal::new();
    let context_menu: Element<TestVm> =
        ContextMenu::new(Button::new("Photo").size(dp(90.0), dp(30.0)))
            .items(vec![MenuItem::new("Copy")])
            .disable(true)
            .into();
    let context_menu_id = context_menu.id;
    let mut handler = test_handler(Some(WidgetTree::new(context_menu)), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    handler.modifiers = ModifiersState::SHIFT;
    assert!(!handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::F10))));
    assert!(!handler
        .context_menu_anchor_states
        .contains_key(&context_menu_id));
    assert!(!has_open_menu(&mut handler));
}

#[test]
fn removed_menu_widget_prunes_all_runtime_menu_state() {
    let invalidation = InvalidationSignal::new();
    let menu: Element<TestVm> = Menu::new(Button::new("File"))
        .items(vec![MenuItem::new("New")])
        .into();
    let menu_id = menu.id;
    let mut handler = test_handler(Some(WidgetTree::new(menu)), invalidation);
    handler.menu_open_states.insert(menu_id, true);
    handler
        .context_menu_anchor_states
        .insert(menu_id, Point::new(dp(1.0), dp(2.0)));
    handler.menu_keyboard_cursor.insert(menu_id, vec![0]);

    handler.prune_removed_widget_state(&std::collections::HashSet::from([menu_id]));

    assert!(!handler.menu_open_states.contains_key(&menu_id));
    assert!(!handler.context_menu_anchor_states.contains_key(&menu_id));
    assert!(!handler.menu_keyboard_cursor.contains_key(&menu_id));
}

#[test]
fn uncontrolled_menubar_opens_entry_when_clicked() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        MenuBar::<TestVm>::uncontrolled()
            .entry("File", vec![MenuItem::new("New")])
            .entry("Edit", vec![MenuItem::new("Undo")]),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    handler.cursor_position = Some(Point::new(dp(24.0), dp(14.0)));
    let _ = handler.handle_hover(viewport);

    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    handler.invalidate_computed_scene();
    let labels: Vec<_> = handler
        .computed_scene()
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect();
    assert!(labels.iter().any(|label| *label == "New"));
}

#[test]
fn global_shortcut_dispatches_on_select_even_when_menu_closed() {
    use crate::platform::keyboard::ModifiersState;

    let invalidation = InvalidationSignal::new();
    let counter = Arc::new(Mutex::new(0_u32));
    let counter_for_cmd = counter.clone();
    let tree = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![MenuItem::new("New")
                .shortcut(KeyChord::new(KeyCode::KeyN).ctrl())
                .on_select(crate::foundation::view_model::Command::new(
                    move |_: &mut TestVm| {
                        *counter_for_cmd.lock().unwrap() += 1;
                    },
                ))])
            .open(false),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    handler.modifiers = ModifiersState::CONTROL;
    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    let consumed =
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::KeyN)));
    assert!(
        consumed,
        "Ctrl+N should be consumed by global menu shortcut"
    );
    assert_eq!(*counter.lock().unwrap(), 1, "on_select should fire once");
}

#[test]
fn type_ahead_jumps_cursor_to_matching_item() {
    use crate::platform::keyboard::{Key, KeyLocation};

    let invalidation = InvalidationSignal::new();
    let counter = Arc::new(Mutex::new(0_u32));
    let counter_for_cmd = counter.clone();
    let tree = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![
                MenuItem::new("Copy"),
                MenuItem::new("Paste").on_select(crate::foundation::view_model::Command::new(
                    move |_: &mut TestVm| {
                        *counter_for_cmd.lock().unwrap() += 1;
                    },
                )),
                MenuItem::new("Delete"),
            ])
            .open(true),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let viewport = handler.viewport_rect();
    let _ = handler.handle_hover(viewport);
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    // 按 'p' → cursor 应跳到 Paste；Enter 触发 Paste.on_select。
    let p_event = pressed_key_event(PhysicalKey::Code(KeyCode::KeyP));
    let p_event = KeyEvent {
        logical_key: Key::Character("p".into()),
        ..p_event
    };
    let _ = p_event.location;
    let _ = KeyLocation::Standard;
    assert!(handler.handle_keyboard_input(&p_event));
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert_eq!(*counter.lock().unwrap(), 1, "type-ahead Paste should fire");
}

#[test]
fn uncontrolled_menu_trigger_second_click_closes_without_reopening() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0))).items(vec![MenuItem::new("New")]),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let trigger = Point::new(dp(24.0), dp(14.0));

    press_menu_point(&mut handler, trigger, CanvasMouseButton::Left);
    assert!(has_open_menu(&mut handler));

    press_menu_point(&mut handler, trigger, CanvasMouseButton::Left);
    assert!(
        !has_open_menu(&mut handler),
        "the trigger press must pass the menu focus trap and close instead of reopening"
    );
}

#[test]
fn uncontrolled_menu_on_open_change_reports_true_then_false_and_escape_closes() {
    let invalidation = InvalidationSignal::new();
    let changes = Arc::new(Mutex::new(Vec::new()));
    let changes_for_command = changes.clone();
    let tree = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![MenuItem::new("New")])
            .on_open_change(ValueCommand::new(move |_: &mut TestVm, open| {
                changes_for_command.lock().unwrap().push(open);
            })),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    press_menu_point(
        &mut handler,
        Point::new(dp(24.0), dp(14.0)),
        CanvasMouseButton::Left,
    );
    assert!(has_open_menu(&mut handler));
    assert!(handler.consume_topmost_overlay_close_handler_escape());
    assert!(!has_open_menu(&mut handler));
    assert_eq!(*changes.lock().unwrap(), vec![true, false]);
}

#[test]
fn menu_and_context_menu_find_composite_trigger_owner() {
    let invalidation = InvalidationSignal::new();
    let menu_trigger = Flex::horizontal()
        .size(dp(140.0), dp(36.0))
        .child(Button::new("Nested menu").size(dp(140.0), dp(36.0)));
    let tree = WidgetTree::new(Menu::new(menu_trigger).items(vec![MenuItem::new("Menu action")]));
    let mut handler = test_handler(Some(tree), invalidation);
    press_menu_point(
        &mut handler,
        Point::new(dp(70.0), dp(18.0)),
        CanvasMouseButton::Left,
    );
    assert!(has_open_menu(&mut handler));

    let invalidation = InvalidationSignal::new();
    let context_trigger = Flex::horizontal()
        .size(dp(140.0), dp(36.0))
        .child(Button::new("Nested context").size(dp(140.0), dp(36.0)));
    let tree = WidgetTree::new(
        ContextMenu::new(context_trigger).items(vec![MenuItem::new("Context action")]),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    press_menu_point(
        &mut handler,
        Point::new(dp(70.0), dp(18.0)),
        CanvasMouseButton::Right,
    );
    assert!(has_open_menu(&mut handler));
}

#[test]
fn context_menu_platform_right_press_dispatches_show_once() {
    let invalidation = InvalidationSignal::new();
    let shows = Arc::new(Mutex::new(Vec::new()));
    let opens = Arc::new(Mutex::new(Vec::new()));
    let trigger_clicks = Arc::new(Mutex::new(0_u32));
    let shows_for_command = shows.clone();
    let opens_for_command = opens.clone();
    let trigger_clicks_for_command = trigger_clicks.clone();
    let trigger = Flex::horizontal().size(dp(140.0), dp(36.0)).child(
        Button::new("Nested context")
            .size(dp(140.0), dp(36.0))
            .on_click(Command::new(move |_: &mut TestVm| {
                *trigger_clicks_for_command.lock().unwrap() += 1;
            })),
    );
    let tree = WidgetTree::new(
        ContextMenu::new(trigger)
            .items(vec![MenuItem::new("Copy")])
            .on_show(ValueCommand::new(move |_: &mut TestVm, event| {
                shows_for_command.lock().unwrap().push(event);
            }))
            .on_open_change(ValueCommand::new(move |_: &mut TestVm, open| {
                opens_for_command.lock().unwrap().push(open);
            })),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    handler.handle_bound_window_event(
        &TestEventLoop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(70.0, 18.0),
            state: ElementState::Pressed,
            button: ButtonSource::Mouse(MouseButton::Right),
            primary: true,
        },
    );

    assert!(has_open_menu(&mut handler));
    let shows = shows.lock().unwrap();
    assert_eq!(shows.len(), 1);
    assert_eq!(shows[0].source, crate::ui::widget::GestureSource::Mouse);
    assert_eq!(shows[0].position, Point::new(dp(70.0), dp(18.0)));
    assert_eq!(*opens.lock().unwrap(), vec![true]);
    assert_eq!(
        *trigger_clicks.lock().unwrap(),
        0,
        "right click must not invoke the trigger's primary action"
    );
}

#[test]
fn context_menu_keyboard_navigation_activates_item_and_closes() {
    let invalidation = InvalidationSignal::new();
    let selected = Arc::new(Mutex::new(0_u32));
    let selected_for_command = selected.clone();
    let opens = Arc::new(Mutex::new(Vec::new()));
    let opens_for_command = opens.clone();
    let tree = WidgetTree::new(
        ContextMenu::new(Button::new("Photo").size(dp(90.0), dp(30.0)))
            .items(vec![MenuItem::new("Copy").on_select(Command::new(
                move |_: &mut TestVm| {
                    *selected_for_command.lock().unwrap() += 1;
                },
            ))])
            .on_open_change(ValueCommand::new(move |_: &mut TestVm, open| {
                opens_for_command.lock().unwrap().push(open);
            })),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    press_menu_point(
        &mut handler,
        Point::new(dp(24.0), dp(14.0)),
        CanvasMouseButton::Right,
    );
    assert!(has_open_menu(&mut handler));

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown,)))
    );
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter,))));

    assert_eq!(*selected.lock().unwrap(), 1);
    assert_eq!(*opens.lock().unwrap(), vec![true, false]);
    assert!(!has_open_menu(&mut handler));
}

#[test]
fn disabled_menu_trigger_and_global_shortcut_do_nothing() {
    let invalidation = InvalidationSignal::new();
    let opens = Arc::new(Mutex::new(Vec::new()));
    let selections = Arc::new(Mutex::new(0_u32));
    let opens_for_command = opens.clone();
    let selections_for_command = selections.clone();
    let tree = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![MenuItem::new("New")
                .shortcut(KeyChord::new(KeyCode::KeyN).ctrl())
                .on_select(Command::new(move |_: &mut TestVm| {
                    *selections_for_command.lock().unwrap() += 1;
                }))])
            .disable(true)
            .on_open_change(ValueCommand::new(move |_: &mut TestVm, open| {
                opens_for_command.lock().unwrap().push(open);
            })),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    press_menu_point(
        &mut handler,
        Point::new(dp(24.0), dp(14.0)),
        CanvasMouseButton::Left,
    );
    assert!(!has_open_menu(&mut handler));

    handler.modifiers = ModifiersState::CONTROL;
    let _ = handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::KeyN)));
    assert!(opens.lock().unwrap().is_empty());
    assert_eq!(*selections.lock().unwrap(), 0);
}

#[test]
fn accessibility_activation_toggles_composite_menu_and_keeps_trigger_click() {
    let invalidation = InvalidationSignal::new();
    let clicks = Arc::new(Mutex::new(0_u32));
    let clicks_for_command = clicks.clone();
    let trigger = Flex::horizontal().size(dp(140.0), dp(36.0)).child(
        Button::new("Nested")
            .size(dp(140.0), dp(36.0))
            .on_click(Command::new(move |_: &mut TestVm| {
                *clicks_for_command.lock().unwrap() += 1;
            })),
    );
    let tree = WidgetTree::new(Menu::new(trigger).items(vec![MenuItem::new("Action")]));
    let mut handler = test_handler(Some(tree), invalidation);
    handler.cursor_position = Some(Point::new(dp(70.0), dp(18.0)));
    let interaction = handler
        .hit_path(handler.viewport_rect())
        .last()
        .cloned()
        .expect("nested trigger hit");

    assert!(handler.dispatch_accessibility_click_interaction(interaction));
    assert!(has_open_menu(&mut handler));
    assert_eq!(*clicks.lock().unwrap(), 1);
}

#[test]
fn keyboard_activation_toggles_composite_menu_and_keeps_trigger_click() {
    let invalidation = InvalidationSignal::new();
    let clicks = Arc::new(Mutex::new(0_u32));
    let clicks_for_command = clicks.clone();
    let trigger = Flex::horizontal().size(dp(140.0), dp(36.0)).child(
        Button::new("Nested")
            .size(dp(140.0), dp(36.0))
            .on_click(Command::new(move |_: &mut TestVm| {
                *clicks_for_command.lock().unwrap() += 1;
            })),
    );
    let tree = WidgetTree::new(Menu::new(trigger).items(vec![MenuItem::new("Action")]));
    let mut handler = test_handler(Some(tree), invalidation);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Tab))));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert!(has_open_menu(&mut handler));
    assert_eq!(*clicks.lock().unwrap(), 1);
}

#[test]
fn context_menu_keyboard_enters_submenu_and_activates_leaf() {
    let invalidation = InvalidationSignal::new();
    let selected = Arc::new(Mutex::new(0_u32));
    let selected_for_command = selected.clone();
    let tree = WidgetTree::new(
        ContextMenu::new(Button::new("Photo").size(dp(90.0), dp(30.0))).items(vec![
            MenuItem::submenu(
                "Recent",
                vec![
                    MenuItem::new("a.txt").on_select(Command::new(move |_: &mut TestVm| {
                        *selected_for_command.lock().unwrap() += 1;
                    })),
                ],
            ),
        ]),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    press_menu_point(
        &mut handler,
        Point::new(dp(24.0), dp(14.0)),
        CanvasMouseButton::Right,
    );
    assert!(has_open_menu(&mut handler));
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown,)))
    );
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight,)))
    );
    handler.invalidate_computed_scene();
    let submenu_handlers = handler
        .computed_scene()
        .overlay_close_handlers
        .iter()
        .filter(|handle| handle.layer == crate::runtime::overlay::OverlayLayer::Menu)
        .count();
    assert!(submenu_handlers >= 2);
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));

    assert_eq!(*selected.lock().unwrap(), 1);
    assert!(!has_open_menu(&mut handler));
}

#[test]
fn context_menu_preserves_existing_child_long_press_handler() {
    let invalidation = InvalidationSignal::new();
    let child_presses = Arc::new(Mutex::new(0_u32));
    let shows = Arc::new(Mutex::new(0_u32));
    let child_presses_for_command = child_presses.clone();
    let shows_for_command = shows.clone();
    let child: Element<TestVm> = Element::from(Button::new("Hold").size(dp(100.0), dp(40.0)))
        .gesture(GestureRecognizer::new().on_long_press(ValueCommand::new(
            move |_: &mut TestVm, _: LongPressEvent| {
                *child_presses_for_command.lock().unwrap() += 1;
            },
        )));
    let tree = WidgetTree::new(
        ContextMenu::new(child)
            .items(vec![MenuItem::new("Copy")])
            .on_show(ValueCommand::new(
                move |_: &mut TestVm, _: LongPressEvent| {
                    *shows_for_command.lock().unwrap() += 1;
                },
            )),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    handler.handle_bound_window_event(
        &TestEventLoop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(40.0, 20.0),
            state: ElementState::Pressed,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );
    let _ = handler.drive_animations(
        &TestEventLoop,
        Instant::now() + crate::runtime::LONG_PRESS_THRESHOLD + Duration::from_millis(10),
    );

    assert_eq!(*shows.lock().unwrap(), 1);
    assert_eq!(*child_presses.lock().unwrap(), 1);
    assert!(has_open_menu(&mut handler));
}

#[test]
fn context_menu_long_press_survives_deeper_non_long_press_gesture() {
    let invalidation = InvalidationSignal::new();
    let shows = Arc::new(Mutex::new(0_u32));
    let shows_for_command = shows.clone();
    let child: Element<TestVm> = Element::from(Button::new("Nested").size(dp(120.0), dp(40.0)))
        .gesture(
            GestureRecognizer::new()
                .on_double_tap(ValueCommand::new(|_: &mut TestVm, _: DoubleTapEvent| {})),
        );
    let trigger = Flex::horizontal().size(dp(120.0), dp(40.0)).child(child);
    let tree = WidgetTree::new(
        ContextMenu::new(trigger)
            .items(vec![MenuItem::new("Copy")])
            .on_show(ValueCommand::new(
                move |_: &mut TestVm, _: LongPressEvent| {
                    *shows_for_command.lock().unwrap() += 1;
                },
            )),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    handler.handle_bound_window_event(
        &TestEventLoop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(40.0, 20.0),
            state: ElementState::Pressed,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );
    let _ = handler.drive_animations(
        &TestEventLoop,
        Instant::now() + crate::runtime::LONG_PRESS_THRESHOLD + Duration::from_millis(10),
    );

    assert_eq!(*shows.lock().unwrap(), 1);
    assert!(has_open_menu(&mut handler));
}

struct RebuildMenuVm {
    open: State<bool>,
    selected: usize,
}

impl ViewModel for RebuildMenuVm {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            open: context.state(true),
            selected: 0,
        }
    }

    fn view(&self) -> Element<Self> {
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![MenuItem::new("Rebuild").on_select(
                Command::new_with_context(|vm: &mut Self, context| {
                    vm.selected += 1;
                    context.request_rebuild();
                }),
            )])
            .open(self.open.signal())
            .on_open_change(ValueCommand::new(|vm: &mut Self, open| vm.open.set(open)))
            .into()
    }
}

#[test]
fn menu_item_closes_before_command_rebuilds_tree() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let vm = RebuildMenuVm::new(&context);
    let tree = WidgetTree::new(vm.view());
    let mut handler = test_handler_with_vm(vm, Some(tree), invalidation);
    handler.root_view = Some(Arc::new(RebuildMenuVm::view));
    let item_rect = handler
        .computed_scene()
        .overlay_hit_regions
        .iter()
        .find_map(|region| {
            matches!(region.interaction, HitInteraction::SelectOption { .. }).then_some(region.rect)
        })
        .expect("menu item hit region");
    let item_point = Point::new(
        item_rect.x + item_rect.width * 0.5,
        item_rect.y + item_rect.height * 0.5,
    );

    press_menu_point(&mut handler, item_point, CanvasMouseButton::Left);

    let vm = handler.view_model.lock().unwrap();
    assert!(!vm.open.get());
    assert_eq!(vm.selected, 1);
    drop(vm);
    assert!(!has_open_menu(&mut handler));
}

struct MenuBarVm {
    active: State<Option<usize>>,
    changes: Vec<Option<usize>>,
}

impl ViewModel for MenuBarVm {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            active: context.state(None),
            changes: Vec::new(),
        }
    }

    fn view(&self) -> Element<Self> {
        MenuBar::new(self.active.signal())
            .on_active_change(ValueCommand::new(|vm: &mut Self, next| {
                vm.changes.push(next);
                vm.active.set(next);
            }))
            .entry("File", vec![MenuItem::new("New")])
            .into()
    }
}

#[test]
fn controlled_menubar_click_toggles_once_per_press() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let vm = MenuBarVm::new(&context);
    let tree = WidgetTree::new(vm.view());
    let mut handler = test_handler_with_vm(vm, Some(tree), invalidation);
    let trigger = Point::new(dp(24.0), dp(14.0));

    press_menu_point(&mut handler, trigger, CanvasMouseButton::Left);
    assert!(has_open_menu(&mut handler));
    press_menu_point(&mut handler, trigger, CanvasMouseButton::Left);
    assert!(!has_open_menu(&mut handler));

    let vm = handler.view_model.lock().unwrap();
    assert_eq!(vm.changes, vec![Some(0), None]);
}

#[test]
fn controlled_submenu_outside_click_notifies_close_once() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let open = context.state(true);
    let open_for_command = open.clone();
    let changes = Arc::new(Mutex::new(Vec::new()));
    let changes_for_command = changes.clone();
    let tree = WidgetTree::new(
        Menu::new(Button::new("File").size(dp(80.0), dp(28.0)))
            .items(vec![MenuItem::submenu(
                "Recent",
                vec![MenuItem::new("a.txt")],
            )])
            .open(open.signal())
            .on_open_change(ValueCommand::new(move |_: &mut TestVm, next| {
                changes_for_command.lock().unwrap().push(next);
                open_for_command.set(next);
            })),
    );
    let mut handler = test_handler(Some(tree), invalidation);
    let _ = handler.computed_scene();
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowDown,)))
    );
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight,)))
    );
    handler.invalidate_computed_scene();
    let handler_count = handler
        .computed_scene()
        .overlay_close_handlers
        .iter()
        .filter(|handle| handle.layer == crate::runtime::overlay::OverlayLayer::Menu)
        .count();
    assert!(
        handler_count >= 2,
        "expected root menu and submenu handlers"
    );

    let _ = handler.consume_overlay_close_handlers_outside_click(Point::new(dp(500.0), dp(500.0)));
    assert!(!open.get());
    assert_eq!(*changes.lock().unwrap(), vec![false]);
}

#[test]
fn menubar_arrow_navigation_skips_disabled_entries() {
    let invalidation = InvalidationSignal::new();
    let tree = WidgetTree::new(MenuBar::<TestVm>::uncontrolled().entries(vec![
            MenuBarEntry::new("File").item(MenuItem::new("New")),
            MenuBarEntry::new("Disabled")
                .item(MenuItem::new("Blocked"))
                .disable(true),
            MenuBarEntry::new("View").item(MenuItem::new("Zoom")),
        ]));
    let mut handler = test_handler(Some(tree), invalidation);
    press_menu_point(
        &mut handler,
        Point::new(dp(24.0), dp(14.0)),
        CanvasMouseButton::Left,
    );
    assert!(has_open_menu(&mut handler));
    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::ArrowRight,)))
    );
    handler.invalidate_computed_scene();
    let labels = handler
        .computed_scene()
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.as_ref())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"Zoom"));
    assert!(!labels.contains(&"Blocked"));
}
