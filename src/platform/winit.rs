//! Minimal real-window adapter used by desktop applications and smoke tests.

use crate::application::WindowSpec;
use crate::core::{DpiScale, Error, Result, Size};
use crate::render::{CompiledScene, wgpu::WgpuRenderer};
use std::sync::Arc;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

/// Platform event after the surface has synchronized its size and DPI state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WinitSurfaceEvent {
    Ignored,
    CloseRequested,
    RedrawRequested,
    Resized {
        logical_size: Size,
        dpi_scale: DpiScale,
    },
}

/// A real OS window paired with the optional wgpu renderer.
///
/// The window is reference counted so wgpu owns a `'static` surface target;
/// platform handles never enter the retained Element or Render trees.
pub struct WinitSurface {
    window: Arc<Window>,
    renderer: WgpuRenderer<'static>,
    logical_size: Size,
    dpi_scale: DpiScale,
}

impl WinitSurface {
    /// Creates and configures a window. This must be called from winit's
    /// `resumed` callback with the active event-loop token.
    pub async fn new(event_loop: &ActiveEventLoop, spec: &WindowSpec) -> Result<Self> {
        spec.validate()?;
        let size = spec.inner_size();
        let mut attributes = Window::default_attributes()
            .with_title(spec.title())
            .with_resizable(spec.resizable())
            .with_transparent(spec.transparent())
            .with_inner_size(LogicalSize::new(
                f64::from(size.width),
                f64::from(size.height),
            ));
        if let Some(min) = spec.min_inner_size() {
            attributes = attributes.with_min_inner_size(LogicalSize::new(
                f64::from(min.width),
                f64::from(min.height),
            ));
        }
        if let Some(max) = spec.max_inner_size() {
            attributes = attributes.with_max_inner_size(LogicalSize::new(
                f64::from(max.width),
                f64::from(max.height),
            ));
        }
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|error| Error::platform("create_window", error.to_string(), false))?,
        );
        let dpi_scale = DpiScale::new(window.scale_factor()).map_err(Error::from)?;
        let physical_size = window.inner_size();
        let logical_size = logical_size(physical_size, dpi_scale)?;
        let mut renderer =
            WgpuRenderer::new_for_window(window.clone(), logical_size, dpi_scale).await?;
        renderer.configure_surface_with_alpha(spec.transparent())?;
        Ok(Self {
            window,
            renderer,
            logical_size,
            dpi_scale,
        })
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub fn renderer(&self) -> &WgpuRenderer<'static> {
        &self.renderer
    }

    pub fn renderer_mut(&mut self) -> &mut WgpuRenderer<'static> {
        &mut self.renderer
    }

    pub const fn logical_size(&self) -> Size {
        self.logical_size
    }

    pub const fn dpi_scale(&self) -> DpiScale {
        self.dpi_scale
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn request_resize(&self, logical_size: Size) -> Result<()> {
        logical_size.validate().map_err(Error::from)?;
        let _ = self.window.request_inner_size(LogicalSize::new(
            f64::from(logical_size.width),
            f64::from(logical_size.height),
        ));
        Ok(())
    }

    /// Synchronizes resize/DPI state before the application receives the
    /// translated event. Zero-sized/minimized windows are retained at a 1x1
    /// physical surface by the renderer until a non-zero resize arrives.
    pub fn handle_event(&mut self, event: &WindowEvent) -> Result<WinitSurfaceEvent> {
        match event {
            WindowEvent::CloseRequested => Ok(WinitSurfaceEvent::CloseRequested),
            WindowEvent::RedrawRequested => Ok(WinitSurfaceEvent::RedrawRequested),
            WindowEvent::Resized(size) => self.resize(*size, self.window.scale_factor()),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.resize(self.window.inner_size(), *scale_factor)
            }
            _ => Ok(WinitSurfaceEvent::Ignored),
        }
    }

    pub fn render(&mut self, scene: &CompiledScene) -> Result<()> {
        self.renderer.render_surface(scene)
    }

    /// Recreates the wgpu device and surface configuration after device loss.
    pub async fn recover_device(&mut self) -> Result<()> {
        self.renderer.recover_device().await
    }

    fn resize(
        &mut self,
        physical_size: PhysicalSize<u32>,
        scale_factor: f64,
    ) -> Result<WinitSurfaceEvent> {
        let dpi_scale = DpiScale::new(scale_factor).map_err(Error::from)?;
        let logical_size = logical_size(physical_size, dpi_scale)?;
        self.renderer.resize(logical_size, dpi_scale)?;
        self.logical_size = logical_size;
        self.dpi_scale = dpi_scale;
        Ok(WinitSurfaceEvent::Resized {
            logical_size,
            dpi_scale,
        })
    }
}

fn logical_size(physical: PhysicalSize<u32>, scale: DpiScale) -> Result<Size> {
    let size = Size::new(
        scale.physical_to_logical(physical.width),
        scale.physical_to_logical(physical.height),
    );
    size.validate().map_err(Error::from)?;
    Ok(size)
}
