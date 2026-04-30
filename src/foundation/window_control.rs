use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowRequest {
    Drag,
    DragResize(WindowResizeDirection),
    Minimize,
    Maximize,
    Restore,
    ToggleMaximize,
    Close,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WindowRequestQueue {
    requests: Arc<Mutex<VecDeque<WindowRequest>>>,
}

impl WindowRequestQueue {
    pub(crate) fn push(&self, request: WindowRequest) {
        self.requests
            .lock()
            .expect("window request queue lock poisoned")
            .push_back(request);
    }

    pub(crate) fn drain(&self) -> Vec<WindowRequest> {
        self.requests
            .lock()
            .expect("window request queue lock poisoned")
            .drain(..)
            .collect()
    }
}

type IsMaximized = dyn Fn() -> bool + Send + Sync;

/// Direction used when starting a native drag-resize operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowResizeDirection {
    East,
    North,
    NorthEast,
    NorthWest,
    South,
    SouthEast,
    SouthWest,
    West,
}

/// Runtime-scoped controls for the native window that dispatched a command.
#[derive(Clone)]
pub struct WindowControl {
    requests: WindowRequestQueue,
    is_maximized: Arc<IsMaximized>,
}

impl fmt::Debug for WindowControl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WindowControl").finish_non_exhaustive()
    }
}

impl Default for WindowControl {
    fn default() -> Self {
        Self::new(WindowRequestQueue::default(), || false)
    }
}

impl WindowControl {
    pub(crate) fn new(
        requests: WindowRequestQueue,
        is_maximized: impl Fn() -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            requests,
            is_maximized: Arc::new(is_maximized),
        }
    }

    /// Starts a platform-native drag operation for the current window.
    pub fn drag_window(&self) {
        self.requests.push(WindowRequest::Drag);
    }

    /// Starts a platform-native resize drag from the requested edge or corner.
    pub fn drag_resize_window(&self, direction: WindowResizeDirection) {
        self.requests.push(WindowRequest::DragResize(direction));
    }

    /// Requests minimizing the current window.
    pub fn minimize(&self) {
        self.requests.push(WindowRequest::Minimize);
    }

    /// Requests maximizing the current window.
    pub fn maximize(&self) {
        self.requests.push(WindowRequest::Maximize);
    }

    /// Requests restoring the current window from the maximized state.
    pub fn restore(&self) {
        self.requests.push(WindowRequest::Restore);
    }

    /// Requests toggling the current window between maximized and restored.
    pub fn toggle_maximize(&self) {
        self.requests.push(WindowRequest::ToggleMaximize);
    }

    /// Requests closing the current window.
    pub fn close(&self) {
        self.requests.push(WindowRequest::Close);
    }

    /// Returns whether the current window is maximized.
    pub fn is_maximized(&self) -> bool {
        (self.is_maximized)()
    }
}

#[cfg(test)]
mod tests {
    use super::{WindowControl, WindowRequest, WindowRequestQueue, WindowResizeDirection};

    #[test]
    fn window_control_methods_enqueue_requests() {
        let queue = WindowRequestQueue::default();
        let control = WindowControl::new(queue.clone(), || true);

        control.drag_window();
        control.drag_resize_window(WindowResizeDirection::SouthEast);
        control.minimize();
        control.maximize();
        control.restore();
        control.toggle_maximize();
        control.close();

        assert_eq!(
            queue.drain(),
            vec![
                WindowRequest::Drag,
                WindowRequest::DragResize(WindowResizeDirection::SouthEast),
                WindowRequest::Minimize,
                WindowRequest::Maximize,
                WindowRequest::Restore,
                WindowRequest::ToggleMaximize,
                WindowRequest::Close,
            ]
        );
        assert!(control.is_maximized());
    }
}
