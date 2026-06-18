use super::{Axis, Container, ContainerKind, ContainerLayout, Flex, WidgetKind, Wrap};

impl<VM> Flex<VM> {
    /// 创建弹性容器。
    ///
    /// # 参数
    /// - `direction`：主轴方向。
    ///
    /// # 返回值
    /// 返回一个采用 `Flex` 布局策略的容器实例。
    pub fn new(direction: Axis) -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Flex {
                direction,
                wrap: Wrap::NoWrap,
            },
            ..ContainerLayout::flow()
        }))
    }

    /// 创建水平方向的弹性容器。
    ///
    /// # 返回值
    /// 返回主轴为水平方向的容器实例。
    pub fn horizontal() -> Self {
        Self::new(Axis::Horizontal)
    }

    /// 创建垂直方向的弹性容器。
    ///
    /// # 返回值
    /// 返回主轴为垂直方向的容器实例。
    pub fn vertical() -> Self {
        Self::new(Axis::Vertical)
    }

    /// 更新弹性容器主轴方向。
    ///
    /// # 参数
    /// - `direction`：新的主轴方向。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn direction(mut self, direction: Axis) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.0.element.kind {
            layout.kind = match layout.kind.clone() {
                ContainerKind::Flex { wrap, .. } => ContainerKind::Flex { direction, wrap },
                other => other,
            };
        }
        self
    }

    /// 更新弹性容器换行策略。
    ///
    /// # 参数
    /// - `wrap`：新的换行配置。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn wrap(mut self, wrap: Wrap) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.0.element.kind {
            layout.kind = match layout.kind.clone() {
                ContainerKind::Flex { direction, .. } => ContainerKind::Flex { direction, wrap },
                other => other,
            };
        }
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.0 = self.0.focusable(focusable);
        self
    }

    pub fn tab_index(mut self, tab_index: i32) -> Self {
        self.0 = self.0.tab_index(tab_index);
        self
    }

    pub fn focus_scope(mut self, options: crate::ui::widget::FocusScopeOptions) -> Self {
        self.0 = self.0.focus_scope(options);
        self
    }

    pub fn auto_focus_first(mut self, auto_focus_first: bool) -> Self {
        self.0 = self.0.auto_focus_first(auto_focus_first);
        self
    }
}
