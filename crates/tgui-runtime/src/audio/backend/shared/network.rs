use ffmpeg_next as ffmpeg;

use crate::foundation::error::TguiError;

pub(crate) enum PacketRead {
    Packet(ffmpeg::Packet),
    Retry,
    Eof,
}

pub(crate) fn read_ffmpeg_packet(
    kind: &str,
    input: &mut ffmpeg::format::context::Input,
) -> Result<PacketRead, TguiError> {
    let mut packet = ffmpeg::Packet::empty();
    match packet.read(input) {
        Ok(()) => Ok(PacketRead::Packet(packet)),
        Err(ffmpeg::Error::Eof) => Ok(PacketRead::Eof),
        Err(ffmpeg::Error::Other {
            errno: ffmpeg::error::EAGAIN,
        }) => Ok(PacketRead::Retry),
        Err(error) => Err(TguiError::Media(format!(
            "failed to read {kind} packet: {error}"
        ))),
    }
}

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
    options.set("rw_timeout", "15000000");
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
