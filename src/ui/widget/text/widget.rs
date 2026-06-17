use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::text::font::FontWeight;
use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};
use crate::ui::unit::Sp;

use super::super::common::{
    CursorStyle, InteractionHandlers, LifecycleEventHandlers, MediaEventHandlers, Point,
    VisualStyle, WidgetId, WidgetKey, WidgetKind,
};
use super::super::container::{
    set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue,
};
use super::super::core::Element;
use super::super::style::{StyleResolver, TextWidgetStyle};
use super::IntoTextContent;

/// 文本展示组件。
#[derive(Clone)]
pub struct Text {
    pub(crate) key: Option<WidgetKey>,
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) content: Value<String>,
    pub(crate) font_family: Option<String>,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) color: Option<Value<Color>>,
    pub(crate) font_size: Option<Sp>,
    pub(crate) line_height: Option<Sp>,
    pub(crate) font_weight: Option<FontWeight>,
    pub(crate) letter_spacing: Option<Sp>,
    pub(crate) cursor_style: Option<Value<CursorStyle>>,
    pub(crate) user_select: bool,
    pub(crate) style: Option<StyleResolver<TextWidgetStyle>>,
}

macro_rules! impl_text_layout_api {
    () => {
        /// 设置组件宽高。
        pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
            set_layout_lengths(&mut self.layout, width, height);
            self
        }

        /// 设置组件宽度。
        pub fn width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.width, width);
            self
        }

        /// 设置组件高度。
        pub fn height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.height, height);
            self
        }

        /// 设置最小宽度。
        pub fn min_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.min_width, width);
            self
        }

        /// 设置最小高度。
        pub fn min_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.min_height, height);
            self
        }

        /// 设置最大宽度。
        pub fn max_width(mut self, width: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.max_width, width);
            self
        }

        /// 设置最大高度。
        pub fn max_height(mut self, height: impl IntoLengthValue) -> Self {
            set_layout_length(&mut self.layout.max_height, height);
            self
        }

        /// 设置宽高比。
        pub fn aspect_ratio(mut self, aspect_ratio: impl Into<Value<f32>>) -> Self {
            self.layout.aspect_ratio = Some(aspect_ratio.into());
            self
        }

        /// 设置外边距。
        pub fn margin(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.layout.margin = insets.into();
            self
        }

        /// 设置内边距。
        pub fn padding(mut self, insets: impl Into<Value<Insets>>) -> Self {
            self.layout.padding = Some(insets.into());
            self
        }

        /// 设置弹性增长值。
        pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
            self.layout.grow = grow.into();
            self
        }

        /// 设置弹性收缩值。
        pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
            self.layout.shrink = shrink.into();
            self
        }

        /// 设置基础尺寸。
        pub fn basis(mut self, basis: impl IntoLengthValue) -> Self {
            self.layout.basis = Some(basis.into_length_value());
            self
        }

        /// 设置自身在交叉轴上的对齐方式。
        pub fn align_self(mut self, align: Align) -> Self {
            self.layout.align_self = Some(align);
            self
        }

        /// 设置自身在主轴上的对齐方式。
        pub fn justify_self(mut self, align: Align) -> Self {
            self.layout.justify_self = Some(align);
            self
        }

        /// 设置网格列起始位置。
        pub fn column(mut self, start: usize) -> Self {
            self.layout.column_start = Some(Value::Static(start.max(1)));
            self
        }

        /// 设置网格行起始位置。
        pub fn row(mut self, start: usize) -> Self {
            self.layout.row_start = Some(Value::Static(start.max(1)));
            self
        }

        /// 设置横跨列数。
        pub fn column_span(mut self, span: usize) -> Self {
            self.layout.column_span = span.max(1);
            self
        }

        /// 设置横跨行数。
        pub fn row_span(mut self, span: usize) -> Self {
            self.layout.row_span = span.max(1);
            self
        }

        /// 切换为绝对定位。
        pub fn position_absolute(mut self) -> Self {
            self.layout.position_type = crate::ui::layout::PositionType::Absolute;
            self
        }

        /// 设置左侧偏移。
        pub fn left(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.left, value);
            self
        }

        /// 设置顶部偏移。
        pub fn top(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.top, value);
            self
        }

        /// 设置右侧偏移。
        pub fn right(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.right, value);
            self
        }

        /// 设置底部偏移。
        pub fn bottom(mut self, value: impl IntoLengthValue) -> Self {
            set_layout_inset(&mut self.layout.bottom, value);
            self
        }

        /// 同时设置四边偏移。
        pub fn inset(mut self, value: impl IntoLengthValue + Copy) -> Self {
            set_layout_inset(&mut self.layout.left, value);
            set_layout_inset(&mut self.layout.top, value);
            set_layout_inset(&mut self.layout.right, value);
            set_layout_inset(&mut self.layout.bottom, value);
            self
        }
    };
}

impl Text {
    /// 创建文本组件。
    ///
    /// # 参数
    /// - `content`：静态或响应式的文本内容。
    ///
    /// # 返回值
    /// 返回新的文本组件实例。
    pub fn new(content: impl IntoTextContent) -> Self {
        Self {
            key: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            content: content.into_text_content(),
            font_family: None,
            background: None,
            color: None,
            font_size: None,
            line_height: None,
            font_weight: None,
            letter_spacing: None,
            cursor_style: None,
            user_select: false,
            style: None,
        }
    }

