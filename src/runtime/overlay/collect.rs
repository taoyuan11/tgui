use crate::ui::unit::Dp;
use crate::ui::widget::{
    BackdropBlurPrimitive, CanvasCompositePrimitive, CanvasTextHitRegion, ClipMask, ComputedScene,
    HitGeometry, HitInteraction, HitRegion, Point, Rect, RenderCommand, ScrollRegion,
    TextPrimitive, TexturePrimitive,
};

use super::close::OverlayCloseHandle;
use super::overlay::{Overlay, OverlayBackdrop, OverlayContent, OverlayPrimitive, PortalEntry};
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

fn translate_quad(quad: [Point; 4], origin: Point) -> [Point; 4] {
    quad.map(|point| Point::new(point.x + origin.x, point.y + origin.y))
}

fn translate_triangles(
    triangles: &std::sync::Arc<[[Point; 3]]>,
    origin: Point,
) -> std::sync::Arc<[[Point; 3]]> {
    let translated: Vec<_> = triangles
        .iter()
        .copied()
        .map(|triangle| triangle.map(|point| Point::new(point.x + origin.x, point.y + origin.y)))
        .collect();
    std::sync::Arc::from(translated)
}

fn translate_backdrop(
    mut primitive: BackdropBlurPrimitive,
    origin: Point,
) -> BackdropBlurPrimitive {
    primitive.rect = translate_rect(primitive.rect, origin);
    primitive.clip_mask = translate_clip_mask(primitive.clip_mask, origin);
    primitive
}

fn translate_texture(mut primitive: TexturePrimitive, origin: Point) -> TexturePrimitive {
    primitive.frame = translate_rect(primitive.frame, origin);
    primitive.quad = primitive.quad.map(|quad| translate_quad(quad, origin));
    primitive.clip_rect = primitive.clip_rect.map(|rect| translate_rect(rect, origin));
    primitive.clip_mask = translate_clip_mask(primitive.clip_mask, origin);
    primitive
}

fn translate_canvas_text_hit(hit: CanvasTextHitRegion, origin: Point) -> CanvasTextHitRegion {
    CanvasTextHitRegion {
        hit: hit.hit,
        quad: translate_quad(hit.quad, origin),
    }
}

fn translate_hit_geometry(geometry: HitGeometry, origin: Point) -> HitGeometry {
    match geometry {
        HitGeometry::Rect => HitGeometry::Rect,
        HitGeometry::Quad(quad) => HitGeometry::Quad(translate_quad(quad, origin)),
        HitGeometry::Triangles(triangles) => {
            HitGeometry::Triangles(translate_triangles(&triangles, origin))
        }
    }
}

fn translate_hit_interaction<VM>(
    interaction: HitInteraction<VM>,
    origin: Point,
) -> HitInteraction<VM> {
    match interaction {
        HitInteraction::SelectableText {
            id,
            frame,
            padding,
            interactions,
            text_style,
            text,
        } => HitInteraction::SelectableText {
            id,
            frame: translate_rect(frame, origin),
            padding,
            interactions,
            text_style,
            text,
        },
        HitInteraction::Slider {
            id,
            interactions,
            on_change,
            on_change_end,
            value,
            min,
            max,
            step,
            track_rect,
            thumb_rect,
        } => HitInteraction::Slider {
            id,
            interactions,
            on_change,
            on_change_end,
            value,
            min,
            max,
            step,
            track_rect: translate_rect(track_rect, origin),
            thumb_rect: translate_rect(thumb_rect, origin),
        },
        HitInteraction::TextInput {
            id,
            interactions,
            controller,
            on_change,
            on_change_set,
            multiline,
            auto_wrap,
            show_scrollbar,
            frame,
            padding,
            text_style,
        } => HitInteraction::TextInput {
            id,
            interactions,
            controller,
            on_change,
            on_change_set,
            multiline,
            auto_wrap,
            show_scrollbar,
            frame: translate_rect(frame, origin),
            padding,
            text_style,
        },
        HitInteraction::CanvasItem {
            canvas_origin,
            item_origin,
            text_hits,
            id,
            item_id,
            item_interactions,
            cursor_style,
            inverse_transform,
        } => HitInteraction::CanvasItem {
            id,
            item_id,
            item_interactions,
            cursor_style,
            canvas_origin: Point::new(canvas_origin.x + origin.x, canvas_origin.y + origin.y),
            item_origin: Point::new(item_origin.x + origin.x, item_origin.y + origin.y),
            inverse_transform,
            text_hits: std::sync::Arc::from(
                text_hits
                    .iter()
                    .cloned()
                    .map(|hit| translate_canvas_text_hit(hit, origin))
                    .collect::<Vec<_>>(),
            ),
        },
        other => other,
    }
}

