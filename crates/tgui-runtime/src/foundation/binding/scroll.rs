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
    request_target: parking_lot::Mutex<Option<Point>>,
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

    fn from_invalidation(invalidation: InvalidationSignal) -> Self {
        Self {
            inner: Arc::new(ScrollViewControllerInner {
                offset: State::new(Point::ZERO, invalidation.clone()),
                request: State::new(None, invalidation.clone()),
                request_target: parking_lot::Mutex::new(None),
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
        let offset = {
            let mut request_target = self.inner.request_target.lock();
            let current = request_target.unwrap_or_else(|| self.inner.offset.get());
            let offset =
                normalized_scroll_offset(Point::new(current.x + delta.x, current.y + delta.y));
            *request_target = Some(offset);
            offset
        };
        self.inner.request.set(Some(ScrollRequest {
            offset,
            mode: ScrollRequestMode::Smooth,
        }));
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

    pub(crate) fn sync_offset(&self, offset: Point, smooth_target: Option<Point>) {
        self.inner.offset.set(offset);
        if self.inner.request.read(Option::is_none) {
            *self.inner.request_target.lock() = smooth_target;
        }
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
        let offset = normalized_scroll_offset(offset);
        *self.inner.request_target.lock() = Some(offset);
        self.inner.request.set(Some(ScrollRequest { offset, mode }));
    }
}

fn normalized_scroll_offset(offset: Point) -> Point {
    Point::new(offset.x.max(0.0), offset.y.max(0.0))
}
