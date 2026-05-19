mod platform;
mod runtime;
mod types;

// `platform::install_android_app` 在 runtime 启动时被调用以装载 Android 对话框桥接。
#[cfg(target_os = "android")]
pub(crate) use platform::install_android_app;

pub use runtime::Dialogs;
pub use types::{
    DialogError, FileDialogOptions, MessageDialogButtons, MessageDialogLevel, MessageDialogOptions,
    MessageDialogResult,
};

pub(crate) use runtime::{async_dialog_channel, AsyncDialogDispatcher, AsyncDialogReceiver};
