use crate::foundation::view_model::ValueCommand;
use crate::ui::widget::{
    BackdropBlurPrimitive, FocusScopeState, HitRegion, RenderCommand, RenderPrimitive,
    TextPrimitive, WidgetId,
};

use super::anchor::Anchor;
use super::placement::{
    FlipPolicy, OverlayId, OverlayLayer, Placement, PlacementOptions,
};

pub(crate) struct Overlay<VM> {
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

#[derive(Clone)]
pub(crate) enum OverlayContent<VM> {
    Primitives(Vec<OverlayPrimitive>),
    Hits(Vec<HitRegion<VM>>),
    Batch {
        primitives: Vec<OverlayPrimitive>,
        hits: Vec<HitRegion<VM>>,
        clip_rect: Option<crate::ui::widget::Rect>,
    },
}

#[derive(Clone)]
pub(crate) enum OverlayPrimitive {
    Shape(RenderPrimitive),
    Text(TextPrimitive),
    BackdropBlur(BackdropBlurPrimitive),
    Command(RenderCommand),
}

#[derive(Clone, Copy)]
pub(crate) enum OverlayBackdrop {
    Scrim {
        primitive: RenderPrimitive,
    },
    Blur {
        primitive: BackdropBlurPrimitive,
    },
}
