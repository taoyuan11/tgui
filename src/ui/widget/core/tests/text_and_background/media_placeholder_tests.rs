use super::*;

#[test]
fn image_loading_placeholder_uses_image_background() {
    let background = Color::hexa(0x11223344);

    assert_eq!(
        super::media_loading_fill_color(true, None, background, true),
        background
    );
}

#[test]
fn image_loading_placeholder_defaults_to_transparent_white() {
    assert_eq!(
        super::media_loading_fill_color(true, None, Color::rgba(255, 255, 255, 0), true),
        Color::rgba(255, 255, 255, 0)
    );
}

#[test]
fn image_error_placeholder_keeps_error_color() {
    assert_eq!(
        super::media_loading_fill_color(false, Some("boom"), Color::WHITE, false),
        crate::media::media_placeholder_color(false, Some("boom"))
    );
}

#[test]
fn idle_media_placeholder_keeps_default_placeholder_color() {
    let background = Color::hexa(0xABCDEF12);

    assert_eq!(
        super::media_loading_fill_color(false, None, background, false),
        crate::media::media_placeholder_color(false, None)
    );
}
