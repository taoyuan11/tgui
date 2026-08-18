//! Backend-neutral input values and retained-tree event dispatch.
//!
//! Platform adapters translate native input into [`UiEvent`]. Pointer hit
//! testing deliberately happens outside this module: [`EventDispatcher::dispatch`]
//! accepts a [`CommittedHitTarget`] resolved from the last committed
//! layout/render snapshot. Dispatch therefore never reads mutable layout, and
//! its capture/target/bubble path stays stable for the complete propagation.

use crate::core::{DpiScale, ElementId, Error, LayoutRevision, Point, Result, Size, WindowId};
use crate::state::{UiCommand, UpdateTxn};
use std::any::TypeId;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

/// Stable identity for one mouse, touch contact, or pen stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointerId(u64);

impl PointerId {
    pub const MOUSE: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for PointerId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerKind {
    Mouse,
    Touch,
    Pen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Primary,
    Secondary,
    Auxiliary,
    Back,
    Forward,
    Other(u16),
}

/// Platform-independent modifier-key state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CONTROL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const SUPER: Self = Self(1 << 3);
    const ALL: u8 = Self::SHIFT.0 | Self::CONTROL.0 | Self::ALT.0 | Self::SUPER.0;

    pub const fn from_bits(bits: u8) -> Self {
        Self(bits & Self::ALL)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self::from_bits(self.0 | other.0)
    }
}

/// Pressed pointer buttons. The first five bits use the named buttons;
/// adapter-specific buttons can occupy the remaining bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PointerButtons(u32);

impl PointerButtons {
    pub const NONE: Self = Self(0);
    pub const PRIMARY: Self = Self(1 << 0);
    pub const SECONDARY: Self = Self(1 << 1);
    pub const AUXILIARY: Self = Self(1 << 2);
    pub const BACK: Self = Self(1 << 3);
    pub const FORWARD: Self = Self(1 << 4);

    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PointerEvent {
    pub id: PointerId,
    pub kind: PointerKind,
    pub position: Point,
    pub button: Option<PointerButton>,
    pub buttons: PointerButtons,
    pub modifiers: Modifiers,
}

impl PointerEvent {
    pub const fn new(id: PointerId, kind: PointerKind, position: Point) -> Self {
        Self {
            id,
            kind,
            position,
            button: None,
            buttons: PointerButtons::NONE,
            modifiers: Modifiers::NONE,
        }
    }

    pub const fn with_button(mut self, button: Option<PointerButton>) -> Self {
        self.button = button;
        self
    }

    pub const fn with_buttons(mut self, buttons: PointerButtons) -> Self {
        self.buttons = buttons;
        self
    }

