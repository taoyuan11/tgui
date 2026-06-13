use tgui::{core::TguiError, layout::Stack, mvvm::ViewModel, prelude::Application};

struct App {}

impl ViewModel for App {
    fn new(context: &tgui::prelude::ViewModelContext) -> Self {
        Self {}
    }

    fn view(&self) -> tgui::prelude::Element<Self>
    where
        Self: Sized,
    {
        Stack::new().into()
    }
}

fn main() -> Result<(), TguiError> {
    println!("Starting minimal tgui test...");
    Application::new()
        .with_view_model(App::new)
        .root_view(App::view)
        .run()
}
