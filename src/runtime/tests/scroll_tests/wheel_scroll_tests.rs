use super::*;
use crate::platform::event::MouseButton;
use crate::ui::widget::{ItemLayout, VirtualList};

fn overlay_select_option_indices<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
) -> Vec<usize> {
    computed
        .overlay_hit_regions
        .iter()
        .filter_map(|hit| match &hit.interaction {
            HitInteraction::SelectOption { option_index, .. } => Some(*option_index),
            _ => None,
        })
        .collect()
}

fn visible_virtual_row_indices<VM>(computed: &crate::ui::widget::ComputedScene<VM>) -> Vec<usize> {
    computed
        .scene
        .texts
        .iter()
        .filter_map(|text| {
            text.content
                .as_ref()
                .strip_prefix("Virtual row ")
                .and_then(|value| value.parse::<usize>().ok())
        })
        .collect()
}

fn virtual_scroll_test_tree() -> (WidgetId, WidgetTree<TestVm>) {
    let rows = (0..1_000).collect::<Vec<_>>();
    let list: Element<TestVm> = VirtualList::new(rows, |_index, item| {
        Stack::new()
            .height(dp(20.0))
            .child(Text::new(format!("Virtual row {item}")))
            .into()
    })
    .item_layout(ItemLayout::Fixed {
        item_extent: dp(20.0),
        spacing: Dp::ZERO,
        overscan: 0,
    })
    .size(dp(180.0), dp(100.0))
    .into();
    let list_id = list.id;
    (list_id, WidgetTree::new(list))
}

fn nested_virtual_scroll_test_tree() -> (WidgetId, WidgetTree<TestVm>) {
    let rows = (0..1_000).collect::<Vec<_>>();
    let list: Element<TestVm> = VirtualList::new(rows, |_index, item| {
        Stack::new()
            .height(dp(20.0))
            .child(Text::new(format!("Virtual row {item}")))
            .into()
    })
    .item_layout(ItemLayout::Fixed {
        item_extent: dp(20.0),
        spacing: Dp::ZERO,
        overscan: 0,
    })
    .size(dp(180.0), dp(100.0))
    .into();
    let list_id = list.id;
    let tree = WidgetTree::new(
        ScrollView::new().size(dp(220.0), dp(160.0)).child(
            Flex::vertical()
                .gap(dp(12.0))
                .child(Stack::new().height(dp(80.0)).child(Text::new("Above")))
                .child(list)
                .child(Stack::new().height(dp(260.0)).child(Text::new("Below"))),
        ),
    );
    (list_id, tree)
}

fn demo_like_virtual_scroll_test_tree() -> (WidgetId, WidgetId, WidgetTree<TestVm>) {
    let rows = (0..1_000).collect::<Vec<_>>();
    let list: Element<TestVm> = VirtualList::new(rows, |_index, item| {
        Stack::new()
            .height(dp(32.0))
            .padding(Insets::symmetric(dp(12.0), dp(6.0)))
            .child(Text::new(format!("Virtual row {item}")))
            .into()
    })
    .item_layout(ItemLayout::Fixed {
        item_extent: dp(32.0),
        spacing: dp(2.0),
        overscan: 1,
    })
    .width(dp(520.0))
    .height(dp(240.0))
    .into();
    let list_id = list.id;
    let outer: Element<TestVm> = ScrollView::new()
        .size(dp(640.0), dp(420.0))
        .child(
            Flex::vertical()
                .width(dp(600.0))
                .gap(dp(18.0))
                .padding(Insets::all(dp(24.0)))
                .child(Stack::new().height(dp(260.0)).child(Text::new("Tabs card")))
                .child(
                    Flex::vertical()
                        .width(dp(560.0))
                        .gap(dp(10.0))
                        .padding(Insets::all(dp(12.0)))
                        .child(Text::new("VirtualList"))
                        .child(
                            Flex::vertical()
                                .width(dp(536.0))
                                .padding(Insets::all(dp(14.0)))
                                .child(list),
                        ),
                )
                .child(
                    Stack::new()
                        .height(dp(320.0))
                        .child(Text::new("DataGrid card")),
                ),
        )
        .into();
    let outer_id = outer.id;
    (outer_id, list_id, WidgetTree::new(outer))
}