fn translate_hit_region<VM>(
    mut hit: HitRegion<VM>,
    origin: Point,
    overlay_scope_path: Option<&Vec<crate::ui::widget::WidgetId>>,
    content_clip: Option<Rect>,
) -> HitRegion<VM> {
    hit.rect = translate_rect(hit.rect, origin);
    hit.clip_rect = content_clip.or_else(|| hit.clip_rect.map(|rect| translate_rect(rect, origin)));
    hit.geometry = translate_hit_geometry(hit.geometry, origin);
    hit.interaction = translate_hit_interaction(hit.interaction, origin);
    if let Some(scope_path) = overlay_scope_path {
        hit.scope_path = scope_path.clone();
        if let Some(focus) = hit.focus.as_mut() {
            focus.scope_path = scope_path.clone();
        }
    }
    hit
}

fn translate_text(mut text: TextPrimitive, origin: Point) -> TextPrimitive {
    text.frame = translate_rect(text.frame, origin);
    text.quad = text.quad.map(|quad| translate_quad(quad, origin));
    text.clip_rect = text.clip_rect.map(|rect| translate_rect(rect, origin));
    text.clip_mask = translate_clip_mask(text.clip_mask, origin);
    text
}

fn translate_canvas_composite(
    mut primitive: CanvasCompositePrimitive,
    origin: Point,
) -> CanvasCompositePrimitive {
    primitive.bounds = translate_rect(primitive.bounds, origin);
    primitive.clip_rect = primitive.clip_rect.map(|rect| translate_rect(rect, origin));
    primitive.clip_mask = translate_clip_mask(primitive.clip_mask, origin);
    primitive.inner_shadow_offset = Point::new(
        primitive.inner_shadow_offset.x,
        primitive.inner_shadow_offset.y,
    );
    primitive.content_commands = std::sync::Arc::from(
        primitive
            .content_commands
            .iter()
            .cloned()
            .map(|command| translate_render_command(command, origin))
            .collect::<Vec<_>>(),
    );
    primitive.mask_commands = primitive.mask_commands.map(|commands| {
        std::sync::Arc::from(
            commands
                .iter()
                .cloned()
                .map(|command| translate_render_command(command, origin))
                .collect::<Vec<_>>(),
        )
    });
    primitive
}

