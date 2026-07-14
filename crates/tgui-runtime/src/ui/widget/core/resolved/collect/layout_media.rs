use super::*;
use crate::ui::widget::canvas::CanvasHitGeometry;
use crate::ui::widget::common;
use crate::ui::widget::common::ContainerLayout;

fn should_skip_fully_clipped_child<VM: 'static>(
    child: &ResolvedElement<VM>,
    child_layout: &LayoutNode,
    origin: Point,
    clip_rect: Rect,
    context: &CollectContext<'_, '_>,
) -> bool {
    if context.gpu_scroll_container.is_some() {
        return false;
    }

    if !child.can_skip_when_fully_clipped() {
        return false;
    }

    let Ok(layout) = context.taffy.layout(child_layout.node) else {
        return false;
    };
    let frame = Rect::new(
        origin.x + layout.location.x,
        origin.y + layout.location.y,
        layout.size.width,
        layout.size.height,
    );
    frame.fully_outside(clip_rect)
}

fn build_monotonic_child_cull_index<VM: 'static>(
    layout: &ContainerLayout,
    children: &[ResolvedElement<VM>],
    layout_node: &LayoutNode,
    context: &CollectContext<'_, '_>,
) -> Option<common::ChildCullIndex> {
    const MIN_INDEXED_CHILDREN: usize = 16;
    let axis = match &layout.kind {
        ContainerKind::Flow => common::ChildCullAxis::Vertical,
        ContainerKind::Flex {
            direction: Axis::Horizontal,
            wrap: Wrap::NoWrap,
        } => common::ChildCullAxis::Horizontal,
        ContainerKind::Flex {
            direction: Axis::Vertical,
            wrap: Wrap::NoWrap,
        } => common::ChildCullAxis::Vertical,
        ContainerKind::Flex {
            wrap: Wrap::Wrap, ..
        }
        | ContainerKind::Grid { .. }
        | ContainerKind::Stack => return None,
    };
    if children.len() < MIN_INDEXED_CHILDREN || children.len() != layout_node.children.len() {
        return None;
    }
    if children.iter().any(|child| {
        child.layout.position_type != PositionType::Relative || !child.can_skip_when_fully_clipped()
    }) {
        return None;
    }

    let mut intervals = Vec::with_capacity(children.len());
    let mut previous_start = None;
    let mut previous_end = None;
    for child_layout in &layout_node.children {
        let layout = context.taffy.layout(child_layout.node).ok()?;
        let (start, extent) = match axis {
            common::ChildCullAxis::Horizontal => (layout.location.x, layout.size.width),
            common::ChildCullAxis::Vertical => (layout.location.y, layout.size.height),
        };
        let end = start + extent;
        if !start.is_finite() || !end.is_finite() || extent < 0.0 {
            return None;
        }
        let start = Dp::new(start);
        let end = Dp::new(end);
        if previous_start.is_some_and(|previous| start < previous)
            || previous_end.is_some_and(|previous| end < previous)
        {
            return None;
        }
        intervals.push(common::ChildCullInterval { start, end });
        previous_start = Some(start);
        previous_end = Some(end);
    }

    Some(common::ChildCullIndex { axis, intervals })
}

fn monotonic_visible_child_range<VM: 'static>(
    layout: &ContainerLayout,
    children: &[ResolvedElement<VM>],
    layout_node: &LayoutNode,
    child_origin: Point,
    clip_rect: Rect,
    context: &CollectContext<'_, '_>,
) -> Option<std::ops::Range<usize>> {
    // GPU scroll retains clipped commands for later translation, so CPU-side omission is
    // forbidden both for an inherited tagged subtree and while building a new tagged subtree.
    if context.gpu_scroll_container.is_some() || context.gpu_scroll_enabled {
        return None;
    }

    let index = layout_node
        .cached_child_cull_index
        .get_or_init(|| build_monotonic_child_cull_index(layout, children, layout_node, context));
    let index = index.as_ref()?;
    let (origin, clip_start, clip_end) = match index.axis {
        common::ChildCullAxis::Horizontal => (child_origin.x, clip_rect.x, clip_rect.right()),
        common::ChildCullAxis::Vertical => (child_origin.y, clip_rect.y, clip_rect.bottom()),
    };
    let margin = Dp::new(common::CLIP_CULL_MARGIN);
    let visible_start = clip_start - origin - margin;
    let visible_end = clip_end - origin + margin;
    let first = index
        .intervals
        .partition_point(|interval| interval.end < visible_start);
    let end = index
        .intervals
        .partition_point(|interval| interval.start <= visible_end);
    Some(first..end.max(first))
}

