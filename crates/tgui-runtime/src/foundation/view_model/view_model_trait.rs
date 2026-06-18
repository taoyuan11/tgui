use crate::foundation::binding::ViewModelContext;
use crate::ui::widget::Element;

/// 标记可作为 `tgui` 应用根视图模型的类型。
///
/// 该 trait 用于约束应用运行时所需的构造和视图输出能力。
pub trait ViewModel: Send + 'static {
    /// 使用给定的视图模型上下文创建实例。
    ///
    /// 参数：
    /// - `context`：用于创建状态、信号和动画值的上下文。
    fn new(context: &ViewModelContext) -> Self;

    /// 构建当前视图模型对应的根视图。
    ///
    /// 返回值：
    /// - 返回当前视图模型的根 `Element`。
    fn view(&self) -> Element<Self>
    where
        Self: Sized;
}
