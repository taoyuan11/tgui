pub(super) use super::*;

use std::time::{Duration, Instant};

use crate::foundation::binding::{Toast, ToastKind, ToastPlacement, ToastQueue};
use crate::ui::widget::style::ToastStyle;
use crate::ui::widget::{Stack, ToastHost};

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
        .find(|text| text.content == "\u{e88e}")
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
