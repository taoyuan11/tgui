use super::*;
use crate::ui::widget::FileDropEvent;
use std::path::PathBuf;

#[derive(Default)]
struct FileDropVm {
    dropped_paths: Vec<PathBuf>,
    drop_position: Option<Point>,
}

impl ViewModel for FileDropVm {
    fn new(_: &ViewModelContext) -> Self {
        Self::default()
    }

    fn view(&self) -> Element<Self> {
        Stack::new().into()
    }
}

impl FileDropVm {
    fn files_dropped(&mut self, event: FileDropEvent) {
        self.drop_position = Some(event.position);
        self.dropped_paths = event.paths;
    }
}

#[test]
fn drag_dropped_dispatches_to_topmost_file_drop_handler() {
    let invalidation = InvalidationSignal::new();
    let drop_zone: Element<FileDropVm> = Stack::new()
        .size(dp(200.0), dp(120.0))
        .on_file_drop(ValueCommand::new(FileDropVm::files_dropped))
        .into();
    let mut handler = test_handler_with_vm(
        FileDropVm::default(),
        Some(WidgetTree::new(drop_zone)),
        invalidation,
    );
    let event_loop = TestEventLoop;
    let paths = vec![
        PathBuf::from("/tmp/report.pdf"),
        PathBuf::from("/tmp/image.png"),
    ];

    let handled = handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::DragDropped {
            paths: paths.clone(),
            position: PhysicalPosition::new(24.0, 36.0),
        },
    );

    assert!(!handled);
    let vm = handler.view_model.lock().unwrap();
    assert_eq!(vm.dropped_paths, paths);
    assert_eq!(vm.drop_position, Some(Point::new(dp(24.0), dp(36.0))));
}

#[test]
fn drag_dropped_ignores_widgets_without_file_drop_handler() {
    let invalidation = InvalidationSignal::new();
    let root: Element<FileDropVm> = Stack::new().size(dp(200.0), dp(120.0)).into();
    let mut handler = test_handler_with_vm(
        FileDropVm::default(),
        Some(WidgetTree::new(root)),
        invalidation,
    );
    let event_loop = TestEventLoop;

    let handled = handler.handle_bound_window_event(
        &event_loop,
        WindowEvent::DragDropped {
            paths: vec![PathBuf::from("/tmp/report.pdf")],
            position: PhysicalPosition::new(24.0, 36.0),
        },
    );

    assert!(!handled);
    let vm = handler.view_model.lock().unwrap();
    assert!(vm.dropped_paths.is_empty());
    assert_eq!(vm.drop_position, None);
}
