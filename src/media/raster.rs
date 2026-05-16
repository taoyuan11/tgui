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
    let _com_scope = ComScope::initialize()?;
    let factory = create_wic_factory()?;
    let stream = create_wic_stream(&factory, bytes)?;
    let decoder = unsafe {
        factory
            .CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)
            .map_err(map_wic_error("failed to create WIC decoder"))?
    };
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
    unsafe {
        source
            .CopyPixels(std::ptr::null(), stride, pixels.as_mut_slice())
            .map_err(map_wic_error("failed to copy WIC pixels"))?;
    }

    Ok(TextureFrame::new(width, height, pixels))
}

#[cfg(target_os = "windows")]
fn create_wic_factory() -> Result<IWICImagingFactory, TguiError> {
    unsafe {
        CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
            .map_err(map_wic_error("failed to create WIC imaging factory"))
    }
}

#[cfg(target_os = "windows")]
fn create_wic_stream(factory: &IWICImagingFactory, bytes: &[u8]) -> Result<IWICStream, TguiError> {
    let stream = unsafe {
        factory
            .CreateStream()
            .map_err(map_wic_error("failed to create WIC stream"))?
    };
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
    let scaler = unsafe {
        factory
            .CreateBitmapScaler()
            .map_err(map_wic_error("failed to create WIC bitmap scaler"))?
    };
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

    let converter = unsafe {
        factory
            .CreateFormatConverter()
            .map_err(map_wic_error("failed to create WIC format converter"))?
    };
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

    Ok(converter
        .cast()
        .expect("WIC format converter should be a bitmap source"))
}

#[cfg(target_os = "windows")]
fn wic_source_size(source: &IWICBitmapSource) -> Result<(u32, u32), TguiError> {
    let mut width = 0;
    let mut height = 0;
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
            unsafe {
                CoUninitialize();
            }
        }
    }
}
