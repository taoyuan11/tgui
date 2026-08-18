//! Reactive UI-thread state, transactions, and worker-message contracts.
//!
//! [`State`] and [`Signal`] are deliberately `!Send + !Sync`: application state
//! belongs to the UI thread. State changes are queued in an [`UpdateTxn`] and
//! become visible together when the transaction is committed. Worker threads
//! can only return generation/revision-stamped [`BackgroundMessage`] values
//! through [`UiDispatcher`].

use crate::core::{ElementId, Error, GenerationStamp, Result, RevisionSet, WindowId};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::ops::{BitOr, BitOrAssign};
use std::rc::{Rc, Weak};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, ThreadId};

/// A non-`Send` token identifying the UI thread that owns a tree or transaction.
#[derive(Clone, Debug)]
pub struct UiThread {
    id: ThreadId,
    // Rc intentionally makes the token (and types containing it) !Send + !Sync.
    _not_send: PhantomData<Rc<()>>,
}

impl UiThread {
    pub fn current() -> Self {
        Self {
            id: thread::current().id(),
            _not_send: PhantomData,
        }
    }

    pub fn id(&self) -> ThreadId {
        self.id
    }

    pub fn is_current(&self) -> bool {
        thread::current().id() == self.id
    }

    pub fn assert_current(&self) -> Result<()> {
        if self.is_current() {
            Ok(())
        } else {
            Err(Error::platform(
                "ui_thread",
                "operation attempted from a non-owner thread",
                false,
            ))
        }
    }
}

/// Selects which requested revisions make a worker result stale.
///
/// The default is [`RevisionMask::ALL`], preserving P0's strict behavior. A
/// resource task can, for example, use `RevisionMask::RESOURCE |
/// RevisionMask::SCENE` when layout and semantics are irrelevant to its result.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RevisionMask(u8);

impl RevisionMask {
    const LAYOUT_BIT: u8 = 1 << 0;
    const SCENE_BIT: u8 = 1 << 1;
    const RESOURCE_BIT: u8 = 1 << 2;
    const SEMANTIC_BIT: u8 = 1 << 3;

    pub const NONE: Self = Self(0);
    pub const LAYOUT: Self = Self(Self::LAYOUT_BIT);
    pub const SCENE: Self = Self(Self::SCENE_BIT);
    pub const RESOURCE: Self = Self(Self::RESOURCE_BIT);
    pub const SEMANTIC: Self = Self(Self::SEMANTIC_BIT);
    pub const ALL: Self =
        Self(Self::LAYOUT_BIT | Self::SCENE_BIT | Self::RESOURCE_BIT | Self::SEMANTIC_BIT);

    pub const fn new(layout: bool, scene: bool, resource: bool, semantic: bool) -> Self {
        let mut bits = 0;
        if layout {
            bits |= Self::LAYOUT_BIT;
        }
        if scene {
            bits |= Self::SCENE_BIT;
        }
        if resource {
            bits |= Self::RESOURCE_BIT;
        }
        if semantic {
            bits |= Self::SEMANTIC_BIT;
        }
        Self(bits)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns whether every relevant requested revision is still current.
    pub const fn matches(self, requested: RevisionSet, current: RevisionSet) -> bool {
        (!self.contains(Self::LAYOUT) || requested.layout.get() == current.layout.get())
            && (!self.contains(Self::SCENE) || requested.scene.get() == current.scene.get())
            && (!self.contains(Self::RESOURCE)
                || requested.resource.get() == current.resource.get())
            && (!self.contains(Self::SEMANTIC)
                || requested.semantic.get() == current.semantic.get())
    }
}

impl Default for RevisionMask {
    fn default() -> Self {
        Self::ALL
    }
}

impl fmt::Debug for RevisionMask {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut names = Vec::new();
        if self.contains(Self::LAYOUT) {
            names.push("LAYOUT");
        }
        if self.contains(Self::SCENE) {
            names.push("SCENE");
        }
        if self.contains(Self::RESOURCE) {
            names.push("RESOURCE");
        }
        if self.contains(Self::SEMANTIC) {
            names.push("SEMANTIC");
        }
        formatter
            .debug_tuple("RevisionMask")
            .field(&names.join(" | "))
            .finish()
    }
}

impl BitOr for RevisionMask {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for RevisionMask {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// One worker result with the identity and revisions observed at request time.
#[derive(Debug)]
pub struct BackgroundMessage<T> {
    pub target: WindowId,
    pub source: GenerationStamp,
    pub requested_revisions: RevisionSet,
    pub revision_mask: RevisionMask,
    pub payload: T,
}

impl<T> BackgroundMessage<T> {
    /// Creates a message for which all four revisions are relevant.
    pub fn new(
        target: WindowId,
        source: GenerationStamp,
        requested_revisions: RevisionSet,
        payload: T,
    ) -> Self {
        Self::new_with_mask(
            target,
            source,
            requested_revisions,
            RevisionMask::ALL,
            payload,
        )
    }

    pub fn new_with_mask(
        target: WindowId,
        source: GenerationStamp,
        requested_revisions: RevisionSet,
        revision_mask: RevisionMask,
        payload: T,
    ) -> Self {
        Self {
            target,
            source,
            requested_revisions,
            revision_mask,
            payload,
        }
    }

    /// Validates the target generation, source generation, and relevant revisions.
    pub fn is_current(
        &self,
        target: WindowId,
        source: GenerationStamp,
        current_revisions: RevisionSet,
    ) -> bool {
        self.target == target
            && self.source == source
            && self
                .revision_mask
                .matches(self.requested_revisions, current_revisions)
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> BackgroundMessage<U> {
        BackgroundMessage {
            target: self.target,
            source: self.source,
            requested_revisions: self.requested_revisions,
            revision_mask: self.revision_mask,
            payload: map(self.payload),
        }
    }
}

/// Cross-thread sender. It has no reference to an application or its arena.
#[derive(Clone, Debug)]
pub struct UiDispatcher<T> {
    sender: Sender<BackgroundMessage<T>>,
}

impl<T: Send + 'static> UiDispatcher<T> {
    pub fn send(&self, message: BackgroundMessage<T>) -> Result<()> {
        self.sender
            .send(message)
            .map_err(|_| Error::platform("ui_dispatch", "the UI receiver has been dropped", true))
    }

