use crate::foundation::view_model::ValueCommand;
use crate::ui::widget::{
    BackdropBlurPrimitive, FocusScopeState, HitRegion, MeshPrimitive, RenderCommand,
    RenderPrimitive, TextPrimitive, WidgetId,
};

use super::anchor::Anchor;
use super::placement::{FlipPolicy, OverlayId, OverlayLayer, Placement, PlacementOptions};

pub(crate) struct Overlay<VM> {
    pub(crate) source_widget_id: Option<WidgetId>,
    pub(crate) id: OverlayId,
    pub(crate) anchor: Anchor,
    pub(crate) options: PlacementOptions,
    pub(crate) layer: OverlayLayer,
    pub(crate) on_close: Option<ValueCommand<VM, bool>>,
    pub(crate) return_focus_to: Option<WidgetId>,
    pub(crate) close_on_outside_click: bool,
    pub(crate) close_on_escape: bool,
    pub(crate) backdrop: Option<OverlayBackdrop>,
    pub(crate) focus_scope: Option<FocusScopeState>,
}

impl<VM> Overlay<VM> {
    pub(crate) fn new(id: OverlayId, anchor: impl Into<Anchor>) -> Self {
        Self {
            source_widget_id: None,
            id,
            anchor: anchor.into(),
            options: PlacementOptions::default(),
            layer: OverlayLayer::default(),
            on_close: None,
            return_focus_to: None,
            close_on_outside_click: false,
            close_on_escape: false,
            backdrop: None,
            focus_scope: None,
        }
    }

    pub(crate) fn source_widget(mut self, widget_id: WidgetId) -> Self {
        self.source_widget_id = Some(widget_id);
        self
    }

    pub(crate) fn placement(mut self, placement: Placement) -> Self {
        self.options.placement = placement;
        self
    }

    pub(crate) fn offset(mut self, offset: impl Into<crate::ui::unit::Dp>) -> Self {
        self.options.offset = offset.into();
        self
    }

    pub(crate) fn cross_offset(mut self, cross_offset: impl Into<crate::ui::unit::Dp>) -> Self {
        self.options.cross_offset = cross_offset.into();
        self
    }

    pub(crate) fn flip_policy(mut self, flip: FlipPolicy) -> Self {
        self.options.flip = flip;
        self
    }

    pub(crate) fn viewport_padding(mut self, padding: impl Into<crate::ui::unit::Dp>) -> Self {
        self.options.viewport_padding = padding.into();
        self
    }

    pub(crate) fn match_anchor_width(mut self, on: bool) -> Self {
        self.options.match_anchor_width = on;
        self
    }

    pub(crate) fn layer(mut self, layer: OverlayLayer) -> Self {
        self.layer = layer;
        self
    }

    pub(crate) fn on_close(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_close = Some(command);
        self
    }

    pub(crate) fn return_focus_to(mut self, widget_id: WidgetId) -> Self {
        self.return_focus_to = Some(widget_id);
        self
    }

    pub(crate) fn close_on_outside_click(mut self, on: bool) -> Self {
        self.close_on_outside_click = on;
        self
    }

    pub(crate) fn close_on_escape(mut self, on: bool) -> Self {
        self.close_on_escape = on;
        self
    }

    pub(crate) fn backdrop(mut self, backdrop: OverlayBackdrop) -> Self {
        self.backdrop = Some(backdrop);
        self
    }

    pub(crate) fn focus_scope(mut self, scope: FocusScopeState) -> Self {
        self.focus_scope = Some(scope);
        self
    }
}

pub(crate) struct PortalEntry<VM> {
    pub(crate) source_widget_id: Option<WidgetId>,
    pub(crate) overlay_id: OverlayId,
    pub(crate) anchor: Anchor,
    pub(crate) options: PlacementOptions,
    pub(crate) layer: OverlayLayer,
    pub(crate) on_close: Option<ValueCommand<VM, bool>>,
    pub(crate) return_focus_to: Option<WidgetId>,
    pub(crate) close_on_outside_click: bool,
    pub(crate) close_on_escape: bool,
    pub(crate) backdrop: Option<OverlayBackdrop>,
    pub(crate) focus_scope: Option<FocusScopeState>,
    pub(crate) content_size: (crate::ui::unit::Dp, crate::ui::unit::Dp),
    pub(crate) content: OverlayContent<VM>,
}

impl<VM> Clone for PortalEntry<VM> {
    fn clone(&self) -> Self {
        Self {
            source_widget_id: self.source_widget_id,
            overlay_id: self.overlay_id,
            anchor: self.anchor,
            options: self.options.clone(),
            layer: self.layer,
            on_close: self.on_close.clone(),
            return_focus_to: self.return_focus_to,
            close_on_outside_click: self.close_on_outside_click,
            close_on_escape: self.close_on_escape,
            backdrop: self.backdrop,
            focus_scope: self.focus_scope.clone(),
            content_size: self.content_size,
            content: self.content.clone(),
        }
    }
}

impl<VM> From<Overlay<VM>> for PortalEntry<VM> {
    fn from(_value: Overlay<VM>) -> Self {
        unreachable!("PortalEntry::from is not used")
    }
}

impl<VM> PortalEntry<VM> {
    pub(crate) fn new(
        overlay: Overlay<VM>,
        content_size: (crate::ui::unit::Dp, crate::ui::unit::Dp),
        content: OverlayContent<VM>,
    ) -> Self {
        Self {
            source_widget_id: overlay.source_widget_id,
            overlay_id: overlay.id,
            anchor: overlay.anchor,
            options: overlay.options,
            layer: overlay.layer,
            on_close: overlay.on_close,
            return_focus_to: overlay.return_focus_to,
            close_on_outside_click: overlay.close_on_outside_click,
            close_on_escape: overlay.close_on_escape,
            backdrop: overlay.backdrop,
            focus_scope: overlay.focus_scope,
            content_size,
            content,
        }
    }
}

pub(crate) enum OverlayContent<VM> {
    Primitives(Vec<OverlayPrimitive>),
    Hits(Vec<HitRegion<VM>>),
    Batch {
        primitives: Vec<OverlayPrimitive>,
        hits: Vec<HitRegion<VM>>,
        clip_rect: Option<crate::ui::widget::Rect>,
    },
}

impl<VM> Clone for OverlayContent<VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Primitives(primitives) => Self::Primitives(primitives.clone()),
            Self::Hits(hits) => Self::Hits(hits.clone()),
            Self::Batch {
                primitives,
                hits,
                clip_rect,
            } => Self::Batch {
                primitives: primitives.clone(),
                hits: hits.clone(),
                clip_rect: *clip_rect,
            },
        }
    }
}

#[derive(Clone)]
pub(crate) enum OverlayPrimitive {
    Shape(RenderPrimitive),
    Text(TextPrimitive),
    Mesh(MeshPrimitive),
    BackdropBlur(BackdropBlurPrimitive),
    Command(RenderCommand),
}

#[derive(Clone, Copy)]
pub(crate) enum OverlayBackdrop {
    Scrim { primitive: RenderPrimitive },
    Blur { primitive: BackdropBlurPrimitive },
}
