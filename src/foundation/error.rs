use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum TguiError {
    EventLoop(crate::platform::error::EventLoopError),
    Os(crate::platform::error::OsError),
    Request(crate::platform::error::RequestError),
    CreateSurface(wgpu::CreateSurfaceError),
    RequestAdapter(wgpu::RequestAdapterError),
    RequestDevice(wgpu::RequestDeviceError),
    NoSurfaceFormat,
    Unsupported(String),
    TextRender(String),
    Media(String),
}

impl Display for TguiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EventLoop(error) => write!(f, "failed to create or run event loop: {error}"),
            Self::Os(error) => write!(f, "window system error: {error}"),
            Self::Request(error) => write!(f, "window request failed: {error}"),
            Self::CreateSurface(error) => write!(f, "failed to create rendering surface: {error}"),
            Self::RequestAdapter(error) => write!(f, "failed to acquire GPU adapter: {error}"),
            Self::RequestDevice(error) => write!(f, "failed to create GPU device: {error}"),
            Self::NoSurfaceFormat => {
                write!(f, "surface does not expose a compatible texture format")
            }
            Self::Unsupported(message) => write!(f, "{message}"),
            Self::TextRender(error) => write!(f, "failed to render text: {error}"),
            Self::Media(error) => write!(f, "{error}"),
        }
    }
}

impl Error for TguiError {}

impl From<crate::platform::error::EventLoopError> for TguiError {
    fn from(value: crate::platform::error::EventLoopError) -> Self {
        Self::EventLoop(value)
    }
}

impl From<crate::platform::error::OsError> for TguiError {
    fn from(value: crate::platform::error::OsError) -> Self {
        Self::Os(value)
    }
}

impl From<crate::platform::error::RequestError> for TguiError {
    fn from(value: crate::platform::error::RequestError) -> Self {
        Self::Request(value)
    }
}

impl From<wgpu::CreateSurfaceError> for TguiError {
    fn from(value: wgpu::CreateSurfaceError) -> Self {
        Self::CreateSurface(value)
    }
}

impl From<wgpu::RequestAdapterError> for TguiError {
    fn from(value: wgpu::RequestAdapterError) -> Self {
        Self::RequestAdapter(value)
    }
}

impl From<wgpu::RequestDeviceError> for TguiError {
    fn from(value: wgpu::RequestDeviceError) -> Self {
        Self::RequestDevice(value)
    }
}
