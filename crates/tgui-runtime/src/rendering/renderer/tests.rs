use super::surface::*;
use super::*;
use crate::application::MsaaMode;
use crate::foundation::color::Color as TguiColorAlias;
use crate::media::TextureFrame;
use crate::text::font::FontWeight;
use crate::ui::widget::{
    CanvasTextHorizontalAlign, CanvasTextOverflow, CanvasTextVerticalAlign, CanvasTextWrap,
    TextPrimitive,
};
use crate::ui::widget::{
    DirtyDrawRange, Rect, RenderPrimitive, SceneCounts, SceneDrawStream, ScenePrimitives,
    ShapePrimitiveSlot, TexturePrimitive,
};
use cosmic_text::{fontdb, CacheKey, CacheKeyFlags, SubpixelBin};

#[cfg(target_os = "windows")]
#[test]
fn windows_default_backends_include_dx12_surface_support() {
    let backends = default_backends();

    assert!(backends.contains(wgpu::Backends::DX12));
    assert!(backends.contains(wgpu::Backends::VULKAN));
}

#[cfg(target_os = "windows")]
#[test]
fn transparent_windows_surfaces_still_use_dx12_visual_swapchain() {
    assert_eq!(
        instance_backends(TguiColor::TRANSPARENT),
        wgpu::Backends::DX12
    );
}

#[test]
fn pipeline_multisample_state_uses_requested_count() {
    assert_eq!(pipeline_multisample_state(1).count, 1);
    assert_eq!(pipeline_multisample_state(4).count, 4);
}

#[test]
fn msaa_mode_default_is_off() {
    assert_eq!(MsaaMode::default(), MsaaMode::Off);
}

#[test]
fn draw_id_distinguishes_stream_and_command_index() {
    use super::prepare::{DrawId, DrawStream};

    let main_zero = DrawId {
        stream: DrawStream::Main,
        command_index: 0,
    };
    let main_zero_again = DrawId {
        stream: DrawStream::Main,
        command_index: 0,
    };
    let main_one = DrawId {
        stream: DrawStream::Main,
        command_index: 1,
    };
    let overlay_zero = DrawId {
        stream: DrawStream::Overlay,
        command_index: 0,
    };
    let composite_zero = DrawId {
        stream: DrawStream::CompositeContent { depth: 0 },
        command_index: 0,
    };

    assert_eq!(main_zero, main_zero_again);
    assert_ne!(main_zero, main_one);
    assert_ne!(main_zero, overlay_zero);
    assert_ne!(overlay_zero, composite_zero);
}

