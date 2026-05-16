use std::sync::{Arc, Mutex};

use crate::foundation::binding::{
    record_dependency_read, DependencyId, InvalidationSignal, Signal,
};

/// 可由动画系统驱动、并可暴露为 `Signal` 的可变值。
#[derive(Clone)]
pub struct AnimatedValue<T> {
    value: Arc<Mutex<T>>,
    invalidation: InvalidationSignal,
    dependency: DependencyId,
}

impl<T> AnimatedValue<T> {
    pub(crate) fn new(value: T, invalidation: InvalidationSignal) -> Self {
        Self {
            value: Arc::new(Mutex::new(value)),
            invalidation,
            dependency: DependencyId::next(),
        }
    }

    /// 直接设置当前值，并触发依赖失效。
    pub fn set(&self, value: T) {
        *self.value.lock().expect("animated value lock poisoned") = value;
        self.invalidation.mark_dependency_dirty(self.dependency);
    }

    /// 返回可供视图层订阅的信号。
    pub fn signal(&self) -> Signal<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        let animated = self.clone();
        Signal::new_tracked(
            move || animated.get(),
            self.invalidation.clone(),
            Some(self.dependency),
        )
    }
}

impl<T: Clone> AnimatedValue<T> {
    /// 读取当前值，并记录依赖访问。
    pub fn get(&self) -> T {
        record_dependency_read(Some(self.dependency));
        self.value
            .lock()
            .expect("animated value lock poisoned")
            .clone()
    }
}

impl<T: PartialEq> AnimatedValue<T> {
    pub(super) fn set_if_changed(&self, value: T) -> bool {
        let mut current = self.value.lock().expect("animated value lock poisoned");
        if *current == value {
            return false;
        }
        *current = value;
        drop(current);
        self.invalidation.mark_dependency_dirty(self.dependency);
        true
    }
}
