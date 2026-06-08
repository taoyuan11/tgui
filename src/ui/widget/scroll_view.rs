use crate::foundation::binding::ScrollViewController;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Insets, Overflow, ScrollbarStyle, Value};

use super::common::{ContainerKind, CursorStyle, Point, ScrollViewConfig, VisualStyle, WidgetKind};
use super::container::{apply_layout_api, set_layout_length, set_layout_lengths, IntoLengthValue};
use super::container::{Container, IntoChildren};
use super::core::Element;
use super::style::ContainerStyle;

/// 独立可滚动容器。
pub struct ScrollView<VM>(Container<VM>);

impl<VM> ScrollView<VM> {
    /// 创建一个默认垂直可滚动的 `ScrollView`。
    pub fn new() -> Self {
        let mut container = Container::with_layout(super::common::ContainerLayout {
            kind: ContainerKind::Stack,
            overflow_y: Overflow::Scroll,
            scroll_view: Some(super::common::ScrollViewConfig::default()),
            ..super::common::ContainerLayout::flow()
        });
        container = container.focusable(true).tab_index(0);
        Self(container)
    }

    pub fn child(self, child: impl IntoChildren<VM>) -> Self {
        Self(self.0.child(child))
    }

    pub fn size(self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
        apply_layout_api(
            self,
            |owner| &mut owner.0.element,
            |layout| set_layout_lengths(layout, width, height),
        )
    }

    pub fn width(self, width: impl IntoLengthValue) -> Self {
        apply_layout_api(
            self,
            |owner| &mut owner.0.element,
            |layout| set_layout_length(&mut layout.width, width),
        )
    }

    pub fn height(self, height: impl IntoLengthValue) -> Self {
        apply_layout_api(
            self,
            |owner| &mut owner.0.element,
            |layout| set_layout_length(&mut layout.height, height),
        )
    }

    pub fn min_width(self, width: impl IntoLengthValue) -> Self {
        apply_layout_api(
            self,
            |owner| &mut owner.0.element,
            |layout| set_layout_length(&mut layout.min_width, width),
        )
    }

    pub fn min_height(self, height: impl IntoLengthValue) -> Self {
        apply_layout_api(
            self,
            |owner| &mut owner.0.element,
            |layout| set_layout_length(&mut layout.min_height, height),
        )
    }

    pub fn max_width(self, width: impl IntoLengthValue) -> Self {
        apply_layout_api(
            self,
            |owner| &mut owner.0.element,
            |layout| set_layout_length(&mut layout.max_width, width),
        )
    }

    pub fn max_height(self, height: impl IntoLengthValue) -> Self {
        apply_layout_api(
            self,
            |owner| &mut owner.0.element,
            |layout| set_layout_length(&mut layout.max_height, height),
        )
    }

    pub fn overflow(self, overflow: Overflow) -> Self {
        Self(self.0.overflow(overflow))
    }

    pub fn overflow_x(self, overflow: Overflow) -> Self {
        Self(self.0.overflow_x(overflow))
    }

    pub fn overflow_y(self, overflow: Overflow) -> Self {
        Self(self.0.overflow_y(overflow))
    }

    pub fn show_scrollbar(self, show: impl Into<Value<bool>>) -> Self {
        Self(self.0.show_scrollbar(show))
    }

    pub fn scrollbar_style(self, style: impl Into<Value<ScrollbarStyle>>) -> Self {
        Self(self.0.scrollbar_style(style))
    }

    pub fn controller(mut self, controller: ScrollViewController) -> Self {
        if let WidgetKind::Container { layout, .. } = &mut self.0.element.kind {
            let config = layout
                .scroll_view
                .get_or_insert_with(ScrollViewConfig::default);
            config.controller = Some(controller);
        }
        self
    }

    pub fn style(
        self,
        mutator: impl Fn(&mut ContainerStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        Self(self.0.style(mutator))
    }

    pub fn style_full(
        self,
        resolver: impl Fn(&StyleContext<'_>) -> ContainerStyle + Send + Sync + 'static,
    ) -> Self {
        Self(self.0.style_full(resolver))
    }

    pub(crate) fn style_full_with_style_sheet(
        self,
        resolver: impl Fn(&StyleContext<'_>, &super::StyleSheet, &VisualStyle, WidgetState) -> ContainerStyle
            + Send
            + Sync
            + 'static,
    ) -> Self {
        Self(self.0.style_full_with_style_sheet(resolver))
    }

    pub fn key(self, key: impl Into<super::WidgetKey>) -> Self {
        Self(self.0.key(key))
    }

    pub fn focusable(self, focusable: bool) -> Self {
        Self(self.0.focusable(focusable))
    }

    pub fn tab_index(self, tab_index: i32) -> Self {
        Self(self.0.tab_index(tab_index))
    }

    pub fn focus_scope(self, options: super::FocusScopeOptions) -> Self {
        Self(self.0.focus_scope(options))
    }

    pub fn auto_focus_first(self, auto_focus_first: bool) -> Self {
        Self(self.0.auto_focus_first(auto_focus_first))
    }

    pub fn on_click(self, command: Command<VM>) -> Self {
        Self(self.0.on_click(command))
    }

    pub fn on_double_click(self, command: Command<VM>) -> Self {
        Self(self.0.on_double_click(command))
    }

    pub fn on_mouse_enter(self, command: Command<VM>) -> Self {
        Self(self.0.on_mouse_enter(command))
    }

    pub fn on_mouse_leave(self, command: Command<VM>) -> Self {
        Self(self.0.on_mouse_leave(command))
    }

    pub fn on_mouse_move(self, command: ValueCommand<VM, Point>) -> Self {
        Self(self.0.on_mouse_move(command))
    }

    pub fn on_mount(self, command: Command<VM>) -> Self {
        Self(self.0.on_mount(command))
    }

    pub fn on_unmount(self, command: Command<VM>) -> Self {
        Self(self.0.on_unmount(command))
    }

    pub fn on_update(self, command: Command<VM>) -> Self {
        Self(self.0.on_update(command))
    }

    pub fn cursor(self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        Self(self.0.cursor(cursor))
    }

    pub fn opacity(self, opacity: impl Into<Value<f32>>) -> Self {
        Self(self.0.opacity(opacity))
    }

    pub fn offset(self, offset: impl Into<Value<Point>>) -> Self {
        Self(self.0.offset(offset))
    }

    pub fn scale(self, scale: impl Into<Value<f32>>) -> Self {
        Self(self.0.scale(scale))
    }

    pub fn border_radius(self, radius: impl Into<Value<crate::ui::unit::Dp>>) -> Self {
        Self(self.0.border_radius(radius))
    }

    pub fn padding(self, padding: impl Into<Value<Insets>>) -> Self {
        Self(self.0.padding(padding))
    }
}

impl<VM> Default for ScrollView<VM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<VM> From<ScrollView<VM>> for Element<VM> {
    fn from(value: ScrollView<VM>) -> Self {
        value.0.into()
    }
}
