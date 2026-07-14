use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::foundation::error::TguiError;

static NEXT_TEMP_MEDIA_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) struct TemporaryMediaFile {
    path: PathBuf,
}

impl TemporaryMediaFile {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryMediaFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(crate) fn media_path_to_url(kind: &str, path: &Path) -> Result<String, TguiError> {
    path.to_str()
        .ok_or_else(|| TguiError::Media(format!("{kind} path is not valid UTF-8")))
        .map(ToString::to_string)
}

pub(crate) fn create_temporary_media_file(
    kind: &str,
    bytes: &Arc<[u8]>,
    extension: Option<&str>,
) -> Result<TemporaryMediaFile, TguiError> {
    if bytes.is_empty() {
        return Err(TguiError::Media(format!("{kind} bytes source is empty")));
    }

    let extension = sanitized_extension(extension).unwrap_or_else(|| "bin".to_string());
    for _ in 0..128 {
        let id = NEXT_TEMP_MEDIA_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tgui-{kind}-{}-{id}.{extension}",
            std::process::id()
        ));
        let file = OpenOptions::new().write(true).create_new(true).open(&path);
        let mut file = match file {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(TguiError::Media(format!(
                    "failed to create temporary {kind} source: {error}"
                )));
            }
        };

        if let Err(error) = file.write_all(bytes) {
            let _ = fs::remove_file(&path);
            return Err(TguiError::Media(format!(
                "failed to write temporary {kind} source: {error}"
            )));
        }
        return Ok(TemporaryMediaFile { path });
    }

    Err(TguiError::Media(format!(
        "failed to allocate a unique temporary {kind} source path"
    )))
}

fn sanitized_extension(extension: Option<&str>) -> Option<String> {
    let extension = extension?.trim().trim_start_matches('.');
    let sanitized = extension
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(16)
        .collect::<String>();
    (!sanitized.is_empty()).then_some(sanitized)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{create_temporary_media_file, media_path_to_url};

    #[test]
    fn temporary_media_file_writes_bytes_and_cleans_up_on_drop() {
        let bytes: Arc<[u8]> = Arc::from(vec![1, 2, 3, 4]);
        let file = create_temporary_media_file("audio", &bytes, Some(".m4a")).expect("temp file");
        let path = file.path().to_path_buf();

        assert_eq!(std::fs::read(&path).expect("temp bytes"), vec![1, 2, 3, 4]);
        assert!(path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".m4a")));
        assert!(media_path_to_url("audio", &path).is_ok());

        drop(file);

        assert!(!path.exists());
    }

    #[test]
    fn temporary_media_file_rejects_empty_bytes() {
        let bytes: Arc<[u8]> = Arc::from(Vec::<u8>::new());

        assert!(create_temporary_media_file("video", &bytes, None).is_err());
    }

    #[test]
    fn temporary_media_file_sanitizes_extension_hint() {
        let bytes: Arc<[u8]> = Arc::from(vec![1]);
        let file =
            create_temporary_media_file("video", &bytes, Some("../mp4!")).expect("temp file");
        let path = file.path().to_path_buf();

        assert!(path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".mp4")));
    }
}
