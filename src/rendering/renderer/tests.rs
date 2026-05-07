use super::*;

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
