use super::{Container, ContainerKind, ContainerLayout, Grid, Track, WidgetKind};

impl<VM> Grid<VM> {
    /// 创建仅指定列轨道的网格容器。
    ///
    /// # 参数
    /// - `columns`：网格列轨道定义。
    ///
    /// # 返回值
    /// 返回一个采用 `Grid` 布局策略的容器实例。
    pub fn columns<const N: usize>(columns: [Track; N]) -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Grid {
                columns: columns.into_iter().collect(),
                rows: Vec::new(),
            },
            ..ContainerLayout::flow()
        }))
    }

    /// 创建仅指定行轨道的网格容器。
    ///
    /// # 参数
    /// - `rows`：网格行轨道定义。
    ///
    /// # 返回值
    /// 返回一个采用 `Grid` 布局策略的容器实例。
    pub fn rows<const N: usize>(rows: [Track; N]) -> Self {
        Self(Container::with_layout(ContainerLayout {
            kind: ContainerKind::Grid {
                columns: Vec::new(),
                rows: rows.into_iter().collect(),
            },
            ..ContainerLayout::flow()
        }))
    }

    /// 更新网格列轨道定义。
    ///
    /// # 参数
    /// - `columns`：新的列轨道定义。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn set_columns<const N: usize>(mut self, columns: [Track; N]) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.0.element.kind {
            layout.kind = match layout.kind.clone() {
                ContainerKind::Grid { rows, .. } => ContainerKind::Grid {
                    columns: columns.into_iter().collect(),
                    rows,
                },
                other => other,
            };
        }
        self
    }

    /// 更新网格行轨道定义。
    ///
    /// # 参数
    /// - `rows`：新的行轨道定义。
    ///
    /// # 返回值
    /// 返回更新后的容器实例。
    pub fn set_rows<const N: usize>(mut self, rows: [Track; N]) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.0.element.kind {
            layout.kind = match layout.kind.clone() {
                ContainerKind::Grid { columns, .. } => ContainerKind::Grid {
                    columns,
                    rows: rows.into_iter().collect(),
                },
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

    pub fn focus_scope(
        mut self,
        options: crate::ui::widget::FocusScopeOptions,
    ) -> Self {
        self.0 = self.0.focus_scope(options);
        self
    }
}
