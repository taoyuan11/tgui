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
use crate::ui::widget::{Rect, ScenePrimitives, TexturePrimitive};
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
        frame: Rect::new(0.0, 0.0, 8.0, 8.0),
        quad: None,
        uv_rect: None,
        corner_radius: 0.0,
        opacity: 1.0,
        clip_rect: None,
        clip_mask: None,
    }
}
