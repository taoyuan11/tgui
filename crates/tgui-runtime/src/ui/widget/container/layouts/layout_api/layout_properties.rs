macro_rules! impl_layout_properties {
    ($name:ident) => {
        impl<VM> $name<VM> {
            /// 设置组件的宽度和高度。
            ///
            /// # 参数
            /// - `width`：目标宽度。
            /// - `height`：目标高度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn size(self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_lengths(layout, width, height);
                    },
                )
            }

            /// 设置组件宽度。
            ///
            /// # 参数
            /// - `width`：目标宽度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn width(self, width: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.width, width);
                    },
                )
            }

            /// 设置组件高度。
            ///
            /// # 参数
            /// - `height`：目标高度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn height(self, height: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.height, height);
                    },
                )
            }

            /// 设置最小宽度。
            ///
            /// # 参数
            /// - `width`：最小宽度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn min_width(self, width: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.min_width, width);
                    },
                )
            }

            /// 设置最小高度。
            ///
            /// # 参数
            /// - `height`：最小高度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn min_height(self, height: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.min_height, height);
                    },
                )
            }

            /// 设置最大宽度。
            ///
            /// # 参数
            /// - `width`：最大宽度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn max_width(self, width: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.max_width, width);
                    },
                )
            }

            /// 设置最大高度。
            ///
            /// # 参数
            /// - `height`：最大高度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn max_height(self, height: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_length(&mut layout.max_height, height);
                    },
                )
            }

            /// 设置宽高比。
            ///
            /// # 参数
            /// - `aspect_ratio`：静态或响应式宽高比。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn aspect_ratio(self, aspect_ratio: impl Into<Value<f32>>) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.aspect_ratio = Some(aspect_ratio.into());
                    },
                )
            }

            /// 设置外边距。
            ///
            /// # 参数
            /// - `insets`：静态或响应式外边距。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn margin(self, insets: impl Into<Value<Insets>>) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.margin = insets.into();
                    },
                )
            }

            /// 设置增长因子。
            ///
            /// # 参数
            /// - `grow`：弹性增长值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn grow(self, grow: impl Into<Value<f32>>) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.grow = grow.into();
                    },
                )
            }

            /// 设置收缩因子。
            ///
            /// # 参数
            /// - `shrink`：弹性收缩值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn shrink(self, shrink: impl Into<Value<f32>>) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.shrink = shrink.into();
                    },
                )
            }

            /// 设置基础尺寸。
            ///
            /// # 参数
            /// - `basis`：布局基础尺寸。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn basis(self, basis: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.basis = Some(basis.into_length_value());
                    },
                )
            }

            /// 设置自身在交叉轴上的对齐方式。
            ///
            /// # 参数
            /// - `align`：自定义对齐策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn align_self(self, align: Align) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.align_self = Some(align);
                    },
                )
            }

            /// 设置自身在主轴上的对齐方式。
            ///
            /// # 参数
            /// - `align`：自定义对齐策略。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn justify_self(self, align: Align) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.justify_self = Some(align);
                    },
                )
            }

            /// 设置网格列起始位置。
            ///
            /// # 参数
            /// - `start`：从 1 开始的列索引。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn column(self, start: impl Into<$crate::ui::layout::Value<usize>>) -> Self {
                let start = start.into();
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.column_start = Some(start);
                    },
                )
            }

            /// 设置网格行起始位置。
            ///
            /// # 参数
            /// - `start`：从 1 开始的行索引。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn row(self, start: impl Into<$crate::ui::layout::Value<usize>>) -> Self {
                let start = start.into();
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.row_start = Some(start);
                    },
                )
            }

            /// 设置横跨列数。
            ///
            /// # 参数
            /// - `span`：至少为 1 的跨度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn column_span(self, span: usize) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.column_span = span.max(1);
                    },
                )
            }

            /// 设置横跨行数。
            ///
            /// # 参数
            /// - `span`：至少为 1 的跨度。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn row_span(self, span: usize) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.row_span = span.max(1);
                    },
                )
            }

            /// 将组件定位方式设为绝对定位。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn position_absolute(self) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        layout.position_type = crate::ui::layout::PositionType::Absolute;
                    },
                )
            }

            /// 设置左侧偏移。
            ///
            /// # 参数
            /// - `value`：左侧 inset 值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn left(self, value: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.left, value);
                    },
                )
            }

            /// 设置顶部偏移。
            ///
            /// # 参数
            /// - `value`：顶部 inset 值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn top(self, value: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.top, value);
                    },
                )
            }

            /// 设置右侧偏移。
            ///
            /// # 参数
            /// - `value`：右侧 inset 值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn right(self, value: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.right, value);
                    },
                )
            }

            /// 设置底部偏移。
            ///
            /// # 参数
            /// - `value`：底部 inset 值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn bottom(self, value: impl IntoLengthValue) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.bottom, value);
                    },
                )
            }

            /// 同时设置四个方向的偏移。
            ///
            /// # 参数
            /// - `value`：应用到四边的 inset 值。
            ///
            /// # 返回值
            /// 返回更新后的容器实例。
            pub fn inset(self, value: impl IntoLengthValue + Copy) -> Self {
                apply_layout_api(
                    self,
                    |owner| &mut owner.0.element,
                    |layout| {
                        set_layout_inset(&mut layout.left, value);
                        set_layout_inset(&mut layout.top, value);
                        set_layout_inset(&mut layout.right, value);
                        set_layout_inset(&mut layout.bottom, value);
                    },
                )
            }
        }
    };
}

pub(crate) use impl_layout_properties;
