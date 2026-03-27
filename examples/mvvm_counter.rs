use tgui::{Application, Command, InputTrigger, ViewModelContext};
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

struct CounterViewModel {
    clicks: tgui::Observable<u32>,
}

impl CounterViewModel {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            clicks: context.observable(0),
        }
    }

    fn title(&self) -> tgui::Binding<String> {
        self.clicks
            .binding()
            .map(|count| format!("tgui mvvm counter - clicks: {count}"))
    }

    fn clear_color(&self) -> tgui::Binding<wgpu::Color> {
        self.clicks.binding().map(|count| {
            let phase = (count % 6) as f64;
            wgpu::Color {
                r: 0.08 + phase * 0.07,
                g: 0.12 + phase * 0.04,
                b: 0.18 + phase * 0.03,
                a: 1.0,
            }
        })
    }

    fn increment(&mut self) {
        self.clicks.update(|count| *count += 1);
    }

    fn reset(&mut self) {
        self.clicks.set(0);
    }
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .window_size(960, 640)
        .with_view_model(CounterViewModel::new)
        .bind_title(CounterViewModel::title)
        .bind_clear_color(CounterViewModel::clear_color)
        .on_input(
            InputTrigger::MousePressed(MouseButton::Left),
            Command::new(CounterViewModel::increment),
        )
        .on_input(
            InputTrigger::KeyPressed(KeyCode::Space),
            Command::new(CounterViewModel::increment),
        )
        .on_input(
            InputTrigger::KeyPressed(KeyCode::KeyR),
            Command::new(CounterViewModel::reset),
        )
        .run()
}
