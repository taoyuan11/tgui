use super::*;
use crate::foundation::binding::{ToastPlacement, ToastQueue};
use crate::platform::event::MouseButton;
use crate::ui::layout::{pct, Align};
use crate::ui::widget::{ItemLayout, ScrollRegion, ToastHost, VirtualList};

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

fn largest_inner_scroll_region<VM>(
    computed: &crate::ui::widget::ComputedScene<VM>,
    outer_id: WidgetId,
) -> ScrollRegion {
    computed
        .scroll_regions
        .iter()
        .filter(|region| region.id != outer_id)
        .max_by(|a, b| a.max_offset().y.get().total_cmp(&b.max_offset().y.get()))
        .copied()
        .expect("inner scroll region should exist")
}

fn demo_page_scroll_region<VM>(computed: &crate::ui::widget::ComputedScene<VM>) -> ScrollRegion {
    computed
        .scroll_regions
        .iter()
        .filter(|region| {
            region.visible_frame.height > dp(500.0)
                && region.max_offset().y > dp(100.0)
                && region.max_offset().y < dp(10_000.0)
        })
        .copied()
        .max_by(|a, b| a.max_offset().y.get().total_cmp(&b.max_offset().y.get()))
        .expect("sectioned demo page scroll region should exist")
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

fn full_demo_shell_virtual_page() -> Element<TestVm> {
    let rows = (0..10_000).collect::<Vec<_>>();
    let list = VirtualList::new(rows, |_index, item| {
        Stack::new()
            .height(dp(32.0))
            .padding(Insets::symmetric(dp(12.0), dp(6.0)))
            .child(Text::new(format!("Virtual row {item}")))
            .into()
    })
    .item_layout(ItemLayout::Fixed {
        item_extent: dp(32.0),
        spacing: dp(2.0),
        overscan: 4,
    })
    .key("demo-virtual-fixed")
    .width(pct(100.0))
    .height(dp(300.0));

    let usage_card = Flex::vertical()
        .width(pct(100.0))
        .gap(dp(10.0))
        .padding(Insets::all(dp(12.0)))
        .child(
            Flex::vertical()
                .gap(dp(4.0))
                .child(Text::new("固定行高"))
                .child(Text::new("10,000 行数据使用固定 item_extent 和 overscan。")),
        )
        .child(
            Flex::vertical()
                .width(pct(100.0))
                .align(Align::Stretch)
                .padding(Insets::all(dp(14.0)))
                .child(list),
        )
        .child(Button::new("Show code"))
        .child(Flex::<TestVm>::vertical().height(dp(0.0)));

    let data_page = Flex::vertical()
        .width(pct(100.0))
        .gap(dp(18.0))
        .padding(Insets::all(dp(24.0)))
        .child(Text::new("Data"))
        .child(Text::new(
            "数据页面展示导航、tabs、列表、虚拟滚动、树和表格型数据控件。",
        ))
        .child(
            Stack::new()
                .height(dp(520.0))
                .child(Text::new("Before VirtualList")),
        )
        .child(
            Flex::vertical()
                .width(pct(100.0))
                .gap(dp(14.0))
                .padding(Insets::all(dp(16.0)))
                .child(Text::new("VirtualList"))
                .child(Text::new(
                    "VirtualList 只构建可见行，适合大数据量滚动列表。",
                ))
                .child(usage_card),
        )
        .child(
            Stack::new()
                .height(dp(420.0))
                .child(Text::new("After VirtualList")),
        )
        .into();
    data_page
}

fn dynamic_full_demo_shell_virtual_scroll_test_tree(
    invalidation: &InvalidationSignal,
) -> (WidgetId, WidgetTree<TestVm>) {
    let page_state = State::new(0_u32, invalidation.clone());
    let dynamic_page = page_state
        .signal()
        .map_unchecked(|_| full_demo_shell_virtual_page().key("dynamic-data-page"));

    let content_scroll: Element<TestVm> = ScrollView::new()
        .key("demo-content-scroll")
        .size(pct(100.0), pct(100.0))
        .dynamic_child(dynamic_page)
        .into();
    let outer_id = content_scroll.id;
    let root = Stack::new().size(dp(900.0), dp(640.0)).child(
        Flex::horizontal()
            .size(pct(100.0), pct(100.0))
            .position_absolute()
            .inset(dp(0.0))
            .child(Stack::new().width(dp(240.0)).height(pct(100.0)))
            .child(
                Flex::vertical()
                    .grow(1.0)
                    .height(pct(100.0))
                    .child(content_scroll),
            ),
    );

    (outer_id, WidgetTree::new_legacy(root))
}

fn actual_demo_shell_virtual_page_test_tree() -> (WidgetId, WidgetTree<TestVm>) {
    let page: Element<TestVm> = VirtualList::new(
        vec![full_demo_shell_virtual_page()],
        |_index, item: &Element<TestVm>| item.clone(),
    )
    .key("page-content-data")
    .width(pct(100.0))
    .height(pct(100.0))
    .item_layout(ItemLayout::Measured {
        estimate: dp(620.0),
        spacing: dp(18.0),
        overscan: 1,
    })
    .into();
    let outer_id = page.id;
    let root = Stack::new().size(dp(900.0), dp(640.0)).child(
        Flex::horizontal()
            .size(pct(100.0), pct(100.0))
            .position_absolute()
            .inset(dp(0.0))
            .child(Stack::new().width(dp(240.0)).height(pct(100.0)))
            .child(Flex::vertical().grow(1.0).height(pct(100.0)).child(page)),
    );

    (outer_id, WidgetTree::new_legacy(root))
}

fn sectioned_demo_virtual_component() -> Element<TestVm> {
    let rows = (0..10_000).collect::<Vec<_>>();
    let list = VirtualList::new(rows, |_index, item| {
        Stack::new()
            .height(dp(32.0))
            .padding(Insets::symmetric(dp(12.0), dp(6.0)))
            .child(Text::new(format!("Virtual row {item}")))
            .into()
    })
    .item_layout(ItemLayout::Fixed {
        item_extent: dp(32.0),
        spacing: dp(2.0),
        overscan: 4,
    })
    .key("demo-virtual-fixed")
    .width(pct(100.0))
    .height(dp(300.0));

    let fixed_card = Flex::vertical()
        .width(pct(100.0))
        .gap(dp(10.0))
        .padding(Insets::all(dp(12.0)))
        .child(
            Flex::vertical()
                .gap(dp(4.0))
                .child(Text::new("固定行高"))
                .child(Text::new("10,000 行数据使用固定 item_extent 和 overscan。")),
        )
        .child(
            Flex::vertical()
                .width(pct(100.0))
                .align(Align::Stretch)
                .padding(Insets::all(dp(14.0)))
                .child(list),
        )
        .child(Button::new("Show code"))
        .child(Flex::<TestVm>::vertical().height(dp(0.0)));

    let measured_card = Flex::vertical()
        .width(pct(100.0))
        .gap(dp(10.0))
        .padding(Insets::all(dp(12.0)))
        .child(Text::new("测量行高"))
        .child(
            Stack::new()
                .height(dp(260.0))
                .child(Text::new("Measured list")),
        );

    Flex::vertical()
        .width(pct(100.0))
        .gap(dp(14.0))
        .padding(Insets::all(dp(16.0)))
        .child(Text::new("VirtualList"))
        .child(Text::new(
            "VirtualList 只构建可见行，适合大数据量滚动列表。",
        ))
        .child(fixed_card)
        .child(measured_card)
        .into()
}

fn sectioned_demo_placeholder(label: &'static str, height: Dp) -> Element<TestVm> {
    Flex::vertical()
        .width(pct(100.0))
        .height(height)
        .gap(dp(12.0))
        .padding(Insets::all(dp(16.0)))
        .child(Text::new(label))
        .child(Text::new("Placeholder section"))
        .into()
}

fn actual_sectioned_demo_shell_root() -> Element<TestVm> {
    let mut items: Vec<Element<TestVm>> = Vec::new();
    items.push(
        Flex::vertical()
            .width(pct(100.0))
            .gap(dp(18.0))
            .padding(Insets::all(dp(24.0)))
            .child(Text::new("Data"))
            .child(Text::new(
                "数据页面展示导航、tabs、列表、虚拟滚动、树和表格型数据控件。",
            ))
            .into(),
    );
    for section in [
        sectioned_demo_placeholder("Breadcrumb / Pagination", dp(360.0)),
        sectioned_demo_placeholder("Tabs", dp(520.0)),
        sectioned_demo_placeholder("List", dp(420.0)),
        sectioned_demo_virtual_component(),
        sectioned_demo_placeholder("Tree", dp(420.0)),
        sectioned_demo_placeholder("DataGrid", dp(520.0)),
    ] {
        items.push(
            Flex::vertical()
                .width(pct(100.0))
                .padding(Insets::symmetric(dp(24.0), Dp::ZERO))
                .child(section)
                .into(),
        );
    }
    items.push(Flex::vertical().height(dp(24.0)).into());

    let page: Element<TestVm> =
        VirtualList::new(items, |_index, item: &Element<TestVm>| item.clone())
            .key("page-content-data")
            .width(pct(100.0))
            .height(pct(100.0))
            .item_layout(ItemLayout::Measured {
                estimate: dp(620.0),
                spacing: dp(18.0),
                overscan: 1,
            })
            .into();

    Stack::new()
        .size(dp(900.0), dp(640.0))
        .child(
            Flex::horizontal()
                .size(pct(100.0), pct(100.0))
                .position_absolute()
                .inset(dp(0.0))
                .child(Stack::new().width(dp(260.0)).height(pct(100.0)))
                .child(Flex::vertical().grow(1.0).height(pct(100.0)).child(page)),
        )
        .into()
}

fn actual_sectioned_demo_shell_root_with_toasts(
    adaptive: ToastQueue<TestVm>,
    top_start: ToastQueue<TestVm>,
    top_center: ToastQueue<TestVm>,
    top_end: ToastQueue<TestVm>,
    bottom_start: ToastQueue<TestVm>,
    bottom_center: ToastQueue<TestVm>,
) -> Element<TestVm> {
    let content = actual_sectioned_demo_shell_root();
    Stack::new()
        .size(dp(900.0), dp(640.0))
        .child(content)
        .child(ToastHost::new(adaptive))
        .child(ToastHost::new(top_start).placement(ToastPlacement::TopStart))
        .child(ToastHost::new(top_center).placement(ToastPlacement::TopCenter))
        .child(ToastHost::new(top_end).placement(ToastPlacement::TopEnd))
        .child(ToastHost::new(bottom_start).placement(ToastPlacement::BottomStart))
        .child(ToastHost::new(bottom_center).placement(ToastPlacement::BottomCenter))
        .into()
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
    let tree = WidgetTree::new_legacy(Stack::new().size(dp(220.0), dp(140.0)).dynamic_child(
        visible.map_unchecked(move |visible| {
            if visible {
                list.clone()
            } else {
                Stack::new()
                    .height(dp(40.0))
                    .child(Text::new("Hidden"))
                    .into()
            }
        }),
    ));
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
fn scroll_offset_change_requests_redraw() {
    let invalidation = InvalidationSignal::new();
    let scroller: Element<TestVm> = Stack::new()
        .size(dp(100.0), dp(100.0))
        .overflow_y(Overflow::Scroll)
        .child(Stack::new().size(dp(100.0), dp(300.0)))
        .into();
    let scroller_id = scroller.id;
    let mut handler = test_handler(Some(WidgetTree::new(scroller)), invalidation);
    let _ = handler.computed_scene();
    assert!(!handler.invalidation.take_redraw_request());

    handler.set_scroll_offset(scroller_id, Point::new(Dp::ZERO, dp(48.0)));
    assert!(
        handler.invalidation.take_redraw_request(),
        "changing scroll offset should request a redraw even if the caller misses the event-level redraw path"
    );

    handler.set_scroll_offset(scroller_id, Point::new(Dp::ZERO, dp(48.0)));
    assert!(
        !handler.invalidation.take_redraw_request(),
        "setting the same scroll offset should not request another redraw"
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
fn actual_demo_shell_virtual_list_scrolls_inside_page_virtual_list() {
    let invalidation = InvalidationSignal::new();
    let (outer_id, tree) = actual_demo_shell_virtual_page_test_tree();
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(900.0, 640.0),
    );

    handler.set_scroll_offset(outer_id, Point::new(Dp::ZERO, dp(360.0)));

    let (region_id, target, baseline_min) = {
        let computed = handler.computed_scene();
        let region = largest_inner_scroll_region(computed, outer_id);
        assert!(
            region.visible_frame.height > dp(180.0),
            "actual demo shell VirtualList should be visible after page scroll, got {:?}",
            region.visible_frame
        );
        assert!(
            region.max_offset().y > dp(10_000.0),
            "actual demo shell VirtualList should have vertical overflow, got {:?}",
            region
        );
        let baseline_min = visible_virtual_row_indices(computed)
            .into_iter()
            .min()
            .expect("initial actual demo shell VirtualList rows should render");
        (
            region.id,
            Point {
                x: region.visible_frame.x + dp(24.0),
                y: region.visible_frame.y + dp(24.0),
            },
            baseline_min,
        )
    };
    assert_eq!(baseline_min, 0);

    handler.cursor_position = Some(target);
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -8.0)));

    let computed = handler.computed_scene();
    let scrolled_region = largest_inner_scroll_region(computed, outer_id);
    assert_eq!(
        scrolled_region.id, region_id,
        "nested actual demo VirtualList should keep the same WidgetId after scroll"
    );
    let scrolled_min = visible_virtual_row_indices(computed)
        .into_iter()
        .min()
        .expect("scrolled actual demo shell VirtualList rows should render");
    assert!(
        scrolled_min > baseline_min,
        "nested actual demo VirtualList should consume wheel input before the page VirtualList"
    );
}

