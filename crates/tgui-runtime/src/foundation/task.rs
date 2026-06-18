use std::sync::{mpsc, Arc, Mutex};

use crate::foundation::view_model::CommandContext;
use crate::platform::backend::event_loop::EventLoopProxy;

type AsyncTaskCallback<VM> = Box<dyn FnOnce(&mut VM, &CommandContext<VM>) + Send>;
type ScopedTaskDispatcher<VM> =
    Arc<dyn Fn(PendingTaskCompletion<VM>) -> Result<(), ()> + Send + Sync>;

pub(crate) struct PendingTaskCompletion<VM> {
    pub(crate) window_key: String,
    pub(crate) window_instance_id: u64,
    pub(crate) callback: AsyncTaskCallback<VM>,
}

pub(crate) struct AsyncTaskReceiver<VM> {
    receiver: mpsc::Receiver<PendingTaskCompletion<VM>>,
}

impl<VM> AsyncTaskReceiver<VM> {
    pub(crate) fn try_iter(&self) -> mpsc::TryIter<'_, PendingTaskCompletion<VM>> {
        self.receiver.try_iter()
    }
}

pub(crate) struct AsyncTaskDispatcher<VM> {
    sender: mpsc::Sender<PendingTaskCompletion<VM>>,
    proxy: Arc<Mutex<Option<EventLoopProxy>>>,
}

impl<VM> Clone for AsyncTaskDispatcher<VM> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            proxy: self.proxy.clone(),
        }
    }
}

impl<VM> AsyncTaskDispatcher<VM> {
    pub(crate) fn set_proxy(&self, proxy: EventLoopProxy) {
        *self.proxy.lock().expect("task proxy lock poisoned") = Some(proxy);
    }

    pub(crate) fn dispatch(&self, completion: PendingTaskCompletion<VM>) -> Result<(), ()> {
        self.sender.send(completion).map_err(|_| ())?;

        if let Some(proxy) = self
            .proxy
            .lock()
            .expect("task proxy lock poisoned")
            .as_ref()
            .cloned()
        {
            proxy.wake_up();
        }

        Ok(())
    }
}

pub(crate) fn async_task_channel<VM>() -> (AsyncTaskDispatcher<VM>, AsyncTaskReceiver<VM>) {
    let (sender, receiver) = mpsc::channel();
    (
        AsyncTaskDispatcher {
            sender,
            proxy: Arc::new(Mutex::new(None)),
        },
        AsyncTaskReceiver { receiver },
    )
}

struct TaskRuntimeContext<VM> {
    window_key: String,
    window_instance_id: u64,
    dispatcher: ScopedTaskDispatcher<VM>,
}

impl<VM> Clone for TaskRuntimeContext<VM> {
    fn clone(&self) -> Self {
        Self {
            window_key: self.window_key.clone(),
            window_instance_id: self.window_instance_id,
            dispatcher: self.dispatcher.clone(),
        }
    }
}

/// Runtime-backed background task service for command handlers.
///
/// Tasks run on a background thread and their completion callback is dispatched
/// back onto the runtime thread before it receives `&mut VM`.
pub struct Tasks<VM> {
    runtime: Option<TaskRuntimeContext<VM>>,
}

impl<VM> Clone for Tasks<VM> {
    fn clone(&self) -> Self {
        Self {
            runtime: self.runtime.clone(),
        }
    }
}

impl<VM: 'static> Tasks<VM> {
    pub(crate) fn detached() -> Self {
        Self { runtime: None }
    }

    pub(crate) fn from_runtime(
        window_key: String,
        window_instance_id: u64,
        dispatcher: AsyncTaskDispatcher<VM>,
    ) -> Self {
        Self {
            runtime: Some(TaskRuntimeContext {
                window_key,
                window_instance_id,
                dispatcher: Arc::new(move |completion| dispatcher.dispatch(completion)),
            }),
        }
    }

    pub(crate) fn scope<ChildVm: 'static>(
        &self,
        selector: Arc<dyn for<'a> Fn(&'a mut VM) -> &'a mut ChildVm + Send + Sync>,
    ) -> Tasks<ChildVm> {
        let Some(runtime) = &self.runtime else {
            return Tasks { runtime: None };
        };

        let dispatcher = runtime.dispatcher.clone();
        Tasks {
            runtime: Some(TaskRuntimeContext {
                window_key: runtime.window_key.clone(),
                window_instance_id: runtime.window_instance_id,
                dispatcher: Arc::new(move |completion: PendingTaskCompletion<ChildVm>| {
                    let scoped_selector = selector.clone();
                    dispatcher(PendingTaskCompletion {
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

    /// Run blocking work on a background thread and dispatch the result to the
    /// runtime thread.
    pub fn spawn_blocking<R>(
        &self,
        work: impl FnOnce() -> R + Send + 'static,
        on_complete: impl FnOnce(&mut VM, R, &CommandContext<VM>) + Send + 'static,
    ) where
        R: Send + 'static,
    {
        let Some(runtime) = self.runtime.clone() else {
            return;
        };

        std::thread::spawn(move || {
            let result = work();
            let _ = (runtime.dispatcher)(PendingTaskCompletion {
                window_key: runtime.window_key,
                window_instance_id: runtime.window_instance_id,
                callback: Box::new(move |view_model, context| {
                    on_complete(view_model, result, context);
                }),
            });
        });
    }
}
