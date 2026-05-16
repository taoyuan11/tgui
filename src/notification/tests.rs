use std::sync::{Arc, Mutex};

use crate::foundation::binding::ViewModelContext;
use crate::foundation::view_model::{CommandContext, ValueCommand, ViewModel};
use crate::ui::widget::Element;

use super::platform::sanitize_windows_shortcut_file_name;
use super::runtime::{async_notification_channel, PendingNotificationCompletion};
use super::types::{NotificationAction, NotificationError, NotificationOptions};

#[test]
fn validates_empty_title() {
    let result = NotificationOptions::new("").validate(false);

    assert!(matches!(result, Err(NotificationError::InvalidOptions(_))));
}

#[test]
fn validates_empty_action_id() {
    let result = NotificationOptions::new("Title")
        .action(NotificationAction::new("", "Open"))
        .validate(true);

    assert!(matches!(result, Err(NotificationError::InvalidOptions(_))));
}

#[test]
fn validates_empty_action_label() {
    let result = NotificationOptions::new("Title")
        .action(NotificationAction::new("open", ""))
        .validate(true);

    assert!(matches!(result, Err(NotificationError::InvalidOptions(_))));
}

#[test]
fn validates_action_limit() {
    let result = NotificationOptions::new("Title")
        .action(NotificationAction::new("one", "One"))
        .action(NotificationAction::new("two", "Two"))
        .action(NotificationAction::new("three", "Three"))
        .validate(true);

    assert!(matches!(result, Err(NotificationError::InvalidOptions(_))));
}

#[test]
fn sanitizes_windows_shortcut_file_names() {
    assert_eq!(
        sanitize_windows_shortcut_file_name("com:tgui/demo?"),
        "com_tgui_demo_"
    );
    assert_eq!(sanitize_windows_shortcut_file_name("."), "tgui");
}

#[derive(Default)]
struct TestVm {
    value: Arc<Mutex<Option<String>>>,
}

impl ViewModel for TestVm {
    fn new(_context: &ViewModelContext) -> Self {
        Self::default()
    }

    fn view(&self) -> Element<Self> {
        unimplemented!()
    }
}

#[test]
fn dispatches_completion_to_value_command() {
    let (dispatcher, receiver) = async_notification_channel();
    let mut vm = TestVm::default();
    let state = vm.value.clone();
    let command = ValueCommand::new(|vm: &mut TestVm, value: String| {
        *vm.value.lock().expect("state lock poisoned") = Some(value);
    });

    dispatcher
        .dispatch(PendingNotificationCompletion {
            window_key: "main".to_string(),
            window_instance_id: 1,
            callback: Box::new(move |view_model, context| {
                command.execute_with_context(view_model, "clicked".to_string(), context);
            }),
        })
        .expect("completion should dispatch");

    let completion = receiver
        .try_iter()
        .next()
        .expect("completion should be queued");
    (completion.callback)(&mut vm, &CommandContext::detached());

    assert_eq!(
        state.lock().expect("state lock poisoned").as_deref(),
        Some("clicked")
    );
}