fn translate_render_command(command: RenderCommand, origin: Point) -> RenderCommand {
    match command {
        RenderCommand::BackdropBlur(primitive) => {
            RenderCommand::BackdropBlur(translate_backdrop(primitive, origin))
        }
        RenderCommand::Brush(mut primitive) => {
            primitive.rect = translate_rect(primitive.rect, origin);
            primitive.clip_rect = primitive.clip_rect.map(|rect| translate_rect(rect, origin));
            primitive.clip_mask = translate_clip_mask(primitive.clip_mask, origin);
            RenderCommand::Brush(primitive)
        }
        RenderCommand::CanvasComposite(primitive) => {
            RenderCommand::CanvasComposite(Box::new(translate_canvas_composite(*primitive, origin)))
        }
        RenderCommand::Shape(mut primitive) => {
            primitive.rect = translate_rect(primitive.rect, origin);
            primitive.clip_rect = primitive.clip_rect.map(|rect| translate_rect(rect, origin));
            primitive.clip_mask = translate_clip_mask(primitive.clip_mask, origin);
            RenderCommand::Shape(primitive)
        }
        RenderCommand::Texture(primitive) => {
            RenderCommand::Texture(translate_texture(primitive, origin))
        }
        #[cfg(feature = "video")]
        RenderCommand::VideoTexture(mut primitive) => {
            primitive.frame = translate_rect(primitive.frame, origin);
            primitive.quad = primitive.quad.map(|quad| translate_quad(quad, origin));
            primitive.clip_rect = primitive.clip_rect.map(|rect| translate_rect(rect, origin));
            primitive.clip_mask = translate_clip_mask(primitive.clip_mask, origin);
            RenderCommand::VideoTexture(primitive)
        }
        RenderCommand::Text(primitive) => {
            RenderCommand::Text(Box::new(translate_text(*primitive, origin)))
        }
        RenderCommand::Mesh(mut primitive) => {
            let triangles = translate_triangles(&primitive.triangles, origin);
            let vertices: Vec<_> = primitive
                .vertices
                .iter()
                .copied()
                .map(|mut vertex| {
                    vertex.position[0] += origin.x.get();
                    vertex.position[1] += origin.y.get();
                    vertex
                })
                .collect();
            primitive.triangles = triangles;
            primitive.vertices = std::sync::Arc::from(vertices);
            primitive.clip_rect = primitive.clip_rect.map(|rect| translate_rect(rect, origin));
            primitive.clip_mask = translate_clip_mask(primitive.clip_mask, origin);
            RenderCommand::Mesh(primitive)
        }
    }
}

fn translate_scroll_region(region: ScrollRegion, origin: Point) -> ScrollRegion {
    ScrollRegion {
        id: region.id,
        content_viewport: translate_rect(region.content_viewport, origin),
        visible_frame: translate_rect(region.visible_frame, origin),
        content_bounds: translate_rect(region.content_bounds, origin),
        scroll_offset: region.scroll_offset,
        overflow_x: region.overflow_x,
        overflow_y: region.overflow_y,
        horizontal_track: region
            .horizontal_track
            .map(|rect| translate_rect(rect, origin)),
        horizontal_thumb: region
            .horizontal_thumb
            .map(|rect| translate_rect(rect, origin)),
        vertical_track: region
            .vertical_track
            .map(|rect| translate_rect(rect, origin)),
        vertical_thumb: region
            .vertical_thumb
            .map(|rect| translate_rect(rect, origin)),
    }
}

fn translate_overlay_anchor_rect(rect: Rect, origin: Point) -> Rect {
    translate_rect(rect, origin)
}

macro_rules! push_overlay_command {
    ($bucket:expr, $command:expr) => {{
        let command = $command;
        match &command {
            RenderCommand::BackdropBlur(primitive) => $bucket.backdrop_blurs.push(*primitive),
            RenderCommand::Shape(primitive) => $bucket.shapes.push(*primitive),
            RenderCommand::Texture(primitive) => $bucket.textures.push(primitive.clone()),
            #[cfg(feature = "video")]
            RenderCommand::VideoTexture(_) => {}
            RenderCommand::Text(primitive) => $bucket.texts.push((**primitive).clone()),
            RenderCommand::Mesh(primitive) => $bucket.meshes.push(primitive.clone()),
            RenderCommand::Brush(_) | RenderCommand::CanvasComposite(_) => {}
        }
        $bucket.commands.push(command);
    }};
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

    computed
        .portal_entries
        .push(PortalEntry::new(overlay, content_size, content));
    solved
}

pub(crate) fn finalize_portal_entries<VM>(computed: &mut ComputedScene<VM>, viewport: Rect) {
    let entries = std::mem::take(&mut computed.portal_entries);
    for entry in entries {
        finalize_portal_entry(computed, viewport, entry);
    }
}