    /// Dispatches a result for which all four revisions are relevant.
    pub fn dispatch(
        &self,
        target: WindowId,
        source: GenerationStamp,
        requested_revisions: RevisionSet,
        payload: T,
    ) -> Result<()> {
        self.send(BackgroundMessage::new(
            target,
            source,
            requested_revisions,
            payload,
        ))
    }

    pub fn dispatch_with_mask(
        &self,
        target: WindowId,
        source: GenerationStamp,
        requested_revisions: RevisionSet,
        revision_mask: RevisionMask,
        payload: T,
    ) -> Result<()> {
        self.send(BackgroundMessage::new_with_mask(
            target,
            source,
            requested_revisions,
            revision_mask,
            payload,
        ))
    }
}

/// UI-owned receiver for worker results.
pub struct UiInbox<T> {
    receiver: Receiver<BackgroundMessage<T>>,
    owner: UiThread,
}

impl<T> UiInbox<T> {
    pub fn try_recv(&self) -> Result<Option<BackgroundMessage<T>>> {
        self.owner.assert_current()?;
        match self.receiver.try_recv() {
            Ok(message) => Ok(Some(message)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Ok(None),
        }
    }

    pub fn drain(&self) -> Result<Vec<BackgroundMessage<T>>> {
        self.owner.assert_current()?;
        let mut messages = Vec::new();
        while let Ok(message) = self.receiver.try_recv() {
            messages.push(message);
        }
        Ok(messages)
    }

    /// Accepts only messages for which the supplied predicate remains true.
    pub fn drain_valid(
        &self,
        mut is_current: impl FnMut(&BackgroundMessage<T>) -> bool,
    ) -> Result<DispatchBatch<T>> {
        self.owner.assert_current()?;
        let mut accepted = Vec::new();
        let mut stale = 0;
        loop {
            match self.receiver.try_recv() {
                Ok(message) if is_current(&message) => accepted.push(message),
                Ok(_) => stale += 1,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        Ok(DispatchBatch { accepted, stale })
    }

    /// Drains messages using the built-in generation and revision validation.
    pub fn drain_current(
        &self,
        target: WindowId,
        source: GenerationStamp,
        current_revisions: RevisionSet,
    ) -> Result<DispatchBatch<T>> {
        self.drain_valid(|message| message.is_current(target, source, current_revisions))
    }
}

/// Result of a validated inbox drain.
#[derive(Debug)]
pub struct DispatchBatch<T> {
    pub accepted: Vec<BackgroundMessage<T>>,
    pub stale: usize,
}

/// Creates the only supported worker-to-UI channel.
pub fn ui_channel<T: Send + 'static>() -> (UiDispatcher<T>, UiInbox<T>) {
    let (sender, receiver) = mpsc::channel();
    (
        UiDispatcher { sender },
        UiInbox {
            receiver,
            owner: UiThread::current(),
        },
    )
}

/// The UI phase in which a state dependency was observed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DependencyPhase {
    Build,
    Measure,
    Layout,
    Paint,
    Semantics,
}

/// One precisely tracked retained-tree/element/phase dependency invalidated by
/// a transaction. The tree namespace is internal so same-shaped windows cannot
/// accidentally coalesce each other's invalidations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StateInvalidation {
    owner: DependencyOwner,
    element: ElementId,
    phase: DependencyPhase,
}

impl StateInvalidation {
    pub(crate) const fn new(
        owner: DependencyOwner,
        element: ElementId,
        phase: DependencyPhase,
    ) -> Self {
        Self {
            owner,
            element,
            phase,
        }
    }

    pub const fn element(self) -> ElementId {
        self.element
    }

    pub const fn phase(self) -> DependencyPhase {
        self.phase
    }

    pub(crate) const fn owner(self) -> DependencyOwner {
        self.owner
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct SignalId(u64);

thread_local! {
    static NEXT_SIGNAL_ID: Cell<u64> = const { Cell::new(1) };
    static NEXT_DEPENDENCY_OWNER: Cell<u64> = const { Cell::new(1) };
    static READ_STACK: RefCell<Vec<ReadFrame>> = const { RefCell::new(Vec::new()) };
    static WRITE_GUARDS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

/// Unique dependency namespace for one retained tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct DependencyOwner(u64);

impl DependencyOwner {
    pub(crate) fn new() -> Self {
        NEXT_DEPENDENCY_OWNER.with(|next| {
            let id = next.get();
            let following = id
                .checked_add(1)
                .expect("reactive dependency-owner ID space exhausted");
            next.set(following);
            Self(id)
        })
    }
}

fn next_signal_id() -> SignalId {
    NEXT_SIGNAL_ID.with(|next| {
        let id = next.get();
        let following = id
            .checked_add(1)
            .expect("reactive signal ID space exhausted");
        next.set(following);
        SignalId(id)
    })
}

#[derive(Clone)]
enum Subscriber {
    Element(StateInvalidation),
    Derived(Weak<dyn DerivedInvalidator>),
}

struct SourceMeta {
    id: SignalId,
    next_subscription: Cell<u64>,
    subscribers: RefCell<BTreeMap<u64, Subscriber>>,
}

impl SourceMeta {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            id: next_signal_id(),
            next_subscription: Cell::new(1),
            subscribers: RefCell::new(BTreeMap::new()),
        })
    }

    fn subscribe(self: &Rc<Self>, subscriber: Subscriber) -> SubscriptionToken {
        let id = self.next_subscription.get();
        self.next_subscription.set(
            id.checked_add(1)
                .expect("reactive subscription ID space exhausted"),
        );
        self.subscribers.borrow_mut().insert(id, subscriber);
        SubscriptionToken {
            source: Rc::downgrade(self),
            id,
        }
    }

