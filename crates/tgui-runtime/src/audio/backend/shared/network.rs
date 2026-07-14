use std::time::Duration;

use ffmpeg_next as ffmpeg;

use crate::foundation::error::TguiError;

const FFMPEG_NETWORK_IO_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) fn validate_ffmpeg_headers(
    kind: &str,
    headers: &[(String, String)],
) -> Result<(), TguiError> {
    for (name, value) in headers {
        if name.is_empty() {
            return Err(TguiError::Media(format!(
                "{kind} header name cannot be empty"
            )));
        }
        if name.contains(['\r', '\n']) {
            return Err(TguiError::Media(format!(
                "{kind} header {name:?} contains an invalid line break"
            )));
        }
        if value.contains(['\r', '\n']) {
            return Err(TguiError::Media(format!(
                "{kind} header value for {name:?} contains an invalid line break"
            )));
        }
    }

    Ok(())
}

fn ffmpeg_timeout_micros(duration: Duration) -> String {
    duration.as_micros().min(i64::MAX as u128).to_string()
}

pub(crate) fn ffmpeg_http_input_options(
    kind: &str,
    headers: &[(String, String)],
) -> Result<ffmpeg::Dictionary<'static>, TguiError> {
    let mut options = ffmpeg::Dictionary::new();
    validate_ffmpeg_headers(kind, headers)?;

    let custom_headers = (!headers.is_empty()).then(|| {
        headers
            .iter()
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect::<String>()
    });

    let has_custom_user_agent = headers
        .iter()
        .any(|(name, _)| name.trim().eq_ignore_ascii_case("user-agent"));

    if !has_custom_user_agent {
        options.set("user_agent", concat!("tgui/", env!("CARGO_PKG_VERSION")));
    }
    options.set("multiple_requests", "1");
    options.set("short_seek_size", "65536");
    options.set("reconnect", "1");
    options.set("reconnect_streamed", "1");
    options.set("reconnect_on_network_error", "1");
    options.set("reconnect_on_http_error", "4xx,5xx");
    options.set("reconnect_delay_max", "2");
    let io_timeout = ffmpeg_timeout_micros(FFMPEG_NETWORK_IO_TIMEOUT);
    options.set("timeout", &io_timeout);
    options.set("rw_timeout", &io_timeout);
    if let Some(headers) = custom_headers {
        options.set("headers", &headers);
    }
    Ok(options)
}

pub(crate) fn open_ffmpeg_input(
    kind: &str,
    source_url: &str,
    headers: Option<&[(String, String)]>,
) -> Result<ffmpeg::format::context::Input, TguiError> {
    match headers {
        Some(headers) => ffmpeg::format::input_with_dictionary(
            source_url,
            ffmpeg_http_input_options(kind, headers)?,
        )
        .map_err(|error| TguiError::Media(format!("failed to open {kind} source: {error}"))),
        None => ffmpeg::format::input(source_url)
            .map_err(|error| TguiError::Media(format!("failed to open {kind} source: {error}"))),
    }
}
