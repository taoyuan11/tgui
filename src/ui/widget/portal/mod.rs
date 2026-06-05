use crate::foundation::view_model::ValueCommand;
use crate::ui::layout::LayoutStyle;
use crate::ui::layout::Value;
use crate::ui::unit::Dp;
use crate::ui::widget::common::{
    InteractionHandlers, LifecycleEventHandlers, MediaEventHandlers, VisualStyle, WidgetKind,
};
use crate::ui::widget::core::Element;
use crate::ui::widget::overlay::{
    AnchorKey, FlipPolicy, OverlayLayer, Placement, PlacementOptions,
};
use crate::ui::widget::{FocusScopeOptions, Point, Rect, WidgetId};

/// Window target for a [`Portal`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum PortalTarget {
    /// Render into the layer stack of the window where the portal is declared.
    #[default]
    CurrentWindow,
    /// Render into the layer stack of another runtime-managed window.
    WindowKey(String),
}

impl PortalTarget {
    pub fn window(key: impl Into<String>) -> Self {
        Self::WindowKey(key.into())
    }
}

impl From<String> for PortalTarget {
    fn from(value: String) -> Self {
        Self::WindowKey(value)
    }
}

impl From<&str> for PortalTarget {
    fn from(value: &str) -> Self {
        Self::WindowKey(value.to_string())
    }
}

/// A target layer stack for portal content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerStack {
    pub(crate) target: PortalTarget,
    pub(crate) layer: OverlayLayer,
}

impl LayerStack {
    pub fn current(layer: OverlayLayer) -> Self {
        Self {
            target: PortalTarget::CurrentWindow,
            layer,
        }
    }

    pub fn window(key: impl Into<String>, layer: OverlayLayer) -> Self {
        Self {
            target: PortalTarget::WindowKey(key.into()),
            layer,
        }
    }

    pub fn target(&self) -> &PortalTarget {
        &self.target
    }

    pub fn layer(&self) -> OverlayLayer {
        self.layer
    }
}

/// Anchor used by a [`Portal`] before it enters the overlay placement solver.
#[derive(Clone, Debug, PartialEq)]
pub enum PortalAnchor {
    /// Use the in-tree frame of the portal widget itself.
    SelfWidget,
    /// Use the target window viewport.
    Viewport,
    Rect(Rect),
    Point(Point),
    Key(AnchorKey),
}

impl From<Rect> for PortalAnchor {
    fn from(value: Rect) -> Self {
        Self::Rect(value)
    }
}

impl From<Point> for PortalAnchor {
    fn from(value: Point) -> Self {
        Self::Point(value)
    }
}

impl From<AnchorKey> for PortalAnchor {
    fn from(value: AnchorKey) -> Self {
        Self::Key(value)
    }
}

/// Render an element subtree into a top-level overlay layer.
pub struct Portal<VM> {
    pub(crate) content: Element<VM>,
    pub(crate) open: Value<bool>,
    pub(crate) target: PortalTarget,
    pub(crate) anchor: Option<PortalAnchor>,
    pub(crate) options: PlacementOptions,
    pub(crate) layer: OverlayLayer,
    pub(crate) on_open_change: Option<ValueCommand<VM, bool>>,
    pub(crate) return_focus_to: Option<WidgetId>,
    pub(crate) close_on_outside_click: bool,
    pub(crate) close_on_escape: bool,
    pub(crate) focus_scope: Option<FocusScopeOptions>,
}

impl<VM: 'static> Portal<VM> {
    pub fn new(content: impl Into<Element<VM>>) -> Self {
        Self {
            content: content.into(),
            open: Value::Static(true),
            target: PortalTarget::CurrentWindow,
            anchor: None,
            options: PlacementOptions::default(),
            layer: OverlayLayer::Popover,
            on_open_change: None,
            return_focus_to: None,
            close_on_outside_click: false,
            close_on_escape: false,
            focus_scope: None,
        }
    }

    pub fn open(mut self, open: impl Into<Value<bool>>) -> Self {
        self.open = open.into();
        self
    }

    pub fn target(mut self, target: impl Into<PortalTarget>) -> Self {
        self.target = target.into();
        self
    }

    pub fn target_window(mut self, key: impl Into<String>) -> Self {
        self.target = PortalTarget::WindowKey(key.into());
        self
    }

    pub fn layer(mut self, layer: OverlayLayer) -> Self {
        self.layer = layer;
        self
    }

    pub fn stack(mut self, stack: LayerStack) -> Self {
        self.target = stack.target;
        self.layer = stack.layer;
        self
    }

    pub fn anchor(mut self, anchor: impl Into<PortalAnchor>) -> Self {
        self.anchor = Some(anchor.into());
        self
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.options.placement = placement;
        self
    }

    pub fn offset(mut self, offset: impl Into<Dp>) -> Self {
        self.options.offset = offset.into();
        self
    }

    pub fn cross_offset(mut self, cross_offset: impl Into<Dp>) -> Self {
        self.options.cross_offset = cross_offset.into();
        self
    }

    pub fn flip_policy(mut self, policy: FlipPolicy) -> Self {
        self.options.flip = policy;
        self
    }

    pub fn viewport_padding(mut self, padding: impl Into<Dp>) -> Self {
        self.options.viewport_padding = padding.into();
        self
    }

    pub fn match_anchor_width(mut self, on: bool) -> Self {
        self.options.match_anchor_width = on;
        self
    }

    pub fn close_on_escape(mut self, close: bool) -> Self {
        self.close_on_escape = close;
        self
    }

    pub fn close_on_outside_click(mut self, close: bool) -> Self {
        self.close_on_outside_click = close;
        self
    }

    pub fn on_open_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_open_change = Some(command);
        self
    }

    pub fn return_focus_to(mut self, widget_id: WidgetId) -> Self {
        self.return_focus_to = Some(widget_id);
        self
    }

    pub fn focus_scope(mut self, options: FocusScopeOptions) -> Self {
        self.focus_scope = Some(options);
        self
    }
}

impl<VM: 'static> From<Portal<VM>> for Element<VM> {
    fn from(portal: Portal<VM>) -> Self {
        Element {
            id: WidgetId::next(),
            key: None,
            layout: LayoutStyle::default(),
            focus: Default::default(),
            visual: VisualStyle::default(),
            interactions: InteractionHandlers::default(),
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
            data_grid_root: None,
            data_grid_cell: None,
            data_grid_header: None,
            data_grid_resize_handle: None,
            kind: WidgetKind::Portal {
                content: Box::new(portal.content),
                open: portal.open,
                target: portal.target,
                anchor: portal.anchor,
                options: portal.options,
                layer: portal.layer,
                on_open_change: portal.on_open_change,
                return_focus_to: portal.return_focus_to,
                close_on_outside_click: portal.close_on_outside_click,
                close_on_escape: portal.close_on_escape,
                focus_scope: portal.focus_scope,
            },
        }
    }
}
