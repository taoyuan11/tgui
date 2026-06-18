use super::container::IntoChildren;
use super::core::Element;
use crate::foundation::binding::Signal;
use crate::ui::layout::Value;
use crate::ui::widget::common::ChildSource;

pub struct Show<VM> {
    visible: Value<bool>,
    child: Element<VM>,
}

impl<VM> Show<VM> {
    pub fn new(visible: impl Into<Value<bool>>, child: impl Into<Element<VM>>) -> Self {
        Self {
            visible: visible.into(),
            child: child.into(),
        }
    }

    pub fn from_signal(visible: Signal<bool>, child: impl Into<Element<VM>>) -> Self {
        Self::new(visible, child)
    }
}

impl<VM> IntoChildren<VM> for Show<VM> {
    #[allow(private_interfaces)]
    fn into_child_source(self) -> ChildSource<VM> {
        ChildSource::Show {
            visible: self.visible,
            child: self.child,
        }
    }
}
