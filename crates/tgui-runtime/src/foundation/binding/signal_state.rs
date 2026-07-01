use std::sync::Arc;

use crate::animation::Transition;

use super::dependency::{record_dependency_read, DependencyId};
use super::invalidation::InvalidationSignal;
use super::reactive::{ReactiveGraph, ReactiveTarget, SignalId};

/// 表示共享的可变响应式状态。
///
/// 通常通过 [`super::ViewModelContext::state`] 创建，再通过 [`State::signal`]
/// 派生出供 UI 使用的只读信号。
#[derive(Clone)]
pub struct State<T> {
    value: Arc<parking_lot::Mutex<T>>,
    invalidation: InvalidationSignal,
    dependency: DependencyId,
    signal_id: SignalId,
}

impl<T> State<T> {
    pub(crate) fn new(value: T, invalidation: InvalidationSignal) -> Self {
        let signal_id = invalidation.reactive_graph().create_signal();
        Self {
            value: Arc::new(parking_lot::Mutex::new(value)),
            invalidation,
            dependency: DependencyId::next(),
            signal_id,
        }
    }

    /// 读取当前值而不克隆它。
    ///
    /// 参数:
    /// - `reader`: 读取内部值的闭包。
    ///
    /// 返回值: 闭包返回的结果。
    pub fn read<R>(&self, reader: impl FnOnce(&T) -> R) -> R {
        self.track_read();
        let value = self.value.lock();
        reader(&value)
    }

    /// 以借用方式访问当前状态值，避免额外克隆。
    pub fn with_ref<R>(&self, reader: impl FnOnce(&T) -> R) -> R {
        self.read(reader)
    }

    #[cfg(feature = "video")]
    pub(crate) fn invalidation(&self) -> &InvalidationSignal {
        &self.invalidation
    }

    /// 创建一个按需读取状态值的缓存信号。
    ///
    /// 返回值: 与当前状态关联的只读信号。
    pub fn signal(&self) -> Signal<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        Signal::from_state(
            self.clone(),
            self.invalidation.clone(),
            self.dependency,
            self.signal_id,
        )
    }

    /// 基于借用式读取创建派生信号，避免在 `map` 链路中提前克隆源值。
    pub fn project<U>(&self, projector: impl Fn(&T) -> U + Send + Sync + 'static) -> Signal<U>
    where
        T: Clone + Send + Sync + 'static,
        U: Clone + PartialEq + Send + Sync + 'static,
    {
        let state = self.clone();
        Signal::new_memo_tracked(
            move || state.read(|value| projector(value)),
            self.invalidation.clone(),
            Some(self.dependency),
            [self.signal_id],
            Some(Arc::new(|left: &U, right: &U| left == right)),
        )
    }

    fn mark_changed(&self) {
        self.invalidation.mark_signal_dirty(self.signal_id);
        self.invalidation.mark_dependency_dirty(self.dependency);
    }

    fn track_read(&self) {
        if let Some(owner) = record_dependency_read(Some(self.dependency)) {
            self.invalidation
                .reactive_graph()
                .subscribe_target(self.signal_id, ReactiveTarget::Owner(owner));
        }
    }
}

impl<T: PartialEq> State<T> {
    /// 设置状态值，仅在值变化时触发失效通知。
    ///
    /// 参数:
    /// - `value`: 新状态值。
    pub fn set(&self, value: T) {
        let mut current = self.value.lock();
        if *current == value {
            return;
        }
        *current = value;
        drop(current);
        self.mark_changed();
    }
}

impl<T: Clone> State<T> {
    /// 获取当前值的克隆副本。
    ///
    /// 返回值: 当前状态值。
    pub fn get(&self) -> T {
        self.track_read();
        self.value.lock().clone()
    }
}

impl<T: Clone + PartialEq> State<T> {
    /// 原地更新状态，仅在值发生变化时触发失效通知。
    ///
    /// 参数:
    /// - `updater`: 对当前值执行原地修改的闭包。
    ///
    /// 返回值: `updater` 的返回结果。
    pub fn update<R>(&self, updater: impl FnOnce(&mut T) -> R) -> R {
        let mut value = self.value.lock();
        let previous = value.clone();
        let result = updater(&mut value);
        let changed = *value != previous;
        drop(value);
        if changed {
            self.mark_changed();
        }
        result
    }
}

impl<T> State<T> {
    /// 原地更新状态，并在更新后无条件触发失效通知。
    ///
    /// 参数:
    /// - `updater`: 对当前值执行原地修改的闭包。
    ///
    /// 返回值: `updater` 的返回结果。
    pub fn mutate<R>(&self, updater: impl FnOnce(&mut T) -> R) -> R {
        let mut value = self.value.lock();
        let result = updater(&mut value);
        drop(value);
        self.mark_changed();
        result
    }
}