#[test]
fn scene_pass_collapses_redundant_pipeline_state_sets() {
    use super::draw::{
        pipeline_state_set_count, scene_vertex_buffer_bind_count, scissor_state_set_count,
        sprite_bind_group_state_set_count, typed_vertex_draw_range, DrawPipeline,
    };
    use super::prepare::PreparedSpritePipeline;

    let repeated_rects = vec![DrawPipeline::Rect; 1024];
    assert_eq!(pipeline_state_set_count(&repeated_rects), 1);

    let mixed = [
        DrawPipeline::Rect,
        DrawPipeline::Rect,
        DrawPipeline::Sprite(PreparedSpritePipeline::Rgba),
        DrawPipeline::Sprite(PreparedSpritePipeline::Rgba),
        DrawPipeline::Mesh,
        DrawPipeline::Rect,
    ];
    assert_eq!(pipeline_state_set_count(&mixed), 4);

    #[cfg(feature = "video")]
    assert_eq!(
        pipeline_state_set_count(&[
            DrawPipeline::Sprite(PreparedSpritePipeline::Rgba),
            DrawPipeline::Sprite(PreparedSpritePipeline::VideoYuv),
            DrawPipeline::Sprite(PreparedSpritePipeline::Rgba),
        ]),
        3
    );

    let viewport = (0, 0, 1920, 1080);
    let clipped = (40, 40, 320, 240);
    let repeated_scissors = vec![viewport; 1024];
    assert_eq!(scissor_state_set_count(&repeated_scissors), 1);
    assert_eq!(
        scissor_state_set_count(&[viewport, viewport, clipped, clipped, viewport]),
        3
    );

    let repeated_draws = vec![true; 1024];
    assert_eq!(scene_vertex_buffer_bind_count(&repeated_draws), 1);
    // Backdrop blur / canvas composite 结束当前 scene pass；后续普通 draw
    // 开始新 pass 并独立绑定一次池 buffer。
    assert_eq!(
        scene_vertex_buffer_bind_count(&[true, true, false, true, false, false, true, true]),
        3
    );

    let repeated_sprite_binding = vec![Some(7); 1024];
    assert_eq!(
        sprite_bind_group_state_set_count(&repeated_sprite_binding),
        1
    );
    assert_eq!(
        sprite_bind_group_state_set_count(&[Some(7), Some(7), Some(8), Some(8), Some(7),]),
        3
    );
    // A non-sprite draw uses a different pipeline layout, so the conservative state model
    // rebinds the texture if the same sprite resource appears again afterwards.
    assert_eq!(
        sprite_bind_group_state_set_count(&[Some(7), None, Some(7)]),
        2
    );

    let rect_stride = std::mem::size_of::<RectVertex>() as u64;
    let brush_stride = std::mem::size_of::<BrushVertex>() as u64;
    let rect_bytes = rect_stride * 6;
    let brush_offset = rect_bytes.div_ceil(brush_stride) * brush_stride;
    assert_eq!(typed_vertex_draw_range::<RectVertex>(0, 6), 0..6);
    assert_eq!(
        typed_vertex_draw_range::<BrushVertex>(brush_offset, 6),
        (brush_offset / brush_stride) as u32..(brush_offset / brush_stride) as u32 + 6
    );
}

#[test]
fn initial_scene_clear_elides_the_dedicated_pass_for_regular_draws() {
    let first_command = first_initial_scene_command(
        Some(InitialSceneCommandKind::Regular),
        Some(InitialSceneCommandKind::SamplesExistingTarget),
    );
    assert_eq!(
        initial_scene_clear_strategy(first_command),
        InitialSceneClear::FirstRegularPass
    );
    assert_eq!(
        InitialSceneClear::FirstRegularPass.dedicated_pass_count(),
        0
    );
}

#[test]
fn initial_scene_clear_preserves_effect_and_empty_scene_ordering() {
    let overlay_regular = first_initial_scene_command(None, Some(InitialSceneCommandKind::Regular));
    assert_eq!(
        initial_scene_clear_strategy(overlay_regular),
        InitialSceneClear::FirstRegularPass
    );

    assert_eq!(
        initial_scene_clear_strategy(Some(InitialSceneCommandKind::SamplesExistingTarget)),
        InitialSceneClear::ExplicitBeforeTargetSampling
    );
    assert_eq!(
        initial_scene_clear_strategy(None),
        InitialSceneClear::ExplicitBeforePresent
    );

    // Blur/composite must observe the explicit clear before sampling the target; an empty scene
    // retains exactly one explicit clear before the present pass samples it.
    assert_eq!(
        InitialSceneClear::ExplicitBeforeTargetSampling.dedicated_pass_count(),
        1
    );
    assert_eq!(
        InitialSceneClear::ExplicitBeforePresent.dedicated_pass_count(),
        1
    );
}

#[test]
fn scene_slot_write_marks_only_the_touched_draw_dirty() {
    let mut scene = ScenePrimitives::default();
    scene.push_shape(RenderPrimitive {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        color: TguiColorAlias::RED,
        corner_radius: 0.0,
        stroke_width: 0.0,
        clip_rect: None,
        clip_mask: None,
    });
    scene.push_shape(RenderPrimitive {
        rect: Rect::new(10.0, 0.0, 10.0, 10.0),
        color: TguiColorAlias::BLUE,
        corner_radius: 0.0,
        stroke_width: 0.0,
        clip_rect: None,
        clip_mask: None,
    });

    assert!(scene.dirty_draw_ranges().is_empty());
    assert!(scene.write_shape_color_slot(
        &SceneCounts::default(),
        ShapePrimitiveSlot {
            shape_index: 1,
            command_index: 1,
        },
        TguiColorAlias::GREEN,
    ));
    assert_eq!(
        scene.dirty_draw_ranges(),
        &[DirtyDrawRange {
            stream: SceneDrawStream::Main,
            range: 1..2,
        }]
    );
    assert!(
        !scene.cache_liveness_dirty(),
        "shape color changes do not alter text or texture cache keys"
    );
}

