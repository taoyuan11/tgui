use crate::foundation::binding::{ToastPlacement, ToastQueue};
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{LayoutStyle, Value};
use crate::ui::widget::common::VisualStyle;
use crate::ui::widget::common::{
    CursorStyle, InteractionHandlers, LifecycleEventHandlers, MediaEventHandlers, WidgetId,
    WidgetKind,
};
use crate::ui::widget::core::Element;
use crate::ui::widget::style::StyleResolver;
use crate::ui::widget::style::ToastStyle;

pub struct ToastHost<VM> {
    queue: ToastQueue<VM>,
    placement: ToastPlacement,
    max_visible: Option<usize>,
    style: Option<StyleResolver<ToastStyle>>,
}

impl<VM> ToastHost<VM> {
    pub fn new(queue: ToastQueue<VM>) -> Self {
        Self {
            queue,
            placement: ToastPlacement::Adaptive,
            max_visible: None,
            style: None,
        }
    }

    pub fn placement(mut self, placement: ToastPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn max_visible(mut self, max_visible: usize) -> Self {
        self.max_visible = Some(max_visible);
        self
    }

    pub fn style(
        mut self,
        style: impl Fn(ResolvedThemeMode) -> ToastStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::new(style));
        self
    }
}

impl<VM> From<ToastHost<VM>> for Element<VM> {
    fn from(value: ToastHost<VM>) -> Self {
        let mut interactions = InteractionHandlers::default();
        interactions.cursor_style = Some(Value::Static(CursorStyle::Default));
        let style = value.style;
        Element {
            id: WidgetId::next(),
            key: None,
            layout: LayoutStyle::default(),
            focus: Default::default(),
            visual: VisualStyle::default(),
            interactions,
            lifecycle_events: LifecycleEventHandlers::default(),
            media_events: MediaEventHandlers::default(),
            background: None,
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
            kind: WidgetKind::ToastHost {
                queue: value.queue,
                placement: value.placement,
                max_visible: value.max_visible,
                style,
            },
        }
    }
}