fn dynamic_virtual_scroll_test_tree(visible: Signal<bool>) -> (WidgetId, WidgetTree<TestVm>) {
    let rows = (0..1_000).collect::<Vec<_>>();
    let list: Element<TestVm> = VirtualList::new(rows, |_index, item| {
        Stack::new()
            .height(dp(20.0))
            .child(Text::new(format!("Virtual row {item}")))
            .into()
    })
    .item_layout(ItemLayout::Fixed {
        item_extent: dp(20.0),
        spacing: Dp::ZERO,
        overscan: 0,
    })
    .size(dp(180.0), dp(100.0))
    .into();
    let list_id = list.id;
    let tree = WidgetTree::new(Stack::new().size(dp(220.0), dp(140.0)).child(visible.map(
        move |visible| {
            if visible {
                list.clone()
            } else {
                Stack::new()
                    .height(dp(40.0))
                    .child(Text::new("Hidden"))
                    .into()
            }
        },
    )));
    (list_id, tree)
}

#[test]
fn textarea_mouse_wheel_scrolls_vertical_overflow() {
    let invalidation = InvalidationSignal::new();
    let value = "line 0\nline 1\nline 2\nline 3\nline 4\nline 5";
    let tree = WidgetTree::new(Textarea::<TestVm>::new(value).height(dp(52.0)));
    let mut handler = test_handler(Some(tree), invalidation);
    let (text_id, frame) = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    id,
                    multiline: true,
                    ..
                } => Some((*id, region.rect)),
                _ => None,
            })
            .expect("textarea hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: frame.x + dp(8.0),
        y: frame.y + dp(8.0),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));
    assert!(
        handler
            .scroll_states
            .get(&text_id)
            .map(|offset| offset.y)
            .unwrap_or(Dp::ZERO)
            > Dp::ZERO
            || handler.smooth_scroll_states.contains_key(&text_id)
    );
}

#[test]
fn mouse_wheel_updates_virtual_list_window_immediately() {
    let invalidation = InvalidationSignal::new();
    let (list_id, tree) = virtual_scroll_test_tree();
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(180.0, 100.0),
    );

    let (target, baseline_min) = {
        let computed = handler.computed_scene();
        let baseline_min = visible_virtual_row_indices(computed)
            .into_iter()
            .min()
            .expect("initial VirtualList rows should render");
        let region = computed
            .scroll_regions
            .iter()
            .find(|region| region.id == list_id)
            .copied()
            .expect("VirtualList should register a scroll region");
        (
            Point {
                x: region.visible_frame.x + dp(8.0),
                y: region.visible_frame.y + dp(8.0),
            },
            baseline_min,
        )
    };
    assert_eq!(baseline_min, 0);

    handler.cursor_position = Some(target);
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -8.0)));

    let scrolled_min = visible_virtual_row_indices(handler.computed_scene())
        .into_iter()
        .min()
        .expect("scrolled VirtualList rows should render");
    assert!(
        scrolled_min > baseline_min,
        "VirtualList should rebuild its visible window as soon as wheel scrolling updates offset"
    );
}

#[test]
fn nested_mouse_wheel_scrolls_inner_virtual_list_before_parent_scroll_view() {
    let invalidation = InvalidationSignal::new();
    let (list_id, tree) = nested_virtual_scroll_test_tree();
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(220.0, 160.0),
    );

    let (target, baseline_min) = {
        let computed = handler.computed_scene();
        let baseline_min = visible_virtual_row_indices(computed)
            .into_iter()
            .min()
            .expect("initial nested VirtualList rows should render");
        let region = computed
            .scroll_regions
            .iter()
            .find(|region| region.id == list_id)
            .copied()
            .expect("nested VirtualList should register a scroll region");
        (
            Point {
                x: region.visible_frame.x + dp(8.0),
                y: region.visible_frame.y + dp(8.0),
            },
            baseline_min,
        )
    };
    assert_eq!(baseline_min, 0);

    handler.cursor_position = Some(target);
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -8.0)));

    let scrolled_min = visible_virtual_row_indices(handler.computed_scene())
        .into_iter()
        .min()
        .expect("nested scrolled VirtualList rows should render");
    assert!(
        scrolled_min > baseline_min,
        "nested VirtualList should consume wheel input before the parent ScrollView"
    );
}