    fn collect_invalidations(
        self: &Rc<Self>,
        accumulator: &mut InvalidationAccumulator,
        visited: &mut HashSet<SignalId>,
    ) {
        if !visited.insert(self.id) {
            return;
        }

        let subscribers: Vec<(u64, Subscriber)> = self
            .subscribers
            .borrow()
            .iter()
            .map(|(&id, subscriber)| (id, subscriber.clone()))
            .collect();
        let mut dead = Vec::new();
        for (id, subscriber) in subscribers {
            match subscriber {
                Subscriber::Element(invalidation) => accumulator.insert(invalidation),
                Subscriber::Derived(derived) => match derived.upgrade() {
                    Some(derived) => {
                        derived.mark_dirty();
                        derived
                            .source_meta()
                            .collect_invalidations(accumulator, visited);
                    }
                    None => dead.push(id),
                },
            }
        }

        if !dead.is_empty() {
            let mut subscribers = self.subscribers.borrow_mut();
            for id in dead {
                subscribers.remove(&id);
            }
        }
    }
}

trait DerivedInvalidator {
    fn mark_dirty(&self);
    fn source_meta(&self) -> Rc<SourceMeta>;
}

struct SubscriptionToken {
    source: Weak<SourceMeta>,
    id: u64,
}

impl Drop for SubscriptionToken {
    fn drop(&mut self) {
        if let Some(source) = self.source.upgrade() {
            source.subscribers.borrow_mut().remove(&self.id);
        }
    }
}

/// RAII subscriptions captured for one element phase.
///
/// Element nodes keep one set per phase. Dropping it during a rebuild or
/// unmount immediately detaches every old dependency.
pub(crate) struct DependencySet {
    subscriptions: Vec<SubscriptionToken>,
}

impl DependencySet {
    fn empty() -> Self {
        Self {
            subscriptions: Vec::new(),
        }
    }

    fn for_element(
        sources: Vec<Rc<SourceMeta>>,
        owner: DependencyOwner,
        element: ElementId,
        phase: DependencyPhase,
    ) -> Self {
        let subscriber = Subscriber::Element(StateInvalidation::new(owner, element, phase));
        let subscriptions = sources
            .into_iter()
            .map(|source| source.subscribe(subscriber.clone()))
            .collect();
        Self { subscriptions }
    }

    fn for_derived(sources: Vec<Rc<SourceMeta>>, derived: Weak<dyn DerivedInvalidator>) -> Self {
        let subscriptions = sources
            .into_iter()
            .map(|source| source.subscribe(Subscriber::Derived(derived.clone())))
            .collect();
        Self { subscriptions }
    }

    pub(crate) fn len(&self) -> usize {
        self.subscriptions.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }
}

impl Default for DependencySet {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Copy, Debug)]
enum ReadKind {
    Element(DependencyPhase),
    Derived,
}

struct DependencyCollector {
    sources: RefCell<Vec<Rc<SourceMeta>>>,
    seen: RefCell<HashSet<SignalId>>,
}

impl DependencyCollector {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            sources: RefCell::new(Vec::new()),
            seen: RefCell::new(HashSet::new()),
        })
    }

    fn record(&self, source: Rc<SourceMeta>) {
        if self.seen.borrow_mut().insert(source.id) {
            self.sources.borrow_mut().push(source);
        }
    }

    fn sources(&self) -> Vec<Rc<SourceMeta>> {
        self.sources.borrow().clone()
    }
}

struct ReadFrame {
    kind: ReadKind,
    collector: Rc<DependencyCollector>,
}

struct ReadFrameGuard;

impl Drop for ReadFrameGuard {
    fn drop(&mut self) {
        READ_STACK.with(|stack| {
            let popped = stack.borrow_mut().pop();
            debug_assert!(popped.is_some());
        });
    }
}

fn capture_sources<R>(
    kind: ReadKind,
    read: impl FnOnce() -> Result<R>,
) -> Result<(R, Vec<Rc<SourceMeta>>)> {
    let collector = DependencyCollector::new();
    READ_STACK.with(|stack| {
        stack.borrow_mut().push(ReadFrame {
            kind,
            collector: collector.clone(),
        });
    });
    let guard = ReadFrameGuard;
    let output = read();
    drop(guard);
    output.map(|output| (output, collector.sources()))
}

fn record_dependency(source: &Rc<SourceMeta>) {
    let collector =
        READ_STACK.with(|stack| stack.borrow().last().map(|frame| frame.collector.clone()));
    if let Some(collector) = collector {
        collector.record(source.clone());
    }
}

fn ensure_state_write_allowed() -> Result<()> {
    let guarded_stage = WRITE_GUARDS.with(|guards| guards.borrow().last().copied());
    if let Some(stage) = guarded_stage {
        return Err(Error::compile(
            "state_update",
            format!("state writes are forbidden during {stage}"),
        ));
    }
    let read_kind = READ_STACK.with(|stack| stack.borrow().last().map(|frame| frame.kind));
    match read_kind {
        None => Ok(()),
        Some(ReadKind::Element(phase)) => Err(Error::compile(
            "state_update",
            format!("state writes are forbidden while reading the {phase:?} phase"),
        )),
        Some(ReadKind::Derived) => Err(Error::compile(
            "state_update",
            "state writes are forbidden while evaluating a derived signal",
        )),
    }
}

/// Runs one element phase while automatically recording all State/Signal reads.
pub(crate) fn capture_dependencies<R>(
    owner: DependencyOwner,
    element: ElementId,
    phase: DependencyPhase,
    read: impl FnOnce() -> Result<R>,
) -> Result<(R, DependencySet)> {
    let (output, sources) = capture_sources(ReadKind::Element(phase), read)?;
    Ok((
        output,
        DependencySet::for_element(sources, owner, element, phase),
    ))
}

/// Rejects state publication while retained-tree reconciliation and lifecycle
/// callbacks are observing a not-yet-committed tree.
pub(crate) struct StateWriteGuard;

impl StateWriteGuard {
    pub(crate) fn enter(stage: &'static str) -> Self {
        WRITE_GUARDS.with(|guards| guards.borrow_mut().push(stage));
        Self
    }
}

impl Drop for StateWriteGuard {
    fn drop(&mut self) {
        WRITE_GUARDS.with(|guards| {
            let popped = guards.borrow_mut().pop();
            debug_assert!(popped.is_some());
        });
    }
}

struct StateCell<T> {
    owner: UiThread,
    source: Rc<SourceMeta>,
    value: RefCell<T>,
}

