use tgui::{
    Align, Application, Axis, Binding, Button, Color, Command, Flex, InputTrigger, Insets,
    Observable, Stack, Text, TguiError, ViewModelContext, dp, pct, sp,
};
use tgui::platform::keyboard::KeyCode;

struct CounterVm {
    count: Observable<i32>,
}

impl CounterVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            count: ctx.observable(0),
        }
    }

    fn title(&self) -> Binding<String> {
        self.count
            .binding()
            .map(|count| format!("tgui mvvm counter - count: {count}"))
    }

    fn clear_color(&self) -> Binding<Color> {
        self.count.binding().map(|count| match count.rem_euclid(4) {
            0 => Color::hexa(0x0F172AFF),
            1 => Color::hexa(0x10253CFF),
            2 => Color::hexa(0x1F2937FF),
            _ => Color::hexa(0x1E1B4BFF),
        })
    }

    fn headline(&self) -> Binding<String> {
        self.count
            .binding()
            .map(|count| format!("Current value: {count}"))
    }

    fn hint(&self) -> Binding<String> {
        self.count.binding().map(|count| {
            if count == 0 {
                "Press Space to increment, Minus to decrement, or R to reset.".to_string()
            } else if count > 0 {
                "Positive counts are great for click counters and lightweight dashboards."
                    .to_string()
            } else {
                "Negative values work too, which is handy for demos that need bidirectional state."
                    .to_string()
            }
        })
    }

    fn increment(&mut self) {
        self.count.update(|count| *count += 1);
    }

    fn decrement(&mut self) {
        self.count.update(|count| *count -= 1);
    }

    fn reset(&mut self) {
        self.count.set(0);
    }

    fn view(&self) -> tgui::Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(28.0)))
            .align(Align::Center)
            .child(
                Flex::new(Axis::Vertical)
                    .width(dp(520.0))
                    .padding(Insets::all(dp(26.0)))
                    .gap(dp(16.0))
                    .background(Color::hexa(0x162033EE))
                    .border(dp(1.0), Color::hexa(0x31415FFF))
                    .border_radius(dp(18.0))
                    .child(
                        Text::new("MVVM counter")
                            .font_size(sp(26.0))
                            .color(Color::hexa(0xF8FAFCFF)),
                    )
                    .child(
                        Text::new(self.headline())
                            .font_size(sp(20.0))
                            .color(Color::hexa(0x7DD3FCFF)),
                    )
                    .child(
                        Text::new(self.hint())
                            .font_size(sp(15.0))
                            .color(Color::hexa(0xCBD5E1FF)),
                    )
                    .child(
                        Flex::new(Axis::Horizontal)
                            .gap(dp(10.0))
                            .child(
                                Button::new(Text::new("-1"))
                                    .grow(1.0)
                                    .background(Color::hexa(0x243247FF))
                                    .border_radius(dp(12.0))
                                    .on_click(Command::new(Self::decrement)),
                            )
                            .child(
                                Button::new(Text::new("+1"))
                                    .grow(1.0)
                                    .background(Color::hexa(0x0F766EFF))
                                    .border_radius(dp(12.0))
                                    .on_click(Command::new(Self::increment)),
                            )
                            .child(
                                Button::new(Text::new("Reset"))
                                    .grow(1.0)
                                    .background(Color::hexa(0x7C2D12FF))
                                    .border_radius(dp(12.0))
                                    .on_click(Command::new(Self::reset)),
                            ),
                    ),
            )
            .into()
    }
}

fn main() -> Result<(), TguiError> {
    Application::new()
        .window_size(dp(960.0), dp(640.0))
        .with_view_model(CounterVm::new)
        .bind_title(CounterVm::title)
        .bind_clear_color(CounterVm::clear_color)
        .on_input(
            InputTrigger::KeyPressed(KeyCode::Space),
            Command::new(CounterVm::increment),
        )
        .on_input(
            InputTrigger::KeyPressed(KeyCode::Minus),
            Command::new(CounterVm::decrement),
        )
        .on_input(
            InputTrigger::KeyPressed(KeyCode::KeyR),
            Command::new(CounterVm::reset),
        )
        .root_view(CounterVm::view)
        .run()
}
