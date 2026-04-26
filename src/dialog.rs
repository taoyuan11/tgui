use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};

use crate::foundation::view_model::{CommandContext, ValueCommand};
use crate::platform::backend::event_loop::EventLoopProxy;
use crate::platform::backend::window::Window;

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogError {
    UnsupportedPlatform,
    Backend(String),
}

impl Display for DialogError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform => write!(f, "dialogs are not supported on this platform"),
            Self::Backend(message) => write!(f, "{message}"),
        }
    }
}

impl Error for DialogError {}

#[derive(Debug, Clone, Default)]
pub struct FileDialogOptions {
    title: Option<String>,
    directory: Option<PathBuf>,
    file_name: Option<String>,
    filters: Vec<FileDialogFilter>,
    can_create_directories: Option<bool>,
}

impl FileDialogOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn directory<P: AsRef<Path>>(mut self, directory: P) -> Self {
        self.directory = Some(directory.as_ref().to_path_buf());
        self
    }

    pub fn file_name(mut self, file_name: impl Into<String>) -> Self {
        self.file_name = Some(file_name.into());
        self
    }

    pub fn add_filter(mut self, name: impl Into<String>, extensions: &[impl ToString]) -> Self {
        self.filters.push(FileDialogFilter {
            name: name.into(),
            extensions: extensions.iter().map(|ext| ext.to_string()).collect(),
        });
        self
    }

    pub fn can_create_directories(mut self, can_create_directories: bool) -> Self {
        self.can_create_directories = Some(can_create_directories);
        self
    }
}

#[derive(Debug, Clone)]
struct FileDialogFilter {
    name: String,
    extensions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageDialogLevel {
    #[default]
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageDialogButtons {
    #[default]
    Ok,
    OkCancel,
    YesNo,
    YesNoCancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageDialogResult {
    Yes,
    No,
    Ok,
    #[default]
    Cancel,
}

#[derive(Debug, Clone, Default)]
pub struct MessageDialogOptions {
    title: Option<String>,
    description: Option<String>,
    level: MessageDialogLevel,
    buttons: MessageDialogButtons,
}

impl MessageDialogOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn level(mut self, level: MessageDialogLevel) -> Self {
        self.level = level;
        self
    }

    pub fn buttons(mut self, buttons: MessageDialogButtons) -> Self {
        self.buttons = buttons;
        self
    }
}

type AsyncDialogCallback<VM> = Box<dyn FnOnce(&mut VM, &CommandContext<VM>) + Send>;
type ScopedDialogDispatcher<VM> =
    Arc<dyn Fn(PendingDialogCompletion<VM>) -> Result<(), DialogError> + Send + Sync>;

pub(crate) struct PendingDialogCompletion<VM> {
    pub(crate) window_key: String,
    pub(crate) window_instance_id: u64,
    pub(crate) callback: AsyncDialogCallback<VM>,
}

pub(crate) struct AsyncDialogReceiver<VM> {
    receiver: mpsc::Receiver<PendingDialogCompletion<VM>>,
}

impl<VM> AsyncDialogReceiver<VM> {
    pub(crate) fn try_iter(&self) -> mpsc::TryIter<'_, PendingDialogCompletion<VM>> {
        self.receiver.try_iter()
    }
}

pub(crate) struct AsyncDialogDispatcher<VM> {
    sender: mpsc::Sender<PendingDialogCompletion<VM>>,
    proxy: Arc<Mutex<Option<EventLoopProxy>>>,
}

impl<VM> Clone for AsyncDialogDispatcher<VM> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            proxy: self.proxy.clone(),
        }
    }
}

impl<VM> AsyncDialogDispatcher<VM> {
    pub(crate) fn set_proxy(&self, proxy: EventLoopProxy) {
        *self.proxy.lock().expect("dialog proxy lock poisoned") = Some(proxy);
    }

