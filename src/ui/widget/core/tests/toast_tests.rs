pub(super) use super::*;

use std::time::{Duration, Instant};

use crate::foundation::binding::{Toast, ToastKind, ToastPlacement, ToastQueue};
use crate::ui::widget::style::ToastStyle;
use crate::ui::widget::{ComputedScene, Stack, ToastHost};

#[test]
fn toast_host_emits_overlay_content_in_stack_order_and_tracks_wakeup() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    // 用过去的时间创建Toast，确保入场动画已完成
    let created_at = now - Duration::from_secs(1);

    queue.push_at(
        Toast::new("first").duration(Duration::from_secs(10)),
        created_at,
    );
    queue.push_at(
        Toast::new("second").duration(Duration::from_secs(9)),
        created_at,
    );
    queue.push_at(
        Toast::new("third").duration(Duration::from_secs(8)),
        created_at,
    );

    let tree = WidgetTree::new(
        Stack::new().child(Text::new("content")).child(
            ToastHost::new(queue.clone())
                .placement(ToastPlacement::BottomEnd)
                .max_visible(2),
        ),
    );
    let viewport = Rect::new(0.0, 0.0, 480.0, 320.0);
    let layout = tree.build_scene_layout_at(
        &font_manager,
        &theme,
        &media,
        &mut animations,
        UnitContext::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        now,
    );
    let collected = tree.collect_scene_cache_from_layout_with_focus_value_at(
        &font_manager,
        &layout,
        &theme,
        &media,
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        now,
        &HashMap::new(),
        None,
        None,
    );

    // Toast在1秒前创建，入场动画已完成（400ms），唤醒时间应该是最早的deadline
    assert!(
        collected.next_toast_wakeup.is_some(),
        "should have a toast wakeup time"
    );
    assert!(
        !collected.computed.scene.overlay_shapes.is_empty(),
        "toast host should emit overlay card shapes"
    );

    let labels: Vec<_> = collected
        .computed
        .scene
        .overlay_texts
        .iter()
        .map(|text| text.content.as_str())
        .collect();
    assert!(
        !labels.iter().any(|text| *text == "first"),
        "max_visible(2) should clip the oldest toast, got {labels:?}"
    );

    let third_index = labels
        .iter()
        .position(|text| *text == "third")
        .expect("latest toast should be rendered");
    let second_index = labels
        .iter()
        .position(|text| *text == "second")
        .expect("second toast should be rendered");
    assert!(
        third_index < second_index,
        "bottom placement should keep the newest toast closest to the anchor, got {labels:?}"
    );
}

#[test]
fn toast_stack_auto_collapses_after_three_and_expands_from_queue_state() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    let created_at = now - Duration::from_secs(1);

    for index in 1..=4 {
        queue.push_at(
            Toast::new(format!("toast {index}")).duration(Duration::from_secs(10)),
            created_at,
        );
    }

    let tree = WidgetTree::new(
        Stack::new()
            .child(Text::new("content"))
            .child(ToastHost::new(queue.clone()).placement(ToastPlacement::BottomEnd)),
    );
    let viewport = Rect::new(0.0, 0.0, 480.0, 360.0);
    let collapsed = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now,
    );
    let collapsed_messages = toast_message_labels(&collapsed);
    assert_eq!(
        collapsed_messages,
        vec!["toast 4"],
        "collapsed stack should show only the front toast content"
    );
    let (front, back_layers) = toast_stack_card_layers(&collapsed);
    assert!(
        back_layers.len() >= 2,
        "collapsed stack should reveal two back card layers, got {back_layers:?}"
    );
    assert!(
        (back_layers[0].x - (front.x + dp(12.0))).abs() <= dp(1.0)
            && (back_layers[0].width - (front.width - dp(24.0))).abs() <= dp(1.0)
            && (back_layers[0].y - (front.y + dp(16.0))).abs() <= dp(1.0),
        "first back layer should be inset and shifted: front={front:?}, layer={:?}",
        back_layers[0]
    );
    assert!(
        (back_layers[1].x - (front.x + dp(24.0))).abs() <= dp(1.0)
            && (back_layers[1].width - (front.width - dp(48.0))).abs() <= dp(1.0)
            && (back_layers[1].y - (front.y + dp(32.0))).abs() <= dp(1.0),
        "second back layer should be inset and shifted: front={front:?}, layer={:?}",
        back_layers[1]
    );

    queue.set_stack_expanded(true);
    let _ = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now,
    );
    let expanded = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now + Duration::from_millis(220),
    );
    let expanded_messages = toast_message_labels(&expanded);
    assert_eq!(
        expanded_messages,
        vec!["toast 4", "toast 3", "toast 2", "toast 1"],
        "expanded stack should render all queued toasts"
    );

    queue.set_stack_expanded(false);
    let _ = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now + Duration::from_millis(220),
    );
    let collapsed_again = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now + Duration::from_millis(440),
    );
    let collapsed_again_messages = toast_message_labels(&collapsed_again);
    assert_eq!(
        collapsed_again_messages,
        vec!["toast 4"],
        "leaving the stack should collapse it back to front toast content"
    );
}