impl<T> StateCell<T> {
    fn with<R>(&self, read: impl FnOnce(&T) -> R) -> Result<R> {
        self.owner.assert_current()?;
        record_dependency(&self.source);
        let value = self.value.try_borrow().map_err(|_| {
            Error::compile(
                "state_read",
                "state was read reentrantly while it was being updated",
            )
        })?;
        Ok(read(&value))
    }
}

/// A writable, UI-thread-owned state handle.
///
/// Writes always require an [`UpdateTxn`], so reads during an event observe the
/// pre-event value until propagation finishes and the transaction commits.
pub struct State<T> {
    cell: Rc<StateCell<T>>,
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        Self {
            cell: self.cell.clone(),
        }
    }
}

impl<T> fmt::Debug for State<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("State")
            .field("id", &self.cell.source.id)
            .finish_non_exhaustive()
    }
}

impl<T: 'static> State<T> {
    pub fn new(value: T) -> Self {
        Self {
            cell: Rc::new(StateCell {
                owner: UiThread::current(),
                source: SourceMeta::new(),
                value: RefCell::new(value),
            }),
        }
    }

    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> Result<R> {
        self.cell.with(read)
    }

    pub fn get(&self) -> Result<T>
    where
        T: Clone,
    {
        self.with(Clone::clone)
    }

    pub fn signal(&self) -> Signal<T> {
        Signal {
            kind: SignalKind::State(self.cell.clone()),
        }
    }

    pub fn set<C>(&self, transaction: &mut UpdateTxn<C>, value: T) -> Result<()>
    where
        T: Clone + PartialEq,
    {
        self.update(transaction, move |current| *current = value)
    }

    pub fn update<C>(
        &self,
        transaction: &mut UpdateTxn<C>,
        update: impl FnOnce(&mut T) + 'static,
    ) -> Result<()>
    where
        T: Clone + PartialEq,
    {
        transaction.enqueue_state_update(self.clone(), update)
    }
}

enum SignalKind<T> {
    State(Rc<StateCell<T>>),
    Derived(Rc<DerivedCell<T>>),
}

impl<T> Clone for SignalKind<T> {
    fn clone(&self) -> Self {
        match self {
            Self::State(state) => Self::State(state.clone()),
            Self::Derived(derived) => Self::Derived(derived.clone()),
        }
    }
}

/// A read-only base or lazily derived reactive value.
pub struct Signal<T> {
    kind: SignalKind<T>,
}

impl<T> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.clone(),
        }
    }
}

impl<T> fmt::Debug for Signal<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = match &self.kind {
            SignalKind::State(state) => state.source.id,
            SignalKind::Derived(derived) => derived.source.id,
        };
        formatter
            .debug_struct("Signal")
            .field("id", &id)
            .finish_non_exhaustive()
    }
}

impl<T: 'static> Signal<T> {
    pub fn derive(compute: impl Fn() -> Result<T> + 'static) -> Self
    where
        T: Clone + PartialEq,
    {
        Self {
            kind: SignalKind::Derived(Rc::new(DerivedCell {
                owner: UiThread::current(),
                source: SourceMeta::new(),
                compute: Box::new(compute),
                value: RefCell::new(None),
                dirty: Cell::new(true),
                computing: Cell::new(false),
                dependencies: RefCell::new(DependencySet::empty()),
            })),
        }
    }

    pub fn with<R>(&self, read: impl FnOnce(&T) -> R) -> Result<R>
    where
        T: Clone + PartialEq,
    {
        match &self.kind {
            SignalKind::State(state) => state.with(read),
            SignalKind::Derived(derived) => {
                derived.owner.assert_current()?;
                record_dependency(&derived.source);
                derived.ensure_evaluated()?;
                let value = derived.value.try_borrow().map_err(|_| {
                    Error::compile("signal_read", "derived signal cache was read reentrantly")
                })?;
                let value = value.as_ref().ok_or_else(|| {
                    Error::compile("signal_read", "derived signal has no computed value")
                })?;
                Ok(read(value))
            }
        }
    }

    pub fn get(&self) -> Result<T>
    where
        T: Clone + PartialEq,
    {
        self.with(Clone::clone)
    }

    pub fn map<U>(&self, map: impl Fn(T) -> U + 'static) -> Signal<U>
    where
        T: Clone + PartialEq,
        U: Clone + PartialEq + 'static,
    {
        let source = self.clone();
        Signal::derive(move || Ok(map(source.get()?)))
    }
}

struct DerivedCell<T> {
    owner: UiThread,
    source: Rc<SourceMeta>,
    compute: Box<dyn Fn() -> Result<T>>,
    value: RefCell<Option<T>>,
    dirty: Cell<bool>,
    computing: Cell<bool>,
    dependencies: RefCell<DependencySet>,
}

struct ComputingGuard<'a>(&'a Cell<bool>);

impl Drop for ComputingGuard<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl<T: Clone + PartialEq + 'static> DerivedCell<T> {
    fn ensure_evaluated(self: &Rc<Self>) -> Result<()> {
        if !self.dirty.get() && self.value.borrow().is_some() {
            return Ok(());
        }
        if self.computing.replace(true) {
            return Err(Error::compile(
                "signal_evaluation",
                "cycle or reentrant derived signal evaluation detected",
            ));
        }
        let guard = ComputingGuard(&self.computing);
        let (next, sources) = capture_sources(ReadKind::Derived, || (self.compute)())?;

        let erased: Rc<dyn DerivedInvalidator> = self.clone();
        let dependencies = DependencySet::for_derived(sources, Rc::downgrade(&erased));
        drop(erased);

        // Replace value and dependency subscriptions only after a successful
        // computation, preserving the last valid cache on a cycle/error.
        *self.value.borrow_mut() = Some(next);
        *self.dependencies.borrow_mut() = dependencies;
        self.dirty.set(false);
        drop(guard);
        Ok(())
    }
}

impl<T: Clone + PartialEq + 'static> DerivedInvalidator for DerivedCell<T> {
    fn mark_dirty(&self) {
        self.dirty.set(true);
    }

    fn source_meta(&self) -> Rc<SourceMeta> {
        self.source.clone()
    }
}

trait PendingStateWrite {
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn preflight(&self) -> Result<()>;
    fn stage(self: Box<Self>) -> Result<Option<Box<dyn StagedStateWrite>>>;
}

