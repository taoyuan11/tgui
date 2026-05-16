#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
mod desktop;
#[cfg(any(target_os = "android", target_env = "ohos"))]
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
unsafe impl Send for DialogParentHandles {}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
unsafe impl Sync for DialogParentHandles {}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
impl HasDisplayHandle for DialogParentHandles {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
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
        Ok(unsafe { WindowHandle::borrow_raw(self.window) })
    }
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub(crate) use desktop::{run_file_dialog_path, run_file_dialog_paths, run_message_dialog};
#[cfg(any(target_os = "android", target_env = "ohos"))]
pub(crate) use unsupported::{run_file_dialog_path, run_file_dialog_paths, run_message_dialog};
