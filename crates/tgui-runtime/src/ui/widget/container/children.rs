use super::super::common::ChildSource;
use super::super::core::Element;
use crate::foundation::binding::Signal;

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

/// 定义可作为容器静态子节点输入的类型。
///
/// 该 trait 支持单个元素、数组和 `Vec`。响应式结构必须使用显式的
/// `dynamic_child` API，并且只能配合 legacy tree 或显式 rebuild 使用。
pub trait IntoChildren<VM> {
    /// 将输入转换为容器内部使用的子节点来源。
    ///
    /// # 返回值
    /// 返回静态子节点提供器，供运行时在布局和渲染阶段读取。
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

/// 定义显式动态子节点输入。
///
/// 这不是默认静态构建路径的一部分；strict reactive tree 会拒绝这类结构。
pub trait IntoDynamicChildren<VM> {
    /// 将输入转换为动态子节点提供器。
    #[allow(private_interfaces)]
    fn into_dynamic_child_source(self) -> ChildSource<VM>;
}

impl<VM, T> IntoDynamicChildren<VM> for Signal<T>
where
    T: Clone + IntoChildGroup<VM> + Send + Sync + 'static,
{
    #[allow(private_interfaces)]
    fn into_dynamic_child_source(self) -> ChildSource<VM> {
        ChildSource::Dynamic(std::sync::Arc::new(move || {
            self.get_uncached().into_elements()
        }))
    }
}