    pub(crate) fn dispatch(
        &self,
        completion: PendingDialogCompletion<VM>,
    ) -> Result<(), DialogError> {
        self.sender.send(completion).map_err(|_| {
            DialogError::Backend("failed to dispatch dialog completion to the runtime".to_string())
        })?;

        if let Some(proxy) = self
            .proxy
            .lock()
            .expect("dialog proxy lock poisoned")
            .as_ref()
            .cloned()
        {
            proxy.wake_up();
        }

        Ok(())
    }
}

pub(crate) fn async_dialog_channel<VM>() -> (AsyncDialogDispatcher<VM>, AsyncDialogReceiver<VM>) {
    let (sender, receiver) = mpsc::channel();
    (
        AsyncDialogDispatcher {
            sender,
            proxy: Arc::new(Mutex::new(None)),
        },
        AsyncDialogReceiver { receiver },
    )
}

struct DialogRuntimeContext<VM> {
    window_key: String,
    window_instance_id: u64,
    parent: Option<DialogParentHandles>,
    dispatcher: ScopedDialogDispatcher<VM>,
}

impl<VM> Clone for DialogRuntimeContext<VM> {
    fn clone(&self) -> Self {
        Self {
            window_key: self.window_key.clone(),
            window_instance_id: self.window_instance_id,
            parent: self.parent.clone(),
            dispatcher: self.dispatcher.clone(),
        }
    }
}

pub struct Dialogs<VM> {
    runtime: Option<DialogRuntimeContext<VM>>,
}

impl<VM> Clone for Dialogs<VM> {
    fn clone(&self) -> Self {
        Self {
            runtime: self.runtime.clone(),
        }
    }
}

