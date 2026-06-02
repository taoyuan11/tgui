pub(super) use super::*;

use std::time::{Duration, Instant};

use crate::foundation::binding::{Toast, ToastPlacement, ToastQueue};
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
