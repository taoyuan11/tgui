use crate::core::{Color, ElementId, Point, PropertyId, Result, WidgetKey};
use crate::event::EventHandler;
use crate::layout::{LayoutBoundaries, LayoutStyle, MeasureSpec};
use crate::state::{Signal, State};
use std::any::TypeId;
use std::fmt;
use std::sync::Arc;

/// Well-known presentation property applied to every retained render node.
pub const OPACITY: PropertyId = PropertyId::new(u64::MAX);
/// Well-known logical-pixel width override consumed by animated layout.
pub const LAYOUT_WIDTH: PropertyId = PropertyId::new(u64::MAX - 1);
/// Well-known logical-pixel height override consumed by animated layout.
pub const LAYOUT_HEIGHT: PropertyId = PropertyId::new(u64::MAX - 2);

/// Observable subsystem impact declared for one Widget property.
///
/// Missing metadata uses [`PropertyImpact::ALL`] as the safe fallback. Layout
/// implies paint, hit-test, and semantics when translated into Dirty flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropertyImpact(u8);

impl PropertyImpact {
    const LAYOUT_BIT: u8 = 1 << 0;
    const PAINT_BIT: u8 = 1 << 1;
    const HIT_TEST_BIT: u8 = 1 << 2;
    const SEMANTICS_BIT: u8 = 1 << 3;
    const RESOURCE_BIT: u8 = 1 << 4;

    pub const NONE: Self = Self(0);
    pub const LAYOUT: Self = Self(Self::LAYOUT_BIT);
    pub const PAINT: Self = Self(Self::PAINT_BIT);
    pub const HIT_TEST: Self = Self(Self::HIT_TEST_BIT);
    pub const SEMANTICS: Self = Self(Self::SEMANTICS_BIT);
    pub const RESOURCE: Self = Self(Self::RESOURCE_BIT);
    pub const ALL: Self = Self(
        Self::LAYOUT_BIT
            | Self::PAINT_BIT
            | Self::HIT_TEST_BIT
            | Self::SEMANTICS_BIT
            | Self::RESOURCE_BIT,
    );

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl Default for PropertyImpact {
    fn default() -> Self {
        Self::ALL
    }
}

/// Stable widget implementation identity.
///
/// Rust's unforgeable [`TypeId`] is the in-process identity used by
/// reconciliation. The type name is retained only for human-readable
/// diagnostics and is never used to decide Element identity.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WidgetType {
    identity: TypeId,
    name: &'static str,
}

impl WidgetType {
    /// Returns the unforgeable runtime identity of a concrete widget type.
    pub fn of<T: 'static>() -> Self {
        Self {
            identity: TypeId::of::<T>(),
            name: std::any::type_name::<T>(),
        }
    }

    pub fn name(&self) -> &str {
        self.name
    }
}

impl fmt::Debug for WidgetType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("WidgetType")
            .field(&self.name)
            .finish()
    }
}

/// Compact, comparable values used by immutable widget declarations.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum PropertyValue {
    Bool(bool),
    I64(i64),
    U64(u64),
    F32(f32),
    Text(Arc<str>),
    Color(Color),
}

impl From<bool> for PropertyValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for PropertyValue {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<u64> for PropertyValue {
    fn from(value: u64) -> Self {
        Self::U64(value)
    }
}

impl From<f32> for PropertyValue {
    fn from(value: f32) -> Self {
        Self::F32(value)
    }
}

impl From<String> for PropertyValue {
    fn from(value: String) -> Self {
        Self::Text(value.into())
    }
}

impl From<&str> for PropertyValue {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}

impl From<Arc<str>> for PropertyValue {
    fn from(value: Arc<str>) -> Self {
        Self::Text(value)
    }
}

impl From<Color> for PropertyValue {
    fn from(value: Color) -> Self {
        Self::Color(value)
    }
}

/// Mount/update/unmount notification for cold-path lifecycle integration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleEvent {
    Mounted(ElementId),
    Updated(ElementId),
    Unmounted(ElementId),
}

/// Explicitly identified lifecycle callback.
///
/// Callback identity participates in declaration comparison, but never in
/// element identity. This avoids trying to compare closure captures.
#[derive(Clone)]
pub struct LifecycleCallback {
    callback_type: TypeId,
    revision: u64,
    callback: Arc<dyn Fn(LifecycleEvent) + 'static>,
}

impl LifecycleCallback {
    /// Creates a callback identified by its closure type and explicit revision.
    /// Increment `revision` when captures change observably across rebuilds.
    pub fn new<F>(revision: u64, callback: F) -> Self
    where
        F: Fn(LifecycleEvent) + 'static,
    {
        Self {
            callback_type: TypeId::of::<F>(),
            revision,
            callback: Arc::new(callback),
        }
    }