impl<VM: 'static> ResolvedElement<VM> {
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
            ResolvedWidgetKind::Container {
                layout, children, ..
            } => {
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
                let base_clip_rect = visual.primitive_clip.unwrap_or(visual_context.clip_rect);
                let child_clip_rect = apply_overflow_clip(
                    base_clip_rect,
                    visual.background_frame,
                    layout.overflow_x,
                    layout.overflow_y,
                );
                let child_overflow_clip_rect =
                    if matches!(layout.overflow_x, Overflow::Hidden | Overflow::Scroll)
                        || matches!(layout.overflow_y, Overflow::Hidden | Overflow::Scroll)
                    {
                        Some(child_clip_rect)
                    } else {
                        visual_context.overflow_clip_rect
                    };
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
                let show_scrollbar = layout
                    .scroll_view
                    .as_ref()
                    .map(|config| config.show_scrollbar.resolve())
                    .unwrap_or(true);
                let visible_frame = visual
                    .frame
                    .intersect(visual_context.clip_rect)
                    .unwrap_or(Rect::new(visual.frame.x, visual.frame.y, 0.0, 0.0));
                computed.scroll_regions.push(ScrollRegion {
                    id: self.id,
                    content_viewport: visual.background_frame,
                    visible_frame,
                    content_bounds,
                    gpu_base_scroll_offset: scroll_offset,
                    scroll_offset,
                    overflow_x: layout.overflow_x,
                    overflow_y: layout.overflow_y,
                    horizontal_track: scrollbar_geometry.horizontal_track,
                    horizontal_thumb: scrollbar_geometry.horizontal_thumb,
                    vertical_track: scrollbar_geometry.vertical_track,
                    vertical_thumb: scrollbar_geometry.vertical_thumb,
                });
                let before_children_cursor = super::ScenePrefixSnapshot::capture(computed);
                let child_origin = Point {
                    x: visual.frame.x - scroll_offset.x,
                    y: visual.frame.y - scroll_offset.y,
                };
                let fast_child_range = monotonic_visible_child_range(
                    layout,
                    children,
                    layout_node,
                    child_origin,
                    child_clip_rect,
                    context,
                );
                let used_fast_child_range = fast_child_range.is_some();
                let child_range =
                    fast_child_range.unwrap_or(0..children.len().min(layout_node.children.len()));
                let previous_gpu_scroll_container = context.gpu_scroll_container;
                if context.gpu_scroll_enabled {
                    context.gpu_scroll_container = Some(self.id);
                }
                for child_index in child_range {
                    let child = &children[child_index];
                    let child_layout = &layout_node.children[child_index];
                    if should_skip_fully_clipped_child(
                        child,
                        child_layout,
                        child_origin,
                        child_clip_rect,
                        context,
                    ) {
                        continue;
                    }
                    let child_id = child.collect_subtree_cache(
                        child_layout,
                        VisualContext {
                            origin: child_origin,
                            opacity: visual.opacity,
                            clip_rect: child_clip_rect,
                            overflow_clip_rect: child_overflow_clip_rect,
                            clip_mask: child_clip_mask,
                        },
                        context,
                        caches.lifecycle_states,
                        caches.chunks,
                        caches.chunk_parts,
                        caches.visual_contexts,
                    );
                    if let Some(child_chunk) = caches.chunks.get(&child_id) {
                        computed.extend(child_chunk);
                    }
                }
                if !used_fast_child_range {
                    push_splitter_handle_hit_regions(
                        children,
                        layout_node,
                        visual.frame,
                        scroll_offset,
                        child_clip_rect,
                        context,
                        computed,
                    );
                }
                {
                    context.gpu_scroll_container = previous_gpu_scroll_container;
                }
                let before_children = before_children_cursor.materialize(computed);
                // `after_children` 仅承载子树收集之后追加的内容(滚动条),它本身就等价于
                // `computed.delta_since(子树之后的快照)` —— 因此无需克隆整份累积场景再做 delta。
                // 对长列表滚动容器,这次克隆会拷贝其全部子节点的图元(每帧一次),代价高昂。
                let mut after_children = ComputedScene::default();
                if show_scrollbar {
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
                }
                computed.extend(&after_children);
                caches.chunk_parts.insert(
                    self.id,
                    SceneChunkParts {
                        before_children,
                        after_children,
                    },
                );
                true
            }
            ResolvedWidgetKind::Virtual {
                arrangement,
                item_layout,
                content_cross_extent,
                overflow_x,
                overflow_y,
                runtime_state,
                window_plan,
                children,
                child_meta,
                ..
            } => {
                let mut scrollbar_layout = ContainerLayout::flow();
                scrollbar_layout.overflow_x = *overflow_x;
                scrollbar_layout.overflow_y = *overflow_y;
                let content_bounds = match arrangement.direction() {
                    crate::ui::widget::VirtualDirection::Vertical => Rect::new(
                        visual.background_frame.x,
                        visual.background_frame.y,
                        content_cross_extent
                            .as_ref()
                            .map(|value| value.resolve())
                            .unwrap_or(visual.background_frame.width)
                            .max(visual.background_frame.width),
                        window_plan
                            .total_main_extent
                            .max(visual.background_frame.height),
                    ),
                    crate::ui::widget::VirtualDirection::Horizontal => Rect::new(
                        visual.background_frame.x,
                        visual.background_frame.y,
                        window_plan
                            .total_main_extent
                            .max(visual.background_frame.width),
                        content_cross_extent
                            .as_ref()
                            .map(|value| value.resolve())
                            .unwrap_or(visual.background_frame.height)
                            .max(visual.background_frame.height),
                    ),
                };
                let max_scroll = Point {
                    x: (content_bounds.right() - visual.background_frame.right()).max(0.0),
                    y: (content_bounds.bottom() - visual.background_frame.bottom()).max(0.0),
                };
                let scroll_offset = Point {
                    x: if *overflow_x == Overflow::Scroll {
                        runtime_state.scroll_offset.x.clamp(0.0, max_scroll.x)
                    } else {
                        Dp::ZERO
                    },
                    y: if *overflow_y == Overflow::Scroll {
                        runtime_state.scroll_offset.y.clamp(0.0, max_scroll.y)
                    } else {
                        Dp::ZERO
                    },
                };
                let base_clip_rect = visual.primitive_clip.unwrap_or(visual_context.clip_rect);
                let child_clip_rect = apply_overflow_clip(
                    base_clip_rect,
                    visual.background_frame,
                    *overflow_x,
                    *overflow_y,
                );
                let child_overflow_clip_rect =
                    if matches!(*overflow_x, Overflow::Hidden | Overflow::Scroll)
                        || matches!(*overflow_y, Overflow::Hidden | Overflow::Scroll)
                    {
                        Some(child_clip_rect)
                    } else {
                        visual_context.overflow_clip_rect
                    };
                let child_clip_mask = apply_overflow_clip_mask(
                    visual_context.clip_mask,
                    visual.background_frame,
                    visual.background_radius.get(),
                    *overflow_x,
                    *overflow_y,
                );
                let scrollbar_geometry = compute_scrollbar_geometry(
                    visual.background_frame,
                    content_bounds,
                    scroll_offset,
                    &scrollbar_layout,
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
                    gpu_base_scroll_offset: scroll_offset,
                    scroll_offset,
                    overflow_x: *overflow_x,
                    overflow_y: *overflow_y,
                    horizontal_track: scrollbar_geometry.horizontal_track,
                    horizontal_thumb: scrollbar_geometry.horizontal_thumb,
                    vertical_track: scrollbar_geometry.vertical_track,
                    vertical_thumb: scrollbar_geometry.vertical_thumb,
                });
                let before_children_cursor = super::ScenePrefixSnapshot::capture(computed);
                let mut measured_extents = Vec::new();
                let mut widget_ids_by_key = Vec::new();
                let mut invalidate_layout = false;
                let child_origin = Point::new(
                    visual.background_frame.x - scroll_offset.x,
                    visual.background_frame.y - scroll_offset.y,
                );
                let previous_gpu_scroll_container = context.gpu_scroll_container;
                if context.gpu_scroll_enabled {
                    context.gpu_scroll_container = Some(self.id);
                }
                for ((child, child_layout), meta) in children
                    .iter()
                    .zip(layout_node.children.iter())
                    .zip(child_meta.iter())
                {
                    if !item_layout.is_measured()
                        && should_skip_fully_clipped_child(
                            child,
                            child_layout,
                            child_origin,
                            child_clip_rect,
                            context,
                        )
                    {
                        continue;
                    }
                    let child_id = child.collect_subtree_cache(
                        child_layout,
                        VisualContext {
                            origin: child_origin,
                            opacity: visual.opacity,
                            clip_rect: child_clip_rect,
                            overflow_clip_rect: child_overflow_clip_rect,
                            clip_mask: child_clip_mask,
                        },
                        context,
                        caches.lifecycle_states,
                        caches.chunks,
                        caches.chunk_parts,
                        caches.visual_contexts,
                    );
                    if item_layout.is_measured() {
                        let layout = context
                            .taffy
                            .layout(child_layout.node)
                            .expect("virtual child layout node should exist");
                        let measured_extent = match arrangement.direction() {
                            crate::ui::widget::VirtualDirection::Vertical => {
                                Dp::new(layout.size.height)
                            }
                            crate::ui::widget::VirtualDirection::Horizontal => {
                                Dp::new(layout.size.width)
                            }
                        }
                        .max(Dp::ZERO);
                        let measured_changed = runtime_state
                            .measurements
                            .measured_extent(meta.item_index)
                            .map(|previous| {
                                (previous - measured_extent).abs()
                                    > crate::ui::widget::MEASURED_EXTENT_INVALIDATION_EPSILON
                            })
                            .unwrap_or(true);
                        if measured_changed {
                            invalidate_layout = true;
                        }
                        measured_extents.push((meta.item_index, measured_extent));
                    }
                    if let Some(key) = child.key.clone() {
                        widget_ids_by_key.push((key, child.id));
                    }
                    if let Some(child_chunk) = caches.chunks.get(&child_id) {
                        computed.extend(child_chunk);
                    }
                }
                {
                    context.gpu_scroll_container = previous_gpu_scroll_container;
                }
                let before_children = before_children_cursor.materialize(computed);
                // `after_children` 收集子树之后追加的所有内容(虚拟状态更新 + 滚动条),
                // 直接构建即可,无需克隆整份累积场景再 `delta_since`。虚拟状态更新此前推入
                // `computed` 且位于快照之后,因此原本就被 delta 纳入 `after_children`;这里
                // 直接写入 `after_children`,再 extend 回 `computed`,行为一致且省掉全场景克隆。
                let mut after_children = ComputedScene::default();
                after_children.virtual_state_updates.push(
                    crate::ui::widget::VirtualSceneStateUpdate {
                        widget_id: self.id,
                        viewport_hint: crate::ui::widget::r#virtual::VirtualViewportHint {
                            width: visual.background_frame.width,
                            height: visual.background_frame.height,
                        },
                        measured_extents,
                        measurement_signature: runtime_state.measurements.signature(),
                        widget_ids_by_key,
                        invalidate_layout,
                    },
                );
                push_scrollbar_primitives(
                    &mut after_children.scene,
                    context.theme,
                    child_clip_rect,
                    visual.opacity,
                    &scrollbar_layout,
                    scrollbar_geometry,
                    self.id,
                    context.hovered_scrollbar,
                    context.active_scrollbar,
                );
                computed.extend(&after_children);
                caches.chunk_parts.insert(
                    self.id,
                    SceneChunkParts {
                        before_children,
                        after_children,
                    },
                );
                true
            }
            ResolvedWidgetKind::Portal { .. } => true,
            ResolvedWidgetKind::Text { text, .. } => {
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
                super::super::collect_profile::timed(
                    super::super::collect_profile::Phase::Text,
                    || {
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
                        )
                    },
                );
                if text.user_select && !visual.disabled {
                    computed.hit_regions.push(HitRegion {
                        rect: visual.frame,
                        clip_rect: visual.primitive_clip,
                        geometry: HitGeometry::Rect,
                        transform_chain: context.transform_stack.clone(),
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
                        gpu_scroll_container: context.gpu_scroll_container,
                    });
                }
                true
            }
            ResolvedWidgetKind::ToastHost { .. } => true,
            #[cfg(feature = "audio")]
            ResolvedWidgetKind::Audio { .. } => true,
            ResolvedWidgetKind::Image { image, .. } => {
                let source = track_property_scope(PropertySlot::Texture, || image.source.resolve());
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
            ResolvedWidgetKind::Icon { icon } => {
                let resolved = crate::ui::widget::icon::resolve_icon_style_with_sheet(
                    icon.style.as_ref(),
                    &context.style_context,
                    context.style_sheet,
                    &self.visual,
                    visual.widget_state,
                );
                crate::ui::widget::icon::push_svg_icon_texture(
                    &mut computed.scene,
                    context.media,
                    context.units,
                    icon.source,
                    resolved.color.resolve(),
                    visual.background_frame,
                    visual.opacity,
                    None,
                    visual.primitive_clip,
                    visual.primitive_clip_mask,
                );
                true
            }
            ResolvedWidgetKind::Canvas {
                scene,
                item_interactions,
                ..
            } => {
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
                    let collect_hit_metadata = item_interactions.has_any();
                    scene.resolve_ref(|scene| {
                        for rendered in tessellate_canvas_scene_items(
                            scene,
                            canvas_origin,
                            visual.opacity,
                            canvas_clip,
                            canvas_clip_mask,
                            collect_hit_metadata,
                            context.font_manager,
                            context.media,
                            context.units,
                        ) {
                            for command in rendered.output.commands {
                                computed.scene.push_render_command(command);
                            }
                            for texture in rendered.output.textures {
                                computed.scene.push_texture(texture);
                            }
                            for text in rendered.output.texts {
                                computed.scene.push_text(text);
                            }
                            for mesh in rendered.output.meshes {
                                computed.scene.push_mesh(mesh);
                            }

                            if collect_hit_metadata {
                                if let Some(bounds) = rendered.hit_bounds {
                                    let geometry = match rendered.hit_geometry {
                                        Some(CanvasHitGeometry::Quad(quad)) => {
                                            HitGeometry::Quad(quad)
                                        }
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
                                        transform_chain: context.transform_stack.clone(),
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
                                        gpu_scroll_container: context.gpu_scroll_container,
                                    });
                                }
                            }
                        }
                    });
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

fn push_splitter_handle_hit_regions<VM: 'static>(
    children: &[ResolvedElement<VM>],
    layout_node: &LayoutNode,
    parent_frame: Rect,
    scroll_offset: Point,
    clip_rect: Rect,
    context: &CollectContext<'_, '_>,
    computed: &mut ComputedScene<VM>,
) {
    for (child_index, child) in children.iter().enumerate() {
        let Some(state) = child.splitter_handle.as_ref() else {
            continue;
        };
        let Some(handle_layout) = layout_node.children.get(child_index) else {
            continue;
        };
        let Some(handle_frame) =
            child_frame_from_parent_layout(handle_layout, parent_frame, scroll_offset, context)
        else {
            continue;
        };
        let before = child_index
            .checked_sub(1)
            .and_then(|index| layout_node.children.get(index))
            .and_then(|node| {
                child_frame_from_parent_layout(node, parent_frame, scroll_offset, context)
            });
        let after = layout_node.children.get(child_index + 1).and_then(|node| {
            child_frame_from_parent_layout(node, parent_frame, scroll_offset, context)
        });
        let pair_extent = match state.axis {
            Axis::Horizontal => {
                before.map(|frame| frame.width).unwrap_or(Dp::ZERO)
                    + after.map(|frame| frame.width).unwrap_or(Dp::ZERO)
            }
            Axis::Vertical => {
                before.map(|frame| frame.height).unwrap_or(Dp::ZERO)
                    + after.map(|frame| frame.height).unwrap_or(Dp::ZERO)
            }
        }
        .max(Dp::new(1.0));
        computed.hit_regions.push(HitRegion {
            rect: handle_frame,
            clip_rect: Some(clip_rect),
            geometry: HitGeometry::Rect,
            transform_chain: context.transform_stack.clone(),
            scope_path: context.focus_scope_path(),
            focus: None,
            interaction: HitInteraction::SplitterHandle {
                id: child.id,
                state: state.clone(),
                interactions: child.interactions.clone(),
                pair_extent,
            },
            gpu_scroll_container: context.gpu_scroll_container,
        });
    }
}

fn child_frame_from_parent_layout(
    layout_node: &LayoutNode,
    parent_frame: Rect,
    scroll_offset: Point,
    context: &CollectContext<'_, '_>,
) -> Option<Rect> {
    let layout = context.taffy.layout(layout_node.node).ok()?;
    Some(Rect::new(
        parent_frame.x - scroll_offset.x + layout.location.x,
        parent_frame.y - scroll_offset.y + layout.location.y,
        layout.size.width,
        layout.size.height,
    ))
}