#[test]
fn toast_stack_expand_animation_interpolates_layers() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    let created_at = now - Duration::from_secs(1);

    for index in 1..=4 {
        queue.push_at(
            Toast::new(format!("toast {index}")).duration(Duration::from_secs(10)),
            created_at,
        );
    }

    let tree = WidgetTree::new(
        Stack::new()
            .child(Text::new("content"))
            .child(ToastHost::new(queue.clone()).placement(ToastPlacement::BottomEnd)),
    );
    let viewport = Rect::new(0.0, 0.0, 480.0, 360.0);
    let collapsed = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now,
    );
    let (collapsed_front, collapsed_back_layers) = toast_stack_card_layers(&collapsed);
    let collapsed_first_back = collapsed_back_layers[0];

    queue.set_stack_expanded(true);
    let _ = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now,
    );
    let mid = compute_scene_at(
        &tree,
        &font_manager,
        &theme,
        &media,
        &mut animations,
        viewport,
        now + Duration::from_millis(90),
    );
    let mid_messages = toast_message_labels(&mid);
    assert!(
        mid_messages.iter().any(|label| label == "toast 3"),
        "second toast content should fade in during expansion, got {mid_messages:?}"
    );
    let (_, mid_back_layers) = toast_stack_card_layers(&mid);
    let mid_first_back = mid_back_layers
        .iter()
        .find(|rect| rect.width < collapsed_front.width && rect.width > collapsed_first_back.width)
        .copied()
        .expect("first animated back layer should have intermediate width");
    let mid_faded_card = mid
        .scene
        .overlay_shapes
        .iter()
        .find(|shape| {
            shape.stroke_width <= 0.1
                && shape.rect.width == mid_first_back.width
                && shape.rect.height == mid_first_back.height
                && shape.color.a < 255
        })
        .expect("intermediate card layer should fade while expanding");
    assert!(
        (mid_first_back.y - collapsed_first_back.y).abs() > dp(4.0)
            && mid_first_back.width > collapsed_first_back.width
            && mid_first_back.width < collapsed_front.width,
        "back layer should interpolate y and width: collapsed={collapsed_first_back:?}, mid={mid_first_back:?}, front={collapsed_front:?}, faded_alpha={}",
        mid_faded_card.color.a
    );
}

#[test]
fn toast_stack_top_placements_animate_new_front_toast_while_collapsed() {
    for placement in [
        ToastPlacement::TopStart,
        ToastPlacement::TopCenter,
        ToastPlacement::TopEnd,
    ] {
        let theme = Theme::default();
        let font_manager = FontManager::new(&FontCatalog::default());
        let media = test_media();
        let mut animations = AnimationEngine::default();
        let context = test_context();
        let queue = ToastQueue::<()>::new(&context);
        let now = Instant::now();
        let old_created_at = now - Duration::from_secs(1);

        for index in 1..=4 {
            queue.push_at(
                Toast::new(format!("toast {index}")).duration(Duration::from_secs(10)),
                old_created_at,
            );
        }

        let tree = WidgetTree::new(
            Stack::new()
                .child(Text::new("content"))
                .child(ToastHost::new(queue.clone()).placement(placement)),
        );
        let viewport = Rect::new(0.0, 0.0, 480.0, 360.0);
        let _ = compute_scene_at(
            &tree,
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
            now,
        );

        queue.push_at(Toast::new("toast 5").duration(Duration::from_secs(10)), now);

        let mid = compute_scene_at(
            &tree,
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
            now + Duration::from_millis(100),
        );
        let settled = compute_scene_at(
            &tree,
            &font_manager,
            &theme,
            &media,
            &mut animations,
            viewport,
            now + Duration::from_millis(500),
        );

        let mid_frame = toast_label_frame(&mid, "toast 5");
        let settled_frame = toast_label_frame(&settled, "toast 5");
        match placement {
            ToastPlacement::TopStart => assert!(
                mid_frame.x < settled_frame.x - dp(10.0),
                "new collapsed top-start toast should slide in from the leading edge: mid={mid_frame:?}, settled={settled_frame:?}"
            ),
            ToastPlacement::TopCenter => assert!(
                mid_frame.y < settled_frame.y - dp(5.0),
                "new collapsed top-center toast should slide in from above: mid={mid_frame:?}, settled={settled_frame:?}"
            ),
            ToastPlacement::TopEnd => assert!(
                mid_frame.x > settled_frame.x + dp(10.0),
                "new collapsed top-end toast should slide in from the trailing edge: mid={mid_frame:?}, settled={settled_frame:?}"
            ),
            _ => unreachable!(),
        }
    }
}