#[test]
fn full_demo_shell_virtual_list_scrolls_after_page_scroll() {
    let invalidation = InvalidationSignal::new();
    let (outer_id, tree) = dynamic_full_demo_shell_virtual_scroll_test_tree(&invalidation);
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(900.0, 640.0),
    );

    handler.set_scroll_offset(outer_id, Point::new(Dp::ZERO, dp(360.0)));

    let (region_id, target, baseline_min) = {
        let computed = handler.computed_scene();
        let region = largest_inner_scroll_region(computed, outer_id);
        assert!(
            region.visible_frame.height > dp(180.0),
            "full demo shell VirtualList should be visible after page scroll, got {:?}",
            region.visible_frame
        );
        assert!(
            region.max_offset().y > dp(10_000.0),
            "full demo shell VirtualList should have vertical overflow, got {:?}",
            region
        );
        let baseline_min = visible_virtual_row_indices(computed)
            .into_iter()
            .min()
            .expect("initial full demo shell VirtualList rows should render");
        (
            region.id,
            Point {
                x: region.visible_frame.x + dp(24.0),
                y: region.visible_frame.y + dp(24.0),
            },
            baseline_min,
        )
    };
    assert_eq!(baseline_min, 0);

    handler.cursor_position = Some(target);
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -8.0)));

    let computed = handler.computed_scene();
    let scrolled_region = largest_inner_scroll_region(computed, outer_id);
    assert_eq!(
        scrolled_region.id, region_id,
        "keyed dynamic VirtualList should keep the same WidgetId after scroll"
    );
    let scrolled_min = visible_virtual_row_indices(computed)
        .into_iter()
        .min()
        .expect("scrolled full demo shell VirtualList rows should render");
    assert!(
        scrolled_min > baseline_min,
        "full demo shell VirtualList should consume wheel input before the page ScrollView"
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
fn full_demo_shell_virtual_list_scrollbar_drag_updates_rows() {
    let invalidation = InvalidationSignal::new();
    let (outer_id, tree) = dynamic_full_demo_shell_virtual_scroll_test_tree(&invalidation);
    let mut handler = test_handler_with_config(
        TestVm,
        Some(tree),
        invalidation,
        test_config_with_size(900.0, 640.0),
    );
    let event_loop = TestEventLoop;

    handler.set_scroll_offset(outer_id, Point::new(Dp::ZERO, dp(360.0)));

    let (region_id, start, target, baseline_min) = {
        let computed = handler.computed_scene();
        let baseline_min = visible_virtual_row_indices(computed)
            .into_iter()
            .min()
            .expect("initial full demo shell VirtualList rows should render");
        let region = largest_inner_scroll_region(computed, outer_id);
        let thumb = region
            .vertical_thumb
            .expect("full demo shell VirtualList should render a vertical scrollbar thumb");
        let track = region
            .vertical_track
            .expect("full demo shell VirtualList should render a vertical scrollbar track");
        let start = Point {
            x: thumb.x + dp(thumb.width.get() * 0.5),
            y: thumb.y + dp(thumb.height.get() * 0.5),
        };
        let travel = (track.height - thumb.height).max(0.0);
        let target = Point {
            x: start.x,
            y: start.y + travel * 0.35,
        };
        (region.id, start, target, baseline_min)
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
        "pressing the full demo shell VirtualList thumb should start scrollbar drag"
    );
    assert_eq!(
        handler.active_scrollbar_drag.map(|drag| drag.handle.id),
        Some(region_id),
        "pressing the full demo shell VirtualList thumb should target the inner scrollbar"
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

    let dragged_computed = handler.computed_scene();
    let dragged_indices = visible_virtual_row_indices(dragged_computed);
    let dragged_min = dragged_indices
        .iter()
        .copied()
        .min()
        .expect("dragged full demo shell VirtualList rows should render");
    let dragged_region = largest_inner_scroll_region(dragged_computed, outer_id);
    let dragged_offset = handler
        .scroll_states
        .get(&dragged_region.id)
        .copied()
        .unwrap_or(Point::ZERO);
    assert!(
        dragged_min > baseline_min,
        "full demo shell VirtualList should update visible rows while its thumb is dragged: baseline_min={baseline_min} dragged_min={dragged_min} dragged_indices={dragged_indices:?} region={dragged_region:?} offset={dragged_offset:?} active_drag={:?}",
        handler.active_scrollbar_drag.map(|drag| drag.handle)
    );

    handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::PointerButton {
            device_id: None,
            position: PhysicalPosition::new(f64::from(target.x.get()), f64::from(target.y.get())),
            state: ElementState::Released,
            button: ButtonSource::Mouse(MouseButton::Left),
            primary: true,
        },
    );

    let released_computed = handler.computed_scene();
    let released_indices = visible_virtual_row_indices(released_computed);
    let released_min = released_indices
        .iter()
        .copied()
        .min()
        .expect("released full demo shell VirtualList rows should render");
    let released_region = largest_inner_scroll_region(released_computed, outer_id);
    let released_offset = handler
        .scroll_states
        .get(&released_region.id)
        .copied()
        .unwrap_or(Point::ZERO);
    assert!(
        released_min > baseline_min,
        "full demo shell VirtualList should keep its scroll offset after releasing the thumb: baseline_min={baseline_min} released_min={released_min} released_indices={released_indices:?} region={released_region:?} offset={released_offset:?} active_drag={:?}",
        handler.active_scrollbar_drag.map(|drag| drag.handle)
    );
}

#[test]
fn rebuilt_sectioned_demo_virtual_list_scrollbar_drag_updates_rows() {
    stacker::grow(16 * 1024 * 1024, || {
        rebuilt_sectioned_demo_virtual_list_scrollbar_drag_updates_rows_impl();
    });
}

fn rebuilt_sectioned_demo_virtual_list_scrollbar_drag_updates_rows_impl() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let adaptive = ToastQueue::new(&context);
    let top_start = ToastQueue::new(&context);
    let top_center = ToastQueue::new(&context);
    let top_end = ToastQueue::new(&context);
    let bottom_start = ToastQueue::new(&context);
    let bottom_center = ToastQueue::new(&context);
    let mut handler = test_handler_with_config(
        TestVm,
        Some(WidgetTree::new(Text::new("initial"))),
        invalidation,
        test_config_with_size(900.0, 640.0),
    );
    handler.root_view = Some(Arc::new(move |_vm: &TestVm| {
        actual_sectioned_demo_shell_root_with_toasts(
            adaptive.clone(),
            top_start.clone(),
            top_center.clone(),
            top_end.clone(),
            bottom_start.clone(),
            bottom_center.clone(),
        )
    }));
    let command = Command::new_with_context(|_vm: &mut TestVm, context| {
        context.request_rebuild();
    });
    handler.execute_command_without_invalidation(&command);
    let event_loop = TestEventLoop;

    let outer_id = {
        let computed = handler.computed_scene();
        demo_page_scroll_region(computed).id
    };
    handler.set_scroll_offset(outer_id, Point::new(Dp::ZERO, dp(1_520.0)));

    let (region_id, start, target, baseline_min) = {
        let computed = handler.computed_scene();
        let region = largest_inner_scroll_region(computed, outer_id);
        assert!(
            region.visible_frame.height > dp(180.0),
            "sectioned demo VirtualList should be visible after page scroll, got {:?}",
            region.visible_frame
        );
        let baseline_min = visible_virtual_row_indices(computed)
            .into_iter()
            .min()
            .expect("initial sectioned demo VirtualList rows should render");
        let thumb = region
            .vertical_thumb
            .expect("sectioned demo VirtualList should render a vertical scrollbar thumb");
        let track = region
            .vertical_track
            .expect("sectioned demo VirtualList should render a vertical scrollbar track");
        let start = Point {
            x: thumb.x + dp(thumb.width.get() * 0.5),
            y: thumb.y + dp(thumb.height.get() * 0.5),
        };
        let travel = (track.height - thumb.height).max(0.0);
        let target = Point {
            x: start.x,
            y: start.y + travel * 0.35,
        };
        (region.id, start, target, baseline_min)
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
    assert_eq!(
        handler.active_scrollbar_drag.map(|drag| drag.handle.id),
        Some(region_id),
        "pressing the rebuilt sectioned demo VirtualList thumb should target the inner scrollbar"
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

    let dragged_computed = handler.computed_scene();
    let dragged_min = visible_virtual_row_indices(dragged_computed)
        .into_iter()
        .min()
        .expect("dragged rebuilt sectioned demo VirtualList rows should render");
    assert!(
        dragged_min > baseline_min,
        "rebuilt sectioned demo VirtualList should update visible rows while its thumb is dragged"
    );
}

#[test]
fn sectioned_demo_virtual_list_wheel_scrolls_inner_list() {
    stacker::grow(16 * 1024 * 1024, || {
        sectioned_demo_virtual_list_wheel_scrolls_inner_list_impl();
    });
}

fn sectioned_demo_virtual_list_wheel_scrolls_inner_list_impl() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler_with_config(
        TestVm,
        Some(WidgetTree::new(actual_sectioned_demo_shell_root())),
        invalidation,
        test_config_with_size(900.0, 640.0),
    );

    let outer_id = {
        let computed = handler.computed_scene();
        demo_page_scroll_region(computed).id
    };
    handler.set_scroll_offset(outer_id, Point::new(Dp::ZERO, dp(1_520.0)));

    let (target, baseline_min) = {
        let computed = handler.computed_scene();
        let region = largest_inner_scroll_region(computed, outer_id);
        let baseline_min = visible_virtual_row_indices(computed)
            .into_iter()
            .min()
            .expect("initial sectioned demo VirtualList rows should render");
        (
            Point {
                x: region.visible_frame.x + dp(40.0),
                y: region.visible_frame.y + dp(80.0),
            },
            baseline_min,
        )
    };
    handler.cursor_position = Some(target);
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -8.0)));
    while handler.advance_smooth_scroll() {}
    let scrolled_indices = visible_virtual_row_indices(handler.computed_scene());
    let scrolled_min = scrolled_indices
        .into_iter()
        .min()
        .expect("scrolled sectioned demo VirtualList rows should render");
    assert!(
        scrolled_min > baseline_min,
        "sectioned demo VirtualList should scroll by wheel"
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
fn smooth_scroll_advances_linearly() {
    let invalidation = InvalidationSignal::new();
    let mut handler = test_handler(None, invalidation);
    let widget_id = WidgetId::from_raw(90_001);
    let frames = usize::from(super::super::super::state::SMOOTH_SCROLL_FRAMES);
    let target_y = dp(frames as f32 * 10.0);

    handler.set_smooth_scroll_target(widget_id, Point::new(Dp::ZERO, target_y));
    let mut offsets = vec![handler
        .scroll_states
        .get(&widget_id)
        .map(|offset| offset.y)
        .unwrap_or(Dp::ZERO)];

    while handler.advance_smooth_scroll() {
        offsets.push(
            handler
                .scroll_states
                .get(&widget_id)
                .map(|offset| offset.y)
                .unwrap_or(Dp::ZERO),
        );
    }

    assert_eq!(offsets.len(), frames);
    for (index, offset) in offsets.iter().enumerate() {
        let expected = (index + 1) as f32 * 10.0;
        assert!(
            (offset.get() - expected).abs() <= 0.001,
            "frame {} should be linear: expected {expected}, got {}",
            index + 1,
            offset.get()
        );
    }
    for pair in offsets.windows(2) {
        let delta = pair[1] - pair[0];
        assert!(
            (delta.get() - 10.0).abs() <= 0.001,
            "smooth scroll frame delta should stay constant, got {}",
            delta.get()
        );
    }
    assert!(!handler.smooth_scroll_states.contains_key(&widget_id));
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
fn scroll_view_controller_cached_hit_visits_only_bound_controllers() {
    let invalidation = InvalidationSignal::new();
    let ctx = ViewModelContext::for_benchmarks();
    let controller = ScrollViewController::new(&ctx);
    let mut content = Flex::<TestVm>::vertical();
    for index in 0..64 {
        content = content.child(
            Stack::new()
                .size(dp(120.0), dp(8.0))
                .child(Text::new(format!("row {index}"))),
        );
    }
    let tree = WidgetTree::new(
        ScrollView::new()
            .size(dp(160.0), dp(540.0))
            .controller(controller)
            .child(content),
    );
    let mut handler = test_handler(Some(tree), invalidation);

    let scroll_region_count = handler.computed_scene().scroll_regions.len();
    assert!(
        scroll_region_count >= 16,
        "test scene should contain many unbound container scroll regions, got {scroll_region_count}"
    );
    crate::runtime::scene_runtime::scroll_view_binding_probe::reset();

    let _ = handler.computed_scene();

    assert_eq!(
        crate::runtime::scene_runtime::scroll_view_binding_probe::rebuild_region_visits(),
        0,
        "a valid cached scene must not rescan all scroll regions"
    );
    assert_eq!(
        crate::runtime::scene_runtime::scroll_view_binding_probe::consume_binding_visits(),
        1,
        "cache hits should visit only the single bound controller"
    );
    assert_eq!(
        crate::runtime::scene_runtime::scroll_view_binding_probe::stale_rebuilds(),
        0
    );
}

#[test]
fn large_scene_scroll_interactions_visit_only_real_scroll_candidates() {
    let invalidation = InvalidationSignal::new();
    let mut content = Flex::<TestVm>::vertical();
    for index in 0..256 {
        content = content.child(
            Stack::new()
                .size(dp(140.0), dp(8.0))
                .child(Text::new(format!("row {index}"))),
        );
    }
    let scroller: Element<TestVm> = ScrollView::new()
        .size(dp(180.0), dp(120.0))
        .child(content)
        .into();
    let scroller_id = scroller.id;
    let mut handler = test_handler_with_config(
        TestVm,
        Some(WidgetTree::new(scroller)),
        invalidation,
        test_config_with_size(180.0, 120.0),
    );

    let (region_count, cursor, thumb_cursor, drag_cursor) = {
        let computed = handler.computed_scene();
        let region = computed
            .scroll_regions
            .iter()
            .find(|region| region.id == scroller_id)
            .copied()
            .expect("scroll view should register a region");
        let thumb = region
            .vertical_thumb
            .expect("overflowing scroll view should render a thumb");
        let track = region
            .vertical_track
            .expect("overflowing scroll view should render a track");
        (
            computed.scroll_regions.len(),
            Point::new(
                region.visible_frame.x + dp(20.0),
                region.visible_frame.y + dp(20.0),
            ),
            Point::new(thumb.x + thumb.width * 0.5, thumb.y + thumb.height * 0.5),
            Point::new(
                thumb.x + thumb.width * 0.5,
                thumb.y + thumb.height * 0.5 + (track.height - thumb.height) * 0.25,
            ),
        )
    };
    assert!(
        region_count >= 16,
        "test scene should cross the lazy lookup threshold, got {region_count} regions"
    );

    handler.cursor_position = Some(cursor);
    crate::runtime::input::scroll_region_lookup_probe::reset();
    assert!(handler.handle_mouse_wheel(MouseScrollDelta::LineDelta(0.0, -2.0)));
    assert_eq!(
        crate::runtime::input::scroll_region_lookup_probe::wheel_candidate_visits(),
        1,
        "wheel targeting should skip non-scrollable Container regions"
    );

    handler.cursor_position = Some(cursor);
    crate::runtime::input::scroll_region_lookup_probe::reset();
    assert!(handler.begin_touch_scroll_drag(handler.viewport_rect()));
    assert_eq!(
        crate::runtime::input::scroll_region_lookup_probe::touch_candidate_visits(),
        1,
        "touch targeting should visit only the actual scroll view"
    );
    handler.active_touch_scroll = None;

    handler.cursor_position = Some(thumb_cursor);
    crate::runtime::input::scroll_region_lookup_probe::reset();
    assert!(handler.sync_scrollbar_hover());
    assert_eq!(
        crate::runtime::input::scroll_region_lookup_probe::scrollbar_candidate_visits(),
        1,
        "scrollbar hover should skip regions without thumbs"
    );
    assert!(handler.begin_scrollbar_drag());

    handler.cursor_position = Some(drag_cursor);
    crate::runtime::input::scroll_region_lookup_probe::reset();
    assert!(handler.handle_scrollbar_drag());
    assert_eq!(
        crate::runtime::input::scroll_region_lookup_probe::drag_id_fallback_visits(),
        0,
        "stable scrollbar drags should validate the retained region index in O(1)"
    );
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
