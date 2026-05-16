use std::path::PathBuf;

use super::{DialogParentHandles, FileDialogRequest};
use crate::dialog::types::{
    DialogError, FileDialogOptions, MessageDialogButtons, MessageDialogLevel, MessageDialogOptions,
    MessageDialogResult,
};

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

pub(crate) fn run_file_dialog_path(
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

pub(crate) fn run_file_dialog_paths(
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

pub(crate) fn run_message_dialog(
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