trait StagedStateWrite {
    fn preflight_publish(&self) -> Result<()>;
    fn publish(self: Box<Self>) -> Result<(Rc<SourceMeta>, Box<dyn Any>)>;
}

type StateMutation<T> = Box<dyn FnOnce(&mut T)>;

struct TypedStateWrite<T> {
    state: State<T>,
    updates: Vec<StateMutation<T>>,
}

struct TypedStagedStateWrite<T> {
    state: State<T>,
    value: T,
}

impl<T: Clone + PartialEq + 'static> PendingStateWrite for TypedStateWrite<T> {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn preflight(&self) -> Result<()> {
        self.state.cell.owner.assert_current()?;
        self.state.cell.value.try_borrow_mut().map_err(|_| {
            Error::compile(
                "state_commit",
                "state is still borrowed while its transaction is committing",
            )
        })?;
        Ok(())
    }

    fn stage(self: Box<Self>) -> Result<Option<Box<dyn StagedStateWrite>>> {
        let original = self.state.cell.value.try_borrow().map_err(|_| {
            Error::compile(
                "state_commit",
                "state was borrowed reentrantly while staging an update",
            )
        })?;
        let mut value = original.clone();
        drop(original);
        for update in self.updates {
            update(&mut value);
        }
        if self.state.cell.with(|original| *original == value)? {
            Ok(None)
        } else {
            Ok(Some(Box::new(TypedStagedStateWrite {
                state: self.state,
                value,
            })))
        }
    }
}

impl<T: 'static> StagedStateWrite for TypedStagedStateWrite<T> {
    fn preflight_publish(&self) -> Result<()> {
        self.state.cell.value.try_borrow_mut().map_err(|_| {
            Error::compile(
                "state_commit",
                "state is still borrowed while its transaction is publishing",
            )
        })?;
        Ok(())
    }

    fn publish(self: Box<Self>) -> Result<(Rc<SourceMeta>, Box<dyn Any>)> {
        let mut current = self.state.cell.value.try_borrow_mut().map_err(|_| {
            Error::compile(
                "state_commit",
                "state was borrowed reentrantly while publishing an update",
            )
        })?;
        let old = std::mem::replace(&mut *current, self.value);
        drop(current);
        let source = self.state.cell.source.clone();
        // Keep the StateCell alive until the complete publish loop finishes;
        // dropping the last State handle could otherwise run `T::drop` between
        // two heterogeneous State publications.
        Ok((source, Box::new((self.state, old))))
    }
}

#[derive(Default)]
struct InvalidationAccumulator {
    ordered: Vec<StateInvalidation>,
    seen: HashSet<StateInvalidation>,
}

impl InvalidationAccumulator {
    fn insert(&mut self, invalidation: StateInvalidation) {
        if self.seen.insert(invalidation) {
            self.ordered.push(invalidation);
        }
    }
}

/// Small command vocabulary used by the application transaction contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiCommand {
    RequestFrame(WindowId),
    CloseWindow(WindowId),
}

/// UI-only batch of commands and reactive state changes.
///
/// Event dispatch shares one transaction across capture, target, and bubble,
/// then calls [`UpdateTxn::commit`] exactly once after propagation finishes.
pub struct UpdateTxn<C = UiCommand> {
    owner: UiThread,
    commands: Vec<C>,
    state_writes: Vec<Box<dyn PendingStateWrite>>,
    state_positions: BTreeMap<SignalId, usize>,
    state_write_count: usize,
    committed: bool,
}

/// Summary of one successfully committed transaction.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TxnReceipt {
    /// Number of commands passed to the commit callback (the P0 field).
    pub command_count: usize,
    /// Number of queued `set`/`update` calls before per-state coalescing.
    pub state_write_count: usize,
    /// Number of distinct states whose final value observably changed.
    pub changed_state_count: usize,
    invalidations: Vec<StateInvalidation>,
}

impl TxnReceipt {
    pub fn invalidations(&self) -> &[StateInvalidation] {
        &self.invalidations
    }
}

impl<C> UpdateTxn<C> {
    pub fn new() -> Self {
        Self {
            owner: UiThread::current(),
            commands: Vec::new(),
            state_writes: Vec::new(),
            state_positions: BTreeMap::new(),
            state_write_count: 0,
            committed: false,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            owner: UiThread::current(),
            commands: Vec::with_capacity(capacity),
            state_writes: Vec::new(),
            state_positions: BTreeMap::new(),
            state_write_count: 0,
            committed: false,
        }
    }

    pub fn push(&mut self, command: C) -> Result<()> {
        self.ensure_open()?;
        self.commands.push(command);
        Ok(())
    }

    pub fn extend(&mut self, commands: impl IntoIterator<Item = C>) -> Result<()> {
        self.ensure_open()?;
        self.commands.extend(commands);
        Ok(())
    }

