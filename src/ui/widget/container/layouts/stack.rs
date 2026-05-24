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

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.0 = self.0.focusable(focusable);
        self
    }

    pub fn tab_index(mut self, tab_index: i32) -> Self {
        self.0 = self.0.tab_index(tab_index);
        self
    }

    pub fn focus_scope(
        mut self,
        options: crate::ui::widget::FocusScopeOptions,
    ) -> Self {
        self.0 = self.0.focus_scope(options);
        self
    }
}

impl<VM> Default for Stack<VM> {
    fn default() -> Self {
        Self::new()
    }
}
