use std::io::Cursor;

#[cfg(target_os = "windows")]
use windows::core::Interface;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{RPC_E_CHANGED_MODE, S_FALSE};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppRGBA, IWICBitmapFrameDecode, IWICBitmapSource,
    IWICImagingFactory, IWICStream, WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant,
    WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};

use crate::foundation::error::TguiError;

use super::types::{clamp_raster_request, MediaBytes, RasterRequest, TextureFrame};

pub(super) fn load_raster_dimensions(bytes: &[u8]) -> Result<(u32, u32), TguiError> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| {
            TguiError::Media(format!("failed to detect raster image format: {error}"))
        })?
        .into_dimensions()
        .map_err(|error| {
            TguiError::Media(format!("failed to read raster image dimensions: {error}"))
        })
}

pub(super) fn decode_raster_texture(
    bytes: &MediaBytes,
    raster_request: RasterRequest,
) -> Result<TextureFrame, TguiError> {
    let raster_request = clamp_raster_request(raster_request.width(), raster_request.height());
    decode_raster_texture_platform(bytes.as_slice(), raster_request)
}

fn decode_raster_texture_platform(
    bytes: &[u8],
    raster_request: RasterRequest,
) -> Result<TextureFrame, TguiError> {
    #[cfg(target_os = "windows")]
    if let Ok(texture) = decode_raster_texture_with_wic(bytes, raster_request) {
        return Ok(texture);
    }

    decode_raster_texture_with_image_crate(bytes, raster_request)
}

fn decode_raster_texture_with_image_crate(
    bytes: &[u8],
    raster_request: RasterRequest,
) -> Result<TextureFrame, TguiError> {
    let image = image::load_from_memory(bytes)
        .map_err(|error| TguiError::Media(format!("failed to decode raster image: {error}")))?;

    let resized = if image.width() == raster_request.width()
        && image.height() == raster_request.height()
    {
        image
    } else if image.width() > raster_request.width() || image.height() > raster_request.height() {
        image.thumbnail_exact(raster_request.width(), raster_request.height())
    } else {
        image.resize_exact(
            raster_request.width(),
            raster_request.height(),
            image::imageops::FilterType::Triangle,
        )
    };

    let rgba = resized.to_rgba8();
    Ok(TextureFrame::new(
        rgba.width(),
        rgba.height(),
        rgba.into_raw(),
    ))
}

#[cfg(target_os = "windows")]
fn decode_raster_texture_with_wic(
    bytes: &[u8],
    raster_request: RasterRequest,
) -> Result<TextureFrame, TguiError> {
    // SAFETY 概览：本函数中所有 `unsafe` 块都是对 `windows-rs` 暴露的 WIC COM
    // 接口的直接转发。`ComScope::initialize` 已经在调用前完成了 COM 初始化，
    // 所有 WIC 智能指针（`IWICImagingFactory` / `IWICStream` / `IWICBitmapSource`
    // 等）都来自前一步合法构造、非空，且只在当前线程使用，满足
    // `windows-rs` 对 `unsafe fn` 的安全要求。
    let _com_scope = ComScope::initialize()?;
    let factory = create_wic_factory()?;
    let stream = create_wic_stream(&factory, bytes)?;
    // SAFETY: `factory` 是合法的 `IWICImagingFactory`，`&stream` 是同进程内
    // 刚刚由 `factory` 创建并初始化的 `IWICStream`，`pIDecoderInfo` 允许传 null。
    let decoder = unsafe {
        factory
            .CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)
            .map_err(map_wic_error("failed to create WIC decoder"))?
    };
    // SAFETY: `decoder` 由上一步成功构造，索引 0 对所有 WIC 解码器都是合法的首帧。
    let frame = unsafe {
        decoder
            .GetFrame(0)
            .map_err(map_wic_error("failed to read WIC frame"))?
    };

    let source = create_scaled_wic_source(&factory, &frame, raster_request)?;
    let (width, height) = wic_source_size(&source)?;
    let stride = width
        .checked_mul(4)
        .ok_or_else(|| TguiError::Media("failed to compute WIC raster stride".to_string()))?;
    let buffer_len = stride
        .checked_mul(height)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| TguiError::Media("failed to allocate WIC raster buffer".to_string()))?;
    let mut pixels = vec![0u8; buffer_len];
    // SAFETY: `pixels` 长度为 `stride * height`，是 `CopyPixels` 文档要求的最小
    // 缓冲；`prc` 传 null 表示拷贝整个图像；`source` 已从转换器派生为合法的
    // `IWICBitmapSource`。
    unsafe {
        source
            .CopyPixels(std::ptr::null(), stride, pixels.as_mut_slice())
            .map_err(map_wic_error("failed to copy WIC pixels"))?;
    }

    Ok(TextureFrame::new(width, height, pixels))
}