    pub const fn with_modifiers(mut self, modifiers: Modifiers) -> Self {
        self.modifiers = modifiers;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WheelDelta {
    Lines { x: f32, y: f32 },
    Pixels { x: f32, y: f32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct WheelEvent {
    pub position: Point,
    pub delta: WheelDelta,
    pub modifiers: Modifiers,
}

impl WheelEvent {
    pub const fn new(position: Point, delta: WheelDelta) -> Self {
        Self {
            position,
            delta,
            modifiers: Modifiers::NONE,
        }
    }

    pub const fn with_modifiers(mut self, modifiers: Modifiers) -> Self {
        self.modifiers = modifiers;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DragEvent {
    pub pointer: PointerEvent,
}

impl DragEvent {
    pub const fn new(pointer: PointerEvent) -> Self {
        Self { pointer }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NamedKey {
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Insert,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Function(u8),
    Other(Arc<str>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Character(Arc<str>),
    Named(NamedKey),
    Unidentified(u32),
}

impl Key {
    pub fn character(value: impl Into<Arc<str>>) -> Self {
        Self::Character(value.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub key: Key,
    /// Adapter-defined physical-key name, kept separate from logical text.
    pub physical_key: Option<Arc<str>>,
    pub modifiers: Modifiers,
    pub repeat: bool,
}

impl KeyEvent {
    pub fn new(key: Key) -> Self {
        Self {
            key,
            physical_key: None,
            modifiers: Modifiers::NONE,
            repeat: false,
        }
    }

    pub fn with_physical_key(mut self, physical_key: impl Into<Arc<str>>) -> Self {
        self.physical_key = Some(physical_key.into());
        self
    }

    pub const fn with_modifiers(mut self, modifiers: Modifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    pub const fn with_repeat(mut self, repeat: bool) -> Self {
        self.repeat = repeat;
        self
    }
}

/// Named shortcut entry produced by an application/platform shortcut map.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShortcutEvent {
    pub id: Arc<str>,
    pub trigger: KeyEvent,
}

impl ShortcutEvent {
    pub fn new(id: impl Into<Arc<str>>, trigger: KeyEvent) -> Self {
        Self {
            id: id.into(),
            trigger,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextInputEvent {
    pub text: Arc<str>,
}

impl TextInputEvent {
    pub fn new(text: impl Into<Arc<str>>) -> Self {
        Self { text: text.into() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImeEvent {
    Enabled,
    Disabled,
    Preedit {
        text: Arc<str>,
        selection: Option<Range<usize>>,
    },
    Commit(Arc<str>),
}

impl ImeEvent {
    pub fn preedit(text: impl Into<Arc<str>>, selection: Option<Range<usize>>) -> Self {
        Self::Preedit {
            text: text.into(),
            selection,
        }
    }

    pub fn commit(text: impl Into<Arc<str>>) -> Self {
        Self::Commit(text.into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FocusReason {
    Pointer,
    Keyboard,
    Programmatic,
    Accessibility,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusEvent {
    pub related_target: Option<ElementId>,
    pub reason: FocusReason,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccessibilityAction {
    Activate,
    Focus,
    Increment,
    Decrement,
    SetValue(Arc<str>),
    ScrollIntoView,
    Custom(Arc<str>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessibilityActionEvent {
    /// Window-scoped identity resolved from the committed semantics tree.
    window: Option<WindowId>,
    /// Generation-checked identity resolved from the committed semantics tree.
    pub target: ElementId,
    pub action: AccessibilityAction,
}

impl AccessibilityActionEvent {
    /// Creates an unscoped action for low-level dispatchers that also use
    /// unscoped [`CommittedHitTarget`] values.
    ///
    /// Application and platform adapters should use [`Self::for_window`]. A
    /// scoped dispatch rejects this legacy form instead of allowing the same
    /// [`ElementId`] from another window to alias the local tree.
    pub fn new(target: ElementId, action: AccessibilityAction) -> Self {
        Self {
            window: None,
            target,
            action,
        }
    }

    pub fn for_window(window: WindowId, target: ElementId, action: AccessibilityAction) -> Self {
        Self {
            window: Some(window),
            target,
            action,
        }
    }

    pub const fn window(&self) -> Option<WindowId> {
        self.window
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventKind {
    PointerDown,
    PointerMove,
    PointerUp,
    PointerCancel,
    PointerEnter,
    PointerLeave,
    Wheel,
    DragStart,
    DragMove,
    DragEnd,
    DragCancel,
    KeyDown,
    KeyUp,
    Shortcut,
    TextInput,
    Ime,
    FocusGained,
    FocusLost,
    WindowResized,
    WindowDpiChanged,
    WindowActivated,
    WindowCloseRequested,
    AccessibilityAction,
}

/// Unified platform-independent input vocabulary.
#[derive(Clone, Debug, PartialEq)]
pub enum UiEvent {
    PointerDown(PointerEvent),
    PointerMove(PointerEvent),
    PointerUp(PointerEvent),
    PointerCancel(PointerEvent),
    PointerEnter(PointerEvent),
    PointerLeave(PointerEvent),
    Wheel(WheelEvent),
    DragStart(DragEvent),
    DragMove(DragEvent),
    DragEnd(DragEvent),
    DragCancel(DragEvent),
    KeyDown(KeyEvent),
    KeyUp(KeyEvent),
    Shortcut(ShortcutEvent),
    TextInput(TextInputEvent),
    Ime(ImeEvent),
    FocusGained(FocusEvent),
    FocusLost(FocusEvent),
    WindowResized(Size),
    WindowDpiChanged(DpiScale),
    WindowActivated(bool),
    WindowCloseRequested,
    AccessibilityAction(AccessibilityActionEvent),
}

impl UiEvent {
    pub const fn kind(&self) -> EventKind {
        match self {
            Self::PointerDown(_) => EventKind::PointerDown,
            Self::PointerMove(_) => EventKind::PointerMove,
            Self::PointerUp(_) => EventKind::PointerUp,
            Self::PointerCancel(_) => EventKind::PointerCancel,
            Self::PointerEnter(_) => EventKind::PointerEnter,
            Self::PointerLeave(_) => EventKind::PointerLeave,
            Self::Wheel(_) => EventKind::Wheel,
            Self::DragStart(_) => EventKind::DragStart,
            Self::DragMove(_) => EventKind::DragMove,
            Self::DragEnd(_) => EventKind::DragEnd,
            Self::DragCancel(_) => EventKind::DragCancel,
            Self::KeyDown(_) => EventKind::KeyDown,
            Self::KeyUp(_) => EventKind::KeyUp,
            Self::Shortcut(_) => EventKind::Shortcut,
            Self::TextInput(_) => EventKind::TextInput,
            Self::Ime(_) => EventKind::Ime,
            Self::FocusGained(_) => EventKind::FocusGained,
            Self::FocusLost(_) => EventKind::FocusLost,
            Self::WindowResized(_) => EventKind::WindowResized,
            Self::WindowDpiChanged(_) => EventKind::WindowDpiChanged,
            Self::WindowActivated(_) => EventKind::WindowActivated,
            Self::WindowCloseRequested => EventKind::WindowCloseRequested,
            Self::AccessibilityAction(_) => EventKind::AccessibilityAction,
        }
    }

    pub const fn pointer_id(&self) -> Option<PointerId> {
        match self {
            Self::PointerDown(event)
            | Self::PointerMove(event)
            | Self::PointerUp(event)
            | Self::PointerCancel(event)
            | Self::PointerEnter(event)
            | Self::PointerLeave(event) => Some(event.id),
            Self::DragStart(event)
            | Self::DragMove(event)
            | Self::DragEnd(event)
            | Self::DragCancel(event) => Some(event.pointer.id),
            _ => None,
        }
    }

    pub const fn position(&self) -> Option<Point> {
        match self {
            Self::PointerDown(event)
            | Self::PointerMove(event)
            | Self::PointerUp(event)
            | Self::PointerCancel(event)
            | Self::PointerEnter(event)
            | Self::PointerLeave(event) => Some(event.position),
            Self::Wheel(event) => Some(event.position),
            Self::DragStart(event)
            | Self::DragMove(event)
            | Self::DragEnd(event)
            | Self::DragCancel(event) => Some(event.pointer.position),
            _ => None,
        }
    }

    const fn is_focus_routed(&self) -> bool {
        matches!(
            self,
            Self::KeyDown(_)
                | Self::KeyUp(_)
                | Self::Shortcut(_)
                | Self::TextInput(_)
                | Self::Ime(_)
                | Self::FocusGained(_)
                | Self::FocusLost(_)
        )
    }

    const fn is_window_routed(&self) -> bool {
        matches!(
            self,
            Self::WindowResized(_)
                | Self::WindowDpiChanged(_)
                | Self::WindowActivated(_)
                | Self::WindowCloseRequested
        )
    }

    const fn automatically_releases_pointer(&self) -> bool {
        matches!(
            self,
            Self::PointerUp(_) | Self::PointerCancel(_) | Self::DragEnd(_) | Self::DragCancel(_)
        )
    }

    const fn focus_reason(&self) -> FocusReason {
        match self {
            Self::PointerDown(_)
            | Self::PointerMove(_)
            | Self::PointerUp(_)
            | Self::PointerCancel(_)
            | Self::PointerEnter(_)
            | Self::PointerLeave(_)
            | Self::Wheel(_)
            | Self::DragStart(_)
            | Self::DragMove(_)
            | Self::DragEnd(_)
            | Self::DragCancel(_) => FocusReason::Pointer,
            Self::KeyDown(_)
            | Self::KeyUp(_)
            | Self::Shortcut(_)
            | Self::TextInput(_)
            | Self::Ime(_) => FocusReason::Keyboard,
            Self::AccessibilityAction(_) => FocusReason::Accessibility,
            _ => FocusReason::Programmatic,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventPhase {
    Capture,
    Target,
    Bubble,
}

/// Immutable result of hit testing one event against a committed layout.
///
/// The target retains its full generation. A stale target is ignored instead
/// of being redirected to a newer Element occupying the same arena slot.
/// Application dispatch additionally requires the window-scoped constructor,
/// preventing same-shaped per-window arenas from aliasing each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommittedHitTarget {
    revision: LayoutRevision,
    target: Option<ElementId>,
    window: Option<WindowId>,
}

impl CommittedHitTarget {
    pub const fn new(revision: LayoutRevision, target: Option<ElementId>) -> Self {
        Self {
            revision,
            target,
            window: None,
        }
    }

    pub const fn for_window(
        window: WindowId,
        revision: LayoutRevision,
        target: Option<ElementId>,
    ) -> Self {
        Self {
            revision,
            target,
            window: Some(window),
        }
    }

    pub const fn miss(revision: LayoutRevision) -> Self {
        Self::new(revision, None)
    }

    pub const fn miss_for_window(window: WindowId, revision: LayoutRevision) -> Self {
        Self::for_window(window, revision, None)
    }

    pub const fn revision(self) -> LayoutRevision {
        self.revision
    }

    pub const fn target(self) -> Option<ElementId> {
        self.target
    }

    pub const fn window(self) -> Option<WindowId> {
        self.window
    }
}

/// Immutable root-to-target identity snapshot used for one propagation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventPath {
    nodes: Vec<ElementId>,
}

impl EventPath {
    pub fn as_slice(&self) -> &[ElementId] {
        &self.nodes
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = ElementId> + ExactSizeIterator + '_ {
        self.nodes.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn target(&self) -> Option<ElementId> {
        self.nodes.last().copied()
    }

    fn snapshot<C, T>(tree: &T, target: ElementId) -> Result<Option<Self>>
    where
        T: EventTargetTree<C> + ?Sized,
    {
        if !tree.contains(target) {
            return Ok(None);
        }

        let mut reverse = Vec::new();
        let mut seen = BTreeSet::new();
        let mut current = Some(target);
        while let Some(id) = current {
            if !tree.contains(id) {
                return Err(Error::compile(
                    "event_path",
                    format!("parent link refers to stale element {id:?}"),
                ));
            }
            if !seen.insert(id) {
                return Err(Error::compile(
                    "event_path",
                    format!("cycle detected at element {id:?}"),
                ));
            }
            reverse.push(id);
            current = tree.parent(id);
        }
        reverse.reverse();

        if let Some(root) = tree.root() {
            if reverse.first().copied() != Some(root) {
                return Err(Error::compile(
                    "event_path",
                    format!("target {target:?} is detached from root {root:?}"),
                ));
            }
        }

        Ok(Some(Self { nodes: reverse }))
    }
}

/// Type-erased UI-thread event callback stored by an Element.
///
/// The callback's concrete [`TypeId`] plus `identity` form declaration identity
/// during reconciliation. They do not participate in `Key + WidgetType`
/// Element identity. Callers must change `identity` when captures/semantics of
/// the same callback type change.
pub struct EventHandler<C = UiCommand> {
    callback_type: TypeId,
    identity: u64,
    callback: Rc<HandlerCallback<C>>,
}

type HandlerCallback<C> =
    dyn for<'a> Fn(&UiEvent, &mut EventContext<'a, C>) -> Result<()> + 'static;

impl<C> EventHandler<C> {
    pub fn new<F>(identity: u64, callback: F) -> Self
    where
        F: for<'a> Fn(&UiEvent, &mut EventContext<'a, C>) -> Result<()> + 'static,
    {
        Self {
            callback_type: TypeId::of::<F>(),
            identity,
            callback: Rc::new(callback),
        }
    }

    pub const fn identity(&self) -> u64 {
        self.identity
    }

    pub fn call(&self, event: &UiEvent, context: &mut EventContext<'_, C>) -> Result<()> {
        (self.callback)(event, context)
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.callback, &other.callback)
    }
}

impl<C> Clone for EventHandler<C> {
    fn clone(&self) -> Self {
        Self {
            callback_type: self.callback_type,
            identity: self.identity,
            callback: self.callback.clone(),
        }
    }
}

impl<C> PartialEq for EventHandler<C> {
    fn eq(&self, other: &Self) -> bool {
        self.callback_type == other.callback_type && self.identity == other.identity
    }
}

impl<C> Eq for EventHandler<C> {}

impl<C> fmt::Debug for EventHandler<C> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EventHandler")
            .field("callback_type", &self.callback_type)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Read-only retained-tree facade used by dispatch.
///
/// Implementations normally wrap the crate-private Element arena. Returning a
/// cloned [`EventHandler`] releases any arena borrow before user code runs.
pub trait EventTargetTree<C = UiCommand> {
    fn root(&self) -> Option<ElementId>;
    fn contains(&self, element: ElementId) -> bool;
    fn parent(&self, element: ElementId) -> Option<ElementId>;
    fn event_handler(&self, element: ElementId) -> Option<EventHandler<C>>;

    fn is_focusable(&self, _element: ElementId) -> bool {
        false
    }

    fn is_enabled(&self, element: ElementId) -> bool {
        self.contains(element)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InputRequest {
    RequestFocus(ElementId),
    ReleaseFocus(ElementId),
    CapturePointer {
        pointer: PointerId,
        owner: ElementId,
    },
    ReleasePointer {
        pointer: PointerId,
        owner: ElementId,
    },
}

/// Mutable controls available only while one handler is running.
///
/// Every phase receives the same transaction. State/application commands are
/// committed by the caller only after the complete propagation.
pub struct EventContext<'a, C = UiCommand> {
    phase: EventPhase,
    target: ElementId,
    current_target: ElementId,
    propagation_stopped: bool,
    default_prevented: bool,
    transaction: &'a mut UpdateTxn<C>,
    input_requests: &'a mut Vec<InputRequest>,
}

impl<'a, C> EventContext<'a, C> {
    pub const fn phase(&self) -> EventPhase {
        self.phase
    }

    pub const fn target(&self) -> ElementId {
        self.target
    }

    pub const fn current_target(&self) -> ElementId {
        self.current_target
    }

    pub fn stop_propagation(&mut self) {
        self.propagation_stopped = true;
    }

    pub const fn is_propagation_stopped(&self) -> bool {
        self.propagation_stopped
    }

    pub fn prevent_default(&mut self) {
        self.default_prevented = true;
    }

    pub const fn is_default_prevented(&self) -> bool {
        self.default_prevented
    }

    pub fn command(&mut self, command: C) -> Result<()> {
        self.transaction.push(command)
    }

    pub fn transaction(&mut self) -> &mut UpdateTxn<C> {
        self.transaction
    }

    /// Requests focus for the Element whose handler is currently running.
    pub fn request_focus(&mut self) {
        self.input_requests
            .push(InputRequest::RequestFocus(self.current_target));
    }

    /// Releases focus only if the current handler's Element owns it.
    pub fn release_focus(&mut self) {
        self.input_requests
            .push(InputRequest::ReleaseFocus(self.current_target));
    }

    pub fn capture_pointer(&mut self, pointer: PointerId) {
        self.input_requests.push(InputRequest::CapturePointer {
            pointer,
            owner: self.current_target,
        });
    }

    /// Releases capture only if the current handler's Element owns it.
    pub fn release_pointer(&mut self, pointer: PointerId) {
        self.input_requests.push(InputRequest::ReleasePointer {
            pointer,
            owner: self.current_target,
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusChange {
    pub previous: Option<ElementId>,
    pub current: Option<ElementId>,
    pub reason: FocusReason,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatchOutcome {
    pub kind: EventKind,
    pub target: Option<ElementId>,
    pub path: EventPath,
    pub propagation_stopped: bool,
    pub default_prevented: bool,
    pub handlers_invoked: usize,
    pub focus_change: Option<FocusChange>,
}

impl DispatchOutcome {
    fn unhandled(kind: EventKind) -> Self {
        Self {
            kind,
            target: None,
            path: EventPath::default(),
            propagation_stopped: false,
            default_prevented: false,
            handlers_invoked: 0,
            focus_change: None,
        }
    }
}

/// Per-window focus and pointer-capture state.
#[derive(Clone, Debug, Default)]
pub struct EventDispatcher {
    focused: Option<ElementId>,
    pointer_capture: BTreeMap<PointerId, ElementId>,
}

impl EventDispatcher {
    const MAX_FOCUS_TRANSITIONS_PER_DISPATCH: usize = 32;

    pub fn new() -> Self {
        Self::default()
    }

    pub const fn focused(&self) -> Option<ElementId> {
        self.focused
    }

    pub fn pointer_capture(&self, pointer: PointerId) -> Option<ElementId> {
        self.pointer_capture.get(&pointer).copied()
    }

    /// Clears input ownership for an Element before or after it is unmounted.
    pub fn element_unmounted(&mut self, element: ElementId) {
        if self.focused == Some(element) {
            self.focused = None;
        }
        self.pointer_capture.retain(|_, owner| *owner != element);
    }

    /// Batch spelling intended for `ReconcileReport::removed_ids()`.
    pub fn elements_unmounted(&mut self, elements: impl IntoIterator<Item = ElementId>) {
        for element in elements {
            self.element_unmounted(element);
        }
    }

    /// Dispatches using a hit target obtained from the last committed snapshot.
    pub fn dispatch<C, T>(
        &mut self,
        tree: &T,
        committed_hit: CommittedHitTarget,
        event: &UiEvent,
        transaction: &mut UpdateTxn<C>,
    ) -> Result<DispatchOutcome>
    where
        T: EventTargetTree<C> + ?Sized,
    {
        self.reconcile_owners(tree);
        let ownership_before = self.clone();
        let focused_before = self.focused;
        let target = self.resolve_target(tree, committed_hit, event);
        let (mut outcome, mut requests) = self.propagate_to(tree, target, event, transaction)?;

        if !outcome.default_prevented {
            let mut defaults = self.default_input_requests(tree, &outcome.path, event);
            defaults.append(&mut requests);
            requests = defaults;
        }

        let first_change = self.apply_input_requests(tree, &requests, event.focus_reason());
        if let Err(error) = self.emit_focus_notifications(tree, first_change, transaction) {
            *self = ownership_before;
            return Err(error);
        }

        if event.automatically_releases_pointer() {
            if let Some(pointer) = event.pointer_id() {
                self.pointer_capture.remove(&pointer);
            }
        }

        if focused_before != self.focused {
            outcome.focus_change = Some(FocusChange {
                previous: focused_before,
                current: self.focused,
                reason: event.focus_reason(),
            });
        }
        Ok(outcome)
    }

    /// Bypasses automatic target selection while retaining stable-path and
    /// generation validation, primarily for deterministic synthesized events.
    /// Default focus behavior, handler input requests, focus notifications,
    /// and automatic pointer release are identical to [`Self::dispatch`].
    /// Window-scoped platform input, including accessibility actions, must use
    /// [`Self::dispatch`] so its committed window identity is validated.
    pub fn dispatch_to<C, T>(
        &mut self,
        tree: &T,
        target: ElementId,
        event: &UiEvent,
        transaction: &mut UpdateTxn<C>,
    ) -> Result<DispatchOutcome>
    where
        T: EventTargetTree<C> + ?Sized,
    {
        self.reconcile_owners(tree);
        let ownership_before = self.clone();
        let focused_before = self.focused;
        let (mut outcome, mut requests) =
            self.propagate_to(tree, Some(target), event, transaction)?;

        if !outcome.default_prevented {
            let mut defaults = self.default_input_requests(tree, &outcome.path, event);
            defaults.append(&mut requests);
            requests = defaults;
        }

        let first_change = self.apply_input_requests(tree, &requests, event.focus_reason());
        if let Err(error) = self.emit_focus_notifications(tree, first_change, transaction) {
            *self = ownership_before;
            return Err(error);
        }

        if event.automatically_releases_pointer() {
            if let Some(pointer) = event.pointer_id() {
                self.pointer_capture.remove(&pointer);
            }
        }

        if focused_before != self.focused {
            outcome.focus_change = Some(FocusChange {
                previous: focused_before,
                current: self.focused,
                reason: event.focus_reason(),
            });
        }
        Ok(outcome)
    }

    fn resolve_target<C, T>(
        &self,
        tree: &T,
        committed_hit: CommittedHitTarget,
        event: &UiEvent,
    ) -> Option<ElementId>
    where
        T: EventTargetTree<C> + ?Sized,
    {
        if let UiEvent::AccessibilityAction(action) = event {
            return (action.window() == committed_hit.window()).then_some(action.target);
        }
        if event.is_window_routed() {
            return tree.root();
        }
        if event.is_focus_routed() {
            return self.focused.or_else(|| tree.root());
        }
        if let Some(pointer) = event.pointer_id() {
            if let Some(owner) = self.pointer_capture(pointer) {
                return Some(owner);
            }
        }
        committed_hit.target()
    }

    fn propagate_to<C, T>(
        &self,
        tree: &T,
        target: Option<ElementId>,
        event: &UiEvent,
        transaction: &mut UpdateTxn<C>,
    ) -> Result<(DispatchOutcome, Vec<InputRequest>)>
    where
        T: EventTargetTree<C> + ?Sized,
    {
        let Some(target) = target else {
            return Ok((DispatchOutcome::unhandled(event.kind()), Vec::new()));
        };
        let Some(path) = EventPath::snapshot(tree, target)? else {
            return Ok((DispatchOutcome::unhandled(event.kind()), Vec::new()));
        };

        let mut requests = Vec::new();
        let (handlers_invoked, propagation_stopped, default_prevented) = {
            let mut context = EventContext {
                phase: EventPhase::Capture,
                target,
                current_target: target,
                propagation_stopped: false,
                default_prevented: false,
                transaction,
                input_requests: &mut requests,
            };
            let mut handlers_invoked = 0;
            let ancestors = &path.as_slice()[..path.len().saturating_sub(1)];

            for element in ancestors.iter().copied() {
                context.phase = EventPhase::Capture;
                context.current_target = element;
                handlers_invoked += Self::invoke(tree, element, event, &mut context)?;
                if context.propagation_stopped {
                    break;
                }
            }

            if !context.propagation_stopped {
                context.phase = EventPhase::Target;
                context.current_target = target;
                handlers_invoked += Self::invoke(tree, target, event, &mut context)?;
            }

            if !context.propagation_stopped {
                for element in ancestors.iter().rev().copied() {
                    context.phase = EventPhase::Bubble;
                    context.current_target = element;
                    handlers_invoked += Self::invoke(tree, element, event, &mut context)?;
                    if context.propagation_stopped {
                        break;
                    }
                }
            }

            (
                handlers_invoked,
                context.propagation_stopped,
                context.default_prevented,
            )
        };
        Ok((
            DispatchOutcome {
                kind: event.kind(),
                target: Some(target),
                path,
                propagation_stopped,
                default_prevented,
                handlers_invoked,
                focus_change: None,
            },
            requests,
        ))
    }

    fn invoke<C, T>(
        tree: &T,
        element: ElementId,
        event: &UiEvent,
        context: &mut EventContext<'_, C>,
    ) -> Result<usize>
    where
        T: EventTargetTree<C> + ?Sized,
    {
        // Exact generation is checked before every call. A stale slot is never
        // redirected even for custom trees using interior mutability.
        if !tree.contains(element) || !tree.is_enabled(element) {
            return Ok(0);
        }
        let Some(handler) = tree.event_handler(element) else {
            return Ok(0);
        };
        handler.call(event, context)?;
        Ok(1)
    }

    fn default_input_requests<C, T>(
        &self,
        tree: &T,
        path: &EventPath,
        event: &UiEvent,
    ) -> Vec<InputRequest>
    where
        T: EventTargetTree<C> + ?Sized,
    {
        match event {
            UiEvent::PointerDown(_) => path
                .iter()
                .rev()
                .find(|element| tree.is_enabled(*element) && tree.is_focusable(*element))
                .map(InputRequest::RequestFocus)
                .into_iter()
                .collect(),
            UiEvent::AccessibilityAction(action)
                if matches!(&action.action, AccessibilityAction::Focus) =>
            {
                path.target()
                    .filter(|target| tree.is_enabled(*target) && tree.is_focusable(*target))
                    .map(InputRequest::RequestFocus)
                    .into_iter()
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn apply_input_requests<C, T>(
        &mut self,
        tree: &T,
        requests: &[InputRequest],
        reason: FocusReason,
    ) -> Option<FocusChange>
    where
        T: EventTargetTree<C> + ?Sized,
    {
        let previous = self.focused;
        for request in requests {
            match *request {
                InputRequest::RequestFocus(element)
                    if tree.contains(element)
                        && tree.is_enabled(element)
                        && tree.is_focusable(element) =>
                {
                    self.focused = Some(element);
                }
                InputRequest::RequestFocus(_) => {}
                InputRequest::ReleaseFocus(owner) if self.focused == Some(owner) => {
                    self.focused = None;
                }
                InputRequest::ReleaseFocus(_) => {}
                InputRequest::CapturePointer { pointer, owner }
                    if tree.contains(owner) && tree.is_enabled(owner) =>
                {
                    self.pointer_capture.insert(pointer, owner);
                }
                InputRequest::CapturePointer { .. } => {}
                InputRequest::ReleasePointer { pointer, owner }
                    if self.pointer_capture(pointer) == Some(owner) =>
                {
                    self.pointer_capture.remove(&pointer);
                }
                InputRequest::ReleasePointer { .. } => {}
            }
        }
        (previous != self.focused).then_some(FocusChange {
            previous,
            current: self.focused,
            reason,
        })
    }

    fn emit_focus_notifications<C, T>(
        &mut self,
        tree: &T,
        mut change: Option<FocusChange>,
        transaction: &mut UpdateTxn<C>,
    ) -> Result<()>
    where
        T: EventTargetTree<C> + ?Sized,
    {
        let mut transition_count = 0;
        while let Some(current_change) = change {
            transition_count += 1;
            if transition_count > Self::MAX_FOCUS_TRANSITIONS_PER_DISPATCH {
                return Err(Error::compile(
                    "event_focus",
                    "focus handlers exceeded the transition/re-entry limit",
                ));
            }

            let mut requests = Vec::new();
            if let Some(previous) = current_change.previous {
                if tree.contains(previous) {
                    let event = UiEvent::FocusLost(FocusEvent {
                        related_target: current_change.current,
                        reason: current_change.reason,
                    });
                    let (_, mut emitted) =
                        self.propagate_to(tree, Some(previous), &event, transaction)?;
                    requests.append(&mut emitted);
                }
            }
            if let Some(current) = current_change.current {
                if tree.contains(current) {
                    let event = UiEvent::FocusGained(FocusEvent {
                        related_target: current_change.previous,
                        reason: current_change.reason,
                    });
                    let (_, mut emitted) =
                        self.propagate_to(tree, Some(current), &event, transaction)?;
                    requests.append(&mut emitted);
                }
            }
            change = self.apply_input_requests(tree, &requests, FocusReason::Programmatic);
        }
        Ok(())
    }

    /// Revalidates retained focus and pointer ownership after reconciliation.
    ///
    /// This must run even when reconciliation preserves an [`ElementId`]: an
    /// update can make that Element disabled or non-focusable without
    /// producing an unmounted-id report.
    pub(crate) fn reconcile_owners<C, T>(&mut self, tree: &T)
    where
        T: EventTargetTree<C> + ?Sized,
    {
        if self.focused.is_some_and(|element| {
            !tree.contains(element) || !tree.is_enabled(element) || !tree.is_focusable(element)
        }) {
            self.focused = None;
        }
        self.pointer_capture
            .retain(|_, owner| tree.contains(*owner) && tree.is_enabled(*owner));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum TestCommand {
        Reparent {
            element: ElementId,
            parent: ElementId,
        },
    }

    #[derive(Clone)]
    struct TestNode {
        parent: Option<ElementId>,
        handler: Option<EventHandler<TestCommand>>,
        focusable: bool,
        enabled: bool,
    }

    struct TestTree {
        root: ElementId,
        nodes: BTreeMap<ElementId, TestNode>,
    }

    impl TestTree {
        fn new(root: ElementId) -> Self {
            let mut nodes = BTreeMap::new();
            nodes.insert(
                root,
                TestNode {
                    parent: None,
                    handler: None,
                    focusable: false,
                    enabled: true,
                },
            );
            Self { root, nodes }
        }

        fn insert(&mut self, element: ElementId, parent: ElementId) {
            self.nodes.insert(
                element,
                TestNode {
                    parent: Some(parent),
                    handler: None,
                    focusable: false,
                    enabled: true,
                },
            );
        }

        fn set_handler(&mut self, element: ElementId, handler: EventHandler<TestCommand>) {
            self.nodes.get_mut(&element).unwrap().handler = Some(handler);
        }

        fn set_focusable(&mut self, element: ElementId, focusable: bool) {
            self.nodes.get_mut(&element).unwrap().focusable = focusable;
        }

        fn set_enabled(&mut self, element: ElementId, enabled: bool) {
            self.nodes.get_mut(&element).unwrap().enabled = enabled;
        }

        fn apply(&mut self, commands: Vec<TestCommand>) {
            for command in commands {
                match command {
                    TestCommand::Reparent { element, parent } => {
                        self.nodes.get_mut(&element).unwrap().parent = Some(parent);
                    }
                }
            }
        }
    }

    impl EventTargetTree<TestCommand> for TestTree {
        fn root(&self) -> Option<ElementId> {
            Some(self.root)
        }

        fn contains(&self, element: ElementId) -> bool {
            self.nodes.contains_key(&element)
        }

        fn parent(&self, element: ElementId) -> Option<ElementId> {
            self.nodes.get(&element).and_then(|node| node.parent)
        }

        fn event_handler(&self, element: ElementId) -> Option<EventHandler<TestCommand>> {
            self.nodes
                .get(&element)
                .and_then(|node| node.handler.clone())
        }

        fn is_focusable(&self, element: ElementId) -> bool {
            self.nodes.get(&element).is_some_and(|node| node.focusable)
        }

        fn is_enabled(&self, element: ElementId) -> bool {
            self.nodes.get(&element).is_some_and(|node| node.enabled)
        }
    }

    fn hit(target: ElementId) -> CommittedHitTarget {
        CommittedHitTarget::new(LayoutRevision::new(7), Some(target))
    }

    fn miss() -> CommittedHitTarget {
        CommittedHitTarget::miss(LayoutRevision::new(7))
    }

    fn hit_for_window(window: WindowId, target: ElementId) -> CommittedHitTarget {
        CommittedHitTarget::for_window(window, LayoutRevision::new(7), Some(target))
    }

    fn pointer(id: PointerId) -> PointerEvent {
        PointerEvent::new(id, PointerKind::Mouse, Point::new(10.0, 5.0))
    }

    fn phase_log_handler(
        identity: u64,
        name: &'static str,
        log: Rc<RefCell<Vec<String>>>,
    ) -> EventHandler<TestCommand> {
        EventHandler::new(identity, move |event, context| {
            if event.kind() == EventKind::PointerDown {
                log.borrow_mut()
                    .push(format!("{name}:{:?}", context.phase()));
            }
            Ok(())
        })
    }

    fn noop_handler(identity: u64) -> EventHandler<TestCommand> {
        EventHandler::new(identity, |_, _| Ok(()))
    }

    #[test]
    fn handler_equality_is_explicit_declaration_identity() {
        let first = noop_handler(9);
        let same_declaration = noop_handler(9);
        let changed_declaration = noop_handler(10);
        let different_callback_type = EventHandler::<TestCommand>::new(9, |event, _| {
            let _ = event.kind();
            Ok(())
        });
        assert_eq!(first, same_declaration);
        assert_ne!(first, changed_declaration);
        assert_ne!(first, different_callback_type);
        assert!(!first.ptr_eq(&same_declaration));
        assert!(format!("{first:?}").contains("identity: 9"));
    }

    #[test]
    fn propagation_order_is_capture_target_bubble() {
        let root = ElementId::from_parts(0, 1);
        let parent = ElementId::from_parts(1, 1);
        let target = ElementId::from_parts(2, 1);
        let mut tree = TestTree::new(root);
        tree.insert(parent, root);
        tree.insert(target, parent);
        let log = Rc::new(RefCell::new(Vec::new()));
        tree.set_handler(root, phase_log_handler(1, "root", log.clone()));
        tree.set_handler(parent, phase_log_handler(2, "parent", log.clone()));
        tree.set_handler(target, phase_log_handler(3, "target", log.clone()));

        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();
        let outcome = dispatcher
            .dispatch(
                &tree,
                hit(target),
                &UiEvent::PointerDown(pointer(PointerId::MOUSE)),
                &mut transaction,
            )
            .unwrap();

        assert_eq!(
            log.borrow().as_slice(),
            [
                "root:Capture",
                "parent:Capture",
                "target:Target",
                "parent:Bubble",
                "root:Bubble",
            ]
        );
        assert_eq!(outcome.path.as_slice(), [root, parent, target]);
        assert_eq!(outcome.handlers_invoked, 5);
    }

    #[test]
    fn disabled_targets_and_ancestors_do_not_invoke_handlers() {
        let root = ElementId::from_parts(0, 1);
        let parent = ElementId::from_parts(1, 1);
        let target = ElementId::from_parts(2, 1);
        let mut tree = TestTree::new(root);
        tree.insert(parent, root);
        tree.insert(target, parent);
        let log = Rc::new(RefCell::new(Vec::new()));
        tree.set_handler(root, phase_log_handler(1, "root", log.clone()));
        tree.set_handler(parent, phase_log_handler(2, "parent", log.clone()));
        tree.set_handler(target, phase_log_handler(3, "target", log.clone()));
        tree.set_enabled(parent, false);
        tree.set_enabled(target, false);

        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();
        let outcome = dispatcher
            .dispatch(
                &tree,
                hit(target),
                &UiEvent::PointerDown(pointer(PointerId::MOUSE)),
                &mut transaction,
            )
            .unwrap();

        assert_eq!(log.borrow().as_slice(), ["root:Capture", "root:Bubble"]);
        assert_eq!(outcome.path.as_slice(), [root, parent, target]);
        assert_eq!(outcome.handlers_invoked, 2);
    }

    #[test]
    fn stop_propagation_and_prevent_default_are_independent() {
        let root = ElementId::from_parts(0, 1);
        let parent = ElementId::from_parts(1, 1);
        let target = ElementId::from_parts(2, 1);
        let mut tree = TestTree::new(root);
        tree.insert(parent, root);
        tree.insert(target, parent);
        let log = Rc::new(RefCell::new(Vec::new()));
        let root_log = log.clone();
        tree.set_handler(
            root,
            EventHandler::new(1, move |_, context| {
                root_log.borrow_mut().push(context.phase());
                Ok(())
            }),
        );
        tree.set_handler(
            parent,
            EventHandler::new(2, |_, context| {
                if context.phase() == EventPhase::Capture {
                    context.prevent_default();
                    context.stop_propagation();
                }
                Ok(())
            }),
        );

        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();
        let outcome = dispatcher
            .dispatch(
                &tree,
                hit(target),
                &UiEvent::PointerDown(pointer(PointerId::MOUSE)),
                &mut transaction,
            )
            .unwrap();

        assert_eq!(log.borrow().as_slice(), [EventPhase::Capture]);
        assert!(outcome.propagation_stopped);
        assert!(outcome.default_prevented);
        assert_eq!(dispatcher.focused(), None);
    }

    #[test]
    fn event_path_is_snapshotted_before_transaction_commit() {
        let root = ElementId::from_parts(0, 1);
        let old_parent = ElementId::from_parts(1, 1);
        let new_parent = ElementId::from_parts(2, 1);
        let target = ElementId::from_parts(3, 1);
        let mut tree = TestTree::new(root);
        tree.insert(old_parent, root);
        tree.insert(new_parent, root);
        tree.insert(target, old_parent);
        tree.set_handler(
            old_parent,
            EventHandler::new(1, move |_, context| {
                if context.phase() == EventPhase::Capture {
                    context.command(TestCommand::Reparent {
                        element: target,
                        parent: new_parent,
                    })?;
                }
                Ok(())
            }),
        );

        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();
        let outcome = dispatcher
            .dispatch(
                &tree,
                hit(target),
                &UiEvent::PointerDown(pointer(PointerId::MOUSE)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(outcome.path.as_slice(), [root, old_parent, target]);

        transaction
            .commit(|commands| {
                tree.apply(commands);
                Ok(())
            })
            .unwrap();
        let mut next_transaction = UpdateTxn::new();
        let next = dispatcher
            .dispatch(
                &tree,
                hit(target),
                &UiEvent::PointerMove(pointer(PointerId::MOUSE)),
                &mut next_transaction,
            )
            .unwrap();
        assert_eq!(next.path.as_slice(), [root, new_parent, target]);
    }

    #[test]
    fn pointer_capture_overrides_hit_target_and_releases_on_up() {
        let root = ElementId::from_parts(0, 1);
        let left = ElementId::from_parts(1, 1);
        let right = ElementId::from_parts(2, 1);
        let pointer_id = PointerId::new(7);
        let mut tree = TestTree::new(root);
        tree.insert(left, root);
        tree.insert(right, root);
        tree.set_handler(
            left,
            EventHandler::new(1, move |event, context| {
                if event.kind() == EventKind::PointerDown && context.phase() == EventPhase::Target {
                    context.capture_pointer(pointer_id);
                }
                Ok(())
            }),
        );

        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();
        dispatcher
            .dispatch(
                &tree,
                hit(left),
                &UiEvent::PointerDown(pointer(pointer_id)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(dispatcher.pointer_capture(pointer_id), Some(left));

        let moved = dispatcher
            .dispatch(
                &tree,
                hit(right),
                &UiEvent::PointerMove(pointer(pointer_id)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(moved.target, Some(left));

        let released = dispatcher
            .dispatch(
                &tree,
                hit(right),
                &UiEvent::PointerUp(pointer(pointer_id)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(released.target, Some(left));
        assert_eq!(dispatcher.pointer_capture(pointer_id), None);

        let after = dispatcher
            .dispatch(
                &tree,
                hit(right),
                &UiEvent::PointerMove(pointer(pointer_id)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(after.target, Some(right));
    }

    #[test]
    fn explicit_focus_and_pointer_ownership_requests_are_applied() {
        let root = ElementId::from_parts(0, 1);
        let target = ElementId::from_parts(1, 1);
        let pointer_id = PointerId::new(71);
        let release = Rc::new(Cell::new(false));
        let mut tree = TestTree::new(root);
        tree.insert(target, root);
        tree.set_focusable(target, true);
        tree.set_handler(
            target,
            EventHandler::new(1, {
                let release = release.clone();
                move |_, context| {
                    if context.phase() == EventPhase::Target {
                        if release.get() {
                            context.release_focus();
                            context.release_pointer(pointer_id);
                        } else {
                            context.request_focus();
                            context.capture_pointer(pointer_id);
                        }
                    }
                    Ok(())
                }
            }),
        );

        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();
        dispatcher
            .dispatch(
                &tree,
                hit(target),
                &UiEvent::PointerMove(pointer(pointer_id)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(dispatcher.focused(), Some(target));
        assert_eq!(dispatcher.pointer_capture(pointer_id), Some(target));

        release.set(true);
        dispatcher
            .dispatch(
                &tree,
                hit(target),
                &UiEvent::PointerMove(pointer(pointer_id)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(dispatcher.focused(), None);
        assert_eq!(dispatcher.pointer_capture(pointer_id), None);
    }

    #[test]
    fn dispatch_to_preserves_default_focus_and_pointer_release_semantics() {
        let root = ElementId::from_parts(0, 1);
        let target = ElementId::from_parts(1, 1);
        let pointer_id = PointerId::new(8);
        let mut tree = TestTree::new(root);
        tree.insert(target, root);
        tree.set_focusable(target, true);
        tree.set_handler(
            target,
            EventHandler::new(1, move |event, context| {
                if event.kind() == EventKind::PointerDown && context.phase() == EventPhase::Target {
                    context.capture_pointer(pointer_id);
                }
                Ok(())
            }),
        );

        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();
        let down = dispatcher
            .dispatch_to(
                &tree,
                target,
                &UiEvent::PointerDown(pointer(pointer_id)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(dispatcher.focused(), Some(target));
        assert_eq!(dispatcher.pointer_capture(pointer_id), Some(target));
        assert_eq!(down.focus_change.unwrap().current, Some(target));

        dispatcher
            .dispatch_to(
                &tree,
                target,
                &UiEvent::PointerUp(pointer(pointer_id)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(dispatcher.pointer_capture(pointer_id), None);
    }

    #[test]
    fn focus_switch_emits_notifications_and_ime_uses_focus() {
        let root = ElementId::from_parts(0, 1);
        let first = ElementId::from_parts(1, 1);
        let second = ElementId::from_parts(2, 1);
        let mut tree = TestTree::new(root);
        tree.insert(first, root);
        tree.insert(second, root);
        tree.set_focusable(first, true);
        tree.set_focusable(second, true);
        let log = Rc::new(RefCell::new(Vec::new()));
        for (element, name, identity) in [(first, "first", 1), (second, "second", 2)] {
            let log = log.clone();
            tree.set_handler(
                element,
                EventHandler::new(identity, move |event, context| {
                    if context.phase() == EventPhase::Target {
                        log.borrow_mut().push((name, event.kind()));
                    }
                    Ok(())
                }),
            );
        }

        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();
        dispatcher
            .dispatch(
                &tree,
                hit(first),
                &UiEvent::PointerDown(pointer(PointerId::MOUSE)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(dispatcher.focused(), Some(first));

        dispatcher
            .dispatch(
                &tree,
                hit(second),
                &UiEvent::PointerDown(pointer(PointerId::MOUSE)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(dispatcher.focused(), Some(second));

        let ime = dispatcher
            .dispatch(
                &tree,
                hit(first),
                &UiEvent::Ime(ImeEvent::preedit("ni", Some(0..2))),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(ime.target, Some(second));
        assert!(log.borrow().contains(&("first", EventKind::FocusLost)));
        assert!(log.borrow().contains(&("second", EventKind::FocusGained)));
        assert!(log.borrow().contains(&("second", EventKind::Ime)));
    }

    #[test]
    fn stale_generation_is_never_retargeted_by_slot() {
        let root = ElementId::from_parts(0, 1);
        let old = ElementId::from_parts(1, 1);
        let replacement = ElementId::from_parts(1, 2);
        let pointer_id = PointerId::new(11);
        let mut old_tree = TestTree::new(root);
        old_tree.insert(old, root);
        old_tree.set_handler(
            old,
            EventHandler::new(1, move |event, context| {
                if event.kind() == EventKind::PointerDown && context.phase() == EventPhase::Target {
                    context.capture_pointer(pointer_id);
                }
                Ok(())
            }),
        );
        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();
        dispatcher
            .dispatch(
                &old_tree,
                hit(old),
                &UiEvent::PointerDown(pointer(pointer_id)),
                &mut transaction,
            )
            .unwrap();

        let mut current_tree = TestTree::new(root);
        current_tree.insert(replacement, root);
        let moved = dispatcher
            .dispatch(
                &current_tree,
                hit(replacement),
                &UiEvent::PointerMove(pointer(pointer_id)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(dispatcher.pointer_capture(pointer_id), None);
        assert_eq!(moved.target, Some(replacement));

        let stale_accessibility = dispatcher
            .dispatch(
                &current_tree,
                miss(),
                &UiEvent::AccessibilityAction(AccessibilityActionEvent::new(
                    old,
                    AccessibilityAction::Activate,
                )),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(stale_accessibility.target, None);
    }

    #[test]
    fn reconciliation_removals_clear_focus_and_pointer_capture() {
        let root = ElementId::from_parts(0, 1);
        let child = ElementId::from_parts(1, 1);
        let pointer_id = PointerId::new(12);
        let mut tree = TestTree::new(root);
        tree.insert(child, root);
        tree.set_focusable(child, true);
        tree.set_handler(
            child,
            EventHandler::new(1, move |event, context| {
                if event.kind() == EventKind::PointerDown && context.phase() == EventPhase::Target {
                    context.capture_pointer(pointer_id);
                }
                Ok(())
            }),
        );
        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();
        dispatcher
            .dispatch(
                &tree,
                hit(child),
                &UiEvent::PointerDown(pointer(pointer_id)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(dispatcher.focused(), Some(child));
        assert_eq!(dispatcher.pointer_capture(pointer_id), Some(child));

        dispatcher.elements_unmounted([child]);
        assert_eq!(dispatcher.focused(), None);
        assert_eq!(dispatcher.pointer_capture(pointer_id), None);
    }

    #[test]
    fn reconciliation_revalidates_ownership_for_retained_elements() {
        let root = ElementId::from_parts(0, 1);
        let child = ElementId::from_parts(1, 1);
        let pointer_id = PointerId::new(13);
        let mut tree = TestTree::new(root);
        tree.insert(child, root);
        tree.set_focusable(child, true);
        tree.set_handler(
            child,
            EventHandler::new(1, move |event, context| {
                if event.kind() == EventKind::PointerDown && context.phase() == EventPhase::Target {
                    context.capture_pointer(pointer_id);
                }
                Ok(())
            }),
        );
        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();
        dispatcher
            .dispatch(
                &tree,
                hit(child),
                &UiEvent::PointerDown(pointer(pointer_id)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(dispatcher.focused(), Some(child));
        assert_eq!(dispatcher.pointer_capture(pointer_id), Some(child));

        tree.set_focusable(child, false);
        dispatcher.reconcile_owners(&tree);
        assert_eq!(dispatcher.focused(), None);
        assert_eq!(dispatcher.pointer_capture(pointer_id), Some(child));

        tree.set_enabled(child, false);
        dispatcher.reconcile_owners(&tree);
        assert_eq!(dispatcher.pointer_capture(pointer_id), None);
    }

    #[test]
    fn window_events_route_to_root_and_accessibility_uses_explicit_target() {
        let root = ElementId::from_parts(0, 1);
        let child = ElementId::from_parts(1, 1);
        let mut tree = TestTree::new(root);
        tree.insert(child, root);
        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();

        let resized = dispatcher
            .dispatch(
                &tree,
                hit(child),
                &UiEvent::WindowResized(Size::new(640.0, 480.0)),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(resized.target, Some(root));

        let action = dispatcher
            .dispatch(
                &tree,
                hit(root),
                &UiEvent::AccessibilityAction(AccessibilityActionEvent::new(
                    child,
                    AccessibilityAction::Activate,
                )),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(action.target, Some(child));
    }

    #[test]
    fn accessibility_target_must_match_the_dispatch_window() {
        let first_window = WindowId::from_parts(0, 1);
        let second_window = WindowId::from_parts(1, 1);
        let root = ElementId::from_parts(0, 1);
        let child = ElementId::from_parts(1, 1);
        let mut tree = TestTree::new(root);
        tree.insert(child, root);
        let invocations = Rc::new(RefCell::new(0));
        let handler_invocations = invocations.clone();
        tree.set_handler(
            child,
            EventHandler::new(1, move |_, context| {
                if context.phase() == EventPhase::Target {
                    *handler_invocations.borrow_mut() += 1;
                }
                Ok(())
            }),
        );
        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();

        let wrong_window = AccessibilityActionEvent::for_window(
            second_window,
            child,
            AccessibilityAction::Activate,
        );
        assert_eq!(wrong_window.window(), Some(second_window));
        let mismatched = dispatcher
            .dispatch(
                &tree,
                hit_for_window(first_window, root),
                &UiEvent::AccessibilityAction(wrong_window),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(mismatched.target, None);

        let unscoped = dispatcher
            .dispatch(
                &tree,
                hit_for_window(first_window, root),
                &UiEvent::AccessibilityAction(AccessibilityActionEvent::new(
                    child,
                    AccessibilityAction::Activate,
                )),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(unscoped.target, None);
        assert_eq!(*invocations.borrow(), 0);

        let matched = dispatcher
            .dispatch(
                &tree,
                hit_for_window(first_window, root),
                &UiEvent::AccessibilityAction(AccessibilityActionEvent::for_window(
                    first_window,
                    child,
                    AccessibilityAction::Activate,
                )),
                &mut transaction,
            )
            .unwrap();
        assert_eq!(matched.target, Some(child));
        assert_eq!(*invocations.borrow(), 1);
    }

    #[test]
    fn invalid_parent_cycles_are_diagnosed() {
        let root = ElementId::from_parts(0, 1);
        let child = ElementId::from_parts(1, 1);
        let mut tree = TestTree::new(root);
        tree.insert(child, root);
        tree.nodes.get_mut(&root).unwrap().parent = Some(child);
        let mut dispatcher = EventDispatcher::new();
        let mut transaction = UpdateTxn::new();
        let error = dispatcher
            .dispatch(
                &tree,
                hit(child),
                &UiEvent::PointerMove(pointer(PointerId::MOUSE)),
                &mut transaction,
            )
            .unwrap_err();
        assert!(error.to_string().contains("cycle"));
    }
}