    /// Preserves P0 semantics: this is the command count, not state-write count.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty() && self.state_writes.is_empty()
    }

    pub fn commands(&self) -> &[C] {
        &self.commands
    }

    pub fn pending_state_count(&self) -> usize {
        self.state_writes.len()
    }

    pub fn queued_state_write_count(&self) -> usize {
        self.state_write_count
    }

    /// Extracts commands and rolls back all pending state writes.
    pub fn into_commands(self) -> Vec<C> {
        self.commands
    }

    /// Finalizes without applying state writes and returns the queued commands.
    pub fn rollback(mut self) -> Vec<C> {
        self.committed = true;
        std::mem::take(&mut self.commands)
    }

    /// Applies the complete command batch, then atomically publishes coalesced
    /// state changes and computes the minimal de-duplicated invalidation set.
    /// If `apply` rejects the commands, every pending state write is discarded.
    ///
    /// Atomicity covers ordinary [`Result::Err`] paths. User implementations of
    /// `Clone`, `PartialEq`, update closures, `apply`, and value destructors must
    /// not unwind across this UI transaction boundary.
    pub fn commit(mut self, apply: impl FnOnce(Vec<C>) -> Result<()>) -> Result<TxnReceipt> {
        self.ensure_open()?;
        ensure_state_write_allowed()?;

        // Move every user-owned value into locals declared after the guard.
        // On any `?` path, commands, updater captures, and the apply callback
        // are therefore dropped while nested State publication is forbidden.
        let _write_guard = StateWriteGuard::enter("state transaction commit");
        let guarded_apply = apply;
        let command_count = self.commands.len();
        let state_write_count = self.state_write_count;
        let commands = std::mem::take(&mut self.commands);
        let writes = std::mem::take(&mut self.state_writes);

        for write in &writes {
            write.preflight()?;
        }

        // User-provided updaters run against cloned values. Keeping every live
        // State unchanged until all updates and commands succeed gives the
        // transaction one coherent pre-commit read snapshot.
        let mut staged_writes = Vec::with_capacity(writes.len());
        for write in writes {
            if let Some(staged) = write.stage()? {
                staged_writes.push(staged);
            }
        }

        guarded_apply(commands)?;

        for staged in &staged_writes {
            staged.preflight_publish()?;
        }

        let mut changed_sources = Vec::with_capacity(staged_writes.len());
        let mut published_values = Vec::with_capacity(staged_writes.len());
        for staged in staged_writes {
            let (source, published_value) = staged.publish()?;
            changed_sources.push(source);
            // Delay user-defined Drop code and last-handle destruction until
            // every changed State is live.
            published_values.push(published_value);
        }

        let mut invalidations = InvalidationAccumulator::default();
        let mut visited = HashSet::new();
        for source in &changed_sources {
            source.collect_invalidations(&mut invalidations, &mut visited);
        }
        drop(published_values);

        self.committed = true;
        Ok(TxnReceipt {
            command_count,
            state_write_count,
            changed_state_count: changed_sources.len(),
            invalidations: invalidations.ordered,
        })
    }

    fn ensure_open(&self) -> Result<()> {
        self.owner.assert_current()?;
        if self.committed {
            Err(Error::platform(
                "update_txn",
                "transaction was already finalized",
                false,
            ))
        } else {
            Ok(())
        }
    }

    fn enqueue_state_update<T>(
        &mut self,
        state: State<T>,
        update: impl FnOnce(&mut T) + 'static,
    ) -> Result<()>
    where
        T: Clone + PartialEq + 'static,
    {
        self.ensure_open()?;
        ensure_state_write_allowed()?;
        state.cell.owner.assert_current()?;

        let id = state.cell.source.id;
        self.state_write_count = self.state_write_count.saturating_add(1);
        if let Some(&position) = self.state_positions.get(&id) {
            let pending = self.state_writes[position]
                .as_any_mut()
                .downcast_mut::<TypedStateWrite<T>>()
                .ok_or_else(|| {
                    Error::compile(
                        "state_update",
                        "reactive state ID was associated with an incompatible value type",
                    )
                })?;
            pending.updates.push(Box::new(update));
        } else {
            let position = self.state_writes.len();
            self.state_positions.insert(id, position);
            self.state_writes.push(Box::new(TypedStateWrite {
                state,
                updates: vec![Box::new(update)],
            }));
        }
        Ok(())
    }
}

