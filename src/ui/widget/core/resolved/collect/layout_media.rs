use super::*;
use crate::ui::widget::canvas::CanvasHitGeometry;
use crate::ui::widget::common;

impl<VM> ResolvedElement<VM> {
    pub(super) fn collect_layout_media_kind(
        &self,
        layout_node: &LayoutNode,
        visual_context: VisualContext,
        context: &mut CollectContext<'_, '_>,
        caches: &mut CollectCaches<'_, VM>,
        computed: &mut ComputedScene<VM>,
        visual: &CollectVisualState,
    ) -> bool {
        match &self.kind {
            ResolvedWidgetKind::Container { layout, children } => {
                let content_bounds = compute_container_content_bounds(
                    self,
                    children,
                    layout_node,
                    visual.frame,
                    context,
                );
                let max_scroll = Point {
                    x: (content_bounds.right() - visual.background_frame.right()).max(0.0),
                    y: (content_bounds.bottom() - visual.background_frame.bottom()).max(0.0),
                };
                let requested_scroll = context
                    .scroll_offsets
                    .get(&self.id)
                    .copied()
                    .unwrap_or(Point::ZERO);
                let scroll_offset = Point {
                    x: if layout.overflow_x == Overflow::Scroll {
                        requested_scroll.x.clamp(0.0, max_scroll.x)
                    } else {
                        Dp::ZERO
                    },
                    y: if layout.overflow_y == Overflow::Scroll {
                        requested_scroll.y.clamp(0.0, max_scroll.y)
                    } else {
                        Dp::ZERO
                    },
                };
                let child_clip_rect = apply_overflow_clip(
                    visual_context.clip_rect,
                    visual.background_frame,
                    layout.overflow_x,
                    layout.overflow_y,
                );
                let child_clip_mask = apply_overflow_clip_mask(
                    visual_context.clip_mask,
                    visual.background_frame,
                    visual.background_radius.get(),
                    layout.overflow_x,
                    layout.overflow_y,
                );
                let scrollbar_geometry = compute_scrollbar_geometry(
                    visual.background_frame,
                    content_bounds,
                    scroll_offset,
                    layout,
                    context.theme,
                    context.units,
                );
                let visible_frame = visual
                    .frame
                    .intersect(visual_context.clip_rect)
                    .unwrap_or(Rect::new(visual.frame.x, visual.frame.y, 0.0, 0.0));
                computed.scroll_regions.push(ScrollRegion {
                    id: self.id,
                    content_viewport: visual.background_frame,
                    visible_frame,
                    content_bounds,
                    scroll_offset,
                    overflow_x: layout.overflow_x,
                    overflow_y: layout.overflow_y,
                    horizontal_track: scrollbar_geometry.horizontal_track,
                    horizontal_thumb: scrollbar_geometry.horizontal_thumb,
                    vertical_track: scrollbar_geometry.vertical_track,
                    vertical_thumb: scrollbar_geometry.vertical_thumb,
                });
                let before_children = computed.clone();
                for (child, child_layout) in children.iter().zip(layout_node.children.iter()) {
                    let child_chunk = child.collect_subtree_cache(
                        child_layout,
                        VisualContext {
                            origin: Point {
                                x: visual.frame.x - scroll_offset.x,
                                y: visual.frame.y - scroll_offset.y,
                            },
                            opacity: visual.opacity,
                            clip_rect: child_clip_rect,
                            clip_mask: child_clip_mask,
                        },
                        context,
                        caches.lifecycle_states,
                        caches.chunks,
                        caches.chunk_parts,
                        caches.visual_contexts,
                    );
                    computed.extend(&child_chunk);
                }
                let mut after_children = ComputedScene::default();
                push_scrollbar_primitives(
                    &mut after_children.scene,
                    context.theme,
                    child_clip_rect,
                    visual.opacity,
                    layout,
                    scrollbar_geometry,
                    self.id,
                    context.hovered_scrollbar,
                    context.active_scrollbar,
                );
                caches.chunk_parts.insert(
                    self.id,
                    SceneChunkParts {
                        before_children,
                        after_children: after_children.clone(),
                    },
                );
                computed.extend(&after_children);
                true
            }
            ResolvedWidgetKind::Text { text } => {
                let padding = text
                    .layout
                    .padding
                    .as_ref()
                    .map(|padding| {
                        padding.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Padding,
                            context.now,
                        )
                    })
                    .unwrap_or(Insets::ZERO);
                push_text_primitives(
                    text,
                    visual.frame,
                    context.font_manager,
                    context.theme,
                    context.units,
                    context.animations,
                    context.now,
                    &mut computed.scene,
                    false,
                    false,
                    padding,
                    None,
                    (text.user_select && context.selected_text == Some(self.id))
                        .then_some(context.selected_text_state)
                        .flatten(),
                    context.theme.colors.on_surface,
                    visual.opacity,
                    self.id,
                    visual.primitive_clip,
                    visual.primitive_clip_mask,
                );
                if text.user_select && !visual.disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: visual.frame,
                        clip_rect: visual.primitive_clip,
                        geometry: HitGeometry::Rect,
                        scope_path: context.focus_scope_path(),
                        focus: None,
                        interaction: HitInteraction::SelectableText {
                            id: self.id,
                            frame: visual.frame,
                            padding,
                            interactions: self.interactions.clone(),
                            text_style: text.clone(),
                            text: text.content.resolve(),
                        },
                    });
                }
                true
            }
            #[cfg(feature = "audio")]
            ResolvedWidgetKind::Audio { .. } => true,
            ResolvedWidgetKind::Image { image } => {
                let source = image.source.resolve();
                let loading_background = image
                    .background
                    .as_ref()
                    .map(|background| {
                        background.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Background,
                            context.now,
                        )
                    })
                    .unwrap_or(Color::rgba(255, 255, 255, 0));
                push_media_texture_or_placeholder(
                    self.id,
                    &source,
                    image.fit,
                    visual.frame,
                    visual.background_frame,
                    visual.background_radius.get(),
                    visual.primitive_clip,
                    visual.primitive_clip_mask,
                    visual.opacity,
                    loading_background,
                    context,
                    computed,
                    "image",
                );
                true
            }
            ResolvedWidgetKind::Canvas {
                scene,
                item_interactions,
            } => {
                let scene = scene.resolve();
                let padding = self
                    .layout
                    .padding
                    .as_ref()
                    .map(|padding| {
                        padding.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Padding,
                            context.now,
                        )
                    })
                    .unwrap_or(Insets::ZERO);
                let canvas_frame = visual.background_frame.inset(padding);
                let canvas_clip = visual
                    .primitive_clip
                    .and_then(|clip| clip.intersect(canvas_frame));
                let canvas_visible = visual.primitive_clip.is_none() || canvas_clip.is_some();
                let canvas_clip_mask = if visual.background_radius > 0.0
                    && canvas_frame.width > Dp::ZERO
                    && canvas_frame.height > Dp::ZERO
                {
                    Some(ClipMask {
                        rect: canvas_frame,
                        corner_radius: visual.background_radius.get(),
                    })
                } else {
                    visual.primitive_clip_mask
                };
                let canvas_origin = Point::new(canvas_frame.x, canvas_frame.y);

                if canvas_frame.width > Dp::ZERO
                    && canvas_frame.height > Dp::ZERO
                    && !visual_context.clip_rect.is_empty()
                    && canvas_visible
                {
                    for rendered in tessellate_canvas_scene_items(
                        &scene,
                        canvas_origin,
                        visual.opacity,
                        canvas_clip,
                        canvas_clip_mask,
                        context.font_manager,
                        context.media,
                        context.units,
                    ) {
                        let meshes = rendered.output.meshes;
                        for command in rendered.output.commands {
                            computed.scene.push_render_command(command);
                        }
                        for texture in rendered.output.textures {
                            computed.scene.push_texture(texture);
                        }
                        for text in rendered.output.texts {
                            computed.scene.push_text(text);
                        }
                        for mesh in &meshes {
                            computed.scene.push_mesh(mesh.clone());
                        }

                        if item_interactions.has_any() {
                            if let Some(bounds) = rendered.hit_bounds {
                                let geometry = match rendered.hit_geometry.clone() {
                                    Some(CanvasHitGeometry::Quad(quad)) => HitGeometry::Quad(quad),
                                    Some(CanvasHitGeometry::Triangles(triangles)) => {
                                        HitGeometry::Triangles(triangles)
                                    }
                                    None => HitGeometry::Rect,
                                };
                                computed.hit_regions.push(HitRegion {
                                    rect: Rect::new(
                                        canvas_frame.x + bounds.min_x,
                                        canvas_frame.y + bounds.min_y,
                                        bounds.width(),
                                        bounds.height(),
                                    ),
                                    clip_rect: canvas_clip,
                                    geometry,
                                    scope_path: context.focus_scope_path(),
                                    focus: None,
                                    interaction: HitInteraction::CanvasItem {
                                        id: self.id,
                                        item_id: rendered.item_id,
                                        item_interactions: item_interactions.clone(),
                                        cursor_style: rendered.cursor,
                                        canvas_origin,
                                        item_origin: Point::new(
                                            canvas_origin.x + rendered.local_origin.x,
                                            canvas_origin.y + rendered.local_origin.y,
                                        ),
                                        inverse_transform: rendered.inverse_transform.matrix,
                                        text_hits: rendered
                                            .text_hits
                                            .iter()
                                            .cloned()
                                            .map(|entry| common::CanvasTextHitRegion {
                                                hit: entry.hit,
                                                quad: entry.quad,
                                            })
                                            .collect::<Vec<_>>()
                                            .into(),
                                    },
                                });
                            }
                        }
                    }
                }
                true
            }
            #[cfg(feature = "video")]
            ResolvedWidgetKind::VideoSurface { video, .. } => {
                let loading_background = video
                    .background
                    .as_ref()
                    .map(|background| {
                        background.resolve_widget(
                            context.animations,
                            self.id,
                            WidgetProperty::Background,
                            context.now,
                        )
                    })
                    .unwrap_or(Color::rgba(255, 255, 255, 0));
                push_video_texture_or_placeholder(
                    self.id,
                    video,
                    visual.frame,
                    visual.background_frame,
                    visual.background_radius.get(),
                    visual.primitive_clip,
                    None,
                    visual.opacity,
                    loading_background,
                    context,
                    computed,
                );
                true
            }
            _ => false,
        }
    }
}
