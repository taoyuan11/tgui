use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

use crate::foundation::view_model::{CommandContext, ValueCommand};
use crate::platform::backend::event_loop::EventLoopProxy;
use crate::platform::backend::window::Window;

use super::platform::{run_file_dialog_path, run_file_dialog_paths, run_message_dialog};
use super::platform::{DialogParentHandles, FileDialogRequest};
use super::types::{DialogError, FileDialogOptions, MessageDialogOptions, MessageDialogResult};

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

        // 原生对话框通常在外部线程或平台回调中结束，这里主动唤醒事件循环，
        // 保证 completion 能在 UI 线程上回投给 ViewModel。
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

/// 提供原生文件对话框和消息对话框能力。
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

    /// 打开单文件选择对话框。
    ///
    /// 参数:
    /// - `options`: 文件对话框配置。
    ///
    /// 返回值: 成功时返回用户选择的单个路径；取消时返回 `None`。
    pub fn open_file(&self, options: FileDialogOptions) -> Result<Option<PathBuf>, DialogError> {
        self.run_file_dialog_path(FileDialogRequest::OpenFile, options)
    }

    /// 打开多文件选择对话框。
    ///
    /// 参数:
    /// - `options`: 文件对话框配置。
    ///
    /// 返回值: 成功时返回用户选择的路径集合；取消时返回 `None`。
    pub fn open_files(
        &self,
        options: FileDialogOptions,
    ) -> Result<Option<Vec<PathBuf>>, DialogError> {
        self.run_file_dialog_paths(FileDialogRequest::OpenFiles, options)
    }

    /// 打开单目录选择对话框。
    ///
    /// 参数:
    /// - `options`: 文件对话框配置。
    ///
    /// 返回值: 成功时返回用户选择的目录路径；取消时返回 `None`。
    pub fn pick_folder(&self, options: FileDialogOptions) -> Result<Option<PathBuf>, DialogError> {
        self.run_file_dialog_path(FileDialogRequest::PickFolder, options)
    }

    /// 打开多目录选择对话框。
    ///
    /// 参数:
    /// - `options`: 文件对话框配置。
    ///
    /// 返回值: 成功时返回用户选择的目录路径集合；取消时返回 `None`。
    pub fn pick_folders(
        &self,
        options: FileDialogOptions,
    ) -> Result<Option<Vec<PathBuf>>, DialogError> {
        self.run_file_dialog_paths(FileDialogRequest::PickFolders, options)
    }

    /// 打开保存文件对话框。
    ///
    /// 参数:
    /// - `options`: 文件对话框配置。
    ///
    /// 返回值: 成功时返回用户选定的保存路径；取消时返回 `None`。
    pub fn save_file(&self, options: FileDialogOptions) -> Result<Option<PathBuf>, DialogError> {
        self.run_file_dialog_path(FileDialogRequest::SaveFile, options)
    }

    /// 异步打开单文件选择对话框。
    pub fn open_file_async(
        &self,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<PathBuf>, DialogError>>,
    ) -> Result<(), DialogError> {
        self.spawn_async_path(FileDialogRequest::OpenFile, options, callback)
    }

    /// 异步打开多文件选择对话框。
    pub fn open_files_async(
        &self,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<Vec<PathBuf>>, DialogError>>,
    ) -> Result<(), DialogError> {
        self.spawn_async_paths(FileDialogRequest::OpenFiles, options, callback)
    }

    /// 异步打开单目录选择对话框。
    pub fn pick_folder_async(
        &self,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<PathBuf>, DialogError>>,
    ) -> Result<(), DialogError> {
        self.spawn_async_path(FileDialogRequest::PickFolder, options, callback)
    }

    /// 异步打开多目录选择对话框。
    pub fn pick_folders_async(
        &self,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<Vec<PathBuf>>, DialogError>>,
    ) -> Result<(), DialogError> {
        self.spawn_async_paths(FileDialogRequest::PickFolders, options, callback)
    }

    /// 异步打开保存文件对话框。
    pub fn save_file_async(
        &self,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<PathBuf>, DialogError>>,
    ) -> Result<(), DialogError> {
        self.spawn_async_path(FileDialogRequest::SaveFile, options, callback)
    }

    /// 打开消息对话框并同步返回结果。
    ///
    /// 参数:
    /// - `options`: 消息对话框配置。
    ///
    /// 返回值: 用户在消息对话框中的选择结果。
    pub fn show_message(
        &self,
        options: MessageDialogOptions,
    ) -> Result<MessageDialogResult, DialogError> {
        let runtime = self.runtime_context()?;
        run_message_dialog(options, runtime.parent.as_ref())
    }

    /// 异步打开消息对话框。
    pub fn show_message_async(
        &self,
        options: MessageDialogOptions,
        callback: ValueCommand<VM, Result<MessageDialogResult, DialogError>>,
    ) -> Result<(), DialogError> {
        let runtime = self.runtime_context()?.clone();
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
        let runtime = self.runtime_context()?;
        run_file_dialog_path(request, options, runtime.parent.as_ref())
    }

    fn run_file_dialog_paths(
        &self,
        request: FileDialogRequest,
        options: FileDialogOptions,
    ) -> Result<Option<Vec<PathBuf>>, DialogError> {
        let runtime = self.runtime_context()?;
        run_file_dialog_paths(request, options, runtime.parent.as_ref())
    }

    fn spawn_async_path(
        &self,
        request: FileDialogRequest,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<PathBuf>, DialogError>>,
    ) -> Result<(), DialogError> {
        let runtime = self.runtime_context()?.clone();
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

    fn spawn_async_paths(
        &self,
        request: FileDialogRequest,
        options: FileDialogOptions,
        callback: ValueCommand<VM, Result<Option<Vec<PathBuf>>, DialogError>>,
    ) -> Result<(), DialogError> {
        let runtime = self.runtime_context()?.clone();
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