#[test]
fn adjacent_scene_slot_writes_coalesce_dirty_draw_ranges() {
    let mut scene = ScenePrimitives::default();
    for index in 0..4 {
        scene.push_shape(RenderPrimitive {
            rect: Rect::new(index as f32 * 10.0, 0.0, 10.0, 10.0),
            color: TguiColorAlias::RED,
            corner_radius: 0.0,
            stroke_width: 0.0,
            clip_rect: None,
            clip_mask: None,
        });
    }

    for index in [1, 2, 3, 0] {
        assert!(scene.write_shape_color_slot(
            &SceneCounts::default(),
            ShapePrimitiveSlot {
                shape_index: index,
                command_index: index,
            },
            TguiColorAlias::GREEN,
        ));
    }

    assert_eq!(
        scene.dirty_draw_ranges(),
        &[DirtyDrawRange {
            stream: SceneDrawStream::Main,
            range: 0..4,
        }]
    );
    assert!(!scene.cache_liveness_dirty());
}

#[test]
fn scene_splice_marks_replaced_draw_range_dirty_without_bumping_serial() {
    let mut scene = ScenePrimitives::new_prepare_cache_root();
    scene.push_shape(RenderPrimitive {
        rect: Rect::new(0.0, 0.0, 10.0, 10.0),
        color: TguiColorAlias::RED,
        corner_radius: 0.0,
        stroke_width: 0.0,
        clip_rect: None,
        clip_mask: None,
    });
    scene.push_shape(RenderPrimitive {
        rect: Rect::new(10.0, 0.0, 10.0, 10.0),
        color: TguiColorAlias::BLUE,
        corner_radius: 0.0,
        stroke_width: 0.0,
        clip_rect: None,
        clip_mask: None,
    });
    let serial = scene.prepare_cache_serial();

    let mut replacement = ScenePrimitives::default();
    replacement.push_shape(RenderPrimitive {
        rect: Rect::new(10.0, 0.0, 10.0, 10.0),
        color: TguiColorAlias::GREEN,
        corner_radius: 0.0,
        stroke_width: 0.0,
        clip_rect: None,
        clip_mask: None,
    });
    assert!(replacement.dirty_draw_ranges().is_empty());

    let mut offset = SceneCounts::default();
    offset.shapes = 1;
    offset.commands = 1;
    assert!(scene.splice_in_place(&offset, &replacement));

    assert_eq!(scene.prepare_cache_serial(), serial);
    assert_eq!(
        scene.dirty_draw_ranges(),
        &[DirtyDrawRange {
            stream: SceneDrawStream::Main,
            range: 1..2,
        }]
    );
    assert!(
        scene.cache_liveness_dirty(),
        "spliced commands may replace active cache keys and must refresh liveness"
    );
}

#[test]
fn retained_prepare_stats_rebuild_only_dirty_scene_draws() {
    use super::prepare::{retained_prepare_stats, DrawStream, PrepareReuseStats};

    let dirty = [DirtyDrawRange {
        stream: SceneDrawStream::Main,
        range: 1..2,
    }];

    assert_eq!(
        retained_prepare_stats(DrawStream::Main, 3, &dirty),
        PrepareReuseStats {
            total: 3,
            rebuild: 1,
            reuse: 2,
        }
    );
    assert_eq!(
        retained_prepare_stats(DrawStream::Overlay, 2, &dirty),
        PrepareReuseStats {
            total: 2,
            rebuild: 0,
            reuse: 2,
        }
    );
}

