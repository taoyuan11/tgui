use tgui::{Align, Application, Axis, Column, Element, Flex, Text, TguiError, ViewModelContext};

fn main() -> Result<(), TguiError> {
    Application::new()
        .title("Layout Demo")
        .with_view_model(LayoutDemo::new)
        .root_view(LayoutDemo::view)
        .run()
}

struct LayoutDemo {}

impl LayoutDemo {

    fn new(_: &ViewModelContext) -> Self {
        Self{}
    }

    fn view(&self) -> Element<Self> {
        Flex::new(Axis::Horizontal)
            .fill_size()
            .child(Self::column_layout())
            .into()
    }


    fn column_layout() -> Element<Self> {
        Column::new()
            .size(200.0, 200.0)
            .background(wgpu::Color::WHITE)
            .align(Align::End)
            .child(
                Text::new("Align::Center".to_string()).color(wgpu::Color::BLACK)
            ).into()
    }

}