#[test]
fn demo_like_nested_virtual_list_scrolls_after_parent_scroll() {
    let invalidation = InvalidationSignal::new();
    let (outer_id, list_id, tree) = demo_like_virtual_scroll_test_tree();
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(640.0, 420.0),
    );

    handler.set_scroll_offset(outer_id, Point::new(Dp::ZERO, dp(220.0)));

    let (target, baseline_min) = {
        let computed = handler.computed_scene();
        let region = computed
            .scroll_regions
            .iter()
            .find(|region| region.id == list_id)
            .copied()
            .expect("demo-like VirtualList should register a scroll region");
        assert!(
            region.visible_frame.height > dp(120.0),
            "demo-like VirtualList should be visible after parent scroll, got {:?}",
            region.visible_frame
        );
        assert!(
            region.max_offset().y > dp(1000.0),
            "demo-like VirtualList should have vertical overflow, got {:?}",
            region
        );
        let baseline_min = visible_virtual_row_indices(computed)
            .into_iter()
            .min()
            .expect("initial demo-like VirtualList rows should render");
        (
            Point {
                x: region.visible_frame.x + dp(24.0),
                y: region.visible_frame.y + dp(24.0),
            },
            baseline_min,
        )
    };

    handler.cursor_position = Some(target);
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -8.0)));

    let scrolled_min = visible_virtual_row_indices(handler.computed_scene())
        .into_iter()
        .min()
        .expect("demo-like scrolled VirtualList rows should render");
    assert!(
        scrolled_min > baseline_min,
        "demo-like VirtualList should consume wheel input even inside a scrolled page"
    );
}

#[test]
fn mouse_wheel_scrolls_virtual_list_inside_dynamic_child() {
    let invalidation = InvalidationSignal::new();
    let visible_state = State::new(true, invalidation.clone());
    let (_source_list_id, tree) = dynamic_virtual_scroll_test_tree(visible_state.signal());
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(220.0, 140.0),
    );

    let (list_id, target, baseline_min) = {
        let computed = handler.computed_scene();
        let baseline_min = visible_virtual_row_indices(computed)
            .into_iter()
            .min()
            .expect("initial dynamic VirtualList rows should render");
        let region = computed
            .scroll_regions
            .iter()
            .find(|region| region.max_offset().y > Dp::ZERO)
            .copied()
            .expect("dynamic VirtualList should register a scroll region");
        (
            region.id,
            Point {
                x: region.visible_frame.x + dp(8.0),
                y: region.visible_frame.y + dp(8.0),
            },
            baseline_min,
        )
    };
    assert_eq!(baseline_min, 0);

    handler.cursor_position = Some(target);
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -8.0)));

    let scrolled_min = visible_virtual_row_indices(handler.computed_scene())
        .into_iter()
        .min()
        .expect("dynamic scrolled VirtualList rows should render");
    assert!(
        scrolled_min > baseline_min,
        "dynamic VirtualList should rebuild its visible window from the stored scroll offset"
    );
    assert!(
        handler
            .scroll_states
            .get(&list_id)
            .map(|offset| offset.y > Dp::ZERO)
            .unwrap_or(false),
        "scroll state should stay attached to the resolved dynamic VirtualList id"
    );
}

#[test]
fn scrollbar_drag_updates_virtual_list_window_while_dragging() {
    let invalidation = InvalidationSignal::new();
    let (list_id, tree) = virtual_scroll_test_tree();
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(180.0, 100.0),
    );
    let event_loop = TestEventLoop;

    let (start, target, baseline_min) = {
        let computed = handler.computed_scene();
        let baseline_min = visible_virtual_row_indices(computed)
            .into_iter()
            .min()
            .expect("initial VirtualList rows should render");
        let region = computed
            .scroll_regions
            .iter()
            .find(|region| region.id == list_id)
            .copied()
            .expect("VirtualList should register a scroll region");
        let thumb = region
            .vertical_thumb
            .expect("VirtualList should render a vertical scrollbar thumb");
        let track = region
            .vertical_track
            .expect("VirtualList should render a vertical scrollbar track");
        let start = Point {
            x: thumb.x + dp(thumb.width.get() * 0.5),
            y: thumb.y + dp(thumb.height.get() * 0.5),
        };
        let travel = (track.height - thumb.height).max(0.0);
        let target = Point {
            x: start.x,
            y: start.y + travel * 0.45,
        };
        (start, target, baseline_min)
    };
    assert_eq!(baseline_min, 0);

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(f64::from(start.x.get()), f64::from(start.y.get())),
            primary: true,
            source: PointerSource::Mouse,
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(f64::from(start.x.get()), f64::from(start.y.get())),
            state: ElementState::Pressed,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );
    assert!(
        handler.active_scrollbar_drag.is_some(),
        "pressing the VirtualList thumb should start scrollbar drag"
    );

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(f64::from(target.x.get()), f64::from(target.y.get())),
            primary: true,
            source: PointerSource::Mouse,
        },
    );

    let dragged_min = visible_virtual_row_indices(handler.computed_scene())
        .into_iter()
        .min()
        .expect("dragged VirtualList rows should render");
    assert!(
        dragged_min > baseline_min,
        "VirtualList should update visible rows while the scrollbar thumb is being dragged"
    );
}