    pub const fn identity(&self) -> u64 {
        self.revision
    }

    pub(crate) fn invoke(&self, event: LifecycleEvent) {
        (self.callback)(event);
    }
}

impl fmt::Debug for LifecycleCallback {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LifecycleCallback")
            .field("callback_type", &self.callback_type)
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

impl PartialEq for LifecycleCallback {
    fn eq(&self, other: &Self) -> bool {
        self.callback_type == other.callback_type && self.revision == other.revision
    }
}

impl Eq for LifecycleCallback {}

/// One immutable widget declaration.
///
/// `key + widget_type` determines persistent identity. Properties, lifecycle
/// callback identity, and children determine whether a retained element is
/// updated; they never cause an otherwise matching element to receive a new ID.
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetNode {
    pub(crate) widget_type: WidgetType,
    pub(crate) key: Option<WidgetKey>,
    pub(crate) properties: Vec<(PropertyId, PropertyValue)>,
    pub(crate) property_impacts: Vec<(PropertyId, PropertyImpact)>,
    pub(crate) children: Vec<WidgetNode>,
    pub(crate) lifecycle: Option<LifecycleCallback>,
    pub(crate) event_handler: Option<EventHandler>,
    pub(crate) focusable: bool,
    pub(crate) enabled: bool,
    pub(crate) layout_style: LayoutStyle,
    pub(crate) measure: Option<MeasureSpec>,
    pub(crate) scroll_offset: Point,
    pub(crate) hit_test: bool,
    pub(crate) boundaries: LayoutBoundaries,
}

impl WidgetNode {
    pub fn new<T: 'static>() -> Self {
        Self::from_type(WidgetType::of::<T>())
    }

    pub fn from_type(widget_type: WidgetType) -> Self {
        Self {
            widget_type,
            key: None,
            properties: Vec::new(),
            property_impacts: Vec::new(),
            children: Vec::new(),
            lifecycle: None,
            event_handler: None,
            focusable: false,
            enabled: true,
            layout_style: LayoutStyle::default(),
            measure: None,
            scroll_offset: Point::ZERO,
            hit_test: true,
            boundaries: LayoutBoundaries::NONE,
        }
    }

    pub fn widget_type(&self) -> &WidgetType {
        &self.widget_type
    }

    pub fn key(&self) -> Option<&WidgetKey> {
        self.key.as_ref()
    }

    pub fn properties(&self) -> &[(PropertyId, PropertyValue)] {
        &self.properties
    }

    pub fn property(&self, id: PropertyId) -> Option<&PropertyValue> {
        self.properties
            .binary_search_by_key(&id, |(property, _)| *property)
            .ok()
            .map(|index| &self.properties[index].1)
    }

    pub fn property_impact(&self, id: PropertyId) -> PropertyImpact {
        self.property_impacts
            .binary_search_by_key(&id, |(property, _)| *property)
            .ok()
            .map_or(PropertyImpact::ALL, |index| self.property_impacts[index].1)
    }

    pub fn children(&self) -> &[WidgetNode] {
        &self.children
    }

    pub fn lifecycle(&self) -> Option<&LifecycleCallback> {
        self.lifecycle.as_ref()
    }

    pub fn event_handler(&self) -> Option<&EventHandler> {
        self.event_handler.as_ref()
    }

    pub const fn is_focusable(&self) -> bool {
        self.focusable
    }

    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub const fn layout_style(&self) -> &LayoutStyle {
        &self.layout_style
    }

    pub const fn measure_spec(&self) -> Option<&MeasureSpec> {
        self.measure.as_ref()
    }

    pub const fn scroll_offset(&self) -> Point {
        self.scroll_offset
    }

    pub const fn participates_in_hit_test(&self) -> bool {
        self.hit_test
    }

    pub const fn layout_boundaries(&self) -> LayoutBoundaries {
        self.boundaries
    }

    pub fn with_key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn with_optional_key(mut self, key: Option<WidgetKey>) -> Self {
        self.key = key;
        self
    }

    pub fn with_property(mut self, id: PropertyId, value: impl Into<PropertyValue>) -> Self {
        let value = value.into();
        match self
            .properties
            .binary_search_by_key(&id, |(property, _)| *property)
        {
            Ok(index) => self.properties[index].1 = value,
            Err(index) => self.properties.insert(index, (id, value)),
        }
        self
    }

