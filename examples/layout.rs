use tgui::{Align, Application, Axis, Color, Column, Element, Flex, Text, TguiError, ViewModelContext, Wrap};

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
            .gap(20.0)
            .wrap(Wrap::Wrap)
            .child(Self::column_layout(Align::Start, "Align::Start".to_string()))
            .child(Self::column_layout(Align::Center, "Align::Center".to_string()))
            .child(Self::column_layout(Align::End, "Align::End".to_string()))
            .child(Self::column_layout(Align::Stretch, "Align::Stretch".to_string()))
            .into()
    }


    fn column_layout(align: Align, align_text: String) -> Element<Self> {
        Column::new()
            .size(200.0, 200.0)
            .background(Color::WHITE)
            .align(align)
            .child(
                Text::new(align_text).color(Color::BLACK)
            ).into()
    }

}