#[test]
fn retained_prepare_cache_lookup_keeps_vertex_storage_borrowed() {
    let payload_bytes = 64 * 1024;
    let (same_storage, observed_bytes) =
        super::prepare::retained_cache_lookup_storage_probe(payload_bytes);

    assert!(same_storage);
    assert_eq!(observed_bytes, payload_bytes);
}

#[test]
fn stable_prepare_frames_reuse_large_command_output_storage() {
    let command_count = 10_000;
    let stable_frames = 120;
    let (storage_growths, retained_capacity, retained_bytes, same_storage) =
        super::prepare::prepared_command_scratch_storage_probe(command_count, stable_frames);

    assert_eq!(
        storage_growths, 1,
        "a stable retained scene should grow prepared command storage only on its first frame"
    );
    assert!(
        same_storage,
        "stable frames should keep the same allocation"
    );
    assert!(retained_capacity >= command_count);
    assert_eq!(
        retained_bytes,
        retained_capacity * std::mem::size_of::<super::prepare::PreparedCommand>()
    );
}

#[test]
fn text_cache_key_tracks_overflow_mode() {
    let clip = TextCacheKey {
        content: Arc::from("hello"),
        content_hash: super::text::text_content_hash("hello"),
        rich_spans: None,
        font_family: None,
        width: 10,
        height: 10,
        color: [255, 255, 255, 255],
        tintable_mask: false,
        force_color: false,
        font_size_bits: 1,
        line_height_bits: 2,
        letter_spacing_bits: 3,
        font_weight: 400,
        wrap_mode: 0,
        overflow_mode: 0,
        horizontal_align: 0,
        vertical_align: 0,
    };
    let ellipsis = TextCacheKey {
        overflow_mode: 1,
        ..clip.clone()
    };

    assert!(clip != ellipsis);
}

#[test]
fn text_primitive_can_represent_ellipsis_overflow() {
    let primitive = TextPrimitive {
        content: Arc::from("very long text"),
        rich_spans: None,
        frame: Rect::new(0.0, 0.0, 60.0, 20.0),
        quad: None,
        color: TguiColorAlias::WHITE,
        force_color: false,
        font_family: None,
        font_size: 14.0,
        font_weight: FontWeight::NORMAL,
        line_height: 16.0,
        letter_spacing: 0.0,
        wrap: CanvasTextWrap::None,
        overflow: CanvasTextOverflow::Ellipsis,
        horizontal_align: CanvasTextHorizontalAlign::Start,
        vertical_align: CanvasTextVerticalAlign::Start,
        clip_rect: None,
        clip_mask: None,
    };

    assert_eq!(primitive.overflow, CanvasTextOverflow::Ellipsis);
}

#[test]
fn active_texture_keys_include_overlay_textures() {
    let main_texture = std::sync::Arc::new(TextureFrame::new(2, 2, vec![255; 2 * 2 * 4]));
    let overlay_texture = std::sync::Arc::new(TextureFrame::new(3, 3, vec![128; 3 * 3 * 4]));
    let mut scene = ScenePrimitives::default();
    scene.push_texture(texture_primitive(main_texture.clone()));
    scene.push_overlay_texture(texture_primitive(overlay_texture.clone()));

    let mut keys = HashSet::new();
    collect_active_texture_keys(&scene, &mut keys);

    assert!(keys.contains(&main_texture.id()));
    assert!(keys.contains(&overlay_texture.id()));
}

#[test]
fn active_texture_key_scratch_reuses_capacity_between_frames() {
    let textures: Vec<_> = (0..32)
        .map(|value| std::sync::Arc::new(TextureFrame::new(2, 2, vec![value; 2 * 2 * 4])))
        .collect();
    let mut scene = ScenePrimitives::default();
    for texture in textures.iter().cloned() {
        scene.push_texture(texture_primitive(texture));
    }
    let mut keys = HashSet::new();

    collect_active_texture_keys(&scene, &mut keys);
    let first_capacity = keys.capacity();
    collect_active_texture_keys(&scene, &mut keys);

    assert_eq!(keys.len(), textures.len());
    assert_eq!(keys.capacity(), first_capacity);
}

