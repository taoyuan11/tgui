use tgui::{el, Align, Application, Axis, Button, Color, Command, Element, Flex, Image, Observable, Text, TguiError, ViewModelContext, dp};

struct App {
    image_source: Observable<String>
}

impl App {

    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            image_source: ctx.observable(String::from("http://47.109.31.100:19900/test.svg"))
        }
    }

    fn view(&self) -> Element<Self> {
        let image1 =
            // Image::from_bytes(include_bytes!("static/juequling_shushu.jpg"))
        Image::from_url("http://47.109.31.100:19900/juequling_shushu.jpg")
            .background(Color::WHITE)
            .size(dp(200.0), dp(200.0));
        // let image2 =
        //     // Image::from_bytes(include_bytes!("static/wlop.jpg"))
        // Image::from_url("http://47.109.31.100:19900/wlop.jpg")
        //     .background(Color::WHITE)
        //     .size(200.0, 200.0);
        let image3 =
            // Image::from_bytes(include_bytes!("static/vvv.svg"))
        Image::from_url("http://47.109.31.100:19900/test.svg")
            .background(Color::rgba(255, 255, 255, 0))
            .size(dp(200.0), dp(200.0));

        let image4 =
            Image::from_url(self.image_source.binding())
                .background(Color::WHITE)
                .size(dp(200.0), dp(200.0));


        let btn = Button::new(
            Text::new("切换图片")
        )
            .on_click(Command::new(|app: &mut App| {
                app.image_source.set(String::from("http://47.109.31.100:19900/wlop.jpg"))
            }));

        Flex::new(Axis::Horizontal)
            .align(Align::Center)
            .gap(dp(20.0))
            .child(el![image1, image3, image4, btn])
            .into()
    }

}

fn main() -> Result<(), TguiError> {
    Application::new()
        .with_view_model(App::new)
        .root_view(App::view)
        .run()
}