#[test]
fn mouse_wheel_scrolls_virtual_select_overlay_region() {
    let invalidation = InvalidationSignal::new();
    let options = (0..1_000)
        .map(|index| SelectOption::new(index, format!("Option {index}")))
        .collect::<Vec<_>>();
    let select: Element<TestVm> = Select::new(options, None::<usize>)
        .open(true)
        .size(dp(180.0), dp(32.0))
        .into();
    let mut handler = test_handler_with_config(
        TestVm,
        Some(WidgetTree::new(Stack::new().child(select))),
        invalidation,
        test_config_with_size(220.0, 180.0),
    );

    let (region_id, target, baseline_min) = {
        let computed = handler.computed_scene();
        let baseline_min = overlay_select_option_indices(computed)
            .into_iter()
            .min()
            .unwrap_or_else(|| {
                panic!(
                    "open Select overlay should render visible options: overlay_hits={} hits={} scroll_regions={} overlay_shapes={} portal_entries={}",
                    computed.overlay_hit_regions.len(),
                    computed.hit_regions.len(),
                    computed.scroll_regions.len(),
                    computed.scene.overlay_shapes.len(),
                    computed.portal_entries.len()
                )
            });
        let region = computed
            .scroll_regions
            .iter()
            .find(|region| region.content_bounds.height > region.content_viewport.height)
            .copied()
            .expect("virtual Select overlay should register a scroll region");
        (
            region.id,
            Point {
                x: region.visible_frame.x + dp(8.0),
                y: region.visible_frame.y + dp(8.0),
            },
            baseline_min,
        )
    };
    assert_eq!(baseline_min, 0);
    assert!(handler.virtual_states.contains_key(&region_id));

    handler.cursor_position = Some(target);
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -4.0)));
    assert!(
        handler
            .scroll_states
            .get(&region_id)
            .map(|offset| offset.y > Dp::ZERO)
            .unwrap_or(false)
            || handler.smooth_scroll_states.contains_key(&region_id),
        "overlay VirtualList scroll region should consume wheel input"
    );

    while handler.advance_smooth_scroll() {}
    let scrolled_min = overlay_select_option_indices(handler.computed_scene())
        .into_iter()
        .min()
        .expect("scrolled Select overlay should still render visible options");
    assert!(
        scrolled_min > baseline_min,
        "visible Select options should advance after wheel scrolling"
    );
}

#[test]
fn mouse_wheel_starts_immediately_and_keeps_smooth_target() {
    let invalidation = InvalidationSignal::new();
    let scroller: Element<TestVm> = Stack::new()
        .size(dp(100.0), dp(100.0))
        .overflow_y(Overflow::Scroll)
        .child(Stack::new().size(dp(100.0), dp(320.0)))
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);
    let mut handler = test_handler(Some(tree), invalidation);
    let region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == scroller_id)
        .copied()
        .expect("scroll region should exist");

    handler.cursor_position = Some(Point {
        x: region.visible_frame.x + dp(8.0),
        y: region.visible_frame.y + dp(8.0),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));

    let offset = handler
        .scroll_states
        .get(&scroller_id)
        .map(|state| state.y)
        .expect("scroll offset should exist");
    assert!(offset > Dp::ZERO);
    assert!(offset < dp(160.0));

    let target = handler
        .smooth_scroll_states
        .get(&scroller_id)
        .map(|state| state.target.y)
        .expect("smooth scroll target should exist");
    assert_eq!(target, dp(160.0));
}

