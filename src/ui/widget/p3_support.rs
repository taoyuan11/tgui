use crate::ui::layout::LayoutStyle;

macro_rules! impl_p3_layout_api {
    ($field:ident) => {
        pub fn key(mut self, key: impl Into<super::WidgetKey>) -> Self {
            self.key = Some(key.into());
            self
        }

        pub fn size(
            mut self,
            width: impl super::container::IntoLengthValue,
            height: impl super::container::IntoLengthValue,
        ) -> Self {
            super::container::set_layout_lengths(&mut self.$field, width, height);
            self
        }

        pub fn width(mut self, width: impl super::container::IntoLengthValue) -> Self {
            super::container::set_layout_length(&mut self.$field.width, width);
            self
        }

        pub fn height(mut self, height: impl super::container::IntoLengthValue) -> Self {
            super::container::set_layout_length(&mut self.$field.height, height);
            self
        }

        pub fn min_width(mut self, width: impl super::container::IntoLengthValue) -> Self {
            super::container::set_layout_length(&mut self.$field.min_width, width);
            self
        }

        pub fn min_height(mut self, height: impl super::container::IntoLengthValue) -> Self {
            super::container::set_layout_length(&mut self.$field.min_height, height);
            self
        }

        pub fn max_width(mut self, width: impl super::container::IntoLengthValue) -> Self {
            super::container::set_layout_length(&mut self.$field.max_width, width);
            self
        }

        pub fn max_height(mut self, height: impl super::container::IntoLengthValue) -> Self {
            super::container::set_layout_length(&mut self.$field.max_height, height);
            self
        }

        pub fn aspect_ratio(
            mut self,
            aspect_ratio: impl Into<crate::ui::layout::Value<f32>>,
        ) -> Self {
            self.$field.aspect_ratio = Some(aspect_ratio.into());
            self
        }

        pub fn margin(
            mut self,
            insets: impl Into<crate::ui::layout::Value<crate::ui::layout::Insets>>,
        ) -> Self {
            self.$field.margin = insets.into();
            self
        }

        pub fn padding(
            mut self,
            insets: impl Into<crate::ui::layout::Value<crate::ui::layout::Insets>>,
        ) -> Self {
            self.$field.padding = Some(insets.into());
            self
        }

        pub fn grow(mut self, grow: impl Into<crate::ui::layout::Value<f32>>) -> Self {
            self.$field.grow = grow.into();
            self
        }

        pub fn shrink(mut self, shrink: impl Into<crate::ui::layout::Value<f32>>) -> Self {
            self.$field.shrink = shrink.into();
            self
        }

        pub fn basis(mut self, basis: impl super::container::IntoLengthValue) -> Self {
            self.$field.basis = Some(basis.into_length_value());
            self
        }

        pub fn align_self(mut self, align: crate::ui::layout::Align) -> Self {
            self.$field.align_self = Some(align);
            self
        }

        pub fn justify_self(mut self, align: crate::ui::layout::Align) -> Self {
            self.$field.justify_self = Some(align);
            self
        }

        pub fn position_absolute(mut self) -> Self {
            self.$field.position_type = crate::ui::layout::PositionType::Absolute;
            self
        }

        pub fn left(mut self, value: impl super::container::IntoLengthValue) -> Self {
            super::container::set_layout_inset(&mut self.$field.left, value);
            self
        }

        pub fn top(mut self, value: impl super::container::IntoLengthValue) -> Self {
            super::container::set_layout_inset(&mut self.$field.top, value);
            self
        }

        pub fn right(mut self, value: impl super::container::IntoLengthValue) -> Self {
            super::container::set_layout_inset(&mut self.$field.right, value);
            self
        }

        pub fn bottom(mut self, value: impl super::container::IntoLengthValue) -> Self {
            super::container::set_layout_inset(&mut self.$field.bottom, value);
            self
        }

        pub fn inset(mut self, value: impl super::container::IntoLengthValue + Copy) -> Self {
            super::container::set_layout_inset(&mut self.$field.left, value);
            super::container::set_layout_inset(&mut self.$field.top, value);
            super::container::set_layout_inset(&mut self.$field.right, value);
            super::container::set_layout_inset(&mut self.$field.bottom, value);
            self
        }
    };
}

pub(crate) use impl_p3_layout_api;

pub(crate) fn merge_layout(mut base: LayoutStyle, override_layout: LayoutStyle) -> LayoutStyle {
    let default = LayoutStyle::default();
    if override_layout.width != default.width {
        base.width = override_layout.width;
    }
    if override_layout.height != default.height {
        base.height = override_layout.height;
    }
    if override_layout.min_width != default.min_width {
        base.min_width = override_layout.min_width;
    }
    if override_layout.min_height != default.min_height {
        base.min_height = override_layout.min_height;
    }
    if override_layout.max_width != default.max_width {
        base.max_width = override_layout.max_width;
    }
    if override_layout.max_height != default.max_height {
        base.max_height = override_layout.max_height;
    }
    if override_layout.aspect_ratio != default.aspect_ratio {
        base.aspect_ratio = override_layout.aspect_ratio;
    }
    if override_layout.margin != default.margin {
        base.margin = override_layout.margin;
    }
    if override_layout.padding != default.padding {
        base.padding = override_layout.padding;
    }
    if override_layout.grow != default.grow {
        base.grow = override_layout.grow;
    }
    if override_layout.shrink != default.shrink {
        base.shrink = override_layout.shrink;
    }
    if override_layout.basis != default.basis {
        base.basis = override_layout.basis;
    }
    if override_layout.position_type != default.position_type {
        base.position_type = override_layout.position_type;
    }
    if override_layout.left != default.left {
        base.left = override_layout.left;
    }
    if override_layout.top != default.top {
        base.top = override_layout.top;
    }
    if override_layout.right != default.right {
        base.right = override_layout.right;
    }
    if override_layout.bottom != default.bottom {
        base.bottom = override_layout.bottom;
    }
    if override_layout.align_self != default.align_self {
        base.align_self = override_layout.align_self;
    }
    if override_layout.justify_self != default.justify_self {
        base.justify_self = override_layout.justify_self;
    }
    if override_layout.column_start != default.column_start {
        base.column_start = override_layout.column_start;
    }
    if override_layout.row_start != default.row_start {
        base.row_start = override_layout.row_start;
    }
    if override_layout.column_span != default.column_span {
        base.column_span = override_layout.column_span;
    }
    if override_layout.row_span != default.row_span {
        base.row_span = override_layout.row_span;
    }
    base
}
