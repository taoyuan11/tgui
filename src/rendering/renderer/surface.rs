use std::sync::Arc;

#[cfg(all(target_env = "ohos", feature = "ohos"))]
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::foundation::color::Color as TguiColor;
use crate::foundation::error::TguiError;
use crate::platform::backend::window::Window;

use super::{MultisampleTarget, OffscreenTarget};

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

pub(super) fn surface_msaa_sample_count(
    adapter: &wgpu::Adapter,
    format: wgpu::TextureFormat,
) -> u32 {
    let features = adapter.get_texture_format_features(format);
    supported_msaa_sample_count(features.flags)
}

pub(super) fn create_multisample_target(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    sample_count: u32,
) -> Option<MultisampleTarget> {
    if sample_count <= 1 || config.width == 0 || config.height == 0 {
        return None;
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("tgui-msaa-color-target"),
        size: wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    Some(MultisampleTarget {
        _texture: texture,
        _view: view,
    })
}

pub(super) fn create_offscreen_target(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    label: &str,
) -> Option<OffscreenTarget> {
    if config.width == 0 || config.height == 0 {
        return None;
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
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

    Some(OffscreenTarget {
        view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
        texture,
    })
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

fn supported_msaa_sample_count(flags: wgpu::TextureFormatFeatureFlags) -> u32 {
    [4, 2]
        .into_iter()
        .find(|count| {
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
