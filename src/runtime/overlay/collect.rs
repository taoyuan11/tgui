use crate::ui::unit::Dp;
use crate::ui::widget::{
    BackdropBlurPrimitive, ClipMask, ComputedScene, Point, Rect, RenderCommand,
};

use super::close::OverlayCloseHandle;
use super::overlay::{Overlay, OverlayBackdrop, OverlayContent, OverlayPrimitive};
use super::placement::SolvedPlacement;
use super::solver::solve_placement;

fn translate_rect(rect: Rect, origin: Point) -> Rect {
    Rect::new(
        rect.x + origin.x.get(),
        rect.y + origin.y.get(),
        rect.width,
        rect.height,
    )
}

fn translate_clip_mask(mask: Option<ClipMask>, origin: Point) -> Option<ClipMask> {
    mask.map(|m| ClipMask {
        rect: translate_rect(m.rect, origin),
        corner_radius: m.corner_radius,
    })
}

fn translate_backdrop(mut primitive: BackdropBlurPrimitive, origin: Point) -> BackdropBlurPrimitive {
    primitive.rect = translate_rect(primitive.rect, origin);
    primitive.clip_mask = translate_clip_mask(primitive.clip_mask, origin);
    primitive
}

pub(crate) fn emit_overlay<VM>(
    computed: &mut ComputedScene<VM>,
    viewport: Rect,
    overlay: Overlay<VM>,
    content_size: (Dp, Dp),
    content: OverlayContent<VM>,
) -> SolvedPlacement {
    let anchor_rect = overlay
        .anchor
        .to_rect_with(|key| computed.resolve_overlay_anchor(key))
        .unwrap_or_else(|| overlay.anchor.to_rect());
    let solved = solve_placement(anchor_rect.into(), content_size, viewport, &overlay.options);

    if solved.was_hidden {
        return solved;
    }

    let origin = Point::new(solved.rect.x, solved.rect.y);
    let content_clip = match &content {
        OverlayContent::Batch { clip_rect, .. } => clip_rect
            .map(|clip| translate_rect(clip, origin))
            .or(Some(solved.clip_rect)),
        _ => Some(solved.clip_rect),
    };
    let bucket = &mut computed.overlay_layers[overlay.layer.index()];
    let overlay_scope_path = overlay.focus_scope.as_ref().map(|scope| scope.path.clone());
    if let Some(scope) = overlay.focus_scope.clone() {
        bucket.focus_scopes.push(scope);
    }

    if let Some(backdrop) = overlay.backdrop {
        match backdrop {
            OverlayBackdrop::Scrim { mut primitive } => {
                primitive.clip_rect = content_clip;
                bucket.shapes.push(primitive);
                bucket.commands.push(RenderCommand::Shape(primitive));
            }
            OverlayBackdrop::Blur { mut primitive } => {
                primitive.clip_rect = content_clip;
                bucket.backdrop_blurs.push(primitive);
                bucket.commands.push(RenderCommand::BackdropBlur(primitive));
            }
        }
    }

    let (primitives, hits) = match content {
        OverlayContent::Primitives(primitives) => (primitives, Vec::new()),
        OverlayContent::Hits(hits) => (Vec::new(), hits),
        OverlayContent::Batch {
            primitives,
            hits,
            clip_rect: _,
        } => (primitives, hits),
    };

    for prim in primitives {
        match prim {
            OverlayPrimitive::Shape(mut shape) => {
                shape.rect = translate_rect(shape.rect, origin);
                shape.clip_rect = content_clip;
                shape.clip_mask = translate_clip_mask(shape.clip_mask, origin);
                bucket.shapes.push(shape);
                bucket.commands.push(RenderCommand::Shape(shape));
            }
            OverlayPrimitive::Text(mut text) => {
                text.frame = translate_rect(text.frame, origin);
                text.clip_rect = content_clip;
                text.clip_mask = translate_clip_mask(text.clip_mask, origin);
                if let Some(quad) = text.quad {
                    text.quad = Some([
                        Point::new(quad[0].x + origin.x, quad[0].y + origin.y),
                        Point::new(quad[1].x + origin.x, quad[1].y + origin.y),
                        Point::new(quad[2].x + origin.x, quad[2].y + origin.y),
                        Point::new(quad[3].x + origin.x, quad[3].y + origin.y),
                    ]);
                }
                bucket.texts.push(text.clone());
                bucket.commands.push(RenderCommand::Text(text));
            }
            OverlayPrimitive::BackdropBlur(primitive) => {
                let primitive = translate_backdrop(primitive, origin);
                bucket.backdrop_blurs.push(primitive);
                bucket.commands.push(RenderCommand::BackdropBlur(primitive));
            }
            OverlayPrimitive::Command(command) => {
                bucket.commands.push(command);
            }
        }
    }

    for mut hit in hits {
        hit.rect = translate_rect(hit.rect, origin);
        hit.clip_rect = content_clip;
        if let Some(scope_path) = overlay_scope_path.as_ref() {
            hit.scope_path = scope_path.clone();
            if let Some(focus) = hit.focus.as_mut() {
                focus.scope_path = scope_path.clone();
            }
        }
        bucket.hits.push(hit);
    }

    let has_close_hook =
        overlay.on_close.is_some() || overlay.close_on_outside_click || overlay.close_on_escape;
    if has_close_hook {
        bucket.close_handlers.push(OverlayCloseHandle {
            overlay_id: overlay.id,
            rect: solved.rect,
            layer: overlay.layer,
            on_close: overlay.on_close,
            close_value: false,
            return_focus_to: overlay.return_focus_to,
            close_on_outside_click: overlay.close_on_outside_click,
            close_on_escape: overlay.close_on_escape,
        });
    }

    solved
}
