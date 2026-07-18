use super::container::IntoChildren;
use super::core::Element;
use super::WidgetKey;
use crate::ui::layout::Value;
use crate::ui::widget::common::ChildSource;

pub struct ViewSwitch<VM> {
    index: Value<usize>,
    cases: Vec<Element<VM>>,
    fallback: Option<Element<VM>>,
}

impl<VM> ViewSwitch<VM> {
    pub fn new(index: impl Into<Value<usize>>) -> Self {
        Self {
            index: index.into(),
            cases: Vec::new(),
            fallback: None,
        }
    }

    pub fn case(mut self, child: impl Into<Element<VM>>) -> Self {
        let case_index = self.cases.len();
        self.cases.push(with_key_if_missing(
            child.into(),
            WidgetKey::from(format!("__tgui_view_switch_case_{case_index}")),
        ));
        self
    }

    pub fn fallback(mut self, child: impl Into<Element<VM>>) -> Self {
        self.fallback = Some(with_key_if_missing(
            child.into(),
            WidgetKey::from("__tgui_view_switch_fallback"),
        ));
        self
    }
}

impl<VM> IntoChildren<VM> for ViewSwitch<VM> {
    #[allow(private_interfaces)]
    fn into_child_source(self) -> ChildSource<VM> {
        ChildSource::Switch {
            index: self.index,
            cases: self.cases,
            fallback: self.fallback.map(Box::new),
        }
    }
}

fn with_key_if_missing<VM>(mut child: Element<VM>, key: WidgetKey) -> Element<VM> {
    if child.key.is_none() {
        child.key = Some(key);
    }
    child
}