#[test]
fn mouse_wheel_scrolls_stack_wrapped_grid_of_canvas_cards() {
    let invalidation = InvalidationSignal::new();
    let card = || {
        Stack::new().height(dp(180.0)).child(
            Canvas::new(CanvasRecorder::build(|canvas| {
                canvas
                    .next_item_id(1_u64)
                    .set_fill(Color::hexa(0x1D4ED8FF))
                    .fill_rect(0.0, 0.0, 80.0, 80.0);
            }))
            .size(dp(120.0), dp(120.0)),
        )
    };
    let scroller: Element<TestVm> = Stack::new()
        .size(dp(320.0), dp(240.0))
        .overflow_y(Overflow::Scroll)
        .child(
            crate::ui::widget::Grid::columns([
                crate::ui::layout::fr(1.0),
                crate::ui::layout::fr(1.0),
            ])
            .height(dp(780.0))
            .gap(dp(12.0))
            .child(card())
            .child(card())
            .child(card())
            .child(card()),
        )
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);
    let mut handler = test_handler(Some(tree), invalidation);
    let region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == scroller_id)
        .copied()
        .expect("scroll region should exist");

    handler.cursor_position = Some(Point {
        x: region.visible_frame.x + dp(24.0),
        y: region.visible_frame.y + dp(24.0),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));
    assert!(
        handler
            .scroll_states
            .get(&scroller_id)
            .map(|offset| offset.y)
            .unwrap_or(Dp::ZERO)
            > Dp::ZERO
            || handler.smooth_scroll_states.contains_key(&scroller_id)
    );
}

#[test]
fn pointer_entered_restores_mouse_wheel_scrolling_after_pointer_left() {
    let invalidation = InvalidationSignal::new();
    let scroller: Element<TestVm> = Stack::new()
        .size(dp(320.0), dp(240.0))
        .overflow_y(Overflow::Scroll)
        .child(
            Flex::vertical().height(dp(860.0)).gap(dp(12.0)).child([
                Element::<TestVm>::from(Input::new("hello world").height(dp(40.0))),
                Element::<TestVm>::from(
                    Textarea::new(
                        (0..10)
                            .map(|index| format!("line {index}"))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                    .height(dp(72.0)),
                ),
                Element::<TestVm>::from(Stack::new().height(dp(640.0))),
            ]),
        )
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(320.0, 240.0),
    );
    let viewport = handler.viewport_rect();
    let event_loop = TestEventLoop;

    let target = {
        let computed = handler.computed_scene();
        let scroll_region = computed
            .scroll_regions
            .iter()
            .find(|region| region.id == scroller_id)
            .copied()
            .expect("parent scroll region should exist");
        assert!(scroll_region.max_offset().y > Dp::ZERO);
        PhysicalPosition::new(
            f64::from((scroll_region.visible_frame.x + dp(24.0)).get()),
            f64::from((scroll_region.visible_frame.bottom() - dp(24.0)).get()),
        )
    };

    let input_frame = {
        let computed = handler.computed_scene();
        computed
            .hit_regions
            .iter()
            .find_map(|region| match &region.interaction {
                HitInteraction::TextInput {
                    multiline: false, ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("input hit region should exist")
    };

    handler.cursor_position = Some(Point {
        x: input_frame.x + dp(8.0),
        y: input_frame.y + dp(8.0),
    });
    handler.handle_mouse_press(viewport, Instant::now(), CanvasMouseButton::Left);
    assert!(handler.focused_widget_id().is_some());

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerLeft {
            device_id: None,
            position: None,
            primary: true,
            kind: PointerKind::Mouse,
        },
    );
    assert!(handler.cursor_position.is_none());

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerEntered {
            device_id: None,
            position: target,
            primary: true,
            kind: PointerKind::Mouse,
        },
    );
    assert!(handler.cursor_position.is_some());

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::MouseWheel {
            device_id: None,
            delta: MouseScrollDelta::LineDelta(0.0, -2.0),
            phase: TouchPhase::Moved,
        },
    );
    assert!(
        handler
            .scroll_states
            .get(&scroller_id)
            .map(|offset| offset.y > Dp::ZERO)
            .unwrap_or(false)
            || handler.smooth_scroll_states.contains_key(&scroller_id),
        "parent scroller should respond after pointer re-enters the window"
    );
}

#[derive(Default)]
struct TouchScrollVm {
    clicks: usize,
}

impl ViewModel for TouchScrollVm {
    fn new(_: &ViewModelContext) -> Self {
        Self::default()
    }

    fn view(&self) -> Element<Self> {
        Stack::new().into()
    }
}

impl TouchScrollVm {
    fn click(&mut self) {
        self.clicks += 1;
    }
}

