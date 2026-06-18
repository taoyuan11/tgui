use super::*;
use crate::ui::widget::{Drawer, DrawerHost, DrawerMode, DrawerPlacement};

struct RuntimeDrawerVm {
    open: State<bool>,
}

impl ViewModel for RuntimeDrawerVm {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            open: context.state(false),
        }
    }

    fn view(&self) -> Element<Self>
    where
        Self: Sized,
    {
        Stack::new().into()
    }
}

#[test]
fn drawer_backdrop_click_closes_after_signal_opens_from_each_edge() {
    for (placement, click_point) in [
        (DrawerPlacement::Left, Point::new(dp(350.0), dp(150.0))),
        (DrawerPlacement::Right, Point::new(dp(50.0), dp(150.0))),
        (DrawerPlacement::Top, Point::new(dp(200.0), dp(275.0))),
        (DrawerPlacement::Bottom, Point::new(dp(200.0), dp(25.0))),
    ] {
        assert_backdrop_click_closes_after_signal_opens(placement, click_point);
    }
}

fn assert_backdrop_click_closes_after_signal_opens(placement: DrawerPlacement, click_point: Point) {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let open = context.state(false);
    let underlying_clicks = Arc::new(AtomicUsize::new(0));
    let close_calls = Arc::new(AtomicUsize::new(0));

    let under_clicks_ref = Arc::clone(&underlying_clicks);
    let tree = WidgetTree::new(
        Stack::<RuntimeDrawerVm>::new()
            .size(dp(400.0), dp(300.0))
            .child(
                Button::new("under")
                    .size(dp(400.0), dp(300.0))
                    .on_click(Command::new(move |_vm: &mut RuntimeDrawerVm| {
                        under_clicks_ref.fetch_add(1, Ordering::SeqCst);
                    })),
            )
            .child({
                let close_calls_ref = Arc::clone(&close_calls);
                Drawer::new(open.signal())
                    .placement(placement)
                    .on_open_change(ValueCommand::new(
                        move |vm: &mut RuntimeDrawerVm, open: bool| {
                            if !open {
                                close_calls_ref.fetch_add(1, Ordering::SeqCst);
                            }
                            vm.open.set(open);
                        },
                    ))
                    .content(Text::new("drawer"))
            }),
    );
    let vm = RuntimeDrawerVm { open: open.clone() };
    let mut handler = test_handler_with_config(
        vm,
        Some(tree),
        invalidation,
        test_config_with_size(400.0, 300.0),
    );
    let viewport = Rect::new(0.0, 0.0, 400.0, 300.0);

    open.set(true);
    handler.invalidate_computed_scene();
    let computed = handler.computed_scene().clone();
    assert!(
        !computed.overlay_hit_regions.is_empty(),
        "open {placement:?} drawer should create backdrop overlay hits; close handlers: {}",
        computed.overlay_close_handlers.len(),
    );
    let hit_path = WidgetTree::hit_path_from_computed(&computed, click_point);
    assert!(
        matches!(
            hit_path.last(),
            Some(HitInteraction::Widget {
                interactions, ..
            }) if interactions.on_click.is_some()
        ),
        "open {placement:?} drawer backdrop should contribute a clickable hit; hit count: {}, overlay hit count: {}",
        computed.hit_regions.len(),
        computed.overlay_hit_regions.len()
    );

    handler.cursor_position = Some(click_point);
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);

    assert_eq!(close_calls.load(Ordering::SeqCst), 1);
    assert!(
        !open.get(),
        "backdrop click should close the open {placement:?} drawer"
    );
    assert_eq!(
        underlying_clicks.load(Ordering::SeqCst),
        0,
        "{placement:?} drawer backdrop click should not fall through to underlying widgets"
    );
}

#[test]
fn drawer_signal_open_auto_focuses_first_control() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let open = context.state(false);
    let inside: Element<RuntimeDrawerVm> = Button::new("inside").size(dp(80.0), dp(30.0)).into();
    let inside_id = inside.id;
    let tree = WidgetTree::new(
        Drawer::new(open.signal())
            .placement(DrawerPlacement::Left)
            .on_open_change(ValueCommand::new(|_: &mut RuntimeDrawerVm, _: bool| {}))
            .content(inside),
    );
    let vm = RuntimeDrawerVm { open: open.clone() };
    let mut handler = test_handler_with_config(
        vm,
        Some(tree),
        invalidation,
        test_config_with_size(400.0, 300.0),
    );

    let _ = handler.computed_scene();
    assert_eq!(handler.focused_widget_id(), None);

    open.set(true);
    handler.invalidate_computed_scene();
    let _ = handler.computed_scene();

    assert_eq!(handler.focused_widget_id(), Some(inside_id));
}

#[test]
fn drawer_backdrop_click_return_focus_to_declared_target() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let open = context.state(true);
    let trigger: Element<RuntimeDrawerVm> = Button::new("open").size(dp(80.0), dp(30.0)).into();
    let trigger_id = trigger.id;
    let open_for_close = open.clone();
    let tree = WidgetTree::new(
        Stack::<RuntimeDrawerVm>::new()
            .size(dp(400.0), dp(300.0))
            .child(trigger)
            .child(
                Drawer::new(open.signal())
                    .placement(DrawerPlacement::Left)
                    .return_focus_to(trigger_id)
                    .on_open_change(ValueCommand::new(move |vm: &mut RuntimeDrawerVm, value| {
                        open_for_close.set(value);
                        vm.open.set(value);
                    }))
                    .content(Button::new("inside").size(dp(80.0), dp(30.0))),
            ),
    );
    let vm = RuntimeDrawerVm { open: open.clone() };
    let mut handler = test_handler_with_config(
        vm,
        Some(tree),
        invalidation,
        test_config_with_size(400.0, 300.0),
    );
    handler.focused_widget = Some(FocusedWidget {
        widget_id: trigger_id,
        scope_path: Vec::new(),
        on_blur: None,
    });
    let _ = handler.computed_scene();
    assert_ne!(handler.focused_widget_id(), Some(trigger_id));

    handler.cursor_position = Some(Point::new(dp(350.0), dp(150.0)));
    handler.handle_mouse_press(
        Rect::new(0.0, 0.0, 400.0, 300.0),
        Instant::now(),
        CanvasMouseButton::Left,
    );

    assert_eq!(handler.focused_widget_id(), Some(trigger_id));
    assert!(!open.get());
}

#[test]
fn drawer_host_push_escape_closes_via_overlay_sentinel() {
    let context = ViewModelContext::for_benchmarks();
    let invalidation = context.invalidation().clone();
    let open = context.state(true);
    let open_for_close = open.clone();
    let tree = WidgetTree::new(
        DrawerHost::new(
            Text::new("main"),
            Drawer::new(open.signal())
                .mode(DrawerMode::Push)
                .placement(DrawerPlacement::Left)
                .on_open_change(ValueCommand::new(move |vm: &mut RuntimeDrawerVm, value| {
                    open_for_close.set(value);
                    vm.open.set(value);
                }))
                .content(Button::new("inside").size(dp(80.0), dp(30.0))),
        )
        .size(dp(400.0), dp(300.0)),
    );
    let vm = RuntimeDrawerVm { open: open.clone() };
    let mut handler = test_handler_with_config(
        vm,
        Some(tree),
        invalidation,
        test_config_with_size(400.0, 300.0),
    );
    let _ = handler.computed_scene();

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Escape))));
    assert!(!open.get());
}
