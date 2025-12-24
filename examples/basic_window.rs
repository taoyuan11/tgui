use tgui::{WindowBuilder, window, defer};
use tgui::gui::widgets::Button;

fn main() {
    WindowBuilder::new("窗口标题");

    let button = Button::build("按钮1")
        .on_click(|| { println!("按钮1被点击") });

    // 现在可以直接在闭包中使用全局 window，不需要传递 app 的引用
    let button2 = Button::build("修改窗口标题")
        .with_position(300, 0)
        .on_click(|| {
            println!("按钮2被点击");
            defer(|| {
                window(|app| {
                    app.set_title("标题已修改");
                    println!("标题已修改");
                });
            });
            println!("按钮2被点击");
        });

    window(|app| {
        app.add_child(Box::new(button));
        app.add_child(Box::new(button2));
    });

    WindowBuilder::run()
}