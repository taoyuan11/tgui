use tgui::prelude::*;

fn main() {
    run(app);
}

fn app() -> impl View {
    // 定义一个响应式状态
    let count = create_signal(0);

    column([
        text("Hello, tgui!").font_size(24),

        row([
            button(text("-")).on_click(move |_| count.update(|n| *n -= 1)),
            text(move || format!("Count: {}", count.get())),
            button(text("+")).on_click(move |_| count.update(|n| *n += 1)),
        ]).spacing(10)
    ])
        .center()
}
