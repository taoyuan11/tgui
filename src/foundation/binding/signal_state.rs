use std::sync::Arc;

use crate::animation::Transition;

use super::dependency::{record_dependency_read, DependencyId};
use super::invalidation::InvalidationSignal;

/// 表示共享的可变响应式状态。
///
/// 通常通过 [`super::ViewModelContext::state`] 创建，再通过 [`State::signal`]
/// 派生出供 UI 使用的只读信号。
#[derive(Clone)]
pub struct State<T> {
    value: Arc<parking_lot::Mutex<T>>,
    invalidation: InvalidationSignal,
    dependency: DependencyId,
}

impl<T> State<T> {
    pub(crate) fn new(value: T, invalidation: InvalidationSignal) -> Self {
        Self {
            value: Arc::new(parking_lot::Mutex::new(value)),
            invalidation,
            dependency: DependencyId::next(),
        }
    }

    /// 读取当前值而不克隆它。
    ///
    /// 参数:
    /// - `reader`: 读取内部值的闭包。
    ///
    /// 返回值: 闭包返回的结果。
    pub fn read<R>(&self, reader: impl FnOnce(&T) -> R) -> R {
        record_dependency_read(Some(self.dependency));
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
        Signal::from_state(self.clone(), self.invalidation.clone(), self.dependency)
    }

    /// 基于借用式读取创建派生信号，避免在 `map` 链路中提前克隆源值。
    pub fn project<U>(&self, projector: impl Fn(&T) -> U + Send + Sync + 'static) -> Signal<U>
    where
        T: Clone + Send + Sync + 'static,
        U: Clone + Send + Sync + 'static,
    {
        let state = self.clone();
        Signal::new_tracked(
            move || state.read(|value| projector(value)),
            self.invalidation.clone(),
            Some(self.dependency),
        )
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
        self.invalidation.mark_dependency_dirty(self.dependency);
    }
}

impl<T: Clone> State<T> {
    /// 获取当前值的克隆副本。
    ///
    /// 返回值: 当前状态值。
    pub fn get(&self) -> T {
        record_dependency_read(Some(self.dependency));
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
            self.invalidation.mark_dependency_dirty(self.dependency);
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
        self.invalidation.mark_dependency_dirty(self.dependency);
        result
    }
}

/// 表示供部件和窗口绑定使用的惰性只读值。
#[derive(Clone)]
pub struct Signal<T> {
    reader: Arc<dyn SignalReader<T>>,
    invalidation: InvalidationSignal,
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
        Self {
            reader: Arc::new(reader),
            invalidation,
            cache: Arc::new(parking_lot::Mutex::new(SignalCache {
                revision: 0,
                dependency_revision: None,
                value: None,
            })),
            transition: None,
            dependency,
        }
    }

    fn with_transition(mut self, transition: Option<Transition>) -> Self {
        self.transition = transition;
        self
    }

    pub(crate) fn from_state(
        state: State<T>,
        invalidation: InvalidationSignal,
        dependency: DependencyId,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
    {
        Self::new_tracked(move || state.get(), invalidation, Some(dependency))
    }
}

impl<T: Clone> Signal<T> {
    /// 读取当前信号值。
    ///
    /// 返回值: 当前信号值的克隆副本。
    pub fn get(&self) -> T {
        record_dependency_read(self.dependency);
        let revision = self.invalidation.revision();
        let dependency_revision = self
            .dependency
            .and_then(|dependency| self.invalidation.dependency_revision(dependency));
        {
            let cache = self.cache.lock();
            let cache_hit = match self.dependency {
                Some(_) => cache.dependency_revision == dependency_revision,
                None => cache.revision == revision,
            };
            if cache_hit {
                if let Some(value) = cache.value.as_ref() {
                    return value.clone();
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

    /// 基于当前信号派生一个新的缓存信号。
    ///
    /// 参数:
    /// - `mapper`: 将当前值映射为新值的闭包。
    ///
    /// 返回值: 派生后的新信号对象。
    pub fn map<U>(&self, mapper: impl Fn(T) -> U + Send + Sync + 'static) -> Signal<U>
    where
        T: Clone + Send + Sync + 'static,
        U: Clone + Send + Sync + 'static,
    {
        let signal = self.clone();
        Signal::new_tracked(
            move || mapper(signal.get()),
            self.invalidation.clone(),
            self.dependency,
        )
        .with_transition(self.transition)
    }

    /// 以借用式投影派生新的信号，避免 `map` 先克隆源值。
    pub fn project<U>(&self, projector: impl Fn(&T) -> U + Send + Sync + 'static) -> Signal<U>
    where
        T: Clone + Send + Sync + 'static,
        U: Clone + Send + Sync + 'static,
    {
        let signal = self.clone();
        Signal::new_tracked(
            move || signal.read(|value| projector(value)),
            self.invalidation.clone(),
            self.dependency,
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
