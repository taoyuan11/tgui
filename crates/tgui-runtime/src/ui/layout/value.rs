use std::fmt;

use crate::animation::Transition;
use crate::foundation::binding::Signal;

/// 表示静态值或响应式信号值的统一包装。
///
/// 该类型用于布局、样式等接口，允许调用方传入常量或 `Signal`。
#[derive(Clone)]
pub enum Value<T> {
    Static(T),
    Signal(Signal<T>),
}

impl<T: Clone> Value<T> {
    /// 解析当前值。
    ///
    /// # 返回值
    /// 如果是静态值则直接克隆返回；如果是信号值则读取信号当前快照。
    pub fn resolve(&self) -> T {
        match self {
            Self::Static(value) => value.clone(),
            Self::Signal(signal) => signal.get(),
        }
    }

    pub fn resolve_ref<R>(&self, reader: impl FnOnce(&T) -> R) -> R {
        match self {
            Self::Static(value) => reader(value),
            Self::Signal(signal) => signal.read(reader),
        }
    }

    pub(crate) fn transition(&self) -> Option<Transition> {
        match self {
            Self::Static(_) => None,
            Self::Signal(signal) => signal.transition(),
        }
    }

    pub(crate) fn resolve_untracked(&self) -> T {
        match self {
            Self::Static(value) => value.clone(),
            Self::Signal(signal) => signal.get_untracked(),
        }
    }

    /// Apply a component's default transition without replacing an explicit
    /// transition supplied by the caller.
    ///
    /// `None` deliberately strips either transition. Components use that path
    /// for reduced motion (and zero-duration motion themes), so an already
    /// active engine slot is settled the next time the value is resolved.
    pub(crate) fn with_default_transition(self, transition: Option<Transition>) -> Self {
        match self {
            Self::Static(value) => Self::Static(value),
            Self::Signal(signal) => match transition {
                None => Self::Signal(signal.without_transition()),
                Some(transition) if signal.transition().is_none() => {
                    Self::Signal(signal.animated(transition))
                }
                Some(_) => Self::Signal(signal),
            },
        }
    }

    /// Structural identity used while diffing runtime-layout overrides.
    ///
    /// `PartialEq` intentionally compares resolved values, which is ideal for
    /// style semantics but cannot detect replacing one equally-valued signal
    /// with another or attaching a transition to the same signal. Runtime
    /// layout needs that distinction so it propagates only fields the resolver
    /// actually changed without freezing unrelated theme/state style fields.
    pub(crate) fn same_binding(&self, other: &Self) -> bool
    where
        T: PartialEq,
    {
        match (self, other) {
            (Self::Static(left), Self::Static(right)) => left == right,
            (Self::Signal(left), Self::Signal(right)) => {
                left.sync_identity() == right.sync_identity()
                    && left.transition() == right.transition()
            }
            _ => false,
        }
    }
}

impl<T: Clone + PartialEq> PartialEq for Value<T> {
    fn eq(&self, other: &Self) -> bool {
        self.resolve() == other.resolve()
    }
}

impl<T> From<T> for Value<T> {
    fn from(value: T) -> Self {
        Self::Static(value)
    }
}

impl<T> From<Signal<T>> for Value<T> {
    fn from(value: Signal<T>) -> Self {
        Self::Signal(value)
    }
}

impl From<&str> for Value<String> {
    fn from(value: &str) -> Self {
        Self::Static(value.to_string())
    }
}

impl<T: fmt::Debug> fmt::Debug for Value<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(value) => f.debug_tuple("Static").field(value).finish(),
            Self::Signal(_) => f.write_str("Signal(..)"),
        }
    }
}
