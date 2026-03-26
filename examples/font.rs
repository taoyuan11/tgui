use tgui::{column, load_font, set_default_font, text, View};

fn main() {
    load_font("icon", include_bytes!("./static/google-icon-font.ttf")).unwrap();
    set_default_font("JetBrains Mono").unwrap();
    tgui::run(app)
}


fn app() -> impl View {
    column([
        text("\u{e88a}"),
        text("Hello, World!"),
        text("Hello, World!")
    ])
}
