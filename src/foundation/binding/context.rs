use crate::animation::{AnimatedValue, AnimationControllerBuilder, AnimationCoordinator};

use super::invalidation::InvalidationSignal;
use super::signal_state::{Signal, State};
use super::text::TextController;

/// 提供给 ViewModel 构造函数的上下文对象。
///
/// 它负责创建响应式状态、信号、文本控制器以及动画相关句柄。
#[derive(Clone)]
pub struct ViewModelContext {
    invalidation: InvalidationSignal,
    animations: AnimationCoordinator,
}

impl ViewModelContext {
    pub(crate) fn new(invalidation: InvalidationSignal, animations: AnimationCoordinator) -> Self {
        Self {
            invalidation,
            animations,
        }
    }

    /// 创建一份可写的响应式状态。
    ///
    /// 参数:
    /// - `value`: 状态初始值。
    ///
    /// 返回值: 会在值变化时自动触发失效通知的状态对象。
    pub fn state<T>(&self, value: T) -> State<T> {
        State::new(value, self.invalidation.clone())
    }

    /// 创建一个只读信号。
    ///
    /// 参数:
    /// - `reader`: 读取当前值的闭包。
    ///
    /// 返回值: 带缓存的惰性信号对象。
    pub fn signal<T>(&self, reader: impl Fn() -> T + Send + Sync + 'static) -> Signal<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        Signal::new(reader, self.invalidation.clone())
    }

    /// 创建一个可供时间线动画驱动的可动画值。
    ///
    /// 参数:
    /// - `value`: 动画值初始值。
    ///
    /// 返回值: 可与时间线控制器配合使用的动画值对象。
    pub fn animated_value<T>(&self, value: T) -> AnimatedValue<T> {
        AnimatedValue::new(value, self.invalidation.clone())
    }

    /// 创建一个时间线动画控制器构建器。
    ///
    /// 返回值: 可继续配置关键帧和播放参数的构建器。
    pub fn timeline(&self) -> AnimationControllerBuilder {
        AnimationControllerBuilder::new(self.animations.clone(), self.invalidation.clone())
    }

    /// 创建一个保留式文本控制器。
    ///
    /// 参数:
    /// - `initial_text`: 初始文本内容。
    ///
    /// 返回值: 适用于 `Input` 和 `Textarea` 的文本控制器。
    pub fn text_controller(&self, initial_text: impl Into<String>) -> TextController {
        TextController::new(initial_text, self.invalidation.clone())
    }
}