/// 表示供部件和窗口绑定使用的惰性只读值。
#[derive(Clone)]
pub struct Signal<T> {
    reader: Arc<dyn SignalReader<T>>,
    invalidation: InvalidationSignal,
    graph: ReactiveGraph,
    signal_id: SignalId,
    cache: Arc<parking_lot::Mutex<SignalCache<T>>>,
    transition: Option<Transition>,
    dependency: Option<DependencyId>,
}

trait SignalReader<T>: Send + Sync {
    fn read(&self) -> T;
}

impl<T, F> SignalReader<T> for F
where
    F: Fn() -> T + Send + Sync,
{
    fn read(&self) -> T {
        self()
    }
}

struct SignalCache<T> {
    revision: u64,
    dependency_revision: Option<u64>,
    value: Option<T>,
}

impl<T> Signal<T> {
    pub(crate) fn new(
        reader: impl Fn() -> T + Send + Sync + 'static,
        invalidation: InvalidationSignal,
    ) -> Self {
        Self::new_tracked(reader, invalidation, None)
    }

    pub(crate) fn new_tracked(
        reader: impl Fn() -> T + Send + Sync + 'static,
        invalidation: InvalidationSignal,
        dependency: Option<DependencyId>,
    ) -> Self {
        let graph = invalidation.reactive_graph();
        let signal_id = graph.create_signal();
        Self::new_with_parts(
            Arc::new(reader),
            invalidation,
            graph,
            signal_id,
            dependency,
            None,
        )
    }

    pub(crate) fn new_memo_tracked<const N: usize>(
        reader: impl Fn() -> T + Send + Sync + 'static,
        invalidation: InvalidationSignal,
        dependency: Option<DependencyId>,
        sources: [SignalId; N],
        equals: Option<Arc<dyn Fn(&T, &T) -> bool + Send + Sync>>,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        let graph = invalidation.reactive_graph();
        let signal_id = graph.create_signal();
        let reader: Arc<dyn SignalReader<T>> = Arc::new(reader);
        let cache = Arc::new(parking_lot::Mutex::new(SignalCache {
            revision: 0,
            dependency_revision: None,
            value: None,
        }));
        let signal = Self {
            reader: reader.clone(),
            invalidation: invalidation.clone(),
            graph: graph.clone(),
            signal_id,
            cache: cache.clone(),
            transition: None,
            dependency,
        };
        let recompute_invalidation = invalidation.clone();
        let recompute = Arc::new(move || {
            let next = reader.read();
            let mut cache = cache.lock();
            let changed = match (cache.value.as_ref(), equals.as_ref()) {
                (Some(previous), Some(equals)) => !equals(previous, &next),
                (Some(_), None) => true,
                (None, _) => true,
            };
            cache.revision = recompute_invalidation.revision();
            cache.dependency_revision = dependency
                .and_then(|dependency| recompute_invalidation.dependency_revision(dependency));
            cache.value = Some(next);
            changed
        });
        graph.register_memo(signal_id, recompute);
        for source in sources {
            graph.subscribe_signal(source, signal_id);
        }
        signal
    }

    fn new_with_parts(
        reader: Arc<dyn SignalReader<T>>,
        invalidation: InvalidationSignal,
        graph: ReactiveGraph,
        signal_id: SignalId,
        dependency: Option<DependencyId>,
        transition: Option<Transition>,
    ) -> Self {
        Self {
            reader,
            invalidation,
            graph,
            signal_id,
            cache: Arc::new(parking_lot::Mutex::new(SignalCache {
                revision: 0,
                dependency_revision: None,
                value: None,
            })),
            transition,
            dependency,
        }
    }

    fn with_transition(mut self, transition: Option<Transition>) -> Self {
        self.transition = transition;
        self
    }

    pub(crate) fn without_transition(mut self) -> Self {
        self.transition = None;
        self
    }

    pub(crate) fn from_state(
        state: State<T>,
        invalidation: InvalidationSignal,
        dependency: DependencyId,
        signal_id: SignalId,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        let graph = invalidation.reactive_graph();
        Self::new_with_parts(
            Arc::new(move || state.get()),
            invalidation,
            graph,
            signal_id,
            Some(dependency),
            None,
        )
    }
}

impl<T: Clone> Signal<T> {
    /// 读取当前信号值。
    ///
    /// 返回值: 当前信号值的克隆副本。
    pub fn get(&self) -> T {
        self.get_with_tracking(true)
    }

    fn get_with_tracking(&self, track: bool) -> T {
        if track {
            self.track_read();
        }
        self.read_cached_value()
    }

    fn track_read(&self) {
        if let Some(owner) = record_dependency_read(self.dependency) {
            self.graph
                .subscribe_target(self.signal_id, ReactiveTarget::Owner(owner));
        }
    }

