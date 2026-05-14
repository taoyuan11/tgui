use std::sync::Arc;

#[cfg(all(target_env = "ohos", feature = "ohos"))]
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::application::MsaaMode;
use crate::foundation::color::Color as TguiColor;
use crate::foundation::error::TguiError;
use crate::platform::backend::window::Window;

use super::OffscreenTarget;

pub(super) fn create_instance(clear_color: TguiColor) -> wgpu::Instance {
    let descriptor = instance_descriptor(clear_color);
    wgpu::Instance::new(descriptor)
}

pub(super) async fn request_adapter(
    instance: &wgpu::Instance,
    surface: &wgpu::Surface<'_>,
    clear_color: TguiColor,
) -> Result<wgpu::Adapter, TguiError> {
    #[cfg(target_os = "windows")]
    {
        if clear_color.a < 255 {
            if let Some(adapter) = instance
                .enumerate_adapters(wgpu::Backends::DX12)
                .await
                .into_iter()
                .find(|adapter| adapter.is_surface_supported(surface))
            {
                return Ok(adapter);
            }
        }
    }

    Ok(instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: adapter_power_preference(),
            compatible_surface: Some(surface),
            force_fallback_adapter: false,
        })
        .await?)
}

pub(super) fn create_surface(
    instance: &wgpu::Instance,
    window: Arc<dyn Window>,
) -> Result<wgpu::Surface<'static>, TguiError> {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        let raw_display_handle = window.display_handle().map_err(|error| {
            TguiError::TextRender(format!("display handle unavailable: {error}"))
        })?;
        let raw_window_handle = window.window_handle().map_err(|error| {
            TguiError::TextRender(format!("window handle unavailable: {error}"))
        })?;

        return Ok(unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display_handle.as_raw()),
                raw_window_handle: raw_window_handle.as_raw(),
            })?
        });
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        instance.create_surface(window).map_err(Into::into)
    }
}

pub(super) fn required_device_limits(adapter: &wgpu::Adapter) -> wgpu::Limits {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        return adapter.limits();
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        let _ = adapter;
        wgpu::Limits::default()
    }
}

pub(super) fn resolve_surface_msaa_sample_count(
    adapter: &wgpu::Adapter,
    format: wgpu::TextureFormat,
    requested_mode: MsaaMode,
) -> u32 {
    let features = adapter.get_texture_format_features(format);
    supported_msaa_sample_count(features.flags, requested_mode)
}

pub(super) fn create_offscreen_target(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    label: &str,
    sample_count: u32,
) -> Option<OffscreenTarget> {
    if config.width == 0 || config.height == 0 {
        return None;
    }

    let single_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(&format!("{label}-single")),
        size: wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let (msaa_texture, msaa_view) = if sample_count > 1 {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{label}-msaa")),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (Some(texture), Some(view))
    } else {
        (None, None)
    };

    Some(OffscreenTarget {
        single_view: single_texture.create_view(&wgpu::TextureViewDescriptor::default()),
        single_texture,
        _msaa_texture: msaa_texture,
        msaa_view,
    })
}

pub(super) fn pipeline_multisample_state(sample_count: u32) -> wgpu::MultisampleState {
    wgpu::MultisampleState {
        count: sample_count.max(1),
        ..Default::default()
    }
}

pub(super) fn instance_backends(clear_color: TguiColor) -> wgpu::Backends {
    #[cfg(target_os = "windows")]
    {
        if clear_color.a < 255 {
            return wgpu::Backends::DX12;
        }
    }

    default_backends()
}

pub(super) fn default_backends() -> wgpu::Backends {
    #[cfg(target_arch = "wasm32")]
    {
        return wgpu::Backends::BROWSER_WEBGPU;
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        return wgpu::Backends::METAL;
    }

    #[cfg(all(
        target_os = "android",
        feature = "android",
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    {
        return wgpu::Backends::GL;
    }

    #[cfg(any(
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos")),
        all(
            target_os = "android",
            feature = "android",
            not(any(target_arch = "x86", target_arch = "x86_64"))
        )
    ))]
    {
        #[cfg(target_os = "windows")]
        {
            return wgpu::Backends::DX12 | wgpu::Backends::VULKAN;
        }

        #[cfg(not(target_os = "windows"))]
        return wgpu::Backends::VULKAN;
    }

    #[allow(unreachable_code)]
    wgpu::Backends::all()
}

pub(super) fn surface_present_mode(modes: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        return modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::Fifo)
            .or_else(|| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::AutoVsync)
            })
            .or_else(|| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::AutoNoVsync)
            })
            .unwrap_or(wgpu::PresentMode::Fifo);
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::AutoNoVsync)
            .or_else(|| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::AutoVsync)
            })
            .or_else(|| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == wgpu::PresentMode::Fifo)
            })
            .unwrap_or(wgpu::PresentMode::Fifo)
    }
}

