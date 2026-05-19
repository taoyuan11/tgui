#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
mod android_bridge;
#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
mod desktop;
#[cfg(target_env = "ohos")]
mod unsupported;

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};

use crate::platform::backend::window::Window;

#[derive(Clone, Copy)]
pub(crate) enum FileDialogRequest {
    OpenFile,
    OpenFiles,
    PickFolder,
    PickFolders,
    SaveFile,
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
#[derive(Clone)]
pub(crate) struct DialogParentHandles {
    display: RawDisplayHandle,
    window: RawWindowHandle,
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
#[derive(Clone, Debug)]
pub(crate) struct DialogParentHandles;

#[cfg(any(target_os = "android", target_env = "ohos"))]
impl DialogParentHandles {
    pub(crate) fn from_window(_window: &dyn Window) -> Option<Self> {
        None
    }
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
impl DialogParentHandles {
    pub(crate) fn from_window(window: &dyn Window) -> Option<Self> {
        Some(Self {
            display: window.display_handle().ok()?.as_raw(),
            window: window.window_handle().ok()?.as_raw(),
        })
    }
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
// SAFETY: `RawDisplayHandle` / `RawWindowHandle` 内部的原始指针只在父窗口
// 仍然存活的事件循环线程上被消费——dialog 通过 `CommandContext::dialogs()`
// 在 winit 主线程同步等待结果，结果回调返回前父窗口不会被释放，因此跨
// 线程移动该结构体不会让指针指向已释放的资源。Send/Sync 仅是为了把结构体
// 包进 `Arc` 与命令通道一起传递，并不会真正在工作线程解引用句柄。
unsafe impl Send for DialogParentHandles {}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
// SAFETY: 见上面 `Send` 的说明：句柄的解引用始终发生在拥有窗口的线程，
// 共享引用在事件循环 / 对话框 worker 之间只用作不可变只读传递。
unsafe impl Sync for DialogParentHandles {}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
impl HasDisplayHandle for DialogParentHandles {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        // SAFETY: `self.display` 来自 `window.display_handle()` 提供的、
        // 与父窗口同生命周期的 raw 句柄；`DialogParentHandles` 仅在父窗口
        // 存活期间被对话框平台后端持有（详见 `Send`/`Sync` 处的说明），
        // 所以 `'_` 借用不会越过句柄的有效生命周期。
        Ok(unsafe { DisplayHandle::borrow_raw(self.display) })
    }
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
impl HasWindowHandle for DialogParentHandles {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: 同 `display_handle`:`self.window` 是从父 `Window` 拿到的
        // 原始句柄，只要父窗口还在事件循环里活着，借用 `'_` 就是有效的。
        Ok(unsafe { WindowHandle::borrow_raw(self.window) })
    }
}

#[cfg(target_os = "android")]
pub(crate) use android::{
    dispatch_file_async_path, dispatch_file_async_paths, dispatch_message_async,
    install_android_app,
};
#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub(crate) use desktop::{run_file_dialog_path, run_file_dialog_paths, run_message_dialog};
#[cfg(target_env = "ohos")]
pub(crate) use unsupported::{run_file_dialog_path, run_file_dialog_paths, run_message_dialog};