#[test]
fn toast_stack_hover_region_toggles_auto_collapse_state() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    let created_at = now - Duration::from_secs(1);

    for index in 1..=4 {
        queue.push_at(
            Toast::new(format!("toast {index}")).duration(Duration::from_secs(10)),
            created_at,
        );
    }

    let tree = WidgetTree::new(
        Stack::new()
            .child(Text::new("content"))
            .child(ToastHost::new(queue.clone()).placement(ToastPlacement::BottomEnd)),
    );
    let computed = tree.compute_scene_with_units_and_widget_state_at(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        Rect::new(0.0, 0.0, 480.0, 360.0),
        None,
        None,
        None,
        None,
        false,
        now,
    );

    let hover_regions: Vec<_> = computed
        .overlay_hit_regions
        .iter()
        .filter_map(|hit| match &hit.interaction {
            HitInteraction::Widget { interactions, .. }
                if interactions.on_mouse_enter.is_some()
                    && interactions.on_mouse_leave.is_some() =>
            {
                Some(interactions.clone())
            }
            _ => None,
        })
        .collect();

    assert!(
        !hover_regions.is_empty(),
        "collapsible toast stack should emit hover enter/leave hit regions"
    );
    assert!(
        !queue.stack_expanded(),
        "toast stack should start collapsed when more than three toasts are visible"
    );

    let mut vm = ();
    for interactions in hover_regions.iter() {
        if let Some(command) = interactions.on_mouse_enter.as_ref() {
            command.execute(&mut vm);
        }
    }
    assert!(
        queue.stack_expanded(),
        "hover enter should expand the toast stack"
    );

    for interactions in hover_regions.iter().rev() {
        if let Some(command) = interactions.on_mouse_leave.as_ref() {
            command.execute(&mut vm);
        }
    }
    assert!(
        !queue.stack_expanded(),
        "hover leave should collapse the toast stack"
    );
}

#[test]
fn toast_close_button_aligns_to_card_trailing_edge() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    let created_at = now - Duration::from_secs(1);

    queue.push_at(
        Toast::new("body")
            .title("Saved")
            .duration(Duration::from_secs(10)),
        created_at,
    );

    let tree = WidgetTree::new(Stack::new().child(Text::new("content")).child(
        ToastHost::new(queue.clone()).style(|mode| {
            let mut style = ToastStyle::default_for(mode);
            style.max_width = dp(320.0);
            style.padding = Insets::all(dp(12.0));
            style
        }),
    ));
    let viewport = Rect::new(0.0, 0.0, 480.0, 320.0);
    let computed = tree.compute_scene_with_units_and_widget_state_at(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
        now,
    );

    let card_rect = computed
        .scene
        .overlay_shapes
        .iter()
        .max_by(|left, right| {
            let left_area = left.rect.width.get() * left.rect.height.get();
            let right_area = right.rect.width.get() * right.rect.height.get();
            left_area
                .partial_cmp(&right_area)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|shape| shape.rect)
        .expect("toast card background should render");
    let close_icon = computed
        .scene
        .overlay_texts
        .iter()
        .find(|text| text.content == "\u{e5cd}")
        .expect("toast close icon should render");

    assert!(
        close_icon.frame.x > card_rect.right() - dp(48.0),
        "close button should sit near the card trailing edge: card={card_rect:?}, close={:?}",
        close_icon.frame
    );
}

