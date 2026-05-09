use tgui::core::Color;
use tgui::platform::keyboard::KeyCode;
use tgui::prelude::*;

fn stateful<T: Clone>(value: T) -> Stateful<T> {
    Stateful {
        normal: value.clone(),
        hovered: value.clone(),
        pressed: value.clone(),
        disabled: value,
    }
}

fn text_style(mode: ResolvedThemeMode, size: Sp, color: Color) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    style.typography.size = size;
    style.color = color.into();
    style
}

fn card_style(mode: ResolvedThemeMode) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
    style.surface.background = Some(Color::hexa(0x162033EE).into());
    style.surface.border_color = Some(Color::hexa(0x31415FFF).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(18.0).into());
    style
}

fn solid_button_style(mode: ResolvedThemeMode, background: Color) -> ButtonStyle {
    let typography = TextWidgetStyle::default_for(mode).typography;
    ButtonStyle {
        surface: WidgetSurfaceStyle::default(),
        background: stateful(background.into()),
        foreground: stateful(Color::WHITE.into()),
        border: stateful(Color::TRANSPARENT.into()),
        focus_ring: None,
        border_width: dp(0.0).into(),
        radius: dp(12.0).into(),
        padding_x: dp(16.0),
        padding_y: dp(10.0),
        min_height: dp(40.0),
        text_style: typography,
    }
}

struct CounterVm {
    count: State<i32>,
}

impl CounterVm {
    

    fn title(&self) -> Signal<String> {
        self.count
            .signal()
            .map(|count| format!("tgui mvvm counter - count: {count}"))
    }

    fn clear_color(&self) -> Signal<Color> {
        self.count.signal().map(|count| match count.rem_euclid(4) {
            0 => Color::hexa(0x0F172AFF),
            1 => Color::hexa(0x10253CFF),
            2 => Color::hexa(0x1F2937FF),
            _ => Color::hexa(0x1E1B4BFF),
        })
    }

    fn headline(&self) -> Signal<String> {
        self.count
            .signal()
            .map(|count| format!("Current value: {count}"))
    }

    fn hint(&self) -> Signal<String> {
        self.count.signal().map(|count| {
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

    
}

impl ViewModel for CounterVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            count: ctx.state(0),
        }
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(28.0)))
            .center()
            .child(
                Flex::new(Axis::Vertical)
                    .width(dp(520.0))
                    .padding(Insets::all(dp(26.0)))
                    .gap(dp(16.0))
                    .style(card_style)
                    .child(
                        Text::new("MVVM counter")
                            .style(|mode| text_style(mode, sp(26.0), Color::hexa(0xF8FAFCFF))),
                    )
                    .child(
                        Text::new(self.headline())
                            .style(|mode| text_style(mode, sp(20.0), Color::hexa(0x7DD3FCFF))),
                    )
                    .child(
                        Text::new(self.hint())
                            .style(|mode| text_style(mode, sp(15.0), Color::hexa(0xCBD5E1FF))),
                    )
                    .child(
                        Flex::new(Axis::Horizontal)
                            .gap(dp(10.0))
                            .child(
                                Button::new("-1")
                                    .grow(1.0)
                                    .style(|mode| solid_button_style(mode, Color::hexa(0x243247FF)))
                                    .on_click(Command::new(Self::decrement)),
                            )
                            .child(
                                Button::new("+1")
                                    .grow(1.0)
                                    .style(|mode| solid_button_style(mode, Color::hexa(0x0F766EFF)))
                                    .on_click(Command::new(Self::increment)),
                            )
                            .child(
                                Button::new("Reset")
                                    .grow(1.0)
                                    .style(|mode| solid_button_style(mode, Color::hexa(0x7C2D12FF)))
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