impl<VM: 'static> Dialogs<VM> {
    pub(crate) fn detached() -> Self {
        Self { runtime: None }
    }

    pub(crate) fn from_runtime(
        window_key: String,
        window_instance_id: u64,
        window: Option<&Arc<dyn Window>>,
        dispatcher: AsyncDialogDispatcher<VM>,
    ) -> Self {
        Self {
            runtime: Some(DialogRuntimeContext {
                window_key,
                window_instance_id,
                parent: window.and_then(|window| DialogParentHandles::from_window(window.as_ref())),
                dispatcher: Arc::new(move |completion| dispatcher.dispatch(completion)),
            }),
        }
    }

    pub(crate) fn scope<ChildVm: 'static>(
        &self,
        selector: Arc<dyn for<'a> Fn(&'a mut VM) -> &'a mut ChildVm + Send + Sync>,
    ) -> Dialogs<ChildVm> {
        let Some(runtime) = &self.runtime else {
            return Dialogs { runtime: None };
        };

        let dispatcher = runtime.dispatcher.clone();
        Dialogs {
            runtime: Some(DialogRuntimeContext {
                window_key: runtime.window_key.clone(),
                window_instance_id: runtime.window_instance_id,
                parent: runtime.parent.clone(),
                dispatcher: Arc::new(move |completion: PendingDialogCompletion<ChildVm>| {
                    let scoped_selector = selector.clone();
                    dispatcher(PendingDialogCompletion {
                        window_key: completion.window_key,
                        window_instance_id: completion.window_instance_id,
                        callback: Box::new(move |view_model, context| {
                            let scoped_context = context.scope(scoped_selector.clone());
                            (completion.callback)(scoped_selector(view_model), &scoped_context);
                        }),
                    })
                }),
            }),
        }
    }

    pub fn open_file(&self, options: FileDialogOptions) -> Result<Option<PathBuf>, DialogError> {
        self.run_file_dialog_path(FileDialogRequest::OpenFile, options)
    }

    pub fn open_files(
        &self,
        options: FileDialogOptions,
    ) -> Result<Option<Vec<PathBuf>>, DialogError> {
        self.run_file_dialog_paths(FileDialogRequest::OpenFiles, options)
    }

    pub fn pick_folder(&self, options: FileDialogOptions) -> Result<Option<PathBuf>, DialogError> {
        self.run_file_dialog_path(FileDialogRequest::PickFolder, options)
    }

    pub fn pick_folders(
        &self,
        options: FileDialogOptions,
    ) -> Result<Option<Vec<PathBuf>>, DialogError> {
        self.run_file_dialog_paths(FileDialogRequest::PickFolders, options)
    }

    pub fn save_file(&self, options: FileDialogOptions) -> Result<Option<PathBuf>, DialogError> {
        self.run_file_dialog_path(FileDialogRequest::SaveFile, options)
    }

    pub fn open_file_async(
        &self,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<PathBuf>, DialogError>>,
    ) -> Result<(), DialogError> {
        self.spawn_async_path(FileDialogRequest::OpenFile, options, callback)
    }

    pub fn open_files_async(
        &self,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<Vec<PathBuf>>, DialogError>>,
    ) -> Result<(), DialogError> {
        self.spawn_async_paths(FileDialogRequest::OpenFiles, options, callback)
    }

    pub fn pick_folder_async(
        &self,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<PathBuf>, DialogError>>,
    ) -> Result<(), DialogError> {
        self.spawn_async_path(FileDialogRequest::PickFolder, options, callback)
    }

    pub fn pick_folders_async(
        &self,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<Vec<PathBuf>>, DialogError>>,
    ) -> Result<(), DialogError> {
        self.spawn_async_paths(FileDialogRequest::PickFolders, options, callback)
    }

    pub fn save_file_async(
        &self,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<PathBuf>, DialogError>>,
    ) -> Result<(), DialogError> {
        self.spawn_async_path(FileDialogRequest::SaveFile, options, callback)
    }

    pub fn show_message(
        &self,
        options: MessageDialogOptions,
    ) -> Result<MessageDialogResult, DialogError> {
        #[cfg(any(target_os = "android", all(target_env = "ohos", feature = "ohos")))]
        {
            let _ = options;
            return Err(DialogError::UnsupportedPlatform);
        }

        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        {
            let runtime = self.runtime_context()?;
            run_message_dialog(options, runtime.parent.as_ref())
        }
    }

    pub fn show_message_async(
        &self,
        options: MessageDialogOptions,
        callback: ValueCommand<VM, Result<MessageDialogResult, DialogError>>,
    ) -> Result<(), DialogError> {
        let runtime = self.runtime_context()?.clone();

        #[cfg(any(target_os = "android", all(target_env = "ohos", feature = "ohos")))]
        {
            let _ = options;
            return runtime.dispatcher.dispatch(PendingDialogCompletion {
                window_key: runtime.window_key,
                window_instance_id: runtime.window_instance_id,
                callback: Box::new(move |view_model, context| {
                    callback.execute_with_context(
                        view_model,
                        Err(DialogError::UnsupportedPlatform),
                        context,
                    );
                }),
            });
        }

        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        {
            let parent = runtime.parent.clone();
            let dispatcher = runtime.dispatcher.clone();
            let window_key = runtime.window_key;
            let window_instance_id = runtime.window_instance_id;
            std::thread::spawn(move || {
                let result = run_message_dialog(options, parent.as_ref());
                let _ = dispatcher(PendingDialogCompletion {
                    window_key,
                    window_instance_id,
                    callback: Box::new(move |view_model, context| {
                        callback.execute_with_context(view_model, result, context);
                    }),
                });
            });
            Ok(())
        }
    }

    fn runtime_context(&self) -> Result<&DialogRuntimeContext<VM>, DialogError> {
        self.runtime.as_ref().ok_or_else(|| {
            DialogError::Backend("dialog context is not available for this command".to_string())
        })
    }

    fn run_file_dialog_path(
        &self,
        request: FileDialogRequest,
        options: FileDialogOptions,
    ) -> Result<Option<PathBuf>, DialogError> {
        #[cfg(any(target_os = "android", all(target_env = "ohos", feature = "ohos")))]
        {
            let _ = (request, options);
            return Err(DialogError::UnsupportedPlatform);
        }

        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        {
            let runtime = self.runtime_context()?;
            run_file_dialog_path(request, options, runtime.parent.as_ref())
        }
    }

    fn run_file_dialog_paths(
        &self,
        request: FileDialogRequest,
        options: FileDialogOptions,
    ) -> Result<Option<Vec<PathBuf>>, DialogError> {
        #[cfg(any(target_os = "android", all(target_env = "ohos", feature = "ohos")))]
        {
            let _ = (request, options);
            return Err(DialogError::UnsupportedPlatform);
        }

        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        {
            let runtime = self.runtime_context()?;
            run_file_dialog_paths(request, options, runtime.parent.as_ref())
        }
    }

    fn spawn_async_path(
        &self,
        request: FileDialogRequest,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<PathBuf>, DialogError>>,
    ) -> Result<(), DialogError> {
        let runtime = self.runtime_context()?.clone();

        #[cfg(any(target_os = "android", all(target_env = "ohos", feature = "ohos")))]
        {
            let _ = (request, options);
            return runtime.dispatcher.dispatch(PendingDialogCompletion {
                window_key: runtime.window_key,
                window_instance_id: runtime.window_instance_id,
                callback: Box::new(move |view_model, context| {
                    callback.execute_with_context(
                        view_model,
                        Err(DialogError::UnsupportedPlatform),
                        context,
                    );
                }),
            });
        }

        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        {
            let parent = runtime.parent.clone();
            let dispatcher = runtime.dispatcher.clone();
            let window_key = runtime.window_key;
            let window_instance_id = runtime.window_instance_id;
            std::thread::spawn(move || {
                let result = run_file_dialog_path(request, options, parent.as_ref());
                let _ = dispatcher(PendingDialogCompletion {
                    window_key,
                    window_instance_id,
                    callback: Box::new(move |view_model, context| {
                        callback.execute_with_context(view_model, result, context);
                    }),
                });
            });
            Ok(())
        }
    }

    fn spawn_async_paths(
        &self,
        request: FileDialogRequest,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<Vec<PathBuf>>, DialogError>>,
    ) -> Result<(), DialogError> {
        let runtime = self.runtime_context()?.clone();

        #[cfg(any(target_os = "android", all(target_env = "ohos", feature = "ohos")))]
        {
            let _ = (request, options);
            return runtime.dispatcher.dispatch(PendingDialogCompletion {
                window_key: runtime.window_key,
                window_instance_id: runtime.window_instance_id,
                callback: Box::new(move |view_model, context| {
                    callback.execute_with_context(
                        view_model,
                        Err(DialogError::UnsupportedPlatform),
                        context,
                    );
                }),
            });
        }

        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        {
            let parent = runtime.parent.clone();
            let dispatcher = runtime.dispatcher.clone();
            let window_key = runtime.window_key;
            let window_instance_id = runtime.window_instance_id;
            std::thread::spawn(move || {
                let result = run_file_dialog_paths(request, options, parent.as_ref());
                let _ = dispatcher(PendingDialogCompletion {
                    window_key,
                    window_instance_id,
                    callback: Box::new(move |view_model, context| {
                        callback.execute_with_context(view_model, result, context);
                    }),
                });
            });
            Ok(())
        }
    }
}

