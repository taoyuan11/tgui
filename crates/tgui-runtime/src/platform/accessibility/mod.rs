use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use accesskit::{ActionHandler, ActionRequest, NodeId, TreeId, TreeUpdate};
use crossbeam_channel::Sender;

use crate::accessibility::{TreeUpdateKey, ROOT_NODE_ID};

use super::backend::window::Window;

#[derive(Clone)]
struct ChannelActionHandler {
    sender: Sender<ActionRequest>,
}

impl ActionHandler for ChannelActionHandler {
    fn do_action(&mut self, request: ActionRequest) {
        let _ = self.sender.send(request);
    }
}

pub(crate) struct PlatformAccessibilityAdapter {
    inner: PlatformAccessibilityAdapterInner,
    cache: AccessibilityUpdateCache,
    needs_full_tree: Arc<AtomicBool>,
}

impl PlatformAccessibilityAdapter {
    pub(crate) fn new(window: &dyn Window, action_sender: Sender<ActionRequest>) -> Option<Self> {
        let needs_full_tree = Arc::new(AtomicBool::new(true));
        PlatformAccessibilityAdapterInner::new(window, action_sender, Arc::clone(&needs_full_tree))
            .map(|inner| Self {
                inner,
                cache: AccessibilityUpdateCache::default(),
                needs_full_tree,
            })
    }

    pub(crate) fn update_if_active(
        &mut self,
        key: TreeUpdateKey,
        candidate_focus: NodeId,
        full_update_factory: impl FnOnce() -> TreeUpdate,
    ) {
        let force_full = self.needs_full_tree.load(Ordering::Acquire);
        let mut invoked = false;
        dispatch_cached_update(
            &mut self.cache,
            force_full,
            key,
            candidate_focus,
            full_update_factory,
            |update_factory| {
                self.inner.update_if_active(|| {
                    invoked = true;
                    update_factory()
                });
            },
        );
        if force_full && invoked {
            self.needs_full_tree.store(false, Ordering::Release);
        }
    }

    pub(crate) fn update_window_focus_state(&mut self, is_focused: bool) {
        self.inner.update_window_focus_state(is_focused);
    }
}

#[derive(Default)]
struct AccessibilityUpdateCache {
    key: Option<TreeUpdateKey>,
    focus: Option<NodeId>,
    included_nodes: HashSet<NodeId>,
}

impl AccessibilityUpdateCache {
    fn normalized_focus(&self, candidate_focus: NodeId) -> NodeId {
        self.included_nodes
            .contains(&candidate_focus)
            .then_some(candidate_focus)
            .unwrap_or(ROOT_NODE_ID)
    }

    fn needs_update(&self, force_full: bool, key: TreeUpdateKey, candidate_focus: NodeId) -> bool {
        force_full
            || self.key != Some(key)
            || self.focus != Some(self.normalized_focus(candidate_focus))
    }

    fn prepare_update(
        &mut self,
        force_full: bool,
        key: TreeUpdateKey,
        candidate_focus: NodeId,
        full_update_factory: impl FnOnce() -> TreeUpdate,
    ) -> TreeUpdate {
        if force_full || self.key != Some(key) {
            let update = full_update_factory();
            self.included_nodes.clear();
            self.included_nodes
                .extend(update.nodes.iter().map(|(node_id, _)| *node_id));
            self.key = Some(key);
            self.focus = Some(update.focus);
            return update;
        }

        let focus = self.normalized_focus(candidate_focus);
        self.focus = Some(focus);
        TreeUpdate {
            nodes: Vec::new(),
            tree: None,
            tree_id: TreeId::ROOT,
            focus,
        }
    }
}

fn dispatch_cached_update(
    cache: &mut AccessibilityUpdateCache,
    force_full: bool,
    key: TreeUpdateKey,
    candidate_focus: NodeId,
    full_update_factory: impl FnOnce() -> TreeUpdate,
    dispatch: impl FnOnce(&mut dyn FnMut() -> TreeUpdate),
) {
    if !cache.needs_update(force_full, key, candidate_focus) {
        return;
    }

    let mut full_update_factory = Some(full_update_factory);
    let mut update_factory = || {
        cache.prepare_update(
            force_full,
            key,
            candidate_focus,
            full_update_factory
                .take()
                .expect("full accessibility update factory may only run once"),
        )
    };
    dispatch(&mut update_factory);
}

