mod effects;

use crate::foundation::error::TguiError;
use crate::text::font::FontManager;

use super::prepare::PreparedCommand;
use super::surface::surface_clear_color;
use super::{
    BrushVertex, MeshVertex, OffscreenTarget, PushTranslate, RectVertex, Renderer,
    SpriteBindGroupId, TextVertex,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DrawPipeline {
    Rect,
    Brush,
    Mesh,
    Sprite,
}

fn select_state<T: Copy + Eq>(current: &mut Option<T>, next: T) -> bool {
    if *current == Some(next) {
        false
    } else {
        *current = Some(next);
        true
    }
}

fn vertex_draw_range<T>(vertex_offset: u64, vertex_count: u32) -> std::ops::Range<u32> {
    let stride = std::mem::size_of::<T>() as u64;
    debug_assert!(stride > 0);
    debug_assert_eq!(vertex_offset % stride, 0);
    let first_vertex = u32::try_from(vertex_offset / stride)
        .expect("tgui vertex pool offset exceeds wgpu draw vertex range");
    let end_vertex = first_vertex
        .checked_add(vertex_count)
        .expect("tgui vertex pool draw range exceeds u32");
    first_vertex..end_vertex
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ContiguousDrawSpan {
    next_command_index: usize,
    vertex_offset: u64,
    vertex_count: u32,
    command_count: usize,
}

#[derive(Clone, Copy)]
struct UnboundDrawMeta {
    clip_rect: Option<crate::ui::widget::Rect>,
    translate: Option<PushTranslate>,
    vertex_offset: u64,
    vertex_count: u32,
}

fn push_translates_compatible(first: Option<PushTranslate>, second: Option<PushTranslate>) -> bool {
    match (first, second) {
        (None, None) => true,
        (Some(first), Some(second)) => {
            first.offset_ndc == second.offset_ndc && first.offset_physical == second.offset_physical
        }
        (None, Some(_)) | (Some(_), None) => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn sprite_draws_compatible(
    first_binding: SpriteBindGroupId,
    first_clip_rect: Option<crate::ui::widget::Rect>,
    first_translate: Option<PushTranslate>,
    expected_next_offset: u64,
    next_binding: SpriteBindGroupId,
    next_clip_rect: Option<crate::ui::widget::Rect>,
    next_translate: Option<PushTranslate>,
    next_offset: u64,
) -> bool {
    first_binding == next_binding
        && first_clip_rect == next_clip_rect
        && push_translates_compatible(first_translate, next_translate)
        && expected_next_offset == next_offset
}

/// Extends one sprite draw across immediately adjacent, state-compatible commands.
///
/// Text atlas entries on the same page share a bind group, while each text primitive still owns
/// its own six vertices. Prepare writes those vertices consecutively into the frame pool. Joining
/// the ranges therefore preserves primitive order and blending exactly; no command is reordered.
/// Clip masks, opacity, UVs, and static transforms are already encoded per vertex. The remaining
/// dynamic state must match exactly: atlas/texture binding, scissor, and scroll/transform
/// translation. A padding gap or count overflow conservatively ends the span.
fn sprite_draw_span(
    commands: &[PreparedCommand],
    command_index: usize,
    batching_enabled: bool,
) -> ContiguousDrawSpan {
    let PreparedCommand::Sprite(first) = &commands[command_index] else {
        unreachable!("sprite_draw_span requires a sprite command");
    };
    let mut span = ContiguousDrawSpan {
        next_command_index: command_index + 1,
        vertex_offset: first.vertex_offset,
        vertex_count: first.vertex_count,
        command_count: 1,
    };
    if !batching_enabled {
        return span;
    }

    while let Some(PreparedCommand::Sprite(next)) = commands.get(span.next_command_index) {
        let stride = std::mem::size_of::<TextVertex>() as u64;
        let Some(span_bytes) = u64::from(span.vertex_count).checked_mul(stride) else {
            break;
        };
        let Some(expected_offset) = span.vertex_offset.checked_add(span_bytes) else {
            break;
        };
        if !sprite_draws_compatible(
            first.binding.id,
            first.clip_rect,
            first.scroll_translate,
            expected_offset,
            next.binding.id,
            next.clip_rect,
            next.scroll_translate,
            next.vertex_offset,
        ) {
            break;
        }
        let Some(vertex_count) = span.vertex_count.checked_add(next.vertex_count) else {
            break;
        };

        span.vertex_count = vertex_count;
        span.command_count += 1;
        span.next_command_index += 1;
    }
    span
}

fn mesh_draw_span(
    commands: &[PreparedCommand],
    command_index: usize,
    batching_enabled: bool,
) -> ContiguousDrawSpan {
    let PreparedCommand::Mesh(first) = &commands[command_index] else {
        unreachable!("mesh_draw_span requires a mesh command");
    };
    let mut span = ContiguousDrawSpan {
        next_command_index: command_index + 1,
        vertex_offset: first.vertex_offset,
        vertex_count: first.vertex_count,
        command_count: 1,
    };
    if !batching_enabled {
        return span;
    }

    while let Some(PreparedCommand::Mesh(next)) = commands.get(span.next_command_index) {
        let stride = std::mem::size_of::<MeshVertex>() as u64;
        let Some(span_bytes) = u64::from(span.vertex_count).checked_mul(stride) else {
            break;
        };
        let Some(expected_offset) = span.vertex_offset.checked_add(span_bytes) else {
            break;
        };
        if !mesh_draws_compatible(
            first.clip_binding.id,
            first.clip_rect,
            first.scroll_translate,
            expected_offset,
            next.clip_binding.id,
            next.clip_rect,
            next.scroll_translate,
            next.vertex_offset,
        ) {
            break;
        }
        let Some(vertex_count) = span.vertex_count.checked_add(next.vertex_count) else {
            break;
        };
        span.vertex_count = vertex_count;
        span.command_count += 1;
        span.next_command_index += 1;
    }
    span
}

#[allow(clippy::too_many_arguments)]
fn mesh_draws_compatible(
    first_binding: super::MeshClipBindGroupId,
    first_clip_rect: Option<crate::ui::widget::Rect>,
    first_translate: Option<PushTranslate>,
    expected_next_offset: u64,
    next_binding: super::MeshClipBindGroupId,
    next_clip_rect: Option<crate::ui::widget::Rect>,
    next_translate: Option<PushTranslate>,
    next_offset: u64,
) -> bool {
    first_binding == next_binding
        && first_clip_rect == next_clip_rect
        && push_translates_compatible(first_translate, next_translate)
        && expected_next_offset == next_offset
}

fn rect_draw_meta(command: &PreparedCommand) -> Option<UnboundDrawMeta> {
    let PreparedCommand::Rect(draw) = command else {
        return None;
    };
    Some(UnboundDrawMeta {
        clip_rect: draw.clip_rect,
        translate: draw.scroll_translate,
        vertex_offset: draw.vertex_offset,
        vertex_count: draw.vertex_count,
    })
}

fn brush_draw_meta(command: &PreparedCommand) -> Option<UnboundDrawMeta> {
    let PreparedCommand::Brush(draw) = command else {
        return None;
    };
    Some(UnboundDrawMeta {
        clip_rect: draw.clip_rect,
        translate: draw.scroll_translate,
        vertex_offset: draw.vertex_offset,
        vertex_count: draw.vertex_count,
    })
}

/// Joins consecutive unbound-pipeline draws (Rect or Brush) without changing primitive order.
/// Every visual parameter is already carried by the vertices, leaving only scissor and the
/// scroll/transform immediate as dynamic compatibility boundaries.
fn unbound_draw_span<T>(
    commands: &[PreparedCommand],
    command_index: usize,
    batching_enabled: bool,
    draw_meta: impl Fn(&PreparedCommand) -> Option<UnboundDrawMeta>,
) -> ContiguousDrawSpan {
    let first =
        draw_meta(&commands[command_index]).expect("unbound_draw_span requires a matching command");
    let mut span = ContiguousDrawSpan {
        next_command_index: command_index + 1,
        vertex_offset: first.vertex_offset,
        vertex_count: first.vertex_count,
        command_count: 1,
    };
    if !batching_enabled {
        return span;
    }

    while let Some(next) = commands.get(span.next_command_index).and_then(&draw_meta) {
        let stride = std::mem::size_of::<T>() as u64;
        let Some(span_bytes) = u64::from(span.vertex_count).checked_mul(stride) else {
            break;
        };
        let Some(expected_offset) = span.vertex_offset.checked_add(span_bytes) else {
            break;
        };
        if next.clip_rect != first.clip_rect
            || !push_translates_compatible(next.translate, first.translate)
            || next.vertex_offset != expected_offset
        {
            break;
        }
        let Some(vertex_count) = span.vertex_count.checked_add(next.vertex_count) else {
            break;
        };
        span.vertex_count = vertex_count;
        span.command_count += 1;
        span.next_command_index += 1;
    }
    span
}

#[cfg(any(test, feature = "bench-support"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SceneDrawStats {
    pub(super) rect_commands: usize,
    pub(super) rect_draw_calls: usize,
    pub(super) brush_commands: usize,
    pub(super) brush_draw_calls: usize,
    pub(super) mesh_commands: usize,
    pub(super) mesh_draw_calls: usize,
    pub(super) sprite_commands: usize,
    pub(super) sprite_draw_calls: usize,
}

#[cfg(test)]
pub(super) fn pipeline_state_set_count(sequence: &[DrawPipeline]) -> usize {
    let mut current = None;
    sequence
        .iter()
        .copied()
        .filter(|pipeline| select_state(&mut current, *pipeline))
        .count()
}

#[cfg(test)]
pub(super) fn scissor_state_set_count(sequence: &[(u32, u32, u32, u32)]) -> usize {
    let mut current = None;
    sequence
        .iter()
        .copied()
        .filter(|scissor| select_state(&mut current, *scissor))
        .count()
}

#[cfg(test)]
pub(super) fn scene_vertex_buffer_bind_count(regular_commands: &[bool]) -> usize {
    let mut in_regular_pass = false;
    regular_commands
        .iter()
        .copied()
        .filter(|regular| {
            if *regular {
                let starts_pass = !in_regular_pass;
                in_regular_pass = true;
                starts_pass
            } else {
                in_regular_pass = false;
                false
            }
        })
        .count()
}

#[cfg(test)]
pub(super) fn sprite_bind_group_state_set_count(sequence: &[Option<u64>]) -> usize {
    let mut active = None;
    sequence
        .iter()
        .copied()
        .filter(|binding| match binding {
            Some(binding) => select_state(&mut active, *binding),
            None => {
                active = None;
                false
            }
        })
        .count()
}

#[cfg(test)]
pub(super) fn typed_vertex_draw_range<T>(
    vertex_offset: u64,
    vertex_count: u32,
) -> std::ops::Range<u32> {
    vertex_draw_range::<T>(vertex_offset, vertex_count)
}

impl Renderer {
    fn apply_cached_scissor<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        clip_rect: Option<crate::ui::widget::Rect>,
        active_scissor: &mut Option<(u32, u32, u32, u32)>,
    ) -> bool {
        let Some(scissor) = self.scissor_rect(clip_rect) else {
            return false;
        };
        if select_state(active_scissor, scissor) {
            pass.set_scissor_rect(scissor.0, scissor.1, scissor.2, scissor.3);
        }
        true
    }

    fn set_scroll_translate(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        translate: Option<super::PushTranslate>,
    ) {
        if self.push_constants_supported {
            let translate = translate.unwrap_or_default();
            pass.set_immediates(0, bytemuck::bytes_of(&translate));
        }
    }

    pub(super) fn has_msaa(&self) -> bool {
        self.msaa_sample_count > 1
    }

    pub(super) fn offscreen_attachment_view<'a>(
        &self,
        target: &'a OffscreenTarget,
    ) -> &'a wgpu::TextureView {
        target.msaa_view.as_ref().unwrap_or(&target.single_view)
    }

    pub(super) fn offscreen_single_view<'a>(
        &self,
        target: &'a OffscreenTarget,
    ) -> &'a wgpu::TextureView {
        &target.single_view
    }

    pub(super) fn offscreen_resolve_target_for_draw<'a>(
        &self,
        target: &'a OffscreenTarget,
    ) -> Option<&'a wgpu::TextureView> {
        if self.has_msaa() {
            target.msaa_view.as_ref().map(|_| &target.single_view)
        } else {
            None
        }
    }

    pub(super) fn offscreen_sampled_view<'a>(
        &self,
        target: &'a OffscreenTarget,
    ) -> &'a wgpu::TextureView {
        let _ = self;
        self.offscreen_single_view(target)
    }

    fn clear_offscreen_target(&self, encoder: &mut wgpu::CommandEncoder, target: &OffscreenTarget) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tgui-offscreen-clear-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: self.offscreen_attachment_view(target),
                resolve_target: self.offscreen_resolve_target_for_draw(target),
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let _ = &mut pass;
    }

    fn copy_target_to_snapshot(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        source: &OffscreenTarget,
    ) -> Result<wgpu::TextureView, TguiError> {
        let snapshot = self.ensure_snapshot_target()?;
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &source.single_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &snapshot.single_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: source.width,
                height: source.height,
                depth_or_array_layers: 1,
            },
        );
        Ok(snapshot.single_view.clone())
    }

    pub(super) fn execute_prepared_commands(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        commands: &[PreparedCommand],
        font_manager: &FontManager,
        cleared_draw_target: &mut bool,
    ) -> Result<(), TguiError> {
        let scene_target = self
            .scene_target
            .as_ref()
            .cloned()
            .ok_or_else(|| TguiError::TextRender("scene target unavailable".into()))?;
        self.execute_prepared_commands_to_target(
            encoder,
            commands,
            font_manager,
            &scene_target,
            cleared_draw_target,
            0,
        )
    }

    pub(super) fn execute_prepared_commands_to_target(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        commands: &[PreparedCommand],
        font_manager: &FontManager,
        target: &OffscreenTarget,
        cleared_draw_target: &mut bool,
        composite_depth: usize,
    ) -> Result<(), TguiError> {
        #[cfg(any(test, feature = "bench-support"))]
        let mut rect_commands = 0usize;
        #[cfg(any(test, feature = "bench-support"))]
        let mut rect_draw_calls = 0usize;
        #[cfg(any(test, feature = "bench-support"))]
        let mut brush_commands = 0usize;
        #[cfg(any(test, feature = "bench-support"))]
        let mut brush_draw_calls = 0usize;
        #[cfg(any(test, feature = "bench-support"))]
        let mut mesh_commands = 0usize;
        #[cfg(any(test, feature = "bench-support"))]
        let mut mesh_draw_calls = 0usize;
        #[cfg(any(test, feature = "bench-support"))]
        let mut sprite_commands = 0usize;
        #[cfg(any(test, feature = "bench-support"))]
        let mut sprite_draw_calls = 0usize;
        let mut index = 0;
        while index < commands.len() {
            let _draw_id = commands[index].draw_id();
            match &commands[index] {
                PreparedCommand::BackdropBlur(blur) => {
                    self.apply_backdrop_blur_to_target(encoder, blur, target, cleared_draw_target)?;
                    index += 1;
                    continue;
                }
                PreparedCommand::CanvasComposite(composite) => {
                    self.apply_canvas_composite_to_target(
                        encoder,
                        composite,
                        font_manager,
                        target,
                        cleared_draw_target,
                        composite_depth,
                    )?;
                    index += 1;
                    continue;
                }
                _ => {}
            }

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tgui-scene-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.offscreen_attachment_view(target),
                    resolve_target: self.offscreen_resolve_target_for_draw(target),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: if *cleared_draw_target {
                            wgpu::LoadOp::Load
                        } else {
                            *cleared_draw_target = true;
                            wgpu::LoadOp::Clear(surface_clear_color(self.clear_color))
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // 所有普通 draw 都来自当前轮转顶点池。prepare 已把每段起点对齐
            // 到对应 pipeline 的 vertex stride，因此 pass 只需绑定一次完整 buffer，
            // 各 draw 用 first_vertex 选择自己的字节子区间。effect/composite 会结束当前
            // pass，它们的独立 pass 仍各自绑定所需 buffer。
            pass.set_vertex_buffer(0, self.vertex_pool.current_buffer().slice(..));
            let mut active_pipeline = None;
            let mut active_scissor = None;
            let mut active_sprite_bind_group: Option<SpriteBindGroupId> = None;
            while index < commands.len() {
                let _draw_id = commands[index].draw_id();
                match &commands[index] {
                    PreparedCommand::BackdropBlur(_) | PreparedCommand::CanvasComposite(_) => break,
                    PreparedCommand::Rect(batch) => {
                        if self.apply_cached_scissor(
                            &mut pass,
                            batch.clip_rect,
                            &mut active_scissor,
                        ) {
                            active_sprite_bind_group = None;
                            if select_state(&mut active_pipeline, DrawPipeline::Rect) {
                                pass.set_pipeline(&self.rect_pipeline);
                            }
                            self.set_scroll_translate(&mut pass, batch.scroll_translate);
                            let span = unbound_draw_span::<RectVertex>(
                                commands,
                                index,
                                self.primitive_draw_batching_enabled(),
                                rect_draw_meta,
                            );
                            pass.draw(
                                vertex_draw_range::<RectVertex>(
                                    span.vertex_offset,
                                    span.vertex_count,
                                ),
                                0..1,
                            );
                            #[cfg(any(test, feature = "bench-support"))]
                            {
                                rect_commands += span.command_count;
                                rect_draw_calls += 1;
                            }
                            index = span.next_command_index;
                            continue;
                        }
                    }
                    PreparedCommand::Brush(batch) => {
                        if self.apply_cached_scissor(
                            &mut pass,
                            batch.clip_rect,
                            &mut active_scissor,
                        ) {
                            active_sprite_bind_group = None;
                            if select_state(&mut active_pipeline, DrawPipeline::Brush) {
                                pass.set_pipeline(&self.brush_pipeline);
                            }
                            self.set_scroll_translate(&mut pass, batch.scroll_translate);
                            let span = unbound_draw_span::<BrushVertex>(
                                commands,
                                index,
                                self.primitive_draw_batching_enabled(),
                                brush_draw_meta,
                            );
                            pass.draw(
                                vertex_draw_range::<BrushVertex>(
                                    span.vertex_offset,
                                    span.vertex_count,
                                ),
                                0..1,
                            );
                            #[cfg(any(test, feature = "bench-support"))]
                            {
                                brush_commands += span.command_count;
                                brush_draw_calls += 1;
                            }
                            index = span.next_command_index;
                            continue;
                        }
                    }
                    PreparedCommand::Mesh(batch) => {
                        if self.apply_cached_scissor(
                            &mut pass,
                            batch.clip_rect,
                            &mut active_scissor,
                        ) {
                            active_sprite_bind_group = None;
                            if select_state(&mut active_pipeline, DrawPipeline::Mesh) {
                                pass.set_pipeline(&self.mesh_pipeline);
                            }
                            self.set_scroll_translate(&mut pass, batch.scroll_translate);
                            pass.set_bind_group(0, &batch.clip_binding.bind_group, &[]);
                            let span = mesh_draw_span(
                                commands,
                                index,
                                self.primitive_draw_batching_enabled(),
                            );
                            pass.draw(
                                vertex_draw_range::<MeshVertex>(
                                    span.vertex_offset,
                                    span.vertex_count,
                                ),
                                0..1,
                            );
                            #[cfg(any(test, feature = "bench-support"))]
                            {
                                mesh_commands += span.command_count;
                                mesh_draw_calls += 1;
                            }
                            index = span.next_command_index;
                            continue;
                        }
                    }
                    PreparedCommand::Sprite(batch) => {
                        if self.apply_cached_scissor(
                            &mut pass,
                            batch.clip_rect,
                            &mut active_scissor,
                        ) {
                            if select_state(&mut active_pipeline, DrawPipeline::Sprite) {
                                pass.set_pipeline(&self.scene_text_pipeline);
                            }
                            self.set_scroll_translate(&mut pass, batch.scroll_translate);
                            if select_state(&mut active_sprite_bind_group, batch.binding.id) {
                                pass.set_bind_group(0, &batch.binding.bind_group, &[]);
                            }
                            let span = sprite_draw_span(
                                commands,
                                index,
                                self.sprite_draw_batching_enabled(),
                            );
                            pass.draw(
                                vertex_draw_range::<TextVertex>(
                                    span.vertex_offset,
                                    span.vertex_count,
                                ),
                                0..1,
                            );
                            #[cfg(any(test, feature = "bench-support"))]
                            {
                                sprite_commands += span.command_count;
                                sprite_draw_calls += 1;
                            }
                            index = span.next_command_index;
                            continue;
                        }
                    }
                }
                index += 1;
            }
        }

        #[cfg(any(test, feature = "bench-support"))]
        {
            self.last_scene_draw_stats.rect_commands += rect_commands;
            self.last_scene_draw_stats.rect_draw_calls += rect_draw_calls;
            self.last_scene_draw_stats.brush_commands += brush_commands;
            self.last_scene_draw_stats.brush_draw_calls += brush_draw_calls;
            self.last_scene_draw_stats.mesh_commands += mesh_commands;
            self.last_scene_draw_stats.mesh_draw_calls += mesh_draw_calls;
            self.last_scene_draw_stats.sprite_commands += sprite_commands;
            self.last_scene_draw_stats.sprite_draw_calls += sprite_draw_calls;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::renderer::prepare::{DrawId, DrawStream, PreparedBrush, PreparedRect};
    use crate::ui::widget::Rect;

    fn translate(x: f32, y: f32) -> PushTranslate {
        PushTranslate {
            offset_ndc: [x, y],
            offset_physical: [x * 100.0, y * 100.0],
        }
    }

    fn rect_command(
        command_index: usize,
        vertex_offset: u64,
        clip_rect: Option<Rect>,
        scroll_translate: Option<PushTranslate>,
    ) -> PreparedCommand {
        PreparedCommand::Rect(PreparedRect {
            draw_id: DrawId {
                stream: DrawStream::Main,
                command_index,
            },
            clip_rect,
            vertex_offset,
            vertex_count: 6,
            scroll_translate,
        })
    }

    fn brush_command(
        command_index: usize,
        vertex_offset: u64,
        clip_rect: Option<Rect>,
        scroll_translate: Option<PushTranslate>,
    ) -> PreparedCommand {
        PreparedCommand::Brush(PreparedBrush {
            draw_id: DrawId {
                stream: DrawStream::Main,
                command_index,
            },
            clip_rect,
            vertex_offset,
            vertex_count: 6,
            scroll_translate,
        })
    }

    #[test]
    fn adjacent_atlas_quads_with_identical_dynamic_state_are_batchable() {
        assert!(sprite_draws_compatible(
            SpriteBindGroupId(7),
            Some(Rect::new(0.0, 0.0, 320.0, 200.0)),
            Some(translate(0.1, -0.2)),
            288,
            SpriteBindGroupId(7),
            Some(Rect::new(0.0, 0.0, 320.0, 200.0)),
            Some(translate(0.1, -0.2)),
            288,
        ));
    }

    #[test]
    fn sprite_batching_falls_back_for_every_dynamic_state_boundary() {
        let clip = Some(Rect::new(0.0, 0.0, 320.0, 200.0));
        let moved_clip = Some(Rect::new(1.0, 0.0, 320.0, 200.0));
        let movement = Some(translate(0.1, -0.2));

        assert!(!sprite_draws_compatible(
            SpriteBindGroupId(7),
            clip,
            movement,
            288,
            SpriteBindGroupId(8),
            clip,
            movement,
            288,
        ));
        assert!(!sprite_draws_compatible(
            SpriteBindGroupId(7),
            clip,
            movement,
            288,
            SpriteBindGroupId(7),
            moved_clip,
            movement,
            288,
        ));
        assert!(!sprite_draws_compatible(
            SpriteBindGroupId(7),
            clip,
            movement,
            288,
            SpriteBindGroupId(7),
            clip,
            Some(translate(0.2, -0.2)),
            288,
        ));
        assert!(!sprite_draws_compatible(
            SpriteBindGroupId(7),
            clip,
            movement,
            288,
            SpriteBindGroupId(7),
            clip,
            movement,
            292,
        ));
        // Even a zero-valued immediate is kept separate from `None`; this is conservative and
        // guarantees batching never changes whether the per-draw state was explicitly supplied.
        assert!(!push_translates_compatible(
            None,
            Some(PushTranslate::default())
        ));
    }

    #[test]
    fn rect_and_brush_spans_merge_only_consecutive_compatible_ranges() {
        let clip = Some(Rect::new(0.0, 0.0, 320.0, 200.0));
        let movement = Some(translate(0.1, -0.2));
        let rect_stride = std::mem::size_of::<RectVertex>() as u64;
        let brush_stride = std::mem::size_of::<BrushVertex>() as u64;

        let rects = [
            rect_command(0, 0, clip, movement),
            rect_command(1, rect_stride * 6, clip, movement),
            // A different pipeline is a hard boundary even when all other state matches.
            brush_command(2, rect_stride * 12, clip, movement),
        ];
        let rect_span = unbound_draw_span::<RectVertex>(&rects, 0, true, rect_draw_meta);
        assert_eq!(rect_span.command_count, 2);
        assert_eq!(rect_span.vertex_count, 12);
        assert_eq!(rect_span.next_command_index, 2);

        let brushes = [
            brush_command(0, 0, clip, movement),
            brush_command(1, brush_stride * 6, clip, movement),
        ];
        let brush_span = unbound_draw_span::<BrushVertex>(&brushes, 0, true, brush_draw_meta);
        assert_eq!(brush_span.command_count, 2);
        assert_eq!(brush_span.vertex_count, 12);
    }

    #[test]
    fn unbound_batching_falls_back_for_clip_immediate_gap_and_control_switch() {
        let clip = Some(Rect::new(0.0, 0.0, 320.0, 200.0));
        let moved_clip = Some(Rect::new(1.0, 0.0, 320.0, 200.0));
        let movement = Some(translate(0.1, -0.2));
        let stride = std::mem::size_of::<RectVertex>() as u64;
        let first = || rect_command(0, 0, clip, movement);

        for incompatible in [
            rect_command(1, stride * 6, moved_clip, movement),
            rect_command(1, stride * 6, clip, Some(translate(0.2, -0.2))),
            rect_command(1, stride * 6 + 4, clip, movement),
        ] {
            let commands = [first(), incompatible];
            assert_eq!(
                unbound_draw_span::<RectVertex>(&commands, 0, true, rect_draw_meta).command_count,
                1
            );
        }

        let compatible = [first(), rect_command(1, stride * 6, clip, movement)];
        assert_eq!(
            unbound_draw_span::<RectVertex>(&compatible, 0, false, rect_draw_meta).command_count,
            1
        );
    }

    #[test]
    fn mesh_batching_requires_shared_clip_binding_and_dynamic_state() {
        let clip = Some(Rect::new(0.0, 0.0, 320.0, 200.0));
        let movement = Some(translate(0.1, -0.2));
        let binding = super::super::MeshClipBindGroupId(7);
        assert!(mesh_draws_compatible(
            binding, clip, movement, 288, binding, clip, movement, 288,
        ));
        assert!(!mesh_draws_compatible(
            binding,
            clip,
            movement,
            288,
            super::super::MeshClipBindGroupId(8),
            clip,
            movement,
            288,
        ));
        assert!(!mesh_draws_compatible(
            binding,
            clip,
            movement,
            288,
            binding,
            Some(Rect::new(1.0, 0.0, 320.0, 200.0)),
            movement,
            288,
        ));
        assert!(!mesh_draws_compatible(
            binding,
            clip,
            movement,
            288,
            binding,
            clip,
            Some(translate(0.2, -0.2)),
            288,
        ));
    }
}