#[derive(Clone, Copy)]
enum FileDialogRequest {
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
struct DialogParentHandles {
    display: RawDisplayHandle,
    window: RawWindowHandle,
}

#[cfg(any(target_os = "android", target_env = "ohos"))]
#[derive(Clone, Debug)]
struct DialogParentHandles;

#[cfg(any(target_os = "android", target_env = "ohos"))]
impl DialogParentHandles {
    fn from_window(_window: &dyn Window) -> Option<Self> {
        None
    }
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
impl DialogParentHandles {
    fn from_window(window: &dyn Window) -> Option<Self> {
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
fn configure_file_dialog(
    options: FileDialogOptions,
    parent: Option<&DialogParentHandles>,
) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new();

    if let Some(parent) = parent {
        dialog = dialog.set_parent(parent);
    }
    if let Some(title) = options.title {
        dialog = dialog.set_title(title);
    }
    if let Some(directory) = options.directory {
        dialog = dialog.set_directory(directory);
    }
    if let Some(file_name) = options.file_name {
        dialog = dialog.set_file_name(file_name);
    }
    if let Some(can_create_directories) = options.can_create_directories {
        dialog = dialog.set_can_create_directories(can_create_directories);
    }

    for filter in options.filters {
        dialog = dialog.add_filter(filter.name, &filter.extensions);
    }

    dialog
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
fn run_file_dialog_path(
    request: FileDialogRequest,
    options: FileDialogOptions,
    parent: Option<&DialogParentHandles>,
) -> Result<Option<PathBuf>, DialogError> {
    let dialog = configure_file_dialog(options, parent);
    let path = match request {
        FileDialogRequest::OpenFile => dialog.pick_file(),
        FileDialogRequest::PickFolder => dialog.pick_folder(),
        FileDialogRequest::SaveFile => dialog.save_file(),
        FileDialogRequest::OpenFiles | FileDialogRequest::PickFolders => {
            return Err(DialogError::Backend(
                "internal dialog request kind mismatch for single-path result".to_string(),
            ));
        }
    };
    Ok(path)
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
fn run_file_dialog_paths(
    request: FileDialogRequest,
    options: FileDialogOptions,
    parent: Option<&DialogParentHandles>,
) -> Result<Option<Vec<PathBuf>>, DialogError> {
    let dialog = configure_file_dialog(options, parent);
    let paths = match request {
        FileDialogRequest::OpenFiles => dialog.pick_files(),
        FileDialogRequest::PickFolders => dialog.pick_folders(),
        FileDialogRequest::OpenFile
        | FileDialogRequest::PickFolder
        | FileDialogRequest::SaveFile => {
            return Err(DialogError::Backend(
                "internal dialog request kind mismatch for multi-path result".to_string(),
            ));
        }
    };
    Ok(paths)
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
fn configure_message_dialog(
    options: MessageDialogOptions,
    parent: Option<&DialogParentHandles>,
) -> rfd::MessageDialog {
    let mut dialog = rfd::MessageDialog::new();

    if let Some(parent) = parent {
        dialog = dialog.set_parent(parent);
    }
    if let Some(title) = options.title {
        dialog = dialog.set_title(title);
    }
    if let Some(description) = options.description {
        dialog = dialog.set_description(description);
    }

    dialog
        .set_level(match options.level {
            MessageDialogLevel::Info => rfd::MessageLevel::Info,
            MessageDialogLevel::Warning => rfd::MessageLevel::Warning,
            MessageDialogLevel::Error => rfd::MessageLevel::Error,
        })
        .set_buttons(match options.buttons {
            MessageDialogButtons::Ok => rfd::MessageButtons::Ok,
            MessageDialogButtons::OkCancel => rfd::MessageButtons::OkCancel,
            MessageDialogButtons::YesNo => rfd::MessageButtons::YesNo,
            MessageDialogButtons::YesNoCancel => rfd::MessageButtons::YesNoCancel,
        })
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    all(target_os = "linux", not(target_env = "ohos"))
))]
fn run_message_dialog(
    options: MessageDialogOptions,
    parent: Option<&DialogParentHandles>,
) -> Result<MessageDialogResult, DialogError> {
    let result = configure_message_dialog(options, parent).show();
    Ok(match result {
        rfd::MessageDialogResult::Yes => MessageDialogResult::Yes,
        rfd::MessageDialogResult::No => MessageDialogResult::No,
        rfd::MessageDialogResult::Ok => MessageDialogResult::Ok,
        rfd::MessageDialogResult::Cancel | rfd::MessageDialogResult::Custom(_) => {
            MessageDialogResult::Cancel
        }
    })
}
