use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, ValueCommand};
use crate::media::{ContentFit, MediaBytes, MediaSource};
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{Align, Insets, LayoutStyle, Value};

use super::super::common::{
    CursorStyle, InteractionHandlers, LifecycleEventHandlers, MediaEventHandlers, Point,
    VisualStyle, WidgetId, WidgetKey, WidgetKind,
};
use super::super::container::{
    set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue,
};
use super::super::core::Element;
use super::super::style::{ImageStyle, StyleResolver};
use super::source::{IntoImagePathSource, IntoImageUrlSource};

/// 图片组件。
#[derive(Clone)]
pub struct Image {
    pub(crate) key: Option<WidgetKey>,
    pub(crate) layout: LayoutStyle,
    pub(crate) visual: VisualStyle,
    pub(crate) source: Value<MediaSource>,
    pub(crate) background: Option<Value<Color>>,
    pub(crate) fit: ContentFit,
    pub(crate) cursor_style: Option<Value<CursorStyle>>,
    pub(crate) style: Option<StyleResolver<ImageStyle>>,
}

macro_rules! impl_image_layout_api {
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
            self.layout.column_start = Some(start.max(1));
            self
        }

        /// 设置网格行起始位置。
        pub fn row(mut self, start: usize) -> Self {
            self.layout.row_start = Some(start.max(1));
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

impl Image {
    /// 直接通过 `MediaSource` 创建图片组件。
    ///
    /// # 参数
    /// - `source`：图片源。
    ///
    /// # 返回值
    /// 返回新的图片组件。
    pub fn new(source: impl Into<Value<MediaSource>>) -> Self {
        Self {
            key: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            source: source.into(),
            background: None,
            fit: ContentFit::Contain,
            cursor_style: None,
            style: None,
        }
    }

    /// 通过本地路径创建图片组件。
    pub fn from_path(path: impl IntoImagePathSource) -> Self {
        Self::new(path.into_image_path_source())
    }

    /// 通过网络地址创建图片组件。
    pub fn from_url(url: impl IntoImageUrlSource) -> Self {
        Self::new(url.into_image_url_source())
    }

    /// 通过内存字节创建图片组件。
    pub fn from_bytes(bytes: impl Into<MediaBytes>) -> Self {
        Self::new(MediaSource::Bytes(bytes.into()))
    }

    impl_image_layout_api!();

    /// 设置组件 key。
    pub fn key(mut self, key: impl Into<WidgetKey>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// 设置图片样式解析器。
    pub fn style(
        mut self,
        resolver: impl Fn(ResolvedThemeMode) -> ImageStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(super::super::style::StyleResolver::new(resolver));
        self
    }

    /// 将图片包装为带点击命令的元素。
    pub fn on_click<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_click: Some(command),
            ..Default::default()
        })
    }

    /// 将图片包装为带双击命令的元素。
    pub fn on_double_click<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_double_click: Some(command),
            ..Default::default()
        })
    }

    /// 将图片包装为带鼠标进入命令的元素。
    pub fn on_mouse_enter<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_enter: Some(command),
            ..Default::default()
        })
    }

    /// 将图片包装为带鼠标离开命令的元素。
    pub fn on_mouse_leave<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_leave: Some(command),
            ..Default::default()
        })
    }

    /// 将图片包装为带鼠标移动命令的元素。
    pub fn on_mouse_move<VM>(self, command: ValueCommand<VM, Point>) -> Element<VM> {
        self.into_element_with_interactions(InteractionHandlers {
            on_mouse_move: Some(command),
            ..Default::default()
        })
    }

    /// 将图片包装为带挂载命令的元素。
    pub fn on_mount<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_mount: Some(command),
            ..Default::default()
        })
    }

    /// 将图片包装为带卸载命令的元素。
    pub fn on_unmount<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_unmount: Some(command),
            ..Default::default()
        })
    }

    /// 将图片包装为带更新命令的元素。
    pub fn on_update<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_lifecycle_events(LifecycleEventHandlers {
            on_update: Some(command),
            ..Default::default()
        })
    }

    /// 将图片包装为带加载中命令的元素。
    pub fn on_loading<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_loading: Some(command),
            ..Default::default()
        })
    }

    /// 将图片包装为带加载成功命令的元素。
    pub fn on_success<VM>(self, command: Command<VM>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_success: Some(command),
            ..Default::default()
        })
    }

    /// 将图片包装为带加载失败命令的元素。
    pub fn on_error<VM>(self, command: ValueCommand<VM, String>) -> Element<VM> {
        self.into_element_with_media_events(MediaEventHandlers {
            on_error: Some(command),
            ..Default::default()
        })
    }

    /// 设置鼠标指针样式。
    pub fn cursor(mut self, cursor: impl Into<Value<CursorStyle>>) -> Self {
        self.cursor_style = Some(cursor.into());
        self
    }

    fn into_element_with_interactions<VM>(
        self,
        mut interactions: InteractionHandlers<VM>,
    ) -> Element<VM> {
        interactions.cursor_style = self.cursor_style.clone();
        Element {
            id: WidgetId::next(),
            key: self.key.clone(),
            layout: self.layout.clone(),
            focus: Default::default(),
            visual: self.visual.clone(),
            interactions,
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events: MediaEventHandlers::default(),
            background: self.background.clone(),
            tooltip: None,
            popover: None,
            menu: None,
            context_menu: None,
            modal: None,
            drawer: None,
            tab_trigger: None,
            list_item: None,
            kind: WidgetKind::Image { image: self },
        }
    }

    fn into_element_with_lifecycle_events<VM>(
        self,
        lifecycle_events: LifecycleEventHandlers<VM>,
    ) -> Element<VM> {
        Element {
            id: WidgetId::next(),
            key: self.key.clone(),
            layout: self.layout.clone(),
            focus: Default::default(),
            visual: self.visual.clone(),
            interactions: InteractionHandlers {
                cursor_style: self.cursor_style.clone(),
                ..Default::default()
            },
            lifecycle_events,
            media_events: MediaEventHandlers::default(),
            background: self.background.clone(),
            tooltip: None,
            popover: None,
            menu: None,
            context_menu: None,
            modal: None,
            drawer: None,
            tab_trigger: None,
            list_item: None,
            kind: WidgetKind::Image { image: self },
        }
    }

    fn into_element_with_media_events<VM>(
        self,
        media_events: MediaEventHandlers<VM>,
    ) -> Element<VM> {
        Element {
            id: WidgetId::next(),
            key: self.key.clone(),
            layout: self.layout.clone(),
            focus: Default::default(),
            visual: self.visual.clone(),
            interactions: InteractionHandlers {
                cursor_style: self.cursor_style.clone(),
                ..Default::default()
            },
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events,
            background: self.background.clone(),
            tooltip: None,
            popover: None,
            menu: None,
            context_menu: None,
            modal: None,
            drawer: None,
            tab_trigger: None,
            list_item: None,
            kind: WidgetKind::Image { image: self },
        }
    }
}

impl<VM> From<Image> for Element<VM> {
    fn from(value: Image) -> Self {
        Element {
            id: WidgetId::next(),
            key: value.key.clone(),
            layout: value.layout.clone(),
            focus: Default::default(),
            visual: value.visual.clone(),
            interactions: InteractionHandlers {
                cursor_style: value.cursor_style.clone(),
                ..Default::default()
            },
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events: MediaEventHandlers::default(),
            background: value.background.clone(),
            tooltip: None,
            popover: None,
            menu: None,
            context_menu: None,
            modal: None,
            drawer: None,
            tab_trigger: None,
            list_item: None,
            kind: WidgetKind::Image { image: value },
        }
    }
}
