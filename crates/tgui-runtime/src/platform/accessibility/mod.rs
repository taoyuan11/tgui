use accesskit::{ActionHandler, ActionRequest, TreeUpdate};
use crossbeam_channel::Sender;

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
}

impl PlatformAccessibilityAdapter {
    pub(crate) fn new(window: &dyn Window, action_sender: Sender<ActionRequest>) -> Option<Self> {
        PlatformAccessibilityAdapterInner::new(window, action_sender).map(|inner| Self { inner })
    }

    pub(crate) fn update_if_active(&mut self, update: TreeUpdate) {
        self.inner.update_if_active(update);
    }

    pub(crate) fn update_window_focus_state(&mut self, is_focused: bool) {
        self.inner.update_window_focus_state(is_focused);
    }
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

        pub(super) fn update_if_active(&mut self, update: TreeUpdate) {
            if let Some(events) = self.adapter.update_if_active(|| update) {
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

        pub(super) fn update_if_active(&mut self, update: TreeUpdate) {
            if let Some(events) = self.adapter.update_if_active(|| update) {
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

    struct NoopDeactivationHandler;

    impl DeactivationHandler for NoopDeactivationHandler {
        fn deactivate_accessibility(&mut self) {}
    }

    impl PlatformAccessibilityAdapterInner {
        pub(super) fn new(
            window: &dyn Window,
            action_sender: Sender<ActionRequest>,
        ) -> Option<Self> {
            let mut adapter = accesskit_unix::Adapter::new(
                DeferredActivationHandler,
                ChannelActionHandler {
                    sender: action_sender,
                },
                NoopDeactivationHandler,
            );
            adapter.update_window_focus_state(window.has_focus());
            Some(Self { adapter })
        }

        pub(super) fn update_if_active(&mut self, update: TreeUpdate) {
            self.adapter.update_if_active(|| update);
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
        ) -> Option<Self> {
            None
        }

        pub(super) fn update_if_active(&mut self, _update: TreeUpdate) {}

        pub(super) fn update_window_focus_state(&mut self, _is_focused: bool) {}
    }
}

use platform::PlatformAccessibilityAdapterInner;
