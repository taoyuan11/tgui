use tgui::{
    children, Align, Application, Column, Element, Input, Observable, Text, ValueCommand,
    ViewModelContext,
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

        Column::new()
            .fill_size()
            .align(Align::Center)
            .child(children![Input::new(text)
                .placeholder_with_str("Please select a value")
                .on_change(ValueCommand::new(
                    |app: &mut App, value: String| {
                        app.input_value.set(value);
                    }
                ))])
            .into()
    }
}
