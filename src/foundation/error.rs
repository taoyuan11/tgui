use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum TguiError {
    EventLoop(winit::error::EventLoopError),
    Os(winit::error::OsError),
    CreateSurface(wgpu::CreateSurfaceError),
    RequestAdapter(wgpu::RequestAdapterError),
    RequestDevice(wgpu::RequestDeviceError),
    NoSurfaceFormat,
    TextRender(String),
}

impl Display for TguiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EventLoop(error) => write!(f, "failed to create or run event loop: {error}"),
            Self::Os(error) => write!(f, "window system error: {error}"),
            Self::CreateSurface(error) => write!(f, "failed to create rendering surface: {error}"),
            Self::RequestAdapter(error) => write!(f, "failed to acquire GPU adapter: {error}"),
            Self::RequestDevice(error) => write!(f, "failed to create GPU device: {error}"),
            Self::NoSurfaceFormat => {
                write!(f, "surface does not expose a compatible texture format")
            }
            Self::TextRender(error) => write!(f, "failed to render text: {error}"),
        }
    }
}

impl Error for TguiError {}

impl From<winit::error::EventLoopError> for TguiError {
    fn from(value: winit::error::EventLoopError) -> Self {
        Self::EventLoop(value)
    }
}

impl From<winit::error::OsError> for TguiError {
    fn from(value: winit::error::OsError) -> Self {
        Self::Os(value)
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
