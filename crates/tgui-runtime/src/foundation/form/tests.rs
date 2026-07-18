use crate::foundation::binding::ViewModelContext;
use crate::foundation::form::{Form, FormSnapshot, ValidationErrors};
use crate::foundation::task::{async_task_channel, Tasks};
use crate::foundation::view_model::{CommandContext, ValueCommand};
use crate::{dialog::Dialogs, log::Log, notification::Notifications};
use std::time::{Duration, Instant};

fn context() -> ViewModelContext {
    ViewModelContext::for_benchmarks()
}

#[test]
fn field_collects_multiple_errors_in_registration_order() {
    let ctx = context();
    let form = Form::new(&ctx);
    let email = form
        .text_field("email", "")
        .validator(|value| {
            if value.trim().is_empty() {
                ValidationErrors::single("required")
            } else {
                ValidationErrors::none()
            }
        })
        .validator(|value| {
            if value.contains('@') {
                ValidationErrors::none()
            } else {
                ValidationErrors::multiple(["missing @", "invalid format"])
            }
        });

    assert!(!email.validate());
    assert_eq!(
        email.errors().get(),
        vec![
            "required".to_string(),
            "missing @".to_string(),
            "invalid format".to_string()
        ]
    );
}

#[test]
fn form_validate_aggregates_errors_by_field_name() {
    let ctx = context();
    let form = Form::new(&ctx);
    let email = form.text_field("email", "").validator(|value| {
        if value.trim().is_empty() {
            ValidationErrors::single("required")
        } else {
            ValidationErrors::none()
        }
    });
    let agree = form.field("agree", false).validator(|value| {
        if *value {
            ValidationErrors::none()
        } else {
            ValidationErrors::single("must agree")
        }
    });

    assert!(!form.validate());
    assert_eq!(email.first_error().get(), Some("required".to_string()));
    assert_eq!(agree.first_error().get(), Some("must agree".to_string()));
    assert_eq!(
        form.errors().get(),
        [
            ("agree".to_string(), vec!["must agree".to_string()]),
            ("email".to_string(), vec!["required".to_string()]),
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn submit_returns_typed_snapshot_values_and_errors() {
    let ctx = context();
    let form = Form::new(&ctx);
    let name = form.text_field("name", "alice").validator(|value| {
        if value.len() >= 3 {
            ValidationErrors::none()
        } else {
            ValidationErrors::single("too short")
        }
    });
    let age = form.field("age", 18_u32);

    name.set_text("al");
    age.set(24);

    let snapshot = form.submit();

    assert!(!snapshot.is_valid());
    assert_eq!(snapshot.get::<String>("name"), Some("al".to_string()));
    assert_eq!(snapshot.get::<u32>("age"), Some(24));
    assert_eq!(
        snapshot.errors_for("name"),
        Some(&["too short".to_string()][..])
    );
    assert_eq!(snapshot.errors_for("age"), None);
}

#[test]
fn reset_restores_initial_values_and_clears_errors() {
    let ctx = context();
    let form = Form::new(&ctx);
    let title = form.text_field("title", "draft").validator(|value| {
        if value.trim().is_empty() {
            ValidationErrors::single("required")
        } else {
            ValidationErrors::none()
        }
    });
    let published = form.field("published", false).validator(|value| {
        if *value {
            ValidationErrors::none()
        } else {
            ValidationErrors::single("must publish")
        }
    });

    title.set_text("");
    published.set(true);
    form.validate();

    form.reset();

    assert_eq!(title.text(), "draft".to_string());
    assert!(!published.get());
    assert!(title.errors().get().is_empty());
    assert!(published.errors().get().is_empty());
    assert!(form.errors().get().is_empty());
}

#[test]
fn bind_change_updates_field_value() {
    let ctx = context();
    let form = Form::new(&ctx);
    let field = form.field("count", 1_i32);
    let command = field.bind_change::<()>();
    let mut vm = ();

    command.execute(&mut vm, 42);

    assert_eq!(field.get(), 42);
}

#[test]
#[should_panic(expected = "form field `email` is already registered")]
fn duplicate_field_names_panic() {
    let ctx = context();
    let form = Form::new(&ctx);

    let _ = form.text_field("email", "");
    let _ = form.field("email", false);
}

fn task_context_and_receiver<VM: 'static>() -> (
    CommandContext<VM>,
    crate::foundation::task::AsyncTaskReceiver<VM>,
) {
    let (dispatcher, receiver) = async_task_channel();
    (
        CommandContext::new(
            Dialogs::detached(),
            Notifications::detached(),
            Tasks::from_runtime("main".to_string(), 1, dispatcher),
            Default::default(),
            Log::default(),
            crate::foundation::binding::InvalidationSignal::new(),
        ),
        receiver,
    )
}

fn drain_task_receiver_until<VM: 'static>(
    receiver: &crate::foundation::task::AsyncTaskReceiver<VM>,
    vm: &mut VM,
    context: &CommandContext<VM>,
    done: impl Fn() -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        for completion in receiver.try_iter() {
            (completion.callback)(vm, context);
        }
        if done() {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for form async task");
}

#[test]
fn async_validation_marks_pending_and_applies_errors() {
    let ctx = context();
    let form = Form::new(&ctx);
    let username = form
        .text_field("username", "taken")
        .async_validator(|value| {
            if value == "taken" {
                ValidationErrors::single("already taken")
            } else {
                ValidationErrors::none()
            }
        });
    let command = form.validate_async_command::<()>();
    let (context, receiver) = task_context_and_receiver();
    let mut vm = ();

    command.execute_with_context(&mut vm, &context);

    assert!(form.is_validating().get());
    assert!(username.is_validating().get());
    assert!(username.validation_state().get().pending);

    drain_task_receiver_until(&receiver, &mut vm, &context, || !form.is_validating().get());

    assert!(!username.is_validating().get());
    assert_eq!(
        username.first_error().get(),
        Some("already taken".to_string())
    );
    assert!(username.validation_state().get().invalid);
    assert_eq!(
        form.errors().get().get("username"),
        Some(&vec!["already taken".to_string()])
    );
}

#[derive(Default)]
struct SubmitVm {
    submitted_name: Option<String>,
}

#[test]
fn async_submit_runs_callback_only_when_valid() {
    let ctx = context();
    let form = Form::new(&ctx);
    let name = form.text_field("name", "alice").async_validator(|value| {
        if value == "alice" {
            ValidationErrors::none()
        } else {
            ValidationErrors::single("not alice")
        }
    });
    let on_submit = ValueCommand::new(|vm: &mut SubmitVm, snapshot: FormSnapshot| {
        vm.submitted_name = snapshot.get::<String>("name");
    });
    let command = form.submit_async_command(on_submit);
    let (context, receiver) = task_context_and_receiver();
    let mut vm = SubmitVm::default();

    command.execute_with_context(&mut vm, &context);
    assert!(form.is_submitting().get());

    drain_task_receiver_until(&receiver, &mut vm, &context, || !form.is_submitting().get());

    assert_eq!(vm.submitted_name, Some("alice".to_string()));
    assert!(form.errors().get().is_empty());
    assert!(!name.validation_state().get().invalid);
}