impl<C> Default for UpdateTxn<C> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        ElementId, LayoutRevision, ResourceRevision, SceneRevision, SemanticRevision,
    };

    #[test]
    fn transactions_apply_one_coalesced_command_batch() {
        let mut txn = UpdateTxn::<u32>::new();
        txn.push(1).unwrap();
        txn.push(2).unwrap();
        let mut seen = Vec::new();
        let receipt = txn.commit(|commands| {
            seen.extend(commands);
            Ok(())
        });

        assert_eq!(receipt.unwrap().command_count, 2);
        assert_eq!(seen, [1, 2]);
    }

    #[test]
    fn worker_messages_are_generation_and_revision_stamped() {
        let (dispatcher, inbox) = ui_channel::<u32>();
        let id = ElementId::from_parts(4, 9);
        let revisions = RevisionSet::ZERO;
        dispatcher
            .dispatch(WindowId::from_parts(0, 1), id.stamp(), revisions, 42)
            .unwrap();
        let batch = inbox
            .drain_valid(|message| message.source.matches(id))
            .unwrap();
        assert_eq!(batch.stale, 0);
        assert_eq!(batch.accepted[0].payload, 42);
        assert_eq!(batch.accepted[0].requested_revisions, revisions);
        assert_eq!(batch.accepted[0].revision_mask, RevisionMask::ALL);
    }

    #[test]
    fn dispatcher_rejects_only_relevant_stale_revisions() {
        let (dispatcher, inbox) = ui_channel::<&'static str>();
        let window = WindowId::from_parts(0, 1);
        let source = ElementId::from_parts(4, 2).stamp();
        let requested = RevisionSet::new(
            LayoutRevision::new(3),
            SceneRevision::new(5),
            ResourceRevision::new(7),
            SemanticRevision::new(11),
        );
        let current = RevisionSet::new(
            requested.layout,
            SceneRevision::new(6),
            requested.resource,
            requested.semantic,
        );

        dispatcher
            .dispatch_with_mask(window, source, requested, RevisionMask::LAYOUT, "accepted")
            .unwrap();
        dispatcher
            .dispatch_with_mask(window, source, requested, RevisionMask::SCENE, "stale")
            .unwrap();
        dispatcher
            .dispatch_with_mask(
                window,
                ElementId::from_parts(4, 3).stamp(),
                requested,
                RevisionMask::NONE,
                "wrong generation",
            )
            .unwrap();

        let batch = inbox.drain_current(window, source, current).unwrap();
        assert_eq!(batch.accepted.len(), 1);
        assert_eq!(batch.accepted[0].payload, "accepted");
        assert_eq!(batch.stale, 2);
    }

    #[test]
    fn state_writes_coalesce_and_invalidate_each_dependency_once() {
        let state = State::new(1_i32);
        let element = ElementId::from_parts(7, 1);
        let owner = DependencyOwner::new();
        let (value, dependencies) =
            capture_dependencies(owner, element, DependencyPhase::Paint, || state.get()).unwrap();
        assert_eq!(value, 1);
        assert_eq!(dependencies.len(), 1);

        let mut txn = UpdateTxn::<()>::new();
        state.set(&mut txn, 2).unwrap();
        state.update(&mut txn, |value| *value += 3).unwrap();
        state.set(&mut txn, 9).unwrap();
        assert_eq!(txn.pending_state_count(), 1);
        assert_eq!(txn.queued_state_write_count(), 3);

        let receipt = txn.commit(|_| Ok(())).unwrap();
        assert_eq!(state.get().unwrap(), 9);
        assert_eq!(receipt.state_write_count, 3);
        assert_eq!(receipt.changed_state_count, 1);
        assert_eq!(
            receipt.invalidations(),
            &[StateInvalidation::new(
                owner,
                element,
                DependencyPhase::Paint
            )]
        );

        drop(dependencies);
        let mut txn = UpdateTxn::<()>::new();
        state.set(&mut txn, 10).unwrap();
        let receipt = txn.commit(|_| Ok(())).unwrap();
        assert!(receipt.invalidations().is_empty());
    }

    #[test]
    fn final_value_equal_to_original_produces_no_change() {
        let state = State::new(String::from("original"));
        let mut txn = UpdateTxn::<()>::new();
        state.set(&mut txn, String::from("temporary")).unwrap();
        state.set(&mut txn, String::from("original")).unwrap();
        let receipt = txn.commit(|_| Ok(())).unwrap();

        assert_eq!(receipt.state_write_count, 2);
        assert_eq!(receipt.changed_state_count, 0);
        assert!(receipt.invalidations().is_empty());
    }

    #[test]
    fn transaction_updaters_observe_one_pre_commit_snapshot() {
        let first = State::new(1_u32);
        let second = State::new(10_u32);
        let observed = Rc::new(Cell::new(0_u32));

        let mut txn = UpdateTxn::<()>::new();
        first.set(&mut txn, 2).unwrap();
        second
            .update(&mut txn, {
                let first = first.clone();
                let observed = observed.clone();
                move |value| {
                    let first = first.get().unwrap();
                    observed.set(first);
                    *value += first;
                }
            })
            .unwrap();

        let receipt = txn.commit(|_| Ok(())).unwrap();
        assert_eq!(receipt.changed_state_count, 2);
        assert_eq!(observed.get(), 1);
        assert_eq!(first.get().unwrap(), 2);
        assert_eq!(second.get().unwrap(), 11);
    }

    #[test]
    fn transaction_commit_rejects_nested_state_publication() {
        let outer = State::new(1_u32);
        let nested = State::new(10_u32);
        let mut nested_txn = UpdateTxn::<()>::new();
        nested.set(&mut nested_txn, 20).unwrap();
        let nested_error = Rc::new(RefCell::new(None::<String>));

        let mut outer_txn = UpdateTxn::<()>::new();
        outer
            .update(&mut outer_txn, {
                let nested_error = nested_error.clone();
                move |value| {
                    *value = 2;
                    let error = nested_txn.commit(|_| Ok(())).unwrap_err();
                    *nested_error.borrow_mut() = Some(error.to_string());
                }
            })
            .unwrap();

        outer_txn.commit(|_| Ok(())).unwrap();
        assert_eq!(outer.get().unwrap(), 2);
        assert_eq!(nested.get().unwrap(), 10);
        assert!(
            nested_error
                .borrow()
                .as_deref()
                .unwrap()
                .contains("state transaction commit")
        );
    }

    #[test]
    fn failed_preflight_drops_updaters_under_the_commit_guard() {
        struct CommitOnDrop {
            transaction: Option<UpdateTxn<()>>,
            nested_commit_succeeded: Rc<Cell<bool>>,
        }

        impl Drop for CommitOnDrop {
            fn drop(&mut self) {
                let transaction = self.transaction.take().unwrap();
                self.nested_commit_succeeded
                    .set(transaction.commit(|_| Ok(())).is_ok());
            }
        }

        let outer = State::new(1_u32);
        let nested = State::new(10_u32);
        let mut nested_txn = UpdateTxn::<()>::new();
        nested.set(&mut nested_txn, 20).unwrap();
        let nested_commit_succeeded = Rc::new(Cell::new(false));
        let commit_on_drop = CommitOnDrop {
            transaction: Some(nested_txn),
            nested_commit_succeeded: nested_commit_succeeded.clone(),
        };

        let mut outer_txn = UpdateTxn::<()>::new();
        outer
            .update(&mut outer_txn, move |_| drop(commit_on_drop))
            .unwrap();

        let commit_result = outer
            .with(|_| outer_txn.commit(|_| Ok(())))
            .expect("the outer read itself remains valid");
        assert!(commit_result.is_err());
        assert!(!nested_commit_succeeded.get());
        assert_eq!(outer.get().unwrap(), 1);
        assert_eq!(nested.get().unwrap(), 10);
    }

    #[test]
    fn failed_late_preflight_publishes_none_of_the_batch() {
        let first = State::new(1_u32);
        let second = State::new(10_u32);
        let mut transaction = UpdateTxn::<()>::new();
        first.set(&mut transaction, 2).unwrap();
        second.set(&mut transaction, 20).unwrap();

        let commit_result = second
            .with(|_| transaction.commit(|_| Ok(())))
            .expect("the blocking read itself remains valid");
        assert!(commit_result.is_err());
        assert_eq!(first.get().unwrap(), 1);
        assert_eq!(second.get().unwrap(), 10);
    }

    #[test]
    fn old_value_drop_observes_the_fully_published_snapshot() {
        struct Observer {
            version: u32,
            observe_on_drop: bool,
            first: State<u32>,
            second: State<u32>,
            observations: Rc<RefCell<Vec<(u32, u32)>>>,
        }

        impl Clone for Observer {
            fn clone(&self) -> Self {
                Self {
                    version: self.version,
                    observe_on_drop: false,
                    first: self.first.clone(),
                    second: self.second.clone(),
                    observations: self.observations.clone(),
                }
            }
        }

        impl PartialEq for Observer {
            fn eq(&self, other: &Self) -> bool {
                self.version == other.version
            }
        }

        impl Drop for Observer {
            fn drop(&mut self) {
                if self.observe_on_drop {
                    self.observations
                        .borrow_mut()
                        .push((self.first.get().unwrap(), self.second.get().unwrap()));
                }
            }
        }

        let first = State::new(1_u32);
        let second = State::new(10_u32);
        let observations = Rc::new(RefCell::new(Vec::new()));
        let observer = State::new(Observer {
            version: 0,
            observe_on_drop: true,
            first: first.clone(),
            second: second.clone(),
            observations: observations.clone(),
        });

        let mut transaction = UpdateTxn::<()>::new();
        first.set(&mut transaction, 2).unwrap();
        observer
            .update(&mut transaction, |value| value.version = 1)
            .unwrap();
        second.set(&mut transaction, 20).unwrap();
        transaction.commit(|_| Ok(())).unwrap();

        assert_eq!(observations.borrow().as_slice(), &[(2, 20)]);
    }

    #[test]
    fn rejected_command_batch_discards_pending_state() {
        let state = State::new(1_u32);
        let mut txn = UpdateTxn::<u32>::new();
        txn.push(5).unwrap();
        state.set(&mut txn, 2).unwrap();

        let result = txn.commit(|_| Err(Error::compile("event_commands", "rejected")));
        assert!(result.is_err());
        assert_eq!(state.get().unwrap(), 1);
    }

    #[test]
    fn tracked_read_phases_forbid_state_writes() {
        let state = State::new(1_u32);
        let element = ElementId::from_parts(1, 1);
        let owner = DependencyOwner::new();
        let mut txn = UpdateTxn::<()>::new();
        let ((), dependencies) =
            capture_dependencies(owner, element, DependencyPhase::Build, || {
                let error = state.set(&mut txn, 2).unwrap_err();
                assert!(error.to_string().contains("Build"));
                state.get().map(|_| ())
            })
            .unwrap();
        assert_eq!(dependencies.len(), 1);
        assert_eq!(state.get().unwrap(), 1);
    }

    #[test]
    fn derived_signals_propagate_and_replace_dynamic_dependencies() {
        let choose_left = State::new(true);
        let left = State::new(2_i32);
        let right = State::new(10_i32);
        let selected = Signal::derive({
            let choose_left = choose_left.clone();
            let left = left.clone();
            let right = right.clone();
            move || {
                if choose_left.get()? {
                    left.get()
                } else {
                    right.get()
                }
            }
        });
        let doubled = selected.map(|value| value * 2);
        let element = ElementId::from_parts(2, 1);
        let owner = DependencyOwner::new();
        let (value, _dependencies) =
            capture_dependencies(owner, element, DependencyPhase::Layout, || doubled.get())
                .unwrap();
        assert_eq!(value, 4);

        let mut txn = UpdateTxn::<()>::new();
        choose_left.set(&mut txn, false).unwrap();
        let receipt = txn.commit(|_| Ok(())).unwrap();
        assert_eq!(
            receipt.invalidations(),
            &[StateInvalidation::new(
                owner,
                element,
                DependencyPhase::Layout
            )]
        );
        assert_eq!(doubled.get().unwrap(), 20);

        // Re-evaluation detached `left` and attached `right`.
        let mut txn = UpdateTxn::<()>::new();
        left.set(&mut txn, 3).unwrap();
        let receipt = txn.commit(|_| Ok(())).unwrap();
        assert!(receipt.invalidations().is_empty());

        let mut txn = UpdateTxn::<()>::new();
        right.set(&mut txn, 11).unwrap();
        let receipt = txn.commit(|_| Ok(())).unwrap();
        assert_eq!(receipt.invalidations().len(), 1);
        assert_eq!(doubled.get().unwrap(), 22);
    }

    #[test]
    fn dirty_derived_signal_keeps_propagating_after_evaluation_error() {
        let source = State::new(1_i32);
        let fail = Rc::new(Cell::new(false));
        let derived = Signal::derive({
            let source = source.clone();
            let fail = fail.clone();
            move || {
                let value = source.get()?;
                if fail.get() {
                    Err(Error::compile("test_signal", "requested failure"))
                } else {
                    Ok(value)
                }
            }
        });
        let owner = DependencyOwner::new();
        let element = ElementId::from_parts(12, 1);
        let (value, dependencies) =
            capture_dependencies(owner, element, DependencyPhase::Build, || derived.get()).unwrap();
        assert_eq!(value, 1);

        fail.set(true);
        let mut txn = UpdateTxn::<()>::new();
        source.set(&mut txn, 2).unwrap();
        let receipt = txn.commit(|_| Ok(())).unwrap();
        assert_eq!(receipt.invalidations().len(), 1);
        assert!(derived.get().is_err());

        fail.set(false);
        let mut txn = UpdateTxn::<()>::new();
        source.set(&mut txn, 3).unwrap();
        let receipt = txn.commit(|_| Ok(())).unwrap();
        assert_eq!(
            receipt.invalidations(),
            &[StateInvalidation::new(
                owner,
                element,
                DependencyPhase::Build
            )]
        );
        assert_eq!(derived.get().unwrap(), 3);
        drop(dependencies);
    }

    #[test]
    fn derived_signal_cycles_return_a_diagnostic_error() {
        let first_slot = Rc::new(RefCell::new(None::<Signal<i32>>));
        let second_slot = Rc::new(RefCell::new(None::<Signal<i32>>));
        let first = Signal::derive({
            let second_slot = second_slot.clone();
            move || second_slot.borrow().as_ref().unwrap().get()
        });
        let second = Signal::derive({
            let first_slot = first_slot.clone();
            move || first_slot.borrow().as_ref().unwrap().get()
        });
        *first_slot.borrow_mut() = Some(first.clone());
        *second_slot.borrow_mut() = Some(second);

        let error = first.get().unwrap_err();
        assert!(error.to_string().contains("cycle or reentrant"));
    }

    #[test]
    fn derived_signal_cannot_enqueue_reentrant_state_updates() {
        let state = State::new(1_u32);
        let transaction = Rc::new(RefCell::new(UpdateTxn::<()>::new()));
        let derived = Signal::derive({
            let state = state.clone();
            let transaction = transaction.clone();
            move || {
                state.set(&mut transaction.borrow_mut(), 2)?;
                Ok(0_u32)
            }
        });

        let error = derived.get().unwrap_err();
        assert!(error.to_string().contains("derived signal"));
        assert_eq!(state.get().unwrap(), 1);
    }
}
