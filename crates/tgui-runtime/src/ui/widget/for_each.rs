use super::common::WidgetId;
use super::container::IntoChildren;
use super::core::Element;
use super::WidgetKey;
use crate::ui::layout::Value;
use crate::ui::widget::common::ChildSource;
use std::collections::{HashMap, VecDeque};
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
        let retained_ids = parking_lot::Mutex::new(HashMap::new());
        Self {
            resolver: Arc::new(move || {
                items.resolve_ref(|items| {
                    retain_keyed_ids(
                        &retained_ids,
                        items
                            .iter()
                            .enumerate()
                            .map(|(index, item)| {
                                let child = render(index, item).into();
                                child.with_key_if_missing(key(item))
                            })
                            .collect(),
                    )
                })
            }),
        }
    }

    pub(crate) fn new_with_resolver<T, K, R>(
        items: impl Fn() -> Vec<T> + Send + Sync + 'static,
        key: impl Fn(&T) -> K + Send + Sync + 'static,
        render: impl Fn(usize, &T) -> R + Send + Sync + 'static,
    ) -> Self
    where
        T: Send + Sync + 'static,
        K: Into<WidgetKey>,
        R: Into<Element<VM>>,
    {
        let key = Arc::new(key);
        let render = Arc::new(render);
        let retained_ids = parking_lot::Mutex::new(HashMap::new());
        Self {
            resolver: Arc::new(move || {
                retain_keyed_ids(
                    &retained_ids,
                    items()
                        .iter()
                        .enumerate()
                        .map(|(index, item)| {
                            let child = render(index, item).into();
                            child.with_key_if_missing(key(item))
                        })
                        .collect(),
                )
            }),
        }
    }
}

fn retain_keyed_ids<VM>(
    retained_ids: &parking_lot::Mutex<HashMap<WidgetKey, VecDeque<WidgetId>>>,
    mut children: Vec<Element<VM>>,
) -> Vec<Element<VM>> {
    let mut retained_ids = retained_ids.lock();
    let mut reusable_ids = std::mem::take(&mut *retained_ids);
    let mut next_ids: HashMap<WidgetKey, VecDeque<WidgetId>> =
        HashMap::with_capacity(children.len());
    for child in &mut children {
        let Some(key) = child.key.clone() else {
            continue;
        };
        if let Some(id) = reusable_ids.get_mut(&key).and_then(VecDeque::pop_front) {
            child.id = id;
        }
        next_ids.entry(key).or_default().push_back(child.id);
    }
    *retained_ids = next_ids;
    children
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widget::Text;

    #[test]
    fn retained_ids_are_stable_and_unique_for_duplicate_keys() {
        let retained = parking_lot::Mutex::new(HashMap::new());
        let children = || {
            vec![
                Element::<()>::from(Text::new("first").key("duplicate")),
                Element::<()>::from(Text::new("second").key("duplicate")),
            ]
        };

        let first = retain_keyed_ids(&retained, children());
        let second = retain_keyed_ids(&retained, children());
        let first_ids = first.iter().map(|child| child.id).collect::<Vec<_>>();
        let second_ids = second.iter().map(|child| child.id).collect::<Vec<_>>();

        assert_ne!(first_ids[0], first_ids[1]);
        assert_eq!(second_ids, first_ids);
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