    impl_text_layout_api!();

    /// 设置文本样式解析器。
    pub fn style(
        mut self,
        mutator: impl Fn(&mut TextWidgetStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| TextWidgetStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> TextWidgetStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    pub(crate) fn style_full_with_style_sheet(
        mut self,
        resolver: impl Fn(
                &StyleContext<'_>,
                &crate::ui::widget::StyleSheet,
                &VisualStyle,
                WidgetState,
            ) -> TextWidgetStyle
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full_with_style_sheet(resolver));
        self
    }

    /// 设置文本组件 key。
    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// 将文本包装为带点击命令的元素。
    pub fn on_click<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_click: Some(command),
            ..Default::default()
        })
    }

    /// 将文本包装为带双击命令的元素。
    pub fn on_double_click<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_double_click: Some(command),
            ..Default::default()
        })
    }

    /// 将文本包装为带鼠标进入命令的元素。
    pub fn on_mouse_enter<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_enter: Some(command),
            ..Default::default()
        })
    }

    /// 将文本包装为带鼠标离开命令的元素。
    pub fn on_mouse_leave<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_leave: Some(command),
            ..Default::default()
        })
    }

    /// 将文本包装为带鼠标移动命令的元素。
    pub fn on_mouse_move<VM>(self, command: ValueCommand<VM, Point>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_move: Some(command),
            ..Default::default()
        })
    }

    /// 将文本包装为带挂载命令的元素。
    pub fn on_mount<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_mount: Some(command),
            ..Default::default()
        })
    }

    /// 将文本包装为带卸载命令的元素。
    pub fn on_unmount<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_unmount: Some(command),
            ..Default::default()
        })
    }

    /// 将文本包装为带更新命令的元素。
    pub fn on_update<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_update: Some(command),
            ..Default::default()
        })
    }

    /// 设置鼠标指针样式。
    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.cursor_style = Some(cursor.into());
        self
    }

    /// 设置文本是否允许用户选择。
    pub fn user_select(mut self, user_select: bool) -> Self {
        self.user_select = user_select;
        self
    }

    fn resolved_cursor_style(&self) -> Option<Value<CursorStyle>> {
        self.cursor_style
            .clone()
            .or_else(|| self.user_select.then_some(Value::Static(CursorStyle::Text)))
    }

    fn into_element_with_interactions<VM>(
        self,
        mut interactions: InteractionHandlers<VM>,
    ) -> Element<VM> {
        let background = self.background.clone();
        let layout = self.layout.clone();
        let visual = self.visual.clone();
        interactions.cursor_style = self.resolved_cursor_style();
        Element {
            id: WidgetId::next(),
            key: self.key.clone(),
            layout,
            focus: Default::default(),
            visual,
            interactions,
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events: MediaEventHandlers::default(),
            background,
            tooltip: None,
            popover: None,
            menu: None,
            context_menu: None,
            modal: None,
            drawer: None,
            tab_trigger: None,
            list_item: None,
            tree_root: None,
            tree_node: None,
            data_grid_root: None,
            data_grid_cell: None,
            data_grid_header: None,
            data_grid_resize_handle: None,
            splitter_handle: None,
            carousel_auto_play: None,
            kind: WidgetKind::Text { text: self },
        }
    }

    fn into_element_with_lifecycle_events<VM>(
        self,
        lifecycle_events: LifecycleEventHandlers<VM>,
    ) -> Element<VM> {
        let background = self.background.clone();
        let layout = self.layout.clone();
        let visual = self.visual.clone();
        Element {
            id: WidgetId::next(),
            key: self.key.clone(),
            layout,
            focus: Default::default(),
            visual,
            interactions: InteractionHandlers {
                cursor_style: self.resolved_cursor_style(),
                ..InteractionHandlers::default()
            },
            lifecycle_events,
            media_events: MediaEventHandlers::default(),
            background,
            tooltip: None,
            popover: None,
            menu: None,
            context_menu: None,
            modal: None,
            drawer: None,
            tab_trigger: None,
            list_item: None,
            tree_root: None,
            tree_node: None,
            data_grid_root: None,
            data_grid_cell: None,
            data_grid_header: None,
            data_grid_resize_handle: None,
            splitter_handle: None,
            carousel_auto_play: None,
            kind: WidgetKind::Text { text: self },
        }
    }
}

impl<VM> From<Text> for Element<VM> {
    fn from(value: Text) -> Self {
        let background = value.background.clone();
        let layout = value.layout.clone();
        let visual = value.visual.clone();
        Element {
            id: WidgetId::next(),
            key: value.key.clone(),
            layout,
            focus: Default::default(),
            visual,
            interactions: InteractionHandlers {
                cursor_style: value.resolved_cursor_style(),
                ..InteractionHandlers::default()
            },
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events: MediaEventHandlers::default(),
            background,
            tooltip: None,
            popover: None,
            menu: None,
            context_menu: None,
            modal: None,
            drawer: None,
            tab_trigger: None,
            list_item: None,
            tree_root: None,
            tree_node: None,
            data_grid_root: None,
            data_grid_cell: None,
            data_grid_header: None,
            data_grid_resize_handle: None,
            splitter_handle: None,
            carousel_auto_play: None,
            kind: WidgetKind::Text { text: value },
        }
    }
}
