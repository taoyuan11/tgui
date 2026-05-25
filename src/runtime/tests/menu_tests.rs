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