    /// Associates precise invalidation metadata with a property. The property
    /// value can be set before or after this call.
    pub fn with_property_impact(mut self, id: PropertyId, impact: PropertyImpact) -> Self {
        match self
            .property_impacts
            .binary_search_by_key(&id, |(property, _)| *property)
        {
            Ok(index) => self.property_impacts[index].1 = impact,
            Err(index) => self.property_impacts.insert(index, (id, impact)),
        }
        self
    }

    pub fn with_child(mut self, child: WidgetNode) -> Self {
        self.children.push(child);
        self
    }

    pub fn with_children(mut self, children: impl IntoIterator<Item = WidgetNode>) -> Self {
        self.children.extend(children);
        self
    }

    pub fn with_lifecycle(mut self, lifecycle: LifecycleCallback) -> Self {
        self.lifecycle = Some(lifecycle);
        self
    }

    pub fn with_event_handler(mut self, handler: EventHandler) -> Self {
        self.event_handler = Some(handler);
        self
    }

    pub const fn with_focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_layout_style(mut self, style: LayoutStyle) -> Self {
        self.layout_style = style;
        self
    }

    pub fn with_measure(mut self, measure: MeasureSpec) -> Self {
        self.measure = Some(measure);
        self
    }

    pub fn without_measure(mut self) -> Self {
        self.measure = None;
        self
    }

    pub const fn with_scroll_offset(mut self, offset: Point) -> Self {
        self.scroll_offset = offset;
        self
    }

    pub const fn with_hit_test(mut self, hit_test: bool) -> Self {
        self.hit_test = hit_test;
        self
    }

    pub const fn with_layout_boundaries(mut self, boundaries: LayoutBoundaries) -> Self {
        self.boundaries = boundaries;
        self
    }

    pub fn same_identity(&self, other: &Self) -> bool {
        self.key == other.key && self.widget_type == other.widget_type
    }

    pub fn same_properties(&self, other: &Self) -> bool {
        self.properties == other.properties
            && self.property_impacts == other.property_impacts
            && self.lifecycle == other.lifecycle
            && self.event_handler == other.event_handler
            && self.focusable == other.focusable
            && self.enabled == other.enabled
            && self.layout_style == other.layout_style
            && self.measure == other.measure
            && self.scroll_offset == other.scroll_offset
            && self.hit_test == other.hit_test
            && self.boundaries == other.boundaries
    }
}

/// Read-only context passed while a declaration is built.
///
/// P1's reactive state layer installs its dependency scope around `build`;
/// this value intentionally exposes no mutation or transaction API.
#[derive(Debug, Default)]
pub struct BuildContext {
    _private: (),
}

impl BuildContext {
    pub const fn new() -> Self {
        Self { _private: () }
    }

    pub fn read<T>(&mut self, signal: &Signal<T>) -> Result<T>
    where
        T: Clone + PartialEq + 'static,
    {
        signal.get()
    }

    pub fn read_state<T>(&mut self, state: &State<T>) -> Result<T>
    where
        T: Clone + 'static,
    {
        state.get()
    }
}

/// Declarative widget builder.
pub trait Widget {
    fn build(&self, context: &mut BuildContext) -> Result<WidgetNode>;
}

/// A value that can produce a widget declaration.
pub trait View {
    fn build_view(&self, context: &mut BuildContext) -> Result<WidgetNode>;
}

impl<T: Widget + ?Sized> View for T {
    fn build_view(&self, context: &mut BuildContext) -> Result<WidgetNode> {
        self.build(context)
    }
}

impl Widget for WidgetNode {
    fn build(&self, _context: &mut BuildContext) -> Result<WidgetNode> {
        Ok(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_and_property_comparison_are_separate() {
        struct Sample;
        struct Other;
        let first = WidgetNode::new::<Sample>()
            .with_key("stable")
            .with_property(PropertyId::new(1), "old");
        let updated = WidgetNode::new::<Sample>()
            .with_key("stable")
            .with_property(PropertyId::new(1), "new");
        let replacement = WidgetNode::new::<Other>().with_key("stable");

        assert!(first.same_identity(&updated));
        assert!(!first.same_properties(&updated));
        assert!(!first.same_identity(&replacement));
    }

    #[test]
    fn properties_are_unique_and_deterministically_sorted() {
        struct Sample;
        let node = WidgetNode::new::<Sample>()
            .with_property(PropertyId::new(9), 1_u64)
            .with_property(PropertyId::new(2), false)
            .with_property(PropertyId::new(9), 3_u64);

        assert_eq!(node.properties.len(), 2);
        assert_eq!(node.properties[0].0, PropertyId::new(2));
        assert_eq!(
            node.property(PropertyId::new(9)),
            Some(&PropertyValue::U64(3))
        );
    }
}
