use super::container::IntoChildren;
use super::core::Element;
use super::WidgetKey;
use crate::ui::layout::Value;
use crate::ui::widget::common::ChildSource;
use std::sync::Arc;

pub struct For<VM> {
    resolver: Arc<dyn Fn() -> Vec<Element<VM>> + Send + Sync>,
}

impl<VM: 'static> For<VM> {
    pub fn new<T, K, R>(
        items: impl Into<Value<Vec<T>>>,
        key: impl Fn(&T) -> K + Send + Sync + 'static,
        render: impl Fn(usize, &T) -> R + Send + Sync + 'static,
    ) -> Self
    where
        T: Clone + Send + Sync + 'static,
        K: Into<WidgetKey>,
        R: Into<Element<VM>>,
        Value<Vec<T>>: Send + Sync + 'static,
    {
        let items = items.into();
        let key = Arc::new(key);
        let render = Arc::new(render);
        Self {
            resolver: Arc::new(move || {
                items.resolve_ref(|items| {
                    items
                        .iter()
                        .enumerate()
                        .map(|(index, item)| {
                            let child = render(index, item).into();
                            child.with_key_if_missing(key(item))
                        })
                        .collect()
                })
            }),
        }
    }
}

impl<VM> IntoChildren<VM> for For<VM> {
    #[allow(private_interfaces)]
    fn into_child_source(self) -> ChildSource<VM> {
        ChildSource::KeyedFor(self.resolver)
    }
}

trait WidgetKeyExt<VM> {
    fn with_key_if_missing(self, key: impl Into<WidgetKey>) -> Element<VM>;
}

impl<VM> WidgetKeyExt<VM> for Element<VM> {
    fn with_key_if_missing(mut self, key: impl Into<WidgetKey>) -> Element<VM> {
        if self.key.is_none() {
            self.key = Some(key.into());
        }
        self
    }
}
