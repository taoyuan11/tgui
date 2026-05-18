macro_rules! impl_container_properties {
    ($name:ident) => {
        impl<VM> $name<VM> {
            /// 设置容器样式解析器。
            ///
            /// # 参数
            /// - `resolver`：根据主题模式生成容器样式的回调。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn style(
                self,
                resolver: impl Fn(ResolvedThemeMode) -> ContainerStyle + Send + Sync + 'static,
            ) -> Self {
                Self(self.0.style(resolver))
            }

            /// 注册点击命令。
            ///
            /// # 参数
            /// - `command`：点击时执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_click(self, command: Command<VM>) -> Self {
                Self(self.0.on_click(command))
            }

            /// 注册双击命令。
            ///
            /// # 参数
            /// - `command`：双击时执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_double_click(self, command: Command<VM>) -> Self {
                Self(self.0.on_double_click(command))
            }

            /// 注册鼠标进入命令。
            ///
            /// # 参数
            /// - `command`：鼠标进入时执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_mouse_enter(self, command: Command<VM>) -> Self {
                Self(self.0.on_mouse_enter(command))
            }

            /// 注册鼠标离开命令。
            ///
            /// # 参数
            /// - `command`：鼠标离开时执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_mouse_leave(self, command: Command<VM>) -> Self {
                Self(self.0.on_mouse_leave(command))
            }

            /// 注册鼠标移动命令。
            ///
            /// # 参数
            /// - `command`：接收当前指针坐标的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_mouse_move(self, command: ValueCommand<VM, Point>) -> Self {
                Self(self.0.on_mouse_move(command))
            }

            /// 注册挂载完成命令。
            ///
            /// # 参数
            /// - `command`：节点进入树后执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_mount(self, command: Command<VM>) -> Self {
                Self(self.0.on_mount(command))
            }

            /// 注册卸载命令。
            ///
            /// # 参数
            /// - `command`：节点移出树时执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_unmount(self, command: Command<VM>) -> Self {
                Self(self.0.on_unmount(command))
            }

            /// 注册更新命令。
            ///
            /// # 参数
            /// - `command`：节点更新后执行的命令。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn on_update(self, command: Command<VM>) -> Self {
                Self(self.0.on_update(command))
            }

            /// 设置鼠标悬停时的光标样式。
            ///
            /// # 参数
            /// - `cursor`：静态或响应式光标样式。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn cursor(self, cursor: impl Into<Value<CursorStyle>>) -> Self {
                Self(self.0.cursor(cursor))
            }

            /// 设置容器整体透明度。
            pub fn opacity(self, opacity: impl Into<Value<f32>>) -> Self {
                Self(self.0.opacity(opacity))
            }

            /// 设置容器视觉偏移。
            pub fn offset(self, offset: impl Into<Value<Point>>) -> Self {
                Self(self.0.offset(offset))
            }

            /// 追加一个子节点来源。
            ///
            /// # 参数
            /// - `child`：单个子节点、子节点集合或动态子节点来源。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn child(self, child: impl IntoChildren<VM>) -> Self {
                Self(self.0.child(child))
            }

            /// 设置容器内边距。
            ///
            /// # 参数
            /// - `padding`：静态或响应式内边距值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn padding(self, padding: impl Into<Value<Insets>>) -> Self {
                Self(self.0.padding(padding))
            }

            /// 设置容器子项间距。
            ///
            /// # 参数
            /// - `gap`：容器主布局使用的间距值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn gap(self, gap: impl IntoLengthValue) -> Self {
                Self(self.0.gap(gap))
            }

            /// 设置主轴分布方式。
            ///
            /// # 参数
            /// - `justify`：容器主轴上的排列策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn justify(self, justify: crate::ui::layout::Justify) -> Self {
                Self(self.0.justify(justify))
            }

            /// 设置交叉轴对齐方式。
            ///
            /// # 参数
            /// - `align`：容器交叉轴上的对齐策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn align(self, align: Align) -> Self {
                Self(self.0.align(align))
            }

            /// 同时将主轴和交叉轴对齐方式设为居中。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn center(self) -> Self {
                Self(self.0.center())
            }

            /// 同时设置水平和垂直方向的溢出策略。
            ///
            /// # 参数
            /// - `overflow`：统一使用的溢出策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn overflow(self, overflow: Overflow) -> Self {
                Self(self.0.overflow(overflow))
            }

            /// 设置水平方向的溢出策略。
            ///
            /// # 参数
            /// - `overflow`：水平溢出策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn overflow_x(self, overflow: Overflow) -> Self {
                Self(self.0.overflow_x(overflow))
            }

            /// 设置垂直方向的溢出策略。
            ///
            /// # 参数
            /// - `overflow`：垂直溢出策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn overflow_y(self, overflow: Overflow) -> Self {
                Self(self.0.overflow_y(overflow))
            }
        }
    };
}

pub(crate) use impl_container_properties;
