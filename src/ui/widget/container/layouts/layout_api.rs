mod container_properties;
mod layout_properties;

pub(crate) use self::container_properties::impl_container_properties;
pub(crate) use self::layout_properties::impl_layout_properties;

macro_rules! impl_layout_api {
    ($name:ident) => {
        $crate::ui::widget::container::layouts::layout_api::impl_layout_properties!($name);
        $crate::ui::widget::container::layouts::layout_api::impl_container_properties!($name);

        impl<VM> From<$name<VM>> for Element<VM> {
            fn from(value: $name<VM>) -> Self {
                value.0.into()
            }
        }
    };
}

pub(crate) use impl_layout_api;
