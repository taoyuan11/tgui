use tgui::Application;

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .title("tgui milestone 1")
        .window_size(960, 640)
        .run()
}
