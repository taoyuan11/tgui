use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Axis, Insets, Overflow, Track, Value, Wrap};

use self::layout_api::impl_layout_api;
use super::super::common::{ContainerKind, ContainerLayout, CursorStyle, Point, WidgetKind};
use super::super::core::Element;
use super::super::style::ContainerStyle;
use super::base::{apply_layout_api, Container};
use super::length::{set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::IntoChildren;

mod flex;
mod grid;
mod layout_api;
mod stack;

/// 以堆叠方式布局子节点的容器。
///
/// 该容器适合用于叠放元素、覆盖层和滚动内容承载。
pub struct Stack<VM>(Container<VM>);

/// 以网格方式布局子节点的容器。
///
/// 该容器通过行列轨道描述网格结构，适合复杂二维排版。
pub struct Grid<VM>(Container<VM>);

/// 以弹性盒模型布局子节点的容器。
///
/// 该容器支持主轴方向和换行配置，适合线性排列场景。
pub struct Flex<VM>(Container<VM>);

impl_layout_api!(Stack);
impl_layout_api!(Grid);
impl_layout_api!(Flex);