#[test]
fn touch_drag_scrolls_clickable_content_without_firing_tap() {
    let invalidation = InvalidationSignal::new();
    let button: Element<TouchScrollVm> = Button::new("drag me")
        .height(dp(80.0))
        .on_click(Command::new(TouchScrollVm::click))
        .into();
    let scroller: Element<TouchScrollVm> = Stack::new()
        .size(dp(320.0), dp(240.0))
        .overflow_y(Overflow::Scroll)
        .child(
            Flex::vertical()
                .height(dp(860.0))
                .gap(dp(12.0))
                .child([button, Stack::new().height(dp(760.0)).into()]),
        )
        .into();
    let scroller_id = scroller.id;
    let mut handler = test_handler_with_vm(
        TouchScrollVm::default(),
        Some(WidgetTree::new(scroller)),
        invalidation,
    );
    let event_loop = TestEventLoop;

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Pressed,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(24.0, 16.0),
            primary: true,
            source: PointerSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 16.0),
            state: ElementState::Released,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );

    assert!(
        handler
            .scroll_states
            .get(&scroller_id)
            .map(|offset| offset.y > Dp::ZERO)
            .unwrap_or(false),
        "touch drag should scroll the parent scroller"
    );
    assert!(
        handler
            .touch_scroll_inertia_states
            .contains_key(&scroller_id),
        "released touch drag should continue with inertia"
    );
    assert_eq!(handler.view_model.lock().unwrap().clicks, 0);
}

#[test]
fn touch_tap_on_clickable_content_still_fires_click() {
    let invalidation = InvalidationSignal::new();
    let button: Element<TouchScrollVm> = Button::new("tap me")
        .height(dp(80.0))
        .on_click(Command::new(TouchScrollVm::click))
        .into();
    let scroller: Element<TouchScrollVm> = Stack::new()
        .size(dp(320.0), dp(240.0))
        .overflow_y(Overflow::Scroll)
        .child(
            Flex::vertical()
                .height(dp(860.0))
                .gap(dp(12.0))
                .child([button, Stack::new().height(dp(760.0)).into()]),
        )
        .into();
    let mut handler = test_handler_with_vm(
        TouchScrollVm::default(),
        Some(WidgetTree::new(scroller)),
        invalidation,
    );
    let event_loop = TestEventLoop;

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Pressed,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Released,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );

    assert_eq!(handler.view_model.lock().unwrap().clicks, 1);
    assert!(handler.touch_scroll_inertia_states.is_empty());
}

#[test]
fn small_touch_move_still_taps_without_starting_inertia() {
    let invalidation = InvalidationSignal::new();
    let button: Element<TouchScrollVm> = Button::new("tap me")
        .height(dp(80.0))
        .on_click(Command::new(TouchScrollVm::click))
        .into();
    let scroller: Element<TouchScrollVm> = ScrollView::new()
        .size(dp(320.0), dp(240.0))
        .child(
            Flex::vertical()
                .height(dp(860.0))
                .gap(dp(12.0))
                .child([button, Stack::new().height(dp(760.0)).into()]),
        )
        .into();
    let mut handler = test_handler_with_vm(
        TouchScrollVm::default(),
        Some(WidgetTree::new(scroller)),
        invalidation,
    );
    let event_loop = TestEventLoop;

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 40.0),
            state: ElementState::Pressed,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerMoved {
            device_id: None,
            position: PhysicalPosition::new(24.0, 36.0),
            primary: true,
            source: PointerSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
        },
    );
    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(24.0, 36.0),
            state: ElementState::Released,
            button: ButtonSource::Touch {
                finger_id: FingerId::from_raw(1),
                force: None,
            },
            primary: true,
        },
    );

    assert_eq!(handler.view_model.lock().unwrap().clicks, 1);
    assert!(handler.touch_scroll_inertia_states.is_empty());
}

#[test]
fn nested_scroll_wheel_bubbles_to_parent_at_child_boundary() {
    let invalidation = InvalidationSignal::new();
    let inner: Element<TestVm> = ScrollView::new()
        .size(dp(160.0), dp(80.0))
        .child(Stack::new().size(dp(160.0), dp(80.0)))
        .into();
    let outer: Element<TestVm> = ScrollView::new()
        .size(dp(220.0), dp(140.0))
        .child(
            Flex::vertical()
                .height(dp(360.0))
                .child([inner, Stack::new().height(dp(240.0)).into()]),
        )
        .into();
    let outer_id = outer.id;
    let tree = WidgetTree::new(outer);
    let mut handler = test_handler(Some(tree), invalidation);
    let target = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id != outer_id)
        .copied()
        .expect("inner scroll region should exist");
    handler.cursor_position = Some(Point {
        x: target.visible_frame.x + dp(8.0),
        y: target.visible_frame.y + dp(8.0),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -3.0)));
    assert!(
        handler
            .scroll_states
            .get(&outer_id)
            .map(|offset| offset.y > Dp::ZERO)
            .unwrap_or(false)
            || handler.smooth_scroll_states.contains_key(&outer_id),
        "parent ScrollView should consume wheel input when inner region cannot scroll"
    );
}

