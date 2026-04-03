use tgui::{
    children, Align, Application, Color, Column, Command, Element, Input, Observable, Stack, Text,
    ValueCommand, ViewModelContext,
};

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .with_view_model(App::new)
        .root_view(App::view)
        .run()
}

struct App {
    input_value: Observable<String>,
}

impl App {
    fn new(context: &ViewModelContext) -> App {
        App {
            input_value: context.observable(String::new()),
        }
    }

    fn view(&self) -> Element<Self> {
        let text = Text::new(self.input_value.binding());

        let input = Input::new(text)
            .border_radius(6.0)
            .placeholder_with_str("Please select a value")
            .on_change(ValueCommand::new(|app: &mut App, value: String| {
                app.input_value.set(value);
            }))
            .on_focus(Command::new(|_| println!("Input focused")))
            .on_blur(Command::new(|_| println!("Input blurred")));

        Column::new()
            .fill_size()
            .align(Align::Center)
            .child(input)
            .gap(50.0)
            .into()
    }
}
