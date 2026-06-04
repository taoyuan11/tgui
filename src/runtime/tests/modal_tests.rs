use super::*;

use std::sync::Arc;
use std::sync::Mutex;

use crate::ui::widget::{Modal, ModalAction};

/// 简单的 modal 测试 VM：记录 on_open_change 被调用的次数与最近一次值。
#[derive(Default)]
struct ModalVm {
    close_calls: Vec<bool>,
}

impl crate::foundation::view_model::ViewModel for ModalVm {
    fn new(_context: &ViewModelContext) -> Self {
        Self::default()
    }
    fn view(&self) -> Element<Self>
    where
        Self: Sized,
    {
        Stack::new().into()
    }
}

#[test]
fn escape_press_closes_open_modal_via_overlay_sentinel() {
    let invalidation = InvalidationSignal::new();
    let close_calls = Arc::new(Mutex::new(0_u32));
    let close_calls_cmd = close_calls.clone();
    let tree = WidgetTree::new(
        Modal::<ModalVm>::new(true)
            .on_open_change(ValueCommand::new(move |_vm: &mut ModalVm, open: bool| {
                if !open {
                    *close_calls_cmd.lock().unwrap() += 1;
                }
            }))
            .title("Test")
            .content(Text::new("Body"))
            .action(ModalAction::primary("OK")),
    );
    let mut handler = test_handler_with_vm(ModalVm::default(), Some(tree), invalidation);

    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Escape))),
        "Esc should be consumed by open modal's sentinel overlay close handler"
    );
    let count = *close_calls.lock().unwrap();
    assert_eq!(
        count, 1,
        "on_open_change(false) should fire once when Esc closes modal"
    );
}

#[test]
fn escape_press_disabled_does_not_close_modal() {
    let invalidation = InvalidationSignal::new();
    let close_calls = Arc::new(Mutex::new(0_u32));
    let close_calls_cmd = close_calls.clone();
    let tree = WidgetTree::new(
        Modal::<ModalVm>::new(true)
            .close_on_escape(false)
            .on_open_change(ValueCommand::new(move |_vm: &mut ModalVm, _open: bool| {
                *close_calls_cmd.lock().unwrap() += 1;
            }))
            .title("Test")
            .action(ModalAction::primary("OK")),
    );
    let mut handler = test_handler_with_vm(ModalVm::default(), Some(tree), invalidation);
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    let consumed =
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Escape)));
    let _ = consumed; // 当 close_on_escape=false 时 sentinel overlay 不消费 Esc
    let count = *close_calls.lock().unwrap();
    assert_eq!(
        count, 0,
        "on_open_change must not fire when close_on_escape=false"
    );
}

#[test]
fn closed_modal_does_not_register_close_handler() {
    let invalidation = InvalidationSignal::new();
    let close_calls = Arc::new(Mutex::new(0_u32));
    let close_calls_cmd = close_calls.clone();
    let tree = WidgetTree::new(
        Modal::<ModalVm>::new(false)
            .on_open_change(ValueCommand::new(move |_vm: &mut ModalVm, _open: bool| {
                *close_calls_cmd.lock().unwrap() += 1;
            }))
            .title("Test"),
    );
    let mut handler = test_handler_with_vm(ModalVm::default(), Some(tree), invalidation);
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    // 关闭状态下按 Esc 不应触发 on_open_change（modal 自始未挂 sentinel overlay）
    let _ = handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Escape)));
    let count = *close_calls.lock().unwrap();
    assert_eq!(
        count, 0,
        "closed modal should not register Esc close handler"
    );
}

#[test]
fn modal_signal_open_auto_focuses_primary_action() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let open = context.state(false);
    let tree = WidgetTree::new(
        Modal::<ModalVm>::new(open.signal())
            .on_open_change(ValueCommand::new(|_: &mut ModalVm, _: bool| {}))
            .title("Test")
            .action(ModalAction::new("Cancel"))
            .action(ModalAction::primary("OK")),
    );
    let mut handler = test_handler_with_vm(ModalVm::default(), Some(tree), invalidation);

    let _ = handler.computed_scene();
    assert_eq!(handler.focused_widget_id(), None);

    open.set(true);
    handler.invalidate_computed_scene();
    let computed = handler.computed_scene().clone();
    let focused = handler
        .focused_widget_id()
        .expect("open modal should auto-focus an action");
    let focused_tab_index = computed.hit_regions.iter().find_map(|region| {
        let focus = region.focus.as_ref()?;
        (focus.widget_id == focused).then_some(focus.tab_index)
    });

    assert_eq!(focused_tab_index, Some(Some(1)));
}

#[test]
fn modal_escape_return_focus_to_declared_target() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let open = context.state(true);
    let trigger: Element<ModalVm> = Button::new("Open").size(dp(80.0), dp(30.0)).into();
    let trigger_id = trigger.id;
    let open_for_close = open.clone();
    let tree = WidgetTree::new(
        Stack::<ModalVm>::new().child([
            trigger,
            Modal::new(open.signal())
                .return_focus_to(trigger_id)
                .on_open_change(ValueCommand::new(move |_: &mut ModalVm, value| {
                    open_for_close.set(value);
                }))
                .title("Test")
                .action(ModalAction::primary("OK"))
                .into(),
        ]),
    );
    let mut handler = test_handler_with_vm(ModalVm::default(), Some(tree), invalidation);
    handler.focused_widget = Some(FocusedWidget {
        widget_id: trigger_id,
        scope_path: Vec::new(),
        on_blur: None,
    });
    let _ = handler.computed_scene();

    assert_ne!(handler.focused_widget_id(), Some(trigger_id));
    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Escape,))));

    assert_eq!(handler.focused_widget_id(), Some(trigger_id));
    assert!(!open.get());
}
