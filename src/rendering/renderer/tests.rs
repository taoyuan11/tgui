use super::surface::*;
use super::*;
use crate::application::MsaaMode;

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
