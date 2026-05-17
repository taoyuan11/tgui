use super::{Container, ContainerKind, ContainerLayout, Stack};

impl<VM> Stack<VM> {
    /// 创建堆叠容器。
    ///
    /// # 返回值
    /// 返回一个采用 `Stack` 布局策略的容器实例。
    pub fn new() -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Stack,
            ..ContainerLayout::flow()
        }))
    }
}

impl<VM> Default for Stack<VM> {
    fn default() -> Self {
        Self::new()
    }
}
