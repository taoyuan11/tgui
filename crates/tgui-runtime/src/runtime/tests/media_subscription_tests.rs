use super::*;
use crate::ui::widget::{FocusScopeOptions, For, Image, Show, ViewSwitch, VirtualList};

fn subscribed_image() -> Element<TestVm> {
    Image::new(crate::media::MediaSource::bytes(
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>"#,
    ))
    .on_loading(Command::new(|_: &mut TestVm| {}))
}

fn assert_subscription_ids_match_full_walk(handler: &BoundRuntimeHandler<TestVm>) -> Vec<WidgetId> {
    let tree = handler.widget_tree.as_ref().unwrap();
    let ids = |states: Vec<crate::ui::widget::MediaEventState<TestVm>>| {
        states
            .into_iter()
            .map(|state| state.widget_id)
            .collect::<Vec<_>>()
    };
    let expected =
        ids(tree.media_event_states_full_walk(&handler.media_manager, &handler.theme, None));
    let actual = ids(tree.media_event_states_with_active_tooltip(
        &handler.media_manager,
        &handler.theme,
        None,
    ));
    assert_eq!(actual, expected);
    actual
}

#[test]
fn media_subscriptions_skip_visual_siblings_even_with_keyed_children() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let items = context.state(vec![1usize]);
    let style_calls = Arc::new(AtomicUsize::new(0));
    let children = (0..1000)
        .map(|_| {
            let calls = style_calls.clone();
            Text::new("visual-only").style_full(move |context| {
                calls.fetch_add(1, Ordering::Relaxed);
                crate::ui::widget::TextWidgetStyle::default_for_theme(context.theme)
            })
        })
        .collect::<Vec<_>>();
    let image = subscribed_image();
    let tree = WidgetTree::new(Stack::new().child(children).child(For::new(
        items.signal(),
        |item| *item,
        move |_, _| image.clone(),
    )));
    let mut handler = test_handler(Some(tree), invalidation);
    for _ in 0..3 {
        handler.dispatch_media_events();
        assert_eq!(handler.media_event_states.len(), 1);
    }
    assert_eq!(
        style_calls.load(Ordering::Relaxed),
        0,
        "media polling must never resolve styles of unrelated widgets"
    );
    items.set(Vec::new());
    handler.dispatch_media_events();
    assert!(handler.media_event_states.is_empty());
    items.set(vec![1]);
    handler.dispatch_media_events();
    assert_eq!(handler.media_event_states.len(), 1);
    assert_eq!(assert_subscription_ids_match_full_walk(&handler).len(), 1);
    assert!(
        style_calls.load(Ordering::Relaxed) >= 1000,
        "the full-walk control must visit siblings"
    );
}

#[test]
fn media_subscriptions_preserve_show_switch_fallback_and_inactive_scope() {
    let invalidation = InvalidationSignal::new();
    let context = ViewModelContext::new(invalidation.clone(), AnimationCoordinator::default());
    let visible = context.state(true);
    let active = context.state(true);
    let selected = context.state(0usize);
    let image = subscribed_image();
    let fallback = subscribed_image();
    let branch = Stack::new().child(
        ViewSwitch::new(selected.signal())
            .case(Text::new("empty subscription branch"))
            .case(image)
            .fallback(fallback),
    );
    let tree = WidgetTree::new(
        Stack::new()
            .focus_scope(
                FocusScopeOptions::new()
                    .active(active.signal())
                    .suppress_interactions_when_inactive(),
            )
            .child(Show::new(visible.signal(), branch)),
    );
    let handler = test_handler(Some(tree), invalidation);
    assert!(assert_subscription_ids_match_full_walk(&handler).is_empty());
    selected.set(1);
    assert_eq!(assert_subscription_ids_match_full_walk(&handler).len(), 1);
    selected.set(20);
    assert_eq!(assert_subscription_ids_match_full_walk(&handler).len(), 1);
    visible.set(false);
    assert!(assert_subscription_ids_match_full_walk(&handler).is_empty());
    visible.set(true);
    active.set(false);
    assert!(assert_subscription_ids_match_full_walk(&handler).is_empty());
    active.set(true);
    assert_eq!(assert_subscription_ids_match_full_walk(&handler).len(), 1);
}

#[test]
fn media_subscriptions_virtual_boundary_matches_full_walk() {
    let image = subscribed_image();
    let tree = WidgetTree::new(Stack::new().child(
        VirtualList::new(vec![1usize], move |_, _| image.clone()).size(dp(100.0), dp(100.0)),
    ));
    let handler = test_handler(Some(tree), InvalidationSignal::new());
    assert_eq!(assert_subscription_ids_match_full_walk(&handler).len(), 1);
}
