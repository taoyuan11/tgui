use super::{Application, MsaaMode, WindowSpec};
use crate::ui::layout::Insets;
use crate::ui::unit::dp;

#[test]
fn window_spec_decorations_override_application_default() {
    let app_config = Application::new().decorations(true).config();
    let window_config = WindowSpec::<()>::main("main")
        .decorations(false)
        .resolved_config(&app_config);

    assert!(!window_config.decorations);
}

#[test]
fn window_spec_decorations_inherit_application_default() {
    let app_config = Application::new().decorations(false).config();
    let window_config = WindowSpec::<()>::main("main").resolved_config(&app_config);

    assert!(!window_config.decorations);
}

#[test]
fn application_defaults_to_off_msaa() {
    let config = Application::new().config();

    assert_eq!(config.msaa, MsaaMode::Off);
}

#[test]
fn window_spec_msaa_overrides_application_default() {
    let app_config = Application::new().msaa(MsaaMode::X4).config();
    let window_config = WindowSpec::<()>::main("main")
        .msaa(MsaaMode::Off)
        .resolved_config(&app_config);

    assert_eq!(window_config.msaa, MsaaMode::Off);
}

#[test]
fn application_viewport_insets_default_to_zero() {
    let config = Application::new().config();

    assert_eq!(config.viewport_insets, Insets::ZERO);
}

#[test]
fn window_spec_viewport_insets_override_application_default() {
    let app_config = Application::new()
        .viewport_insets(Insets::top(dp(24.0)))
        .config();
    let window_config = WindowSpec::<()>::main("main")
        .viewport_insets(Insets::bottom(dp(12.0)))
        .resolved_config(&app_config);

    assert_eq!(window_config.viewport_insets, Insets::bottom(dp(12.0)));
}