fn finalize_portal_entry<VM>(
    computed: &mut ComputedScene<VM>,
    viewport: Rect,
    entry: PortalEntry<VM>,
) -> Option<SolvedPlacement> {
    let anchor_rect = entry
        .anchor
        .to_rect_with(|key| computed.resolve_overlay_anchor(key))
        .unwrap_or_else(|| entry.anchor.to_rect());
    let solved = solve_placement(
        anchor_rect.into(),
        entry.content_size,
        viewport,
        &entry.options,
    );

    if solved.was_hidden {
        return Some(solved);
    }

    let origin = Point::new(solved.rect.x, solved.rect.y);
    let content_clip = match &entry.content {
        OverlayContent::Batch { clip_rect, .. } => clip_rect
            .map(|clip| translate_rect(clip, origin))
            .or(Some(solved.clip_rect)),
        OverlayContent::Scene(_) | OverlayContent::SceneWithPrimitives { .. } => {
            Some(solved.clip_rect)
        }
        _ => Some(solved.clip_rect),
    };
    let bucket = &mut computed.overlay_layers[entry.layer.index()];
    let overlay_scope_path = entry.focus_scope.as_ref().map(|scope| scope.path.clone());
    if let Some(scope) = entry.focus_scope.clone() {
        bucket.focus_scopes.push(scope);
    }

    if let Some(backdrop) = entry.backdrop {
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

    let (primitives, hits, nested_scene, nested_scene_offset) = match entry.content {
        OverlayContent::Primitives(primitives) => (primitives, Vec::new(), None, Point::ZERO),
        OverlayContent::Hits(hits) => (Vec::new(), hits, None, Point::ZERO),
        OverlayContent::Batch {
            primitives,
            hits,
            clip_rect: _,
        } => (primitives, hits, None, Point::ZERO),
        OverlayContent::Scene(scene) => (Vec::new(), Vec::new(), Some(scene), Point::ZERO),
        OverlayContent::SceneWithPrimitives {
            scene,
            scene_offset,
            primitives,
        } => (primitives, Vec::new(), Some(scene), scene_offset),
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
                bucket
                    .commands
                    .push(RenderCommand::Text(Box::new(text.clone())));
                bucket.texts.push(text);
            }
            OverlayPrimitive::Texture(texture) => {
                let mut texture = translate_texture(texture, origin);
                texture.clip_rect = content_clip;
                bucket.textures.push(texture.clone());
                bucket.commands.push(RenderCommand::Texture(texture));
            }
            OverlayPrimitive::Mesh(mut mesh) => {
                let triangles: Vec<_> = mesh
                    .triangles
                    .iter()
                    .copied()
                    .map(|triangle| {
                        triangle.map(|point| Point::new(point.x + origin.x, point.y + origin.y))
                    })
                    .collect();
                let vertices: Vec<_> = mesh
                    .vertices
                    .iter()
                    .copied()
                    .map(|mut vertex| {
                        vertex.position[0] += origin.x.get();
                        vertex.position[1] += origin.y.get();
                        vertex
                    })
                    .collect();
                mesh.triangles = std::sync::Arc::from(triangles);
                mesh.vertices = std::sync::Arc::from(vertices);
                mesh.clip_rect = content_clip;
                mesh.clip_mask = translate_clip_mask(mesh.clip_mask, origin);
                bucket.meshes.push(mesh.clone());
                bucket.commands.push(RenderCommand::Mesh(mesh));
            }
            OverlayPrimitive::BackdropBlur(primitive) => {
                let primitive = translate_backdrop(primitive, origin);
                bucket.backdrop_blurs.push(primitive);
                bucket.commands.push(RenderCommand::BackdropBlur(primitive));
            }
            OverlayPrimitive::Command(command) => {
                push_overlay_command!(bucket, translate_render_command(command, origin));
            }
        }
    }

    for mut hit in hits {
        hit = translate_hit_region(hit, origin, overlay_scope_path.as_ref(), content_clip);
        bucket.hits.push(hit);
    }

    if let Some(scene) = nested_scene {
        let scene = *scene;
        let origin = Point::new(
            origin.x + nested_scene_offset.x,
            origin.y + nested_scene_offset.y,
        );
        let translated_overlay_anchors: std::collections::HashMap<_, _> = scene
            .overlay_anchors
            .iter()
            .map(|(key, rect)| (*key, translate_overlay_anchor_rect(*rect, origin)))
            .collect();

        for command in scene.scene.commands {
            push_overlay_command!(bucket, translate_render_command(command, origin));
        }
        // 处理嵌套场景的 overlay_commands（例如光标）
        for command in scene.scene.overlay_commands {
            push_overlay_command!(bucket, translate_render_command(command, origin));
        }
        for hit in scene.hit_regions {
            bucket.hits.push(translate_hit_region(
                hit,
                origin,
                overlay_scope_path.as_ref(),
                content_clip,
            ));
        }
        for hit in scene.overlay_hit_regions {
            bucket.hits.push(translate_hit_region(
                hit,
                origin,
                overlay_scope_path.as_ref(),
                content_clip,
            ));
        }
        for scope in scene.focus_scopes {
            bucket.focus_scopes.push(scope);
        }
        for (key, rect) in translated_overlay_anchors.iter() {
            computed.overlay_anchors.insert(*key, *rect);
        }
        for portal in scene.portal_entries {
            computed.portal_entries.push(PortalEntry {
                source_widget_id: portal.source_widget_id,
                overlay_id: portal.overlay_id,
                anchor: match portal.anchor {
                    super::anchor::Anchor::Key(key) => {
                        if let Some(rect) = translated_overlay_anchors.get(&key).copied() {
                            super::anchor::Anchor::Rect(rect)
                        } else {
                            super::anchor::Anchor::Key(key)
                        }
                    }
                    super::anchor::Anchor::Rect(rect) => {
                        super::anchor::Anchor::Rect(translate_rect(rect, origin))
                    }
                    super::anchor::Anchor::Point(point) => super::anchor::Anchor::Point(
                        Point::new(point.x + origin.x, point.y + origin.y),
                    ),
                },
                options: portal.options,
                layer: portal.layer,
                on_close: portal.on_close,
                return_focus_to: portal.return_focus_to,
                close_on_outside_click: portal.close_on_outside_click,
                close_on_escape: portal.close_on_escape,
                backdrop: portal.backdrop.map(|backdrop| match backdrop {
                    super::overlay::OverlayBackdrop::Scrim { primitive } => {
                        super::overlay::OverlayBackdrop::Scrim {
                            primitive: match translate_render_command(
                                RenderCommand::Shape(primitive),
                                origin,
                            ) {
                                RenderCommand::Shape(primitive) => primitive,
                                _ => unreachable!(),
                            },
                        }
                    }
                    super::overlay::OverlayBackdrop::Blur { primitive } => {
                        super::overlay::OverlayBackdrop::Blur {
                            primitive: translate_backdrop(primitive, origin),
                        }
                    }
                }),
                focus_scope: portal.focus_scope,
                content_size: portal.content_size,
                content: portal.content,
            });
        }
        for region in scene.scroll_regions {
            computed
                .scroll_regions
                .push(translate_scroll_region(region, origin));
        }
        if computed.ime_cursor_area.is_none() {
            computed.ime_cursor_area = scene
                .ime_cursor_area
                .map(|rect| translate_rect(rect, origin));
        }
        computed
            .virtual_state_updates
            .extend(scene.virtual_state_updates.into_iter());
    }

    let has_close_hook =
        entry.on_close.is_some() || entry.close_on_outside_click || entry.close_on_escape;
    let track_popover_hover = matches!(entry.layer, super::placement::OverlayLayer::Popover)
        && entry.source_widget_id.is_some();
    if has_close_hook || track_popover_hover {
        bucket.close_handlers.push(OverlayCloseHandle {
            source_widget_id: entry.source_widget_id,
            overlay_id: entry.overlay_id,
            rect: solved.rect,
            layer: entry.layer,
            on_close: entry.on_close,
            close_value: false,
            return_focus_to: entry.return_focus_to,
            close_on_outside_click: entry.close_on_outside_click,
            close_on_escape: entry.close_on_escape,
        });
    }

    Some(solved)
}
