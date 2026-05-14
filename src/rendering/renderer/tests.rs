use super::surface::*;
use super::*;
use crate::application::MsaaMode;
use crate::foundation::color::Color as TguiColorAlias;
use crate::text::font::FontWeight;
use crate::ui::widget::Rect;
use crate::ui::widget::{
    CanvasTextHorizontalAlign, CanvasTextOverflow, CanvasTextVerticalAlign, CanvasTextWrap,
    TextPrimitive,
};

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
fn msaa_mode_default_is_auto() {
    assert_eq!(MsaaMode::default(), MsaaMode::Auto);
}

#[test]
fn text_cache_key_tracks_overflow_mode() {
    let clip = TextCacheKey {
        content: "hello".to_string(),
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
        content: "very long text".to_string(),
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
