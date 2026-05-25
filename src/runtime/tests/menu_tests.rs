use super::*;

use std::sync::Arc;
use std::sync::Mutex;

use crate::ui::widget::{Button, KeyChord, Menu, MenuItem};

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
    use winit_core::event::ElementState;
    use winit_core::event::KeyEvent;

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
    let _ = ElementState::Pressed;
    assert!(handler.handle_keyboard_input(&p_event));
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Enter))));
    assert_eq!(*counter.lock().unwrap(), 1, "type-ahead Paste should fire");
}