#[test]
fn stable_retained_frames_scan_cache_liveness_once() {
    let scene_serial = 41;
    let mut last_scene_serial = None;
    let mut refreshes = 0;

    for _ in 0..10_000 {
        if cache_liveness_needs_refresh(last_scene_serial, scene_serial, false) {
            refreshes += 1;
            last_scene_serial = Some(scene_serial);
        }
    }

    assert_eq!(
        refreshes, 1,
        "stable frames should not repeat command walks"
    );
}

#[test]
fn dirty_draws_refresh_liveness_even_when_scene_serial_is_stable() {
    assert!(cache_liveness_needs_refresh(Some(17), 17, true));
    assert!(cache_liveness_needs_refresh(Some(17), 18, false));
    assert!(!cache_liveness_needs_refresh(Some(17), 17, false));
}

fn glyph_cache_key(index: usize) -> CacheKey {
    CacheKey {
        font_id: fontdb::ID::dummy(),
        glyph_id: index as u16,
        font_size_bits: (12.0 + (index / (u16::MAX as usize + 1)) as f32).to_bits(),
        x_bin: SubpixelBin::Zero,
        y_bin: SubpixelBin::Zero,
        font_weight: fontdb::Weight::NORMAL,
        flags: CacheKeyFlags::empty(),
    }
}

#[test]
fn text_system_retains_bounded_swash_cache_across_frames() {
    let mut text_system = TextSystem::new();
    assert!(!text_system.prepare_font_system(7));
    text_system
        .swash_cache
        .image_cache
        .insert(glyph_cache_key(1), None);
    text_system.finish_raster_batch(0);
    text_system.finish_frame();

    assert_eq!(text_system.swash_cache.image_cache.len(), 1);
    assert_eq!(text_system.stats().image_insertions, 1);
    assert_eq!(text_system.stats().frame_resets, 0);
    text_system.reset_stats();
    assert_eq!(text_system.stats().image_insertions, 0);
    assert_eq!(text_system.stats().frame_resets, 0);
}

#[test]
fn text_system_legacy_control_releases_swash_cache_each_frame() {
    let mut text_system = TextSystem::new();
    text_system.set_retention(false);
    text_system
        .swash_cache
        .image_cache
        .insert(glyph_cache_key(1), None);
    text_system.finish_raster_batch(0);
    text_system.finish_frame();

    assert!(text_system.swash_cache.image_cache.is_empty());
    assert!(text_system.swash_cache.outline_command_cache.is_empty());
    assert_eq!(text_system.stats().frame_resets, 1);
}

#[test]
fn text_system_switching_font_databases_invalidates_rasters() {
    let mut text_system = TextSystem::new();
    assert!(!text_system.prepare_font_system(11));
    text_system
        .swash_cache
        .image_cache
        .insert(glyph_cache_key(1), None);
    assert!(text_system.prepare_font_system(12));

    assert!(text_system.swash_cache.image_cache.is_empty());
    assert_eq!(text_system.stats().font_system_resets, 1);
}

#[test]
fn text_system_evicts_retained_cache_over_entry_budget() {
    let mut text_system = TextSystem::new();
    for index in 0..=RETAINED_GLYPH_IMAGE_ENTRY_LIMIT {
        text_system
            .swash_cache
            .image_cache
            .insert(glyph_cache_key(index), None);
    }
    text_system.finish_raster_batch(0);
    text_system.finish_frame();

    assert!(text_system.swash_cache.image_cache.is_empty());
    assert_eq!(text_system.stats().budget_evictions, 1);
}

fn texture_primitive(texture: std::sync::Arc<TextureFrame>) -> TexturePrimitive {
    TexturePrimitive {
        texture,
        media_key: None,
        media_layout: None,
        frame: Rect::new(0.0, 0.0, 8.0, 8.0),
        quad: None,
        uv_rect: None,
        corner_radius: 0.0,
        opacity: 1.0,
        mask_tint: None,
        clip_rect: None,
        clip_mask: None,
    }
}
