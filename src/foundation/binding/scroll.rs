use std::sync::Arc;

use super::dependency::{record_dependency_read, DependencyId};
use super::invalidation::InvalidationSignal;
use super::{Signal, State, ViewModelContext};
use crate::ui::widget::{Point, WidgetId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollRequestMode {
    Immediate,
    Smooth,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollRequest {
    pub offset: Point,
    pub mode: ScrollRequestMode,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ScrollViewController {
    inner: Arc<ScrollViewControllerInner>,
}

struct ScrollViewControllerInner {
    offset: State<Point>,
    request: State<Option<ScrollRequest>>,
    widget_id: Arc<parking_lot::Mutex<Option<WidgetId>>>,
    dependency: DependencyId,
    invalidation: InvalidationSignal,
}

impl PartialEq for ScrollViewControllerInner {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for ScrollViewControllerInner {}

impl std::fmt::Debug for ScrollViewController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrollViewController")
            .field("offset", &self.scroll_offset())
            .field("widget_id", &self.widget_id())
            .finish()
    }
}

impl ScrollViewController {
    pub fn new(ctx: &ViewModelContext) -> Self {
        Self::from_invalidation(ctx.invalidation().clone())
    }

    pub(crate) fn new_legacy() -> Self {
        Self::from_invalidation(InvalidationSignal::new())
    }

    fn from_invalidation(invalidation: InvalidationSignal) -> Self {
        Self {
            inner: Arc::new(ScrollViewControllerInner {
                offset: State::new(Point::ZERO, invalidation.clone()),
                request: State::new(None, invalidation.clone()),
                widget_id: Arc::new(parking_lot::Mutex::new(None)),
                dependency: DependencyId::next(),
                invalidation,
            }),
        }
    }

    pub fn offset(&self) -> Signal<Point> {
        self.inner.offset.signal()
    }

    pub fn scroll_offset(&self) -> Point {
        self.inner.offset.get()
    }

    pub fn scroll_to(&self, offset: Point) {
        self.enqueue_request(offset, ScrollRequestMode::Smooth);
    }

    pub fn jump_to(&self, offset: Point) {
        self.enqueue_request(offset, ScrollRequestMode::Immediate);
    }

    pub fn scroll_by(&self, delta: Point) {
        let current = self.inner.offset.get();
        self.enqueue_request(
            Point::new(current.x + delta.x, current.y + delta.y),
            ScrollRequestMode::Smooth,
        );
    }

    pub(crate) fn bind_widget(&self, widget_id: WidgetId) {
        let mut bound = self.inner.widget_id.lock();
        if *bound != Some(widget_id) {
            *bound = Some(widget_id);
            self.inner
                .invalidation
                .mark_dependency_dirty(self.inner.dependency);
        }
    }

    pub(crate) fn widget_id(&self) -> Option<WidgetId> {
        record_dependency_read(Some(self.inner.dependency));
        *self.inner.widget_id.lock()
    }

    pub(crate) fn sync_offset(&self, offset: Point) {
        self.inner.offset.set(offset);
    }

    pub(crate) fn take_request(&self) -> Option<ScrollRequest> {
        self.inner.request.read(|request| *request)
    }

    pub(crate) fn clear_request(&self, request: ScrollRequest) {
        self.inner.request.update(|slot| {
            if *slot == Some(request) {
                *slot = None;
            }
        });
    }

    fn enqueue_request(&self, offset: Point, mode: ScrollRequestMode) {
        self.inner.request.set(Some(ScrollRequest {
            offset: Point::new(offset.x.max(0.0), offset.y.max(0.0)),
            mode,
        }));
    }
}
