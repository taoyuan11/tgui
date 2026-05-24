use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Range;
use std::sync::Arc;

use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Insets, Overflow, Value};
use crate::ui::unit::Dp;

use super::background::{BackgroundBrush, BackgroundImage};
use super::common::{
    CursorStyle, FocusScopeOptions, InteractionHandlers, LifecycleEventHandlers,
    MediaEventHandlers, Point, VisualStyle, WidgetId, WidgetKey, WidgetKind,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;
use super::style::StyleResolver;
use super::ContainerStyle;

pub trait ItemSource<T>: Send + Sync + 'static {
    fn len(&self) -> usize;
    fn item(&self, index: usize) -> Option<T>;

    fn key(&self, _index: usize) -> Option<WidgetKey> {
        None
    }
}

impl<T> ItemSource<T> for Vec<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn len(&self) -> usize {
        self.len()
    }

    fn item(&self, index: usize) -> Option<T> {
        self.get(index).cloned()
    }
}

impl<T> ItemSource<T> for Arc<[T]>
where
    T: Clone + Send + Sync + 'static,
{
    fn len(&self) -> usize {
        self.as_ref().len()
    }

    fn item(&self, index: usize) -> Option<T> {
        self.as_ref().get(index).cloned()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualDirection {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VirtualArrangement {
    Linear(VirtualDirection),
    Grid {
        direction: VirtualDirection,
        lanes: usize,
    },
}

impl VirtualArrangement {
    pub(crate) fn direction(self) -> VirtualDirection {
        match self {
            Self::Linear(direction) => direction,
            Self::Grid { direction, .. } => direction,
        }
    }

    pub(crate) fn lanes(self) -> usize {
        match self {
            Self::Linear(_) => 1,
            Self::Grid { lanes, .. } => lanes.max(1),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemLayout {
    Fixed {
        item_extent: Dp,
        spacing: Dp,
        overscan: usize,
    },
    Estimated {
        estimate: Dp,
        spacing: Dp,
        overscan: usize,
    },
    Measured {
        estimate: Dp,
        spacing: Dp,
        overscan: usize,
    },
}

impl ItemLayout {
    pub(crate) fn estimate(self) -> Dp {
        match self {
            Self::Fixed { item_extent, .. } => item_extent,
            Self::Estimated { estimate, .. } => estimate,
            Self::Measured { estimate, .. } => estimate,
        }
    }

    pub(crate) fn spacing(self) -> Dp {
        match self {
            Self::Fixed { spacing, .. } => spacing,
            Self::Estimated { spacing, .. } => spacing,
            Self::Measured { spacing, .. } => spacing,
        }
    }

    pub(crate) fn overscan(self) -> usize {
        match self {
            Self::Fixed { overscan, .. } => overscan,
            Self::Estimated { overscan, .. } => overscan,
            Self::Measured { overscan, .. } => overscan,
        }
    }

    pub(crate) fn is_measured(self) -> bool {
        matches!(self, Self::Measured { .. })
    }

    pub(crate) fn is_fixed(self) -> bool {
        matches!(self, Self::Fixed { .. })
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct VirtualViewportHint {
    pub(crate) width: Dp,
    pub(crate) height: Dp,
}

#[derive(Clone)]
pub(crate) struct VirtualRuntimeState {
    pub(crate) fallback_viewport_hint: VirtualViewportHint,
    pub(crate) viewport_hint: Option<VirtualViewportHint>,
    pub(crate) scroll_offset: Point,
    pub(crate) measured_extents: HashMap<usize, Dp>,
    pub(crate) widget_ids_by_key: HashMap<WidgetKey, WidgetId>,
    pub(crate) bootstrap: bool,
}

#[derive(Clone, Default)]
pub(crate) struct VirtualCacheState {
    pub(crate) viewport_hint: Option<VirtualViewportHint>,
    pub(crate) measured_extents: HashMap<usize, Dp>,
    pub(crate) widget_ids_by_key: HashMap<WidgetKey, WidgetId>,
}

impl Default for VirtualRuntimeState {
    fn default() -> Self {
        Self {
            fallback_viewport_hint: VirtualViewportHint::default(),
            viewport_hint: None,
            scroll_offset: Point::ZERO,
            measured_extents: HashMap::new(),
            widget_ids_by_key: HashMap::new(),
            bootstrap: true,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct VirtualResolvedItemMeta {
    pub(crate) item_index: usize,
    pub(crate) stripe_index: usize,
    pub(crate) lane_index: usize,
    pub(crate) main_extent: Dp,
    pub(crate) main_offset: Dp,
    pub(crate) cross_offset: Dp,
    pub(crate) cross_extent: Dp,
}

#[derive(Clone, Debug)]
pub(crate) struct VirtualItemPlacement {
    pub(crate) item_index: usize,
    pub(crate) stripe_index: usize,
    pub(crate) lane_index: usize,
    pub(crate) main_extent: Dp,
    pub(crate) main_offset: Dp,
    pub(crate) cross_offset: Dp,
    pub(crate) cross_extent: Dp,
}

#[derive(Clone, Debug)]
pub(crate) struct VirtualWindowPlan {
    pub(crate) total_items: usize,
    pub(crate) visible_range: Range<usize>,
    pub(crate) placements: Vec<VirtualItemPlacement>,
    pub(crate) total_main_extent: Dp,
    pub(crate) viewport_hint: VirtualViewportHint,
    pub(crate) bootstrap: bool,
}

#[derive(Clone)]
pub(crate) struct VirtualSceneStateUpdate {
    pub(crate) widget_id: WidgetId,
    pub(crate) viewport_hint: VirtualViewportHint,
    pub(crate) measured_extents: Vec<(usize, Dp)>,
    pub(crate) widget_ids_by_key: Vec<(WidgetKey, WidgetId)>,
    pub(crate) invalidate_layout: bool,
}

pub(crate) fn apply_virtual_runtime_state_to_element<VM>(
    element: &mut Element<VM>,
    scroll_offsets: &HashMap<WidgetId, Point>,
    virtual_states: &HashMap<WidgetId, VirtualCacheState>,
    fallback_viewport_hint: VirtualViewportHint,
) {
    match &mut element.kind {
        WidgetKind::Container { children, .. } => {
            for child_source in children {
                if let super::common::ChildSource::Static(children) = child_source {
                    for child in children {
                        apply_virtual_runtime_state_to_element(
                            child,
                            scroll_offsets,
                            virtual_states,
                            fallback_viewport_hint.clone(),
                        );
                    }
                }
            }
        }
        WidgetKind::Virtual {
            runtime_state,
            arrangement,
            ..
        } => {
            if let Some(cache) = virtual_states.get(&element.id) {
                runtime_state.viewport_hint = cache.viewport_hint.clone();
                runtime_state.measured_extents = cache.measured_extents.clone();
                runtime_state.widget_ids_by_key = cache.widget_ids_by_key.clone();
                runtime_state.bootstrap = runtime_state.viewport_hint.is_none();
            } else {
                runtime_state.viewport_hint = None;
                runtime_state.measured_extents.clear();
                runtime_state.widget_ids_by_key.clear();
                runtime_state.bootstrap = true;
            }
            runtime_state.fallback_viewport_hint = fallback_viewport_hint.clone();
            runtime_state.scroll_offset = scroll_offsets
                .get(&element.id)
                .copied()
                .unwrap_or(Point::ZERO);
            if matches!(arrangement.direction(), VirtualDirection::Vertical) {
                runtime_state.scroll_offset.x = Dp::ZERO;
            } else {
                runtime_state.scroll_offset.y = Dp::ZERO;
            }
        }
        _ => {}
    }
}

pub(crate) fn resolve_virtual_window_plan(
    arrangement: VirtualArrangement,
    item_layout: ItemLayout,
    runtime_state: &VirtualRuntimeState,
    total_items: usize,
    fallback_viewport_hint: VirtualViewportHint,
) -> VirtualWindowPlan {
    let viewport_hint = runtime_state
        .viewport_hint
        .clone()
        .unwrap_or(fallback_viewport_hint);
    let viewport_main = match arrangement.direction() {
        VirtualDirection::Vertical => viewport_hint.height.max(Dp::ZERO),
        VirtualDirection::Horizontal => viewport_hint.width.max(Dp::ZERO),
    };
    let viewport_cross = match arrangement.direction() {
        VirtualDirection::Vertical => viewport_hint.width.max(Dp::ZERO),
        VirtualDirection::Horizontal => viewport_hint.height.max(Dp::ZERO),
    };
    let scroll_main = match arrangement.direction() {
        VirtualDirection::Vertical => runtime_state.scroll_offset.y.max(Dp::ZERO),
        VirtualDirection::Horizontal => runtime_state.scroll_offset.x.max(Dp::ZERO),
    };
    let lanes = arrangement.lanes().max(1);
    let stripe_count = total_items.div_ceil(lanes);
    let spacing = item_layout.spacing().max(Dp::ZERO);
    let overscan = item_layout.overscan();
    let bootstrap = runtime_state.bootstrap;

    let item_main_extent = |item_index: usize| -> Dp {
        if item_layout.is_fixed() {
            return item_layout.estimate().max(Dp::ZERO);
        }
        if item_layout.is_measured() {
            runtime_state
                .measured_extents
                .get(&item_index)
                .copied()
                .unwrap_or(item_layout.estimate())
                .max(Dp::ZERO)
        } else {
            item_layout.estimate().max(Dp::ZERO)
        }
    };

    let mut stripe_offsets = Vec::with_capacity(stripe_count);
    let mut total_main_extent = Dp::ZERO;
    for stripe_index in 0..stripe_count {
        stripe_offsets.push(total_main_extent);
        let start = stripe_index * lanes;
        let end = ((stripe_index + 1) * lanes).min(total_items);
        let mut stripe_extent = item_layout.estimate().max(Dp::ZERO);
        for item_index in start..end {
            stripe_extent = stripe_extent.max(item_main_extent(item_index));
        }
        total_main_extent += stripe_extent;
        if stripe_index + 1 < stripe_count {
            total_main_extent += spacing;
        }
    }

    let overscan_span = viewport_main.max(item_layout.estimate().max(Dp::ZERO));
    let start_target = if bootstrap {
        (scroll_main - overscan_span).max(Dp::ZERO)
    } else {
        scroll_main
    };
    let end_target = scroll_main + viewport_main + overscan_span * (overscan as f32 + 1.0);

    let mut first_stripe = 0usize;
    while first_stripe + 1 < stripe_count {
        let start = first_stripe * lanes;
        let end = ((first_stripe + 1) * lanes).min(total_items);
        let mut extent = item_layout.estimate().max(Dp::ZERO);
        for item_index in start..end {
            extent = extent.max(item_main_extent(item_index));
        }
        let end = stripe_offsets[first_stripe] + extent + spacing;
        if end > start_target {
            break;
        }
        first_stripe += 1;
    }

    let mut last_stripe = first_stripe;
    while last_stripe + 1 < stripe_count {
        let start = stripe_offsets[last_stripe];
        if start > end_target {
            break;
        }
        last_stripe += 1;
    }
    if stripe_count == 0 {
        last_stripe = 0;
    } else if last_stripe >= stripe_count {
        last_stripe = stripe_count - 1;
    }

    let visible_start = (first_stripe.saturating_sub(overscan)).saturating_mul(lanes);
    let visible_end = if stripe_count == 0 {
        0
    } else {
        ((last_stripe + 1 + overscan).min(stripe_count) * lanes).min(total_items)
    };

    let lane_extent = if lanes == 0 {
        Dp::ZERO
    } else {
        let spacing_total = spacing * (lanes.saturating_sub(1) as f32);
        ((viewport_cross - spacing_total).max(0.0)) / lanes as f32
    };
    let mut placements = Vec::with_capacity(visible_end.saturating_sub(visible_start));
    for item_index in visible_start..visible_end {
        let stripe_index = item_index / lanes;
        let lane_index = item_index % lanes;
        placements.push(VirtualItemPlacement {
            item_index,
            stripe_index,
            lane_index,
            main_extent: item_main_extent(item_index),
            main_offset: stripe_offsets
                .get(stripe_index)
                .copied()
                .unwrap_or(Dp::ZERO),
            cross_offset: (lane_extent + spacing) * lane_index as f32,
            cross_extent: lane_extent,
        });
    }

    VirtualWindowPlan {
        total_items,
        visible_range: visible_start..visible_end,
        placements,
        total_main_extent,
        viewport_hint,
        bootstrap,
    }
}

pub(crate) struct ErasedVirtualItemSource<VM> {
    len_fn: Arc<dyn Fn() -> usize + Send + Sync>,
    key_fn: Arc<dyn Fn(usize) -> Option<WidgetKey> + Send + Sync>,
    build_fn: Arc<dyn Fn(usize) -> Option<Element<VM>> + Send + Sync>,
}

impl<VM> Clone for ErasedVirtualItemSource<VM> {
    fn clone(&self) -> Self {
        Self {
            len_fn: self.len_fn.clone(),
            key_fn: self.key_fn.clone(),
            build_fn: self.build_fn.clone(),
        }
    }
}

impl<VM: 'static> ErasedVirtualItemSource<VM> {
    pub(crate) fn new<T, S>(
        source: Arc<S>,
        render: Arc<dyn Fn(usize, &T) -> Element<VM> + Send + Sync>,
    ) -> Self
    where
        T: Send + Sync + 'static,
        S: ItemSource<T>,
    {
        Self {
            len_fn: {
                let source = source.clone();
                Arc::new(move || source.len())
            },
            key_fn: {
                let source = source.clone();
                Arc::new(move |index| source.key(index))
            },
            build_fn: Arc::new(move |index| {
                let item = source.item(index)?;
                Some(render(index, &item))
            }),
        }
    }

    pub(crate) fn len(&self) -> usize {
        (self.len_fn)()
    }

    pub(crate) fn key(&self, index: usize) -> Option<WidgetKey> {
        (self.key_fn)(index)
    }

    pub(crate) fn build(&self, index: usize) -> Option<Element<VM>> {
        (self.build_fn)(index)
    }

    pub(crate) fn scope<RootVm: 'static>(
        &self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> ErasedVirtualItemSource<RootVm>
    where
        VM: 'static,
    {
        let len_fn = self.len_fn.clone();
        let key_fn = self.key_fn.clone();
        let build_fn = self.build_fn.clone();
        ErasedVirtualItemSource {
            len_fn,
            key_fn,
            build_fn: Arc::new(move |index| {
                build_fn(index).map(|element| element.scope_with_selector(selector.clone()))
            }),
        }
    }
}

pub struct VirtualViewport<T, VM> {
    element: Element<VM>,
    marker: PhantomData<fn() -> T>,
}

impl<T, VM: 'static> VirtualViewport<T, VM> {
    pub fn new<S>(
        source: S,
        arrangement: VirtualArrangement,
        item_layout: ItemLayout,
        render: impl Fn(usize, &T) -> Element<VM> + Send + Sync + 'static,
    ) -> Self
    where
        T: Send + Sync + 'static,
        S: ItemSource<T>,
    {
        let source = Arc::new(source);
        let render = Arc::new(render);
        let (overflow_x, overflow_y) = match arrangement.direction() {
            VirtualDirection::Vertical => (Overflow::Hidden, Overflow::Scroll),
            VirtualDirection::Horizontal => (Overflow::Scroll, Overflow::Hidden),
        };

        Self {
            element: Element {
                id: WidgetId::next(),
                key: None,
                layout: crate::ui::layout::LayoutStyle::default(),
                focus: Default::default(),
                visual: VisualStyle::default(),
                interactions: InteractionHandlers::default(),
                lifecycle_events: LifecycleEventHandlers::default(),
                media_events: MediaEventHandlers::default(),
                background: None,
                tooltip: None,
                kind: WidgetKind::Virtual {
                    arrangement,
                    item_layout,
                    source: ErasedVirtualItemSource::new::<T, S>(source, render),
                    overflow_x,
                    overflow_y,
                    style: None,
                    runtime_state: VirtualRuntimeState::default(),
                },
            },
            marker: PhantomData,
        }
    }

    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.element.key = Some(key.into());
        self
    }

    pub fn style(
        mut self,
        resolver: impl Fn(ResolvedThemeMode) -> ContainerStyle + Send + Sync + 'static,
    ) -> Self {
        if let WidgetKind::Virtual { style, .. } = &mut self.element.kind {
            *style = Some(StyleResolver::new(resolver));
        }
        self
    }

    pub fn on_click(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_click = Some(command);
        self
    }

    pub fn on_double_click(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_double_click = Some(command);
        self
    }

    pub fn on_mouse_enter(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_mouse_enter = Some(command);
        self
    }

    pub fn on_mouse_leave(mut self, command: Command<VM>) -> Self {
        self.element.interactions.on_mouse_leave = Some(command);
        self
    }

    pub fn on_mouse_move(mut self, command: ValueCommand<VM, Point>) -> Self {
        self.element.interactions.on_mouse_move = Some(command);
        self
    }

    pub fn on_mount(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_mount = Some(command);
        self
    }

    pub fn on_unmount(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_unmount = Some(command);
        self
    }

    pub fn on_update(mut self, command: Command<VM>) -> Self {
        self.element.lifecycle_events.on_update = Some(command);
        self
    }

    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.element.interactions.cursor_style = Some(cursor.into());
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.element.focus.focusable = Some(focusable);
        self
    }

    pub fn tab_index(mut self, tab_index: i32) -> Self {
        self.element.focus.tab_index = Some(tab_index);
        self
    }

    pub fn focus_scope(mut self, options: FocusScopeOptions) -> Self {
        self.element.focus.scope = Some(options);
        self
    }

    pub fn overflow(mut self, overflow: Overflow) -> Self {
        if let WidgetKind::Virtual {
            overflow_x,
            overflow_y,
            ..
        } = &mut self.element.kind
        {
            *overflow_x = overflow;
            *overflow_y = overflow;
        }
        self
    }

    pub fn overflow_x(mut self, overflow: Overflow) -> Self {
        if let WidgetKind::Virtual { overflow_x, .. } = &mut self.element.kind {
            *overflow_x = overflow;
        }
        self
    }

    pub fn overflow_y(mut self, overflow: Overflow) -> Self {
        if let WidgetKind::Virtual { overflow_y, .. } = &mut self.element.kind {
            *overflow_y = overflow;
        }
        self
    }

    pub fn opacity(mut self, opacity: impl Into<Value<f32>>) -> Self {
        self.element.visual.opacity = opacity.into();
        self
    }

    pub fn offset(mut self, offset: impl Into<Value<Point>>) -> Self {
        self.element.visual.offset = offset.into();
        self
    }

    pub fn border_radius(mut self, radius: impl Into<Value<Dp>>) -> Self {
        self.element.visual.border_radius = Some(radius.into());
        self
    }

    pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
        set_layout_lengths(&mut self.element.layout, width, height);
        self
    }

    pub fn width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.element.layout.width, width);
        self
    }

    pub fn height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.element.layout.height, height);
        self
    }

    pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.element.layout.min_width, width);
        self
    }

    pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.element.layout.min_height, height);
        self
    }

    pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.element.layout.max_width, width);
        self
    }

    pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.element.layout.max_height, height);
        self
    }

    pub fn aspect_ratio(mut self, aspect_ratio: impl Into<Value<f32>>) -> Self {
        self.element.layout.aspect_ratio = Some(aspect_ratio.into());
        self
    }

    pub fn margin(mut self, margin: impl Into<Value<Insets>>) -> Self {
        self.element.layout.margin = margin.into();
        self
    }

    pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
        self.element.layout.grow = grow.into();
        self
    }

    pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
        self.element.layout.shrink = shrink.into();
        self
    }

    pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
        self.element.layout.basis = Some(basis.into_length_value());
        self
    }

    pub fn align_self(mut self, align: Align) -> Self {
        self.element.layout.align_self = Some(align);
        self
    }

    pub fn justify_self(mut self, align: Align) -> Self {
        self.element.layout.justify_self = Some(align);
        self
    }

    pub fn position_absolute(mut self) -> Self {
        self.element.layout.position_type = crate::ui::layout::PositionType::Absolute;
        self
    }

    pub fn left(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.element.layout.left, value);
        self
    }

    pub fn top(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.element.layout.top, value);
        self
    }

    pub fn right(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.element.layout.right, value);
        self
    }

    pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
        set_layout_inset(&mut self.element.layout.bottom, value);
        self
    }

    pub fn inset(
        self,
        left: impl IntoLengthValue,
        top: impl IntoLengthValue,
        right: impl IntoLengthValue,
        bottom: impl IntoLengthValue,
    ) -> Self {
        self.left(left).top(top).right(right).bottom(bottom)
    }

    pub fn background(mut self, color: impl Into<Value<crate::foundation::color::Color>>) -> Self {
        self.element.background = Some(color.into());
        self
    }

    pub fn background_brush(mut self, brush: impl Into<Value<BackgroundBrush>>) -> Self {
        self.element.visual.background_brush = Some(brush.into());
        self
    }

    pub fn background_image(mut self, image: impl Into<Value<BackgroundImage>>) -> Self {
        self.element.visual.background_image = Some(image.into());
        self
    }

    pub fn background_blur(mut self, blur: impl Into<Value<Dp>>) -> Self {
        self.element.visual.background_blur = blur.into();
        self
    }
}

impl<T, VM> From<VirtualViewport<T, VM>> for Element<VM> {
    fn from(value: VirtualViewport<T, VM>) -> Self {
        value.element
    }
}