fn focused_scroll_view_handler(
    scroller: Element<TestVm>,
) -> (BoundRuntimeHandler<TestVm>, WidgetId) {
    let invalidation = InvalidationSignal::new();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);
    let mut handler = test_handler(Some(tree), invalidation);
    let region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == scroller_id)
        .copied()
        .expect("scroll region should exist");
    handler.cursor_position = Some(Point {
        x: region.visible_frame.x + dp(8.0),
        y: region.visible_frame.y + dp(8.0),
    });
    handler.handle_mouse_press(
        handler.viewport_rect(),
        Instant::now(),
        CanvasMouseButton::Left,
    );
    assert_eq!(handler.focused_widget_id(), Some(scroller_id));
    (handler, scroller_id)
}

#[test]
fn scroll_view_page_keys_scroll_focused_region() {
    let scroller: Element<TestVm> = ScrollView::new()
        .size(dp(160.0), dp(100.0))
        .child(Stack::new().size(dp(160.0), dp(360.0)))
        .into();
    let (mut handler, scroller_id) = focused_scroll_view_handler(scroller);

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageDown,)))
    );
    while handler.advance_smooth_scroll() {}
    let page_down_y = handler
        .scroll_states
        .get(&scroller_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);
    assert!(page_down_y > Dp::ZERO);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageUp,))));
    while handler.advance_smooth_scroll() {}
    let page_up_y = handler
        .scroll_states
        .get(&scroller_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);
    assert!(page_up_y < page_down_y);
}

#[test]
fn scroll_view_home_end_scroll_focused_region_to_edges() {
    let scroller: Element<TestVm> = ScrollView::new()
        .size(dp(160.0), dp(100.0))
        .child(Stack::new().size(dp(160.0), dp(360.0)))
        .into();
    let (mut handler, scroller_id) = focused_scroll_view_handler(scroller);
    let max_y = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == scroller_id)
        .expect("scroll region should exist")
        .max_offset()
        .y;

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End))));
    while handler.advance_smooth_scroll() {}
    assert_eq!(
        handler
            .scroll_states
            .get(&scroller_id)
            .map(|offset| offset.y)
            .unwrap_or(Dp::ZERO),
        max_y
    );

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Home))));
    while handler.advance_smooth_scroll() {}
    assert_eq!(
        handler
            .scroll_states
            .get(&scroller_id)
            .map(|offset| offset.y)
            .unwrap_or(Dp::ZERO),
        Dp::ZERO
    );
}

#[test]
fn horizontal_scroll_view_keyboard_scrolls_x_axis() {
    let scroller: Element<TestVm> = ScrollView::new()
        .size(dp(100.0), dp(80.0))
        .overflow_x(Overflow::Scroll)
        .overflow_y(Overflow::Hidden)
        .child(Stack::new().size(dp(620.0), dp(80.0)))
        .into();
    let (mut handler, scroller_id) = focused_scroll_view_handler(scroller);
    let max_x = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == scroller_id)
        .expect("scroll region should exist")
        .max_offset()
        .x;

    assert!(
        handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::PageDown,)))
    );
    while handler.advance_smooth_scroll() {}
    let page_x = handler
        .scroll_states
        .get(&scroller_id)
        .map(|offset| offset.x)
        .unwrap_or(Dp::ZERO);
    assert!(page_x > Dp::ZERO);

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::End))));
    while handler.advance_smooth_scroll() {}
    assert_eq!(
        handler
            .scroll_states
            .get(&scroller_id)
            .map(|offset| offset.x)
            .unwrap_or(Dp::ZERO),
        max_x
    );

    assert!(handler.handle_keyboard_input(&pressed_key_event(PhysicalKey::Code(KeyCode::Home))));
    while handler.advance_smooth_scroll() {}
    assert_eq!(
        handler
            .scroll_states
            .get(&scroller_id)
            .map(|offset| offset.x)
            .unwrap_or(Dp::ZERO),
        Dp::ZERO
    );
}

