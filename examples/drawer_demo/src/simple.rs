use tgui::application::Application;
use tgui::layout::{Axis, Insets};
use tgui::mvvm::{Command, State, ValueCommand, ViewModel, ViewModelContext};
use tgui::prelude::Dp;
use tgui::widgets::{Button, Drawer, DrawerPlacement, Element, Flex, Text};

struct SimpleDrawerVm {
    drawer_open: State<bool>,
}

impl SimpleDrawerVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            drawer_open: ctx.state(false),
        }
    }

    fn toggle_drawer(&mut self) {
        self.drawer_open.update(|open| *open = !*open);
    }

    fn view(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .gap(16.0)
            .padding(Insets::all(Dp(24.0)))
            .child(Text::new("简化的 Drawer 测试"))
            .child(
                Button::new("打开 Drawer")
                    .on_click(Command::new(Self::toggle_drawer)),
            )
            .child(
                Drawer::new(self.drawer_open.signal())
                    .placement(DrawerPlacement::Left)
                    .on_open_change(ValueCommand::new(|vm: &mut Self, open| {
                        vm.drawer_open.set(open);
                    }))
                    .content({
                        let content: Element<Self> = Flex::new(Axis::Vertical)
                            .gap(16.0)
                            .child(Text::new("Drawer 内容"))
                            .child(
                                Button::new("关闭")
                                    .on_click(Command::new(Self::toggle_drawer)),
                            )
                            .into();
                        content
                    }),
            )
            .into()
    }
}

impl ViewModel for SimpleDrawerVm {
    fn new(ctx: &ViewModelContext) -> Self {
        SimpleDrawerVm::new(ctx)
    }

    fn view(&self) -> Element<Self> {
        SimpleDrawerVm::view(self)
    }
}

fn main() -> Result<(), tgui::core::TguiError> {
    Application::new()
        .app_id("com.example.simple_drawer")
        .with_view_model(SimpleDrawerVm::new)
        .root_view(SimpleDrawerVm::view)
        .run()
}