#[test]
fn toast_kind_icon_is_centered_inside_circle() {
    let theme = Theme::default();
    let font_manager = FontManager::new(&FontCatalog::default());
    let media = test_media();
    let mut animations = AnimationEngine::default();
    let context = test_context();
    let queue = ToastQueue::<()>::new(&context);
    let now = Instant::now();
    let created_at = now - Duration::from_secs(1);

    queue.push_at(
        Toast::new("body")
            .title("Info")
            .kind(ToastKind::Info)
            .duration(Duration::from_secs(10)),
        created_at,
    );

    let tree = WidgetTree::new(Stack::new().child(Text::new("content")).child(
        ToastHost::new(queue.clone()).style(|mode| {
            let mut style = ToastStyle::default_for(mode);
            style.max_width = dp(320.0);
            style.padding = Insets::all(dp(12.0));
            style
        }),
    ));
    let viewport = Rect::new(0.0, 0.0, 480.0, 320.0);
    let computed = tree.compute_scene_with_units_and_widget_state_at(
        &font_manager,
        &theme,
        &media,
        UnitContext::default(),
        &mut animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
        now,
    );

    let circle = computed
        .scene
        .overlay_shapes
        .iter()
        .find(|shape| {
            (shape.rect.width - dp(18.0)).abs() <= dp(0.1)
                && (shape.rect.height - dp(18.0)).abs() <= dp(0.1)
        })
        .expect("toast kind icon circle should render");
    let icon = computed
        .scene
        .overlay_texts
        .iter()
        .find(|text| text.content == "i")
        .expect("toast info icon should render");

    let circle_center_x = circle.rect.x + circle.rect.width * 0.5;
    let circle_center_y = circle.rect.y + circle.rect.height * 0.5;
    let icon_center_x = icon.frame.x + icon.frame.width * 0.5;
    let icon_center_y = icon.frame.y + icon.frame.height * 0.5;

    assert!(
        icon.frame.width < circle.rect.width,
        "toast icon should be laid out by glyph width, not by the full circle frame: circle={:?}, icon={:?}",
        circle.rect,
        icon.frame
    );
    assert!(
        (icon_center_x - circle_center_x).abs() <= dp(1.0)
            && (icon_center_y - circle_center_y).abs() <= dp(1.0),
        "toast icon should be centered in its circle: circle={:?}, icon={:?}",
        circle.rect,
        icon.frame
    );
}

fn compute_scene_at(
    tree: &WidgetTree<()>,
    font_manager: &FontManager,
    theme: &Theme,
    media: &MediaManager,
    animations: &mut AnimationEngine,
    viewport: Rect,
    now: Instant,
) -> ComputedScene<()> {
    tree.compute_scene_with_units_and_widget_state_at(
        font_manager,
        theme,
        media,
        UnitContext::default(),
        animations,
        false,
        None,
        None,
        &WidgetStateMap::default(),
        &HashMap::new(),
        &HashMap::new(),
        viewport,
        None,
        None,
        None,
        None,
        false,
        now,
    )
}

fn toast_message_labels(computed: &ComputedScene<()>) -> Vec<String> {
    computed
        .scene
        .overlay_texts
        .iter()
        .filter_map(|text| {
            text.content
                .starts_with("toast ")
                .then(|| text.content.clone())
        })
        .collect()
}

fn toast_label_frame(computed: &ComputedScene<()>, label: &str) -> Rect {
    computed
        .scene
        .overlay_texts
        .iter()
        .find_map(|text| (text.content == label).then_some(text.frame))
        .unwrap_or_else(|| panic!("toast label {label:?} should render"))
}

fn toast_stack_card_layers(computed: &ComputedScene<()>) -> (Rect, Vec<Rect>) {
    let mut cards: Vec<_> = computed
        .scene
        .overlay_shapes
        .iter()
        .filter_map(|shape| {
            (shape.stroke_width <= 0.1
                && shape.rect.width >= dp(180.0)
                && shape.rect.height >= dp(40.0))
            .then_some(shape.rect)
        })
        .collect();
    cards.sort_by(|left, right| {
        let left_area = left.width.get() * left.height.get();
        let right_area = right.width.get() * right.height.get();
        right_area
            .partial_cmp(&left_area)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let front = cards
        .first()
        .copied()
        .expect("toast stack should have a front card");
    let mut back_layers: Vec<_> = cards
        .into_iter()
        .filter(|rect| rect.width < front.width && rect.y > front.y)
        .collect();
    back_layers.sort_by(|left, right| {
        left.y
            .partial_cmp(&right.y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    (front, back_layers)
}
