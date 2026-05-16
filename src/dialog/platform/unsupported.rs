use std::path::PathBuf;

use super::{DialogParentHandles, FileDialogRequest};
use crate::dialog::types::{
    DialogError, FileDialogOptions, MessageDialogOptions, MessageDialogResult,
};

pub(crate) fn run_file_dialog_path(
    request: FileDialogRequest,
    options: FileDialogOptions,
    parent: Option<&DialogParentHandles>,
) -> Result<Option<PathBuf>, DialogError> {
    let _ = (request, options, parent);
    Err(DialogError::UnsupportedPlatform)
}

pub(crate) fn run_file_dialog_paths(
    request: FileDialogRequest,
    options: FileDialogOptions,
    parent: Option<&DialogParentHandles>,
) -> Result<Option<Vec<PathBuf>>, DialogError> {
    let _ = (request, options, parent);
    Err(DialogError::UnsupportedPlatform)
}

pub(crate) fn run_message_dialog(
    options: MessageDialogOptions,
    parent: Option<&DialogParentHandles>,
) -> Result<MessageDialogResult, DialogError> {
    let _ = (options, parent);
    Err(DialogError::UnsupportedPlatform)
}
