use super::{Application, MsaaMode, WindowSpec};

#[test]
fn application_decorations_updates_config() {
    let config = Application::new().decorations(false).config();

    assert!(!config.decorations);
}

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
fn application_defaults_to_auto_msaa() {
    let config = Application::new().config();

    assert_eq!(config.msaa, MsaaMode::Auto);
}

#[test]
fn window_spec_msaa_overrides_application_default() {
    let app_config = Application::new().msaa(MsaaMode::X4).config();
    let window_config = WindowSpec::<()>::main("main")
        .msaa(MsaaMode::Off)
        .resolved_config(&app_config);

    assert_eq!(window_config.msaa, MsaaMode::Off);
}
