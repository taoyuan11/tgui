use tgui::application::Application;
use tgui::layout::{Axis, Insets};
use tgui::logging::{tgui_log, LogLevel};
use tgui::mvvm::{Command, State, ValueCommand, ViewModel, ViewModelContext};
use tgui::prelude::Dp;
use tgui::widgets::{Button, Drawer, DrawerPlacement, Element, Flex, Text};

struct DrawerDemoVm {
    left_drawer_open: State<bool>,
    right_drawer_open: State<bool>,
    top_drawer_open: State<bool>,
    bottom_drawer_open: State<bool>,
}

impl DrawerDemoVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            left_drawer_open: ctx.state(false),
            right_drawer_open: ctx.state(false),
            top_drawer_open: ctx.state(false),
            bottom_drawer_open: ctx.state(false),
        }
    }

    fn toggle_left_drawer(&mut self) {
        tgui_log(LogLevel::Info, "toggle_left_drawer");
        self.left_drawer_open.update(|open| *open = !*open);
    }

    fn toggle_right_drawer(&mut self) {
        tgui_log(LogLevel::Info, "toggle_right_drawer");
        self.right_drawer_open.update(|open| *open = !*open);
    }

    fn toggle_top_drawer(&mut self) {
        tgui_log(LogLevel::Info, "toggle_top_drawer");
        self.top_drawer_open.update(|open| *open = !*open);
    }

    fn toggle_bottom_drawer(&mut self) {
        tgui_log(LogLevel::Info, "toggle_bottom_drawer");
        self.bottom_drawer_open.update(|open| *open = !*open);
    }

    fn view(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .gap(16.0)
            .padding(Insets::all(Dp(24.0)))
            .child(Text::new("Drawer / Sidebar 示例"))
            .child(Text::new("点击按钮从不同方向打开侧边栏"))
            .child(
                Flex::new(Axis::Horizontal)
                    .gap(12.0)
                    .child(
                        Button::new("打开左侧抽屉")
                            .on_click(Command::new(Self::toggle_left_drawer)),
                    )
                    .child(
                        Button::new("打开右侧抽屉")
                            .on_click(Command::new(Self::toggle_right_drawer)),
                    )
                    .child(
                        Button::new("打开顶部抽屉").on_click(Command::new(Self::toggle_top_drawer)),
                    )
                    .child(
                        Button::new("打开底部抽屉")
                            .on_click(Command::new(Self::toggle_bottom_drawer)),
                    ),
            )
            .child(Text::new("提示："))
            .child(Text::new("• 按 Esc 键关闭抽屉"))
            .child(Text::new("• 点击遮罩层关闭抽屉"))
            .child(Text::new("• Tab 键在抽屉内循环聚焦"))
            // Left Drawer
            .child(
                Drawer::new(self.left_drawer_open.signal())
                    .placement(DrawerPlacement::Left)
                    .on_open_change(ValueCommand::new(|vm: &mut Self, open| {
                        vm.left_drawer_open.set(open);
                    }))
                    .content({
                        let content: Element<Self> = Flex::new(Axis::Vertical)
                            .gap(16.0)
                            .child(Text::new("左侧导航"))
                            .child(Button::new("首页"))
                            .child(Button::new("设置"))
                            .child(Button::new("关于"))
                            .child(
                                Button::new("关闭")
                                    .on_click(Command::new(Self::toggle_left_drawer)),
                            )
                            .into();
                        content
                    }),
            )
            // Right Drawer
            .child(
                Drawer::new(self.right_drawer_open.signal())
                    .placement(DrawerPlacement::Right)
                    .on_open_change(ValueCommand::new(|vm: &mut Self, open| {
                        vm.right_drawer_open.set(open);
                    }))
                    .content({
                        let content: Element<Self> = Flex::new(Axis::Vertical)
                            .gap(16.0)
                            .child(Text::new("右侧面板"))
                            .child(Text::new("这是右侧抽屉的内容"))
                            .child(Text::new("可以放置任意组件"))
                            .child(
                                Button::new("关闭")
                                    .on_click(Command::new(Self::toggle_right_drawer)),
                            )
                            .into();
                        content
                    }),
            )
            // Top Drawer
            .child(
                Drawer::new(self.top_drawer_open.signal())
                    .placement(DrawerPlacement::Top)
                    .on_open_change(ValueCommand::new(|vm: &mut Self, open| {
                        vm.top_drawer_open.set(open);
                    }))
                    .content({
                        let content: Element<Self> = Flex::new(Axis::Vertical)
                            .gap(16.0)
                            .child(Text::new("顶部面板"))
                            .child(Text::new("这是从顶部滑出的抽屉"))
                            .child(
                                Button::new("关闭").on_click(Command::new(Self::toggle_top_drawer)),
                            )
                            .into();
                        content
                    }),
            )
            // Bottom Drawer
            .child(
                Drawer::new(self.bottom_drawer_open.signal())
                    .placement(DrawerPlacement::Bottom)
                    .on_open_change(ValueCommand::new(|vm: &mut Self, open| {
                        vm.bottom_drawer_open.set(open);
                    }))
                    .content({
                        let content: Element<Self> = Flex::new(Axis::Vertical)
                            .gap(16.0)
                            .child(Text::new("底部面板"))
                            .child(Text::new("这是从底部滑出的抽屉"))
                            .child(Text::new("适合承载临时操作和上下文详情"))
                            .child(
                                Button::new("关闭")
                                    .on_click(Command::new(Self::toggle_bottom_drawer)),
                            )
                            .into();
                        content
                    }),
            )
            .into()
    }
}

impl ViewModel for DrawerDemoVm {
    fn new(ctx: &ViewModelContext) -> Self {
        DrawerDemoVm::new(ctx)
    }

    fn view(&self) -> Element<Self> {
        DrawerDemoVm::view(self)
    }
}

fn main() -> Result<(), tgui::core::TguiError> {
    Application::new()
        .app_id("com.example.drawer_demo")
        .with_view_model(DrawerDemoVm::new)
        .root_view(DrawerDemoVm::view)
        .run()
}