#[cfg(target_os = "windows")]
mod platform {
    use super::*;
    use raw_window_handle::RawWindowHandle;
    use windows::Win32::Foundation::HWND;

    pub(super) struct PlatformAccessibilityAdapterInner {
        adapter: accesskit_windows::Adapter,
    }

    impl PlatformAccessibilityAdapterInner {
        pub(super) fn new(
            window: &dyn Window,
            action_sender: Sender<ActionRequest>,
            _needs_full_tree: Arc<AtomicBool>,
        ) -> Option<Self> {
            let handle = window.window_handle().ok()?.as_raw();
            let RawWindowHandle::Win32(handle) = handle else {
                return None;
            };
            let hwnd = HWND(handle.hwnd.get() as *mut core::ffi::c_void);
            let adapter = accesskit_windows::Adapter::new(
                hwnd,
                window.has_focus(),
                ChannelActionHandler {
                    sender: action_sender,
                },
            );
            Some(Self { adapter })
        }

        pub(super) fn update_if_active(&mut self, update_factory: impl FnOnce() -> TreeUpdate) {
            if let Some(events) = self.adapter.update_if_active(update_factory) {
                events.raise();
            }
        }

        pub(super) fn update_window_focus_state(&mut self, is_focused: bool) {
            if let Some(events) = self.adapter.update_window_focus_state(is_focused) {
                events.raise();
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use raw_window_handle::RawWindowHandle;

    pub(super) struct PlatformAccessibilityAdapterInner {
        adapter: accesskit_macos::Adapter,
    }

    impl PlatformAccessibilityAdapterInner {
        pub(super) fn new(
            window: &dyn Window,
            action_sender: Sender<ActionRequest>,
            _needs_full_tree: Arc<AtomicBool>,
        ) -> Option<Self> {
            let handle = window.window_handle().ok()?.as_raw();
            let RawWindowHandle::AppKit(handle) = handle else {
                return None;
            };
            let adapter = unsafe {
                accesskit_macos::Adapter::new(
                    handle.ns_view.as_ptr(),
                    window.has_focus(),
                    ChannelActionHandler {
                        sender: action_sender,
                    },
                )
            };
            Some(Self { adapter })
        }

        pub(super) fn update_if_active(&mut self, update_factory: impl FnOnce() -> TreeUpdate) {
            if let Some(events) = self.adapter.update_if_active(update_factory) {
                events.raise();
            }
        }

        pub(super) fn update_window_focus_state(&mut self, is_focused: bool) {
            if let Some(events) = self.adapter.update_view_focus_state(is_focused) {
                events.raise();
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use accesskit::{ActivationHandler, DeactivationHandler};

    pub(super) struct PlatformAccessibilityAdapterInner {
        adapter: accesskit_unix::Adapter,
    }

    struct DeferredActivationHandler;

    impl ActivationHandler for DeferredActivationHandler {
        fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
            None
        }
    }

    struct CacheInvalidatingDeactivationHandler {
        needs_full_tree: Arc<AtomicBool>,
    }

    impl DeactivationHandler for CacheInvalidatingDeactivationHandler {
        fn deactivate_accessibility(&mut self) {
            self.needs_full_tree.store(true, Ordering::Release);
        }
    }

    impl PlatformAccessibilityAdapterInner {
        pub(super) fn new(
            window: &dyn Window,
            action_sender: Sender<ActionRequest>,
            needs_full_tree: Arc<AtomicBool>,
        ) -> Option<Self> {
            let mut adapter = accesskit_unix::Adapter::new(
                DeferredActivationHandler,
                ChannelActionHandler {
                    sender: action_sender,
                },
                CacheInvalidatingDeactivationHandler { needs_full_tree },
            );
            adapter.update_window_focus_state(window.has_focus());
            Some(Self { adapter })
        }

        pub(super) fn update_if_active(&mut self, update_factory: impl FnOnce() -> TreeUpdate) {
            self.adapter.update_if_active(update_factory);
        }

        pub(super) fn update_window_focus_state(&mut self, is_focused: bool) {
            self.adapter.update_window_focus_state(is_focused);
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
mod platform {
    use super::*;

    pub(super) struct PlatformAccessibilityAdapterInner;

    impl PlatformAccessibilityAdapterInner {
        pub(super) fn new(
            _window: &dyn Window,
            _action_sender: Sender<ActionRequest>,
            _needs_full_tree: Arc<AtomicBool>,
        ) -> Option<Self> {
            None
        }

        pub(super) fn update_if_active(&mut self, _update_factory: impl FnOnce() -> TreeUpdate) {}

        pub(super) fn update_window_focus_state(&mut self, _is_focused: bool) {}
    }
}

use platform::PlatformAccessibilityAdapterInner;

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use accesskit::{Node, Role, Tree};

    use super::*;
    use crate::ui::theme::Density;
    use crate::ui::widget::Rect;

    fn key(invalidation_revision: u64, scroll_epoch: Option<u64>) -> TreeUpdateKey {
        TreeUpdateKey {
            invalidation_revision,
            scene_serial: 7,
            viewport: Rect::new(0.0, 0.0, 800.0, 600.0),
            theme_epoch: 2,
            style_sheet_version: 3,
            density: Density::Comfortable,
            reduced_motion: false,
            text_scale_bits: 1.0f32.to_bits(),
            accessibility_animation_epoch: 4,
            scroll_epoch,
            text_input_epoch: 7,
            external_portal_revision: 8,
        }
    }

    fn full_update(focus: NodeId) -> TreeUpdate {
        let child_id = NodeId(2);
        let mut root = Node::new(Role::Window);
        root.set_children(vec![child_id]);
        TreeUpdate {
            nodes: vec![(ROOT_NODE_ID, root), (child_id, Node::new(Role::Button))],
            tree: Some(Tree::new(ROOT_NODE_ID)),
            tree_id: TreeId::ROOT,
            focus,
        }
    }

    #[test]
    fn inactive_adapter_does_not_build_or_prime_cache() {
        let builds = Cell::new(0);
        let mut cache = AccessibilityUpdateCache::default();

        dispatch_cached_update(
            &mut cache,
            true,
            key(1, None),
            ROOT_NODE_ID,
            || {
                builds.set(builds.get() + 1);
                full_update(ROOT_NODE_ID)
            },
            |_update_factory| {},
        );

        assert_eq!(builds.get(), 0);
        assert!(cache.key.is_none());
        assert!(cache.included_nodes.is_empty());
    }

    #[test]
    fn stable_and_paint_only_frames_build_once_and_skip_platform_dispatch() {
        let builds = Cell::new(0);
        let dispatches = Cell::new(0);
        let mut cache = AccessibilityUpdateCache::default();
        let stable_key = key(1, None);

        dispatch_cached_update(
            &mut cache,
            true,
            stable_key,
            ROOT_NODE_ID,
            || {
                builds.set(builds.get() + 1);
                full_update(ROOT_NODE_ID)
            },
            |update_factory| {
                dispatches.set(dispatches.get() + 1);
                let _ = update_factory();
            },
        );
        // Raw hover and paint-only animation epochs are intentionally absent from TreeUpdateKey,
        // so 1024 such frames reach the cache as this same stable key.
        for _ in 0..1024 {
            dispatch_cached_update(
                &mut cache,
                false,
                stable_key,
                ROOT_NODE_ID,
                || {
                    builds.set(builds.get() + 1);
                    full_update(ROOT_NODE_ID)
                },
                |_update_factory| dispatches.set(dispatches.get() + 1),
            );
        }

        assert_eq!(builds.get(), 1);
        assert_eq!(dispatches.get(), 1);
    }

    #[test]
    fn non_scrollable_epoch_is_ignored_but_effective_scroll_rebuilds() {
        let builds = Cell::new(0);
        let dispatches = Cell::new(0);
        let mut cache = AccessibilityUpdateCache::default();
        let mut run = |update_key| {
            let force_full = cache.key.is_none();
            dispatch_cached_update(
                &mut cache,
                force_full,
                update_key,
                ROOT_NODE_ID,
                || {
                    builds.set(builds.get() + 1);
                    full_update(ROOT_NODE_ID)
                },
                |update_factory| {
                    dispatches.set(dispatches.get() + 1);
                    let _ = update_factory();
                },
            );
        };

        // Runtime normalizes every raw scroll epoch to `None` while no region can actually scroll.
        run(key(1, None));
        for _ in 0..1024 {
            run(key(1, None));
        }
        assert_eq!(builds.get(), 1);
        assert_eq!(dispatches.get(), 1);

        // Once a scrollable region exists, each effective offset epoch remains correctness-critical.
        run(key(1, Some(1)));
        run(key(1, Some(2)));
        assert_eq!(builds.get(), 3);
        assert_eq!(dispatches.get(), 3);
    }

    #[test]
    fn focus_only_update_has_no_nodes_and_does_not_rebuild() {
        let builds = Cell::new(0);
        let mut cache = AccessibilityUpdateCache::default();
        let stable_key = key(1, None);
        dispatch_cached_update(
            &mut cache,
            true,
            stable_key,
            ROOT_NODE_ID,
            || {
                builds.set(builds.get() + 1);
                full_update(ROOT_NODE_ID)
            },
            |update_factory| {
                let _ = update_factory();
            },
        );

        let mut focus_update = None;
        dispatch_cached_update(
            &mut cache,
            false,
            stable_key,
            NodeId(2),
            || {
                builds.set(builds.get() + 1);
                full_update(NodeId(2))
            },
            |update_factory| focus_update = Some(update_factory()),
        );

        let focus_update = focus_update.expect("focus change should dispatch an update");
        assert_eq!(builds.get(), 1);
        assert!(focus_update.nodes.is_empty());
        assert!(focus_update.tree.is_none());
        assert_eq!(focus_update.focus, NodeId(2));
    }

    #[test]
    fn focus_outside_cached_tree_is_normalized_to_root() {
        let mut cache = AccessibilityUpdateCache::default();
        let stable_key = key(1, None);
        dispatch_cached_update(
            &mut cache,
            true,
            stable_key,
            NodeId(2),
            || full_update(NodeId(2)),
            |update_factory| {
                let _ = update_factory();
            },
        );

        let mut update = None;
        dispatch_cached_update(
            &mut cache,
            false,
            stable_key,
            NodeId(99),
            || panic!("focus-only update must not rebuild the tree"),
            |update_factory| update = Some(update_factory()),
        );

        assert_eq!(
            update.expect("focus should return to root").focus,
            ROOT_NODE_ID
        );
    }

    #[test]
    fn structure_value_scroll_and_reactivation_force_full_rebuilds() {
        let builds = Cell::new(0);
        let mut cache = AccessibilityUpdateCache::default();
        let mut run = |force_full, update_key| {
            let mut was_full = false;
            dispatch_cached_update(
                &mut cache,
                force_full,
                update_key,
                ROOT_NODE_ID,
                || {
                    builds.set(builds.get() + 1);
                    full_update(ROOT_NODE_ID)
                },
                |update_factory| was_full = update_factory().tree.is_some(),
            );
            assert!(was_full);
        };

        run(true, key(1, Some(0)));
        // A reactive semantic value change advances the invalidation revision.
        run(false, key(2, Some(0)));
        // Direct retained-scene replacement assigns a new scene serial.
        let mut structure_key = key(2, Some(0));
        structure_key.scene_serial += 1;
        run(false, structure_key);
        // Scroll changes bounds and scroll range/value semantics.
        let mut scroll_key = structure_key;
        scroll_key.scroll_epoch = scroll_key.scroll_epoch.map(|epoch| epoch + 1);
        run(false, scroll_key);
        // A deactivated adapter must ignore the matching key and send a complete tree again.
        run(true, scroll_key);

        assert_eq!(builds.get(), 5);
    }

    #[test]
    fn bounds_overlay_and_virtual_key_changes_force_full_rebuilds() {
        let builds = Cell::new(0);
        let mut cache = AccessibilityUpdateCache::default();
        let mut run = |force_full, update_key| {
            dispatch_cached_update(
                &mut cache,
                force_full,
                update_key,
                ROOT_NODE_ID,
                || {
                    builds.set(builds.get() + 1);
                    full_update(ROOT_NODE_ID)
                },
                |update_factory| assert!(update_factory().tree.is_some()),
            );
        };

        let base = key(1, None);
        run(true, base);

        let mut bounds = base;
        bounds.viewport = Rect::new(0.0, 0.0, 1024.0, 768.0);
        run(false, bounds);

        let mut overlay = bounds;
        overlay.external_portal_revision += 1;
        run(false, overlay);

        let mut geometry_animation = overlay;
        geometry_animation.accessibility_animation_epoch += 1;
        run(false, geometry_animation);

        assert_eq!(builds.get(), 4);
    }
}
