use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use crate::ui::widget::{Modal, ModalAction};

#[derive(Default)]
struct ModalVm;

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
fn closed_signal_modal_does_not_intercept_underlying_button_click() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let open = context.state(false);
    let button_clicks = Arc::new(AtomicUsize::new(0));
    let close_calls = Arc::new(AtomicUsize::new(0));

    let button_clicks_ref = Arc::clone(&button_clicks);
    let close_calls_ref = Arc::clone(&close_calls);
    let tree = WidgetTree::new(
        Stack::<ModalVm>::new()
            .size(dp(400.0), dp(300.0))
            .child(
                Button::new("Open")
                    .size(dp(120.0), dp(40.0))
                    .on_click(Command::new(move |_vm: &mut ModalVm| {
                        button_clicks_ref.fetch_add(1, Ordering::SeqCst);
                    })),
            )
            .child(
                Modal::new(open.signal())
                    .on_open_change(ValueCommand::new(move |_: &mut ModalVm, _open| {
                        close_calls_ref.fetch_add(1, Ordering::SeqCst);
                    }))
                    .title("Hidden")
                    .content(Text::new("This should not catch clicks"))
                    .action(ModalAction::primary("OK")),
            ),
    );
    let mut handler = test_handler_with_config(
        ModalVm::default(),
        Some(tree),
        invalidation,
        test_config_with_size(400.0, 300.0),
    );
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let click_point = Point::new(dp(60.0), dp(20.0));

    let computed = handler.computed_scene().clone();
    let hit_path = WidgetTree::hit_path_from_computed(&computed, click_point);
    assert!(
        matches!(
            hit_path.last(),
            Some(HitInteraction::Widget { interactions, .. }) if interactions.on_click.is_some()
        ),
        "closed modal should leave the underlying button as the top hit, got {} hits",
        hit_path.len()
    );

    handler.cursor_position = Some(click_point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert_eq!(button_clicks.load(Ordering::SeqCst), 1);
    assert_eq!(
        close_calls.load(Ordering::SeqCst),
        0,
        "closed modal backdrop must not fire on_open_change"
    );
}

#[test]
fn open_modal_backdrop_click_still_closes() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let open = context.state(true);
    let close_calls = Arc::new(AtomicUsize::new(0));

    let open_for_close = open.clone();
    let close_calls_ref = Arc::clone(&close_calls);
    let tree = WidgetTree::new(
        Stack::<ModalVm>::new()
            .size(dp(400.0), dp(300.0))
            .child(Text::new("under"))
            .child(
                Modal::new(open.signal())
                    .on_open_change(ValueCommand::new(move |_: &mut ModalVm, value| {
                        open_for_close.set(value);
                        if !value {
                            close_calls_ref.fetch_add(1, Ordering::SeqCst);
                        }
                    }))
                    .title("Open")
                    .content(Text::new("Body"))
                    .action(ModalAction::primary("OK")),
            ),
    );
    let mut handler = test_handler_with_config(
        ModalVm::default(),
        Some(tree),
        invalidation,
        test_config_with_size(400.0, 300.0),
    );
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);
    let click_point = Point::new(dp(10.0), dp(10.0));

    let _ = handler.computed_scene();
    handler.cursor_position = Some(click_point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    assert!(!open.get());
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
fn dynamic_modal_auto_focus_survives_full_scene_invalidation() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let page = context.state(());
    let open = context.state(false);
    let open_for_dynamic_page = open.clone();
    let dynamic_page = page.signal().map(move |_| {
        Element::from(
            Stack::<ModalVm>::new()
                .size(dp(400.0), dp(300.0))
                .child(Button::new("Open").size(dp(120.0), dp(40.0)))
                .child(
                    Modal::new(open_for_dynamic_page.signal())
                        .title("Dynamic")
                        .action(ModalAction::primary("OK")),
                ),
        )
    });
    let tree = WidgetTree::new(
        Stack::<ModalVm>::new()
            .size(dp(400.0), dp(300.0))
            .child(dynamic_page),
    );
    let mut handler = test_handler_with_config(
        ModalVm::default(),
        Some(tree),
        invalidation,
        test_config_with_size(400.0, 300.0),
    );

    let _ = handler.computed_scene();
    assert_eq!(handler.focused_widget_id(), None);

    open.set(true);
    handler.invalidate_scene_with_reason("test_open_dynamic_modal");
    let _ = handler.computed_scene();

    let focused = handler
        .focused_widget_id()
        .expect("open dynamic modal should auto-focus its primary action");
    let active_scope = handler
        .active_auto_focus_scope
        .clone()
        .expect("open dynamic modal should record its active auto-focus scope");

    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    assert_eq!(handler.focused_widget_id(), Some(focused));
    assert_eq!(handler.active_auto_focus_scope, Some(active_scope));
}

#[test]
fn dynamic_modal_open_button_click_does_not_recurse() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let page = context.state(());
    let open = context.state(false);
    let open_for_button = open.clone();
    let open_for_dynamic_page = open.clone();
    let dynamic_page = page.signal().map(move |_| {
        Element::from(
            Stack::<ModalVm>::new()
                .size(dp(400.0), dp(300.0))
                .child(
                    Button::new("Open")
                        .size(dp(120.0), dp(40.0))
                        .on_click(Command::new({
                            let open_for_button = open_for_button.clone();
                            move |_vm: &mut ModalVm| {
                                open_for_button.set(true);
                            }
                        })),
                )
                .child(
                    Modal::new(open_for_dynamic_page.signal())
                        .title("Dynamic")
                        .action(ModalAction::primary("OK")),
                ),
        )
    });
    let tree = WidgetTree::new(
        Stack::<ModalVm>::new()
            .size(dp(400.0), dp(300.0))
            .child(dynamic_page),
    );
    let mut handler = test_handler_with_config(
        ModalVm::default(),
        Some(tree),
        invalidation,
        test_config_with_size(400.0, 300.0),
    );
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);

    let _ = handler.computed_scene();
    handler.cursor_position = Some(Point::new(dp(60.0), dp(20.0)));
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(open.get());

    handler.request_redraw_if_dirty(Instant::now());
    let _ = handler.computed_scene();

    assert!(
        handler.focused_widget_id().is_some(),
        "button-opened dynamic modal should auto-focus without recursive overflow"
    );
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