    fn read_cached_value(&self) -> T {
        let revision = self.invalidation.revision();
        {
            let cache = self.cache.lock();
            if cache.revision == revision {
                if let Some(value) = cache.value.as_ref() {
                    return value.clone();
                }
            }
        }

        let dependency_revision = self
            .dependency
            .and_then(|dependency| self.invalidation.dependency_revision(dependency));
        {
            let mut cache = self.cache.lock();
            let cache_hit = match self.dependency {
                Some(_) => cache.dependency_revision == dependency_revision,
                None => cache.revision == revision,
            };
            if cache_hit {
                if let Some(value) = cache.value.as_ref().cloned() {
                    cache.revision = revision;
                    return value;
                }
            }
        }

        let value = self.reader.read();
        let mut cache = self.cache.lock();
        cache.revision = revision;
        cache.dependency_revision = self
            .dependency
            .and_then(|dependency| self.invalidation.dependency_revision(dependency));
        cache.value = Some(value.clone());
        value
    }

    pub(crate) fn get_uncached(&self) -> T {
        record_dependency_read(self.dependency);
        let value = self.reader.read();
        let mut cache = self.cache.lock();
        cache.revision = self.invalidation.revision();
        cache.dependency_revision = self
            .dependency
            .and_then(|dependency| self.invalidation.dependency_revision(dependency));
        cache.value = Some(value.clone());
        value
    }

    pub(crate) fn get_untracked(&self) -> T {
        self.get_with_tracking(false)
    }

    #[cfg(test)]
    pub(crate) fn subscribe_target(&self, target: ReactiveTarget) {
        self.graph.subscribe_target(self.signal_id, target);
    }

    /// 将信号标记为可动画属性。
    ///
    /// 参数:
    /// - `transition`: 应用于受支持 UI 属性的过渡描述。
    ///
    /// 返回值: 带过渡配置的新信号对象。
    pub fn animated(mut self, transition: impl Into<Transition>) -> Self {
        self.transition = Some(transition.into());
        self
    }

    /// 基于当前信号派生一个有相等性剪枝的 memo 信号。
    ///
    /// 参数:
    /// - `mapper`: 将当前值映射为新值的闭包。
    ///
    /// 返回值: 派生后的新信号对象。
    pub fn map<U>(&self, mapper: impl Fn(T) -> U + Send + Sync + 'static) -> Signal<U>
    where
        T: Clone + Send + Sync + 'static,
        U: Clone + PartialEq + Send + Sync + 'static,
    {
        let signal = self.clone();
        Signal::new_memo_tracked(
            move || mapper(signal.get_untracked()),
            self.invalidation.clone(),
            self.dependency,
            [self.signal_id],
            Some(Arc::new(|left: &U, right: &U| left == right)),
        )
        .with_transition(self.transition)
    }

    /// 基于当前信号派生一个有相等性剪枝的 memo 信号。
    ///
    /// 这是 [`Signal::map`] 的显式 memo 兼容别名。
    pub fn map_memo<U>(&self, mapper: impl Fn(T) -> U + Send + Sync + 'static) -> Signal<U>
    where
        T: Clone + Send + Sync + 'static,
        U: Clone + PartialEq + Send + Sync + 'static,
    {
        let signal = self.clone();
        Signal::new_memo_tracked(
            move || mapper(signal.get_untracked()),
            self.invalidation.clone(),
            self.dependency,
            [self.signal_id],
            Some(Arc::new(|left: &U, right: &U| left == right)),
        )
        .with_transition(self.transition)
    }

    /// 基于当前信号派生一个无相等性剪枝的 legacy 结构信号。
    ///
    /// 这个入口用于显式兼容旧的 signal-driven 结构 rebuild 场景，例如 legacy
    /// `dynamic_child`。普通属性派生应使用 [`Signal::map`] / [`Signal::project`]，
    /// 它们会在值不变时停止传播。
    pub fn map_unchecked<U>(&self, mapper: impl Fn(T) -> U + Send + Sync + 'static) -> Signal<U>
    where
        T: Clone + Send + Sync + 'static,
        U: Clone + Send + Sync + 'static,
    {
        let signal = self.clone();
        Signal::new_memo_tracked(
            move || mapper(signal.get_untracked()),
            self.invalidation.clone(),
            self.dependency,
            [self.signal_id],
            None,
        )
        .with_transition(self.transition)
    }

    /// 以借用式投影派生新的信号，避免 `map` 先克隆源值。
    pub fn project<U>(&self, projector: impl Fn(&T) -> U + Send + Sync + 'static) -> Signal<U>
    where
        T: Clone + Send + Sync + 'static,
        U: Clone + PartialEq + Send + Sync + 'static,
    {
        let signal = self.clone();
        Signal::new_memo_tracked(
            move || {
                let value = signal.get_untracked();
                projector(&value)
            },
            self.invalidation.clone(),
            self.dependency,
            [self.signal_id],
            Some(Arc::new(|left: &U, right: &U| left == right)),
        )
        .with_transition(self.transition)
    }

    pub(crate) fn read<R>(&self, reader: impl FnOnce(&T) -> R) -> R {
        let value = self.get();
        reader(&value)
    }

    pub(crate) fn transition(&self) -> Option<Transition> {
        self.transition
    }
}
