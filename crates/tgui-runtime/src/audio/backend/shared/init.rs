use std::fmt::Display;
use std::sync::OnceLock;

use crate::foundation::error::TguiError;

static FFMPEG_INIT_RESULT: OnceLock<Result<(), String>> = OnceLock::new();

pub(crate) fn ensure_ffmpeg_initialized() -> Result<(), TguiError> {
    ensure_ffmpeg_initialized_with(&FFMPEG_INIT_RESULT, ffmpeg_next::init)
}

fn ensure_ffmpeg_initialized_with<E, F>(
    cache: &OnceLock<Result<(), String>>,
    init: F,
) -> Result<(), TguiError>
where
    E: Display,
    F: FnOnce() -> Result<(), E>,
{
    cache
        .get_or_init(|| init().map_err(|error| error.to_string()))
        .as_ref()
        .map(|_| ())
        .map_err(|message| TguiError::Media(format!("failed to initialize FFmpeg: {message}")))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::OnceLock;

    use crate::foundation::error::TguiError;

    use super::ensure_ffmpeg_initialized_with;

    #[test]
    fn init_failure_is_cached_without_retry() {
        let cache = OnceLock::new();
        let calls = AtomicUsize::new(0);

        let first = ensure_ffmpeg_initialized_with(&cache, || {
            calls.fetch_add(1, Ordering::SeqCst);
            Err("boom")
        });
        let second = ensure_ffmpeg_initialized_with(&cache, || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<(), &str>(())
        });

        assert!(matches!(
            first,
            Err(TguiError::Media(message))
                if message == "failed to initialize FFmpeg: boom"
        ));
        assert!(matches!(
            second,
            Err(TguiError::Media(message))
                if message == "failed to initialize FFmpeg: boom"
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn init_success_is_cached_without_retry() {
        let cache = OnceLock::new();
        let calls = AtomicUsize::new(0);

        let first = ensure_ffmpeg_initialized_with(&cache, || {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<(), &str>(())
        });
        let second = ensure_ffmpeg_initialized_with(&cache, || {
            calls.fetch_add(1, Ordering::SeqCst);
            Err("boom")
        });

        assert!(first.is_ok());
        assert!(second.is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