#[test]
fn scroll_view_controller_scroll_to_updates_runtime_offset() {
    let invalidation = InvalidationSignal::new();
    let ctx = ViewModelContext::for_benchmarks();
    let controller = ScrollViewController::new(&ctx);
    let scroller: Element<TestVm> = ScrollView::new()
        .size(dp(160.0), dp(100.0))
        .controller(controller.clone())
        .child(Stack::new().size(dp(160.0), dp(360.0)))
        .into();
    let scroller_id = scroller.id;
    let tree = WidgetTree::new(scroller);
    let mut handler = test_handler(Some(tree), invalidation);

    let _ = handler.computed_scene();
    assert_eq!(controller.widget_id(), Some(scroller_id));
    controller.scroll_to(Point::new(Dp::ZERO, dp(120.0)));
    let _ = handler.computed_scene();

    assert!(
        handler
            .scroll_states
            .get(&scroller_id)
            .map(|offset| offset.y > Dp::ZERO)
            .unwrap_or(false)
            || handler.smooth_scroll_states.contains_key(&scroller_id)
    );
    assert!(controller.scroll_offset().y > Dp::ZERO);
}

#[test]
fn touch_scroll_inertia_advances_and_stops_within_bounds() {
    let invalidation = InvalidationSignal::new();
    let scroller: Element<TestVm> = ScrollView::new()
        .size(dp(160.0), dp(100.0))
        .child(Stack::new().size(dp(160.0), dp(500.0)))
        .into();
    let scroller_id = scroller.id;
    let mut handler = test_handler(Some(WidgetTree::new(scroller)), invalidation);
    let max = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == scroller_id)
        .expect("scroll region should exist")
        .max_offset();
    let start = Instant::now();
    handler.touch_scroll_inertia_states.insert(
        scroller_id,
        TouchScrollInertiaState {
            velocity: Point::new(Dp::ZERO, dp(900.0)),
            max_offset: max,
            can_scroll_x: false,
            can_scroll_y: true,
            last_advanced_at: start,
        },
    );

    assert!(handler.advance_touch_scroll_inertia(start + Duration::from_millis(16)));
    let advanced = handler
        .scroll_states
        .get(&scroller_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO);
    assert!(advanced > Dp::ZERO);

    for frame in 2..240 {
        let _ = handler.advance_touch_scroll_inertia(start + Duration::from_millis(frame * 16));
        let offset = handler
            .scroll_states
            .get(&scroller_id)
            .copied()
            .unwrap_or(Point::ZERO);
        assert!(offset.y >= Dp::ZERO && offset.y <= max.y);
        if handler.touch_scroll_inertia_states.is_empty() {
            break;
        }
    }
    assert!(
        handler.touch_scroll_inertia_states.is_empty(),
        "inertia should eventually decay below threshold or stop at the edge"
    );
}

#[test]
fn touch_scroll_inertia_is_cancelled_by_wheel_and_controller_requests() {
    let invalidation = InvalidationSignal::new();
    let ctx = ViewModelContext::for_benchmarks();
    let controller = ScrollViewController::new(&ctx);
    let scroller: Element<TestVm> = ScrollView::new()
        .size(dp(160.0), dp(100.0))
        .controller(controller.clone())
        .child(Stack::new().size(dp(160.0), dp(500.0)))
        .into();
    let scroller_id = scroller.id;
    let mut handler = test_handler(Some(WidgetTree::new(scroller)), invalidation);
    let region = handler
        .computed_scene()
        .scroll_regions
        .iter()
        .find(|region| region.id == scroller_id)
        .copied()
        .expect("scroll region should exist");
    let max = region.max_offset();
    handler.touch_scroll_inertia_states.insert(
        scroller_id,
        TouchScrollInertiaState {
            velocity: Point::new(Dp::ZERO, dp(800.0)),
            max_offset: max,
            can_scroll_x: false,
            can_scroll_y: true,
            last_advanced_at: Instant::now(),
        },
    );
    handler.cursor_position = Some(Point {
        x: region.visible_frame.x + dp(8.0),
        y: region.visible_frame.y + dp(8.0),
    });

    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -1.0)));
    assert!(!handler
        .touch_scroll_inertia_states
        .contains_key(&scroller_id));

    handler.touch_scroll_inertia_states.insert(
        scroller_id,
        TouchScrollInertiaState {
            velocity: Point::new(Dp::ZERO, dp(800.0)),
            max_offset: max,
            can_scroll_x: false,
            can_scroll_y: true,
            last_advanced_at: Instant::now(),
        },
    );
    controller.jump_to(Point::new(Dp::ZERO, dp(120.0)));
    let _ = handler.computed_scene();
    assert!(!handler
        .touch_scroll_inertia_states
        .contains_key(&scroller_id));
}
