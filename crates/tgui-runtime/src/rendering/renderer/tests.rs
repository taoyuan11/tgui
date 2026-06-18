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
fn text_cache_key_tracks_overflow_mode() {
    let clip = TextCacheKey {
        content: Arc::from("hello"),
        font_family: None,
        width: 10,
        height: 10,
        color: [255, 255, 255, 255],
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
    scene.textures.push(texture_primitive(main_texture.clone()));
    scene
        .overlay_textures
        .push(texture_primitive(overlay_texture.clone()));

    let keys = active_texture_keys(&scene);

    assert!(keys.contains(&main_texture.id()));
    assert!(keys.contains(&overlay_texture.id()));
}

#[test]
fn text_system_releases_swash_frame_cache() {
    let mut text_system = TextSystem::new();
    text_system.swash_cache.image_cache.insert(
        CacheKey {
            font_id: fontdb::ID::dummy(),
            glyph_id: 1,
            font_size_bits: 16.0_f32.to_bits(),
            x_bin: SubpixelBin::Zero,
            y_bin: SubpixelBin::Zero,
            font_weight: fontdb::Weight::NORMAL,
            flags: CacheKeyFlags::empty(),
        },
        None,
    );

    text_system.release_frame_raster_cache();

    assert!(text_system.swash_cache.image_cache.is_empty());
    assert!(text_system.swash_cache.outline_command_cache.is_empty());
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
        clip_rect: None,
        clip_mask: None,
    }
}
