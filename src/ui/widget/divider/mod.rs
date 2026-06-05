use crate::foundation::color::Color;
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};
use crate::ui::unit::Dp;

use super::common::{
    CursorStyle, DividerOrientation, InteractionHandlers, LifecycleEventHandlers,
    MediaEventHandlers, VisualStyle, WidgetId, WidgetKind,
};
use super::container::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::core::Element;
use super::style::{DividerStyle, StyleResolver};

/// 分隔线组件。
pub struct Divider<VM> {
    element: Element<VM>,
}

macro_rules! impl_widget_layout_api {
    () => {
        pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
            set_layout_lengths(&mut self.element.layout, width, height);
            self
        }

        pub fn width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.width, width);
            self
        }

        pub fn height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.height, height);
            self
        }

        pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.min_width, width);
            self
        }

        pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.min_height, height);
            self
        }

        pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.max_width, width);
            self
        }

        pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.element.layout.max_height, height);
            self
        }

        pub fn aspect_ratio(mut self, aspect_ratio: impl Into<Value<f32>>) -> Self {
            self.element.layout.aspect_ratio = Some(aspect_ratio.into());
            self
        }

        pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.element.layout.margin = insets.into();
            self
        }

        pub fn padding(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.element.layout.padding = Some(insets.into());
            self
        }

        pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
            self.element.layout.grow = grow.into();
            self
        }

        pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
            self.element.layout.shrink = shrink.into();
            self
        }

        pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
            self.element.layout.basis = Some(basis.into_length_value());
            self
        }

        pub fn align_self(mut self, align: Align) -> Self {
            self.element.layout.align_self = Some(align);
            self
        }

        pub fn justify_self(mut self, align: Align) -> Self {
            self.element.layout.justify_self = Some(align);
            self
        }

        pub fn column(mut self, start: usize) -> Self {
            self.element.layout.column_start = Some(start.max(1));
            self
        }

        pub fn row(mut self, start: usize) -> Self {
            self.element.layout.row_start = Some(start.max(1));
            self
        }

        pub fn column_span(mut self, span: usize) -> Self {
            self.element.layout.column_span = span.max(1);
            self
        }

        pub fn row_span(mut self, span: usize) -> Self {
            self.element.layout.row_span = span.max(1);
            self
        }

        pub fn position_absolute(mut self) -> Self {
            self.element.layout.position_type = crate::ui::layout::PositionType::Absolute;
            self
        }

        pub fn left(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.left, value);
            self
        }

        pub fn top(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.top, value);
            self
        }

        pub fn right(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.right, value);
            self
        }

        pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.element.layout.bottom, value);
            self
        }

        pub fn inset(mut self, value: impl IntoLengthValue + Copy) -> Self {
            set_layout_inset(&mut self.element.layout.left, value);
            set_layout_inset(&mut self.element.layout.top, value);
            set_layout_inset(&mut self.element.layout.right, value);
            set_layout_inset(&mut self.element.layout.bottom, value);
            self
        }
    };
}

impl<VM> Divider<VM> {
    pub fn new() -> Self {
        Self {
            element: Element {
                id: WidgetId::next(),
                key: None,
                layout: LayoutStyle::default(),
                focus: Default::default(),
                visual: VisualStyle::default(),
                interactions: InteractionHandlers {
                    cursor_style: Some(Value::Static(CursorStyle::Default)),
                    ..Default::default()
                },
                lifecycle_events: LifecycleEventHandlers::default(),
                media_events: MediaEventHandlers::default(),
                background: None,
                tooltip: None,
                popover: None,
                menu: None,
                context_menu: None,
                modal: None,
                drawer: None,
                tab_trigger: None,
                list_item: None,
                data_grid_root: None,
                data_grid_cell: None,
                data_grid_header: None,
                data_grid_resize_handle: None,
                kind: WidgetKind::Divider {
                    orientation: DividerOrientation::Horizontal,
                    dashed: Value::Static(false),
                    color_override: None,
                    thickness_override: None,
                    inset_override: None,
                    label: None,
                    style: None,
                },
            },
        }
    }

    impl_widget_layout_api!();

    pub fn key(mut self, key: impl Into<super::WidgetKey>) -> Self {
        self.element.key = Some(key.into());
        self
    }

    /// 设置分隔线朝向。
    pub fn orientation(mut self, orientation: DividerOrientation) -> Self {
        if let WidgetKind::Divider {
            orientation: slot, ..
        } = &mut self.element.kind
        {
            *slot = orientation;
        }
        self
    }

    /// 水平分隔线（默认）。
    pub fn horizontal(self) -> Self {
        self.orientation(DividerOrientation::Horizontal)
    }

    /// 垂直分隔线。
    pub fn vertical(self) -> Self {
        self.orientation(DividerOrientation::Vertical)
    }

    /// 设置线条粗细。
    pub fn thickness(mut self, thickness: impl Into<Value<Dp>>) -> Self {
        if let WidgetKind::Divider {
            thickness_override, ..
        } = &mut self.element.kind
        {
            *thickness_override = Some(thickness.into());
        }
        self
    }

    /// 是否使用虚线。
    pub fn dashed(mut self, dashed: impl Into<Value<bool>>) -> Self {
        if let WidgetKind::Divider { dashed: slot, .. } = &mut self.element.kind {
            *slot = dashed.into();
        }
        self
    }

    /// 覆盖线条颜色。
    pub fn color(mut self, color: impl Into<Value<Color>>) -> Self {
        if let WidgetKind::Divider { color_override, .. } = &mut self.element.kind {
            *color_override = Some(color.into());
        }
        self
    }

    /// 两端内缩（线条沿主轴方向从两端缩进的距离）。
    pub fn end_inset(mut self, inset: impl Into<Value<Dp>>) -> Self {
        if let WidgetKind::Divider { inset_override, .. } = &mut self.element.kind {
            *inset_override = Some(inset.into());
        }
        self
    }

    /// 设置中间标签（仅水平分隔线生效）。
    pub fn label(mut self, label: impl Into<Value<String>>) -> Self {
        if let WidgetKind::Divider { label: slot, .. } = &mut self.element.kind {
            *slot = Some(label.into());
        }
        self
    }

    pub fn style(
        mut self,
        resolver: impl Fn(ResolvedThemeMode) -> DividerStyle + Send + Sync + 'static,
    ) -> Self {
        if let WidgetKind::Divider { style, .. } = &mut self.element.kind {
            *style = Some(StyleResolver::new(resolver));
        }
        self
    }
}

impl<VM> Default for Divider<VM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<VM> From<Divider<VM>> for Element<VM> {
    fn from(value: Divider<VM>) -> Self {
        value.element
    }
}