pub(super) fn surface_alpha_mode(
    modes: &[wgpu::CompositeAlphaMode],
    clear_color: TguiColor,
) -> wgpu::CompositeAlphaMode {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        return modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        if clear_color.a < 255 {
            return transparent_surface_alpha_mode(modes);
        }

        modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
            .unwrap_or(wgpu::CompositeAlphaMode::Auto)
    }
}

pub(super) fn surface_clear_color(color: TguiColor) -> wgpu::Color {
    let alpha = color.a as f64 / 255.0;
    wgpu::Color {
        r: (color.r as f64 / 255.0) * alpha,
        g: (color.g as f64 / 255.0) * alpha,
        b: (color.b as f64 / 255.0) * alpha,
        a: alpha,
    }
}

fn instance_descriptor(clear_color: TguiColor) -> wgpu::InstanceDescriptor {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = instance_backends(clear_color);
    #[cfg(target_os = "windows")]
    {
        if clear_color.a < 255 {
            descriptor.backend_options.dx12.presentation_system =
                wgpu::Dx12SwapchainKind::DxgiFromVisual;
        }
    }
    #[cfg(all(target_os = "android", feature = "android"))]
    {
        descriptor.flags = wgpu::InstanceFlags::empty();
        descriptor.backend_options.gl.debug_fns = wgpu::GlDebugFns::Disabled;
    }
    descriptor
}

fn adapter_power_preference() -> wgpu::PowerPreference {
    #[cfg(all(target_env = "ohos", feature = "ohos"))]
    {
        return wgpu::PowerPreference::HighPerformance;
    }

    #[cfg(not(all(target_env = "ohos", feature = "ohos")))]
    {
        wgpu::PowerPreference::default()
    }
}

fn supported_msaa_sample_count(flags: wgpu::TextureFormatFeatureFlags, requested_mode: MsaaMode) -> u32 {
    let candidates: &[u32] = match requested_mode {
        MsaaMode::Off => &[1],
        MsaaMode::Auto => &[4, 2, 1],
        MsaaMode::X2 => &[2, 1],
        MsaaMode::X4 => &[4, 2, 1],
        MsaaMode::X8 => &[8, 4, 2, 1],
    };

    candidates
        .into_iter()
        .copied()
        .find(|count| {
            if *count == 1 {
                return true;
            }
            flags.sample_count_supported(*count)
                && flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE)
        })
        .unwrap_or(1)
}

fn transparent_surface_alpha_mode(modes: &[wgpu::CompositeAlphaMode]) -> wgpu::CompositeAlphaMode {
    #[cfg(target_os = "macos")]
    const PREFERRED: &[wgpu::CompositeAlphaMode] = &[
        wgpu::CompositeAlphaMode::PostMultiplied,
        wgpu::CompositeAlphaMode::PreMultiplied,
        wgpu::CompositeAlphaMode::Inherit,
    ];

    #[cfg(not(target_os = "macos"))]
    const PREFERRED: &[wgpu::CompositeAlphaMode] = &[
        wgpu::CompositeAlphaMode::PreMultiplied,
        wgpu::CompositeAlphaMode::PostMultiplied,
        wgpu::CompositeAlphaMode::Inherit,
    ];

    PREFERRED
        .iter()
        .copied()
        .find(|mode| modes.contains(mode))
        .or_else(|| {
            modes
                .iter()
                .copied()
                .find(|mode| *mode != wgpu::CompositeAlphaMode::Opaque)
        })
        .unwrap_or(wgpu::CompositeAlphaMode::Auto)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn multisample_flags_for(counts: &[u32]) -> wgpu::TextureFormatFeatureFlags {
        let mut flags = wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE;
        for count in counts {
            flags |= match count {
                2 => wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X2,
                4 => wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4,
                8 => wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X8,
                16 => wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X16,
                _ => wgpu::TextureFormatFeatureFlags::empty(),
            };
        }
        flags
    }

    #[test]
    fn auto_msaa_prefers_four_samples() {
        let flags = multisample_flags_for(&[2, 4]);

        assert_eq!(supported_msaa_sample_count(flags, MsaaMode::Auto), 4);
    }

    #[test]
    fn auto_msaa_falls_back_to_two_samples() {
        let flags = multisample_flags_for(&[2]);

        assert_eq!(supported_msaa_sample_count(flags, MsaaMode::Auto), 2);
    }

    #[test]
    fn explicit_msaa_modes_downgrade_in_order() {
        let flags = multisample_flags_for(&[2, 4]);

        assert_eq!(supported_msaa_sample_count(flags, MsaaMode::X8), 4);
        assert_eq!(supported_msaa_sample_count(flags, MsaaMode::X4), 4);
        assert_eq!(supported_msaa_sample_count(flags, MsaaMode::X2), 2);
        assert_eq!(supported_msaa_sample_count(flags, MsaaMode::Off), 1);
    }
}
