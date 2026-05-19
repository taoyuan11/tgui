//! Android dialog 后端：同步入口返回 Unsupported，异步入口通过
//! `android_bridge` 把请求送进 JNI 桥接，再由 JNI 回调把结果交回
//! `AsyncDialogDispatcher`，最终在 ViewModel 上执行回调。

use std::path::PathBuf;

use super::android_bridge::{
    self, discard_slot, dispatch_file, dispatch_message, BridgeResult, FILE_OPEN, FILE_OPEN_MULTI,
    FILE_PICK_FOLDER, FILE_SAVE,
};
use super::FileDialogRequest;
use crate::dialog::types::{
    DialogError, FileDialogOptions, MessageDialogOptions, MessageDialogResult,
};

/// 启动 AlertDialog；`on_done` 会在 JNI 回调线程中被调用一次。
pub(crate) fn dispatch_message_async<F>(
    options: MessageDialogOptions,
    on_done: F,
) -> Result<(), DialogError>
where
    F: FnOnce(Result<MessageDialogResult, DialogError>) + Send + 'static,
{
    let request_id = android_bridge::alloc_request_id();
    android_bridge::install_slot(
        request_id,
        Box::new(move |result| match result {
            BridgeResult::Message(value) => on_done(Ok(value)),
            BridgeResult::File { .. } => on_done(Err(DialogError::Backend(
                "bridge returned file result for message request".to_string(),
            ))),
        }),
    );
    dispatch_message(
        request_id,
        options.title.clone(),
        options.description.clone(),
        options.buttons,
    )
    .inspect_err(|_| discard_slot(request_id))
}

/// 启动单选 / 单目录 / 保存对话框。
pub(crate) fn dispatch_file_async_path<F>(
    request: FileDialogRequest,
    options: FileDialogOptions,
    on_done: F,
) -> Result<(), DialogError>
where
    F: FnOnce(Result<Option<PathBuf>, DialogError>) + Send + 'static,
{
    let kind = match request {
        FileDialogRequest::OpenFile => FILE_OPEN,
        FileDialogRequest::PickFolder => FILE_PICK_FOLDER,
        FileDialogRequest::SaveFile => FILE_SAVE,
        FileDialogRequest::OpenFiles | FileDialogRequest::PickFolders => {
            return Err(DialogError::Backend(
                "internal dialog request kind mismatch for single-path result".to_string(),
            ));
        }
    };
    let request_id = android_bridge::alloc_request_id();
    android_bridge::install_slot(
        request_id,
        Box::new(move |result| match result {
            BridgeResult::File { ok, uris } => {
                if ok {
                    on_done(Ok(uris.into_iter().next().map(PathBuf::from)));
                } else {
                    on_done(Ok(None));
                }
            }
            BridgeResult::Message(_) => on_done(Err(DialogError::Backend(
                "bridge returned message result for file request".to_string(),
            ))),
        }),
    );
    let mime_types = collect_mime_types(&options);
    dispatch_file(
        request_id,
        kind,
        options.title,
        options.file_name,
        mime_types,
    )
    .inspect_err(|_| discard_slot(request_id))
}

/// 启动多选文件 / 多目录对话框。
pub(crate) fn dispatch_file_async_paths<F>(
    request: FileDialogRequest,
    options: FileDialogOptions,
    on_done: F,
) -> Result<(), DialogError>
where
    F: FnOnce(Result<Option<Vec<PathBuf>>, DialogError>) + Send + 'static,
{
    let kind = match request {
        FileDialogRequest::OpenFiles => FILE_OPEN_MULTI,
        // SAF 没有原生的多目录选择能力，复用 ACTION_OPEN_DOCUMENT_TREE，返回单个 tree URI。
        FileDialogRequest::PickFolders => FILE_PICK_FOLDER,
        FileDialogRequest::OpenFile
        | FileDialogRequest::PickFolder
        | FileDialogRequest::SaveFile => {
            return Err(DialogError::Backend(
                "internal dialog request kind mismatch for multi-path result".to_string(),
            ));
        }
    };
    let request_id = android_bridge::alloc_request_id();
    android_bridge::install_slot(
        request_id,
        Box::new(move |result| match result {
            BridgeResult::File { ok, uris } => {
                if ok && !uris.is_empty() {
                    on_done(Ok(Some(uris.into_iter().map(PathBuf::from).collect())));
                } else {
                    on_done(Ok(None));
                }
            }
            BridgeResult::Message(_) => on_done(Err(DialogError::Backend(
                "bridge returned message result for file request".to_string(),
            ))),
        }),
    );
    let mime_types = collect_mime_types(&options);
    dispatch_file(
        request_id,
        kind,
        options.title,
        options.file_name,
        mime_types,
    )
    .inspect_err(|_| discard_slot(request_id))
}

fn collect_mime_types(options: &FileDialogOptions) -> Vec<String> {
    let mut mimes: Vec<String> = Vec::new();
    for filter in &options.filters {
        for ext in &filter.extensions {
            if let Some(mime) = extension_to_mime(ext) {
                if !mimes.iter().any(|existing| existing == mime) {
                    mimes.push(mime.to_string());
                }
            }
        }
    }
    mimes
}

fn extension_to_mime(ext: &str) -> Option<&'static str> {
    let trimmed = ext.trim_start_matches('.').to_ascii_lowercase();
    Some(match trimmed.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" | "log" => "text/plain",
        "md" => "text/markdown",
        "json" => "application/json",
        "xml" => "application/xml",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "zip" => "application/zip",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        _ => return None,
    })
}

// 公开给上层装载用。
pub(crate) use android_bridge::install_android_app;
