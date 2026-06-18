use crate::application::{ApplicationConfig, WindowRole};
use crate::foundation::binding::DirtyDependencySet;
use crate::foundation::error::TguiError;
#[cfg(target_os = "windows")]
use crate::log::Log;
#[cfg(target_os = "windows")]
use crate::notification::prepare_platform_notifications;
use crate::platform::backend::event_loop::{ActiveEventLoop, ControlFlow};
use crate::platform::backend::window::Window;
use crate::platform::backend::EventLoop;
use crate::platform::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use crate::platform::window::WindowAttributes;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(target_os = "windows")]
use winit_win32::WindowAttributesWindows;

#[cfg(any(target_os = "windows", target_os = "macos"))]
#[derive(Clone, Copy)]
struct NativeModalParent {
    window: RawWindowHandle,
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
impl NativeModalParent {
    fn from_window(window: &dyn Window) -> Option<Self> {
        Some(Self {
            window: window.window_handle().ok()?.as_raw(),
        })
    }
}

#[cfg(target_os = "windows")]
pub(super) fn configure_native_modal_window(
    attributes: WindowAttributes,
    parent: &dyn Window,
) -> WindowAttributes {
    let Some(parent) = NativeModalParent::from_window(parent) else {
        return attributes;
    };

    match parent.window {
        RawWindowHandle::Win32(handle) => attributes.with_platform_attributes(Box::new(
            WindowAttributesWindows::default()
                .with_owner_window(handle.hwnd.get() as *mut core::ffi::c_void),
        )),
        _ => attributes,
    }
}

#[cfg(target_os = "macos")]
pub(super) fn configure_native_modal_window(
    attributes: WindowAttributes,
    parent: &dyn Window,
) -> WindowAttributes {
    let Some(parent) = NativeModalParent::from_window(parent) else {
        return attributes;
    };

    // SAFETY: `parent.window` 来自 `NativeModalParent::from_window`，里面调用
    // `window_handle()` 拿到的 `RawWindowHandle::AppKit` 与 `parent` 同生命周期；
    // 这里仅作为父窗口属性传给 macOS NSWindow，不会持有跨线程的原始指针，由
    // `winit-appkit` 在主线程立即使用，因此满足 `with_parent_window` 的安全契约。
    unsafe { attributes.with_parent_window(Some(parent.window)) }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(super) fn configure_native_modal_window(
    attributes: WindowAttributes,
    _parent: &dyn Window,
) -> WindowAttributes {
    attributes
}

pub(crate) fn window_sync_priority(role: WindowRole) -> u8 {
    match role {
        WindowRole::Main => 0,
        WindowRole::Child { .. } => 1,
    }
}

pub(super) fn dirty_dependency_set_label(kind: DirtyDependencySet) -> &'static str {
    match kind {
        DirtyDependencySet::Clean => "clean",
        DirtyDependencySet::Global => "global",
        DirtyDependencySet::Dependencies { .. } => "dependencies",
    }
}

pub(crate) fn centered_window_position_for_monitor(
    monitor_position: Option<PhysicalPosition<i32>>,
    monitor_size: PhysicalSize<u32>,
    monitor_scale_factor: f64,
    window_size: LogicalSize<f64>,
) -> Option<PhysicalPosition<i32>> {
    let monitor_position = monitor_position?;
    let monitor_scale_factor = if monitor_scale_factor.is_finite() && monitor_scale_factor > 0.0 {
        monitor_scale_factor
    } else {
        1.0
    };

    let window_width = (window_size.width.max(1.0) * monitor_scale_factor).round() as i64;
    let window_height = (window_size.height.max(1.0) * monitor_scale_factor).round() as i64;
    let horizontal_gap = (i64::from(monitor_size.width) - window_width).max(0);
    let vertical_gap = (i64::from(monitor_size.height) - window_height).max(0);

    let x = i64::from(monitor_position.x) + horizontal_gap / 2;
    let y = i64::from(monitor_position.y) + vertical_gap / 2;

    Some(PhysicalPosition::new(
        x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    ))
}

pub(super) fn default_window_position(
    event_loop: &dyn ActiveEventLoop,
    window_size: LogicalSize<f64>,
) -> Option<PhysicalPosition<i32>> {
    let monitor = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())?;
    let monitor_size = monitor
        .current_video_mode()
        .map(|mode| mode.size())
        .or_else(|| monitor.video_modes().next().map(|mode| mode.size()))?;
    centered_window_position_for_monitor(
        monitor.position(),
        monitor_size,
        monitor.scale_factor(),
        window_size,
    )
}

pub(super) fn prepare_notifications_for_runtime(_config: &ApplicationConfig) {
    #[cfg(target_os = "windows")]
    {
        if let Some(app_id) = _config.app_id.as_deref() {
            if let Err(error) = prepare_platform_notifications(Some(app_id), &_config.title) {
                Log::with_tag("tgui-runtime").warn(format_args!(
                    "failed to prepare Windows notifications: {error}"
                ));
            }
        }
    }
}

pub(super) fn build_event_loop(control_flow: ControlFlow) -> Result<EventLoop, TguiError> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(control_flow);
    Ok(event_loop)
}