#[cfg(target_os = "windows")]
fn create_wic_factory() -> Result<IWICImagingFactory, TguiError> {
    // SAFETY: 调用前已通过 `ComScope::initialize` 完成 COM 初始化，
    // `CLSID_WICImagingFactory` 是常量 GUID，`pUnkOuter` 传 None。
    unsafe {
        CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
            .map_err(map_wic_error("failed to create WIC imaging factory"))
    }
}

#[cfg(target_os = "windows")]
fn create_wic_stream(factory: &IWICImagingFactory, bytes: &[u8]) -> Result<IWICStream, TguiError> {
    // SAFETY: `factory` 由调用方传入，已是合法的 `IWICImagingFactory`。
    let stream = unsafe {
        factory
            .CreateStream()
            .map_err(map_wic_error("failed to create WIC stream"))?
    };
    // SAFETY: `stream` 刚被 `CreateStream` 创建，`bytes` 切片在调用期间保持有效；
    // WIC 仅在调用期间读取，不会跨线程持有该缓冲区。
    unsafe {
        stream
            .InitializeFromMemory(bytes)
            .map_err(map_wic_error("failed to initialize WIC stream from memory"))?;
    }
    Ok(stream)
}

#[cfg(target_os = "windows")]
fn create_scaled_wic_source(
    factory: &IWICImagingFactory,
    frame: &IWICBitmapFrameDecode,
    raster_request: RasterRequest,
) -> Result<IWICBitmapSource, TguiError> {
    // SAFETY: `factory` 合法，`CreateBitmapScaler` 不接受参数。
    let scaler = unsafe {
        factory
            .CreateBitmapScaler()
            .map_err(map_wic_error("failed to create WIC bitmap scaler"))?
    };
    // SAFETY: `scaler` 由上一步成功构造；`frame` 由调用方保证有效；宽高经过
    // `clamp_raster_request` 限制，不会为 0。
    unsafe {
        scaler
            .Initialize(
                frame,
                raster_request.width(),
                raster_request.height(),
                WICBitmapInterpolationModeFant,
            )
            .map_err(map_wic_error("failed to scale image with WIC"))?;
    }

    // SAFETY: `factory` 仍然合法。
    let converter = unsafe {
        factory
            .CreateFormatConverter()
            .map_err(map_wic_error("failed to create WIC format converter"))?
    };
    // SAFETY: `converter` 已构造，`scaler` 上一步初始化成功，
    // `GUID_WICPixelFormat32bppRGBA` 是常量；`alphaThresholdPercent` = 0 与
    // `WICBitmapPaletteTypeCustom` 是文档允许的组合。
    unsafe {
        converter
            .Initialize(
                &scaler,
                &GUID_WICPixelFormat32bppRGBA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(map_wic_error("failed to convert WIC pixel format"))?;
    }

    converter
        .cast()
        .map_err(map_wic_error("WIC format converter is not a bitmap source"))
}

#[cfg(target_os = "windows")]
fn wic_source_size(source: &IWICBitmapSource) -> Result<(u32, u32), TguiError> {
    let mut width = 0;
    let mut height = 0;
    // SAFETY: `source` 是合法的 `IWICBitmapSource`，两个出参均为栈上 `u32`，
    // `GetSize` 写入它们不会越界。
    unsafe {
        source
            .GetSize(&mut width, &mut height)
            .map_err(map_wic_error("failed to query WIC source size"))?;
    }
    Ok((width, height))
}

#[cfg(target_os = "windows")]
fn map_wic_error(context: &'static str) -> impl FnOnce(windows::core::Error) -> TguiError {
    move |error| TguiError::Media(format!("{context}: {error}"))
}

#[cfg(target_os = "windows")]
struct ComScope {
    should_uninitialize: bool,
}

#[cfg(target_os = "windows")]
impl ComScope {
    fn initialize() -> Result<Self, TguiError> {
        // SAFETY: `CoInitializeEx` 可被任意线程调用以加入 COM 多线程套间。
        // 该函数在内部维护引用计数；若返回 `S_FALSE` / `RPC_E_CHANGED_MODE`
        // 表示当前线程已被其他模块初始化过，我们记录 `should_uninitialize`
        // 以避免错误地撤销别人加的引用。
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result.is_ok() {
            return Ok(Self {
                should_uninitialize: true,
            });
        }

        if result == S_FALSE || result == RPC_E_CHANGED_MODE {
            return Ok(Self {
                should_uninitialize: false,
            });
        }

        Err(TguiError::Media(format!(
            "failed to initialize COM for WIC: {result}"
        )))
    }
}

#[cfg(target_os = "windows")]
impl Drop for ComScope {
    fn drop(&mut self) {
        if self.should_uninitialize {
            // SAFETY: 仅在 `initialize` 中拿到 `S_OK` 时才把
            // `should_uninitialize` 设为 true，因此这里恰好抵消我们自己加的
            // 一次 `CoInitializeEx` 引用，不会影响外部模块持有的初始化状态。
            unsafe {
                CoUninitialize();
            }
        }
    }
}
