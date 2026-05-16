use crate::foundation::binding::Signal;

use super::super::common::ChildSource;
use super::super::core::Element;

trait IntoChildGroup<VM> {
    fn into_elements(self) -> Vec<Element<VM>>;
}

impl<VM, T> IntoChildGroup<VM> for T
where
    T: Into<Element<VM>>,
{
    fn into_elements(self) -> Vec<Element<VM>> {
        vec![self.into()]
    }
}

impl<VM, T, const N: usize> IntoChildGroup<VM> for [T; N]
where
    T: Into<Element<VM>>,
{
    fn into_elements(self) -> Vec<Element<VM>> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<VM, T> IntoChildGroup<VM> for Vec<T>
where
    T: Into<Element<VM>>,
{
    fn into_elements(self) -> Vec<Element<VM>> {
        self.into_iter().map(Into::into).collect()
    }
}

/// 定义可作为容器子节点输入的类型。
///
/// 该 trait 支持单个元素、数组、`Vec`，以及由 `Signal` 驱动的动态子节点集合。
pub trait IntoChildren<VM> {
    /// 将输入转换为容器内部使用的子节点来源。
    ///
    /// # 返回值
    /// 返回静态或动态的子节点提供器，供运行时在布局和渲染阶段读取。
    #[allow(private_interfaces)]
    fn into_child_source(self) -> ChildSource<VM>;
}

impl<VM, T> IntoChildren<VM> for T
where
    T: Into<Element<VM>>,
{
    #[allow(private_interfaces)]
    fn into_child_source(self) -> ChildSource<VM> {
        ChildSource::Static(vec![self.into()])
    }
}

impl<VM, T, const N: usize> IntoChildren<VM> for [T; N]
where
    T: Into<Element<VM>>,
{
    #[allow(private_interfaces)]
    fn into_child_source(self) -> ChildSource<VM> {
        ChildSource::Static(self.into_elements())
    }
}

impl<VM, T> IntoChildren<VM> for Vec<T>
where
    T: Into<Element<VM>>,
{
    #[allow(private_interfaces)]
    fn into_child_source(self) -> ChildSource<VM> {
        ChildSource::Static(self.into_elements())
    }
}

impl<VM, T> IntoChildren<VM> for Signal<T>
where
    T: Clone + IntoChildGroup<VM> + Send + Sync + 'static,
{
    #[allow(private_interfaces)]
    fn into_child_source(self) -> ChildSource<VM> {
        ChildSource::Dynamic(std::sync::Arc::new(move || self.get().into_elements()))
    }
}
