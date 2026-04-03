use std::time::Duration;

use tgui::{
    Application, Binding, Button, Color, Column, Command, Insets, PlaybackDirection, Point, Text,
    Transition, ViewModelContext,
};

struct AnimationShowcaseVm {
    expanded: tgui::Observable<bool>,
}

impl AnimationShowcaseVm {
    fn new(context: &ViewModelContext) -> Self {
        Self {
            expanded: context.observable(false),
        }
    }

    fn title(&self) -> Binding<String> {
        self.expanded.binding().map(|expanded| {
            if expanded {
                "tgui animation showcase - expanded".to_string()
            } else {
                "tgui animation showcase - compact".to_string()
            }
        })
    }

    fn clear_color(&self) -> Binding<Color> {
        self.expanded
            .binding()
            .map(|expanded| {
                if expanded {
                    Color::hexa(0x0F172AFF)
                } else {
                    Color::hexa(0xFFF7EDFF)
                }
            })
            .animated(Transition::ease_in_out(Duration::from_millis(320)))
    }

    fn card_color(&self) -> Binding<Color> {
        self.expanded
            .binding()
            .map(|expanded| {
                if expanded {
                    Color::hexa(0x2563EBFF)
                } else {
                    Color::hexa(0xF97316FF)
                }
            })
            .animated(Transition::ease_out(Duration::from_millis(240)))
    }

    fn card_opacity(&self) -> Binding<f32> {
        self.expanded
            .binding()
            .map(|expanded| if expanded { 1.0 } else { 0.72 })
            .animated(
                Transition::ease_out(Duration::from_millis(220)).delay(Duration::from_millis(20)),
            )
    }

    fn card_offset(&self) -> Binding<Point> {
        self.expanded
            .binding()
            .map(|expanded| {
                if expanded {
                    Point { x: 0.0, y: 0.0 }
                } else {
                    Point { x: 0.0, y: 28.0 }
                }
            })
            .animated(Transition::ease_in_out(Duration::from_millis(260)))
    }

    fn card_width(&self) -> Binding<f32> {
        self.expanded
            .binding()
            .map(|expanded| if expanded { 280.0 } else { 180.0 })
            .animated(
                Transition::ease_in_out(Duration::from_millis(320))
                    .delay(Duration::from_millis(30)),
            )
    }

    fn card_padding(&self) -> Binding<Insets> {
        self.expanded
            .binding()
            .map(|expanded| {
                if expanded {
                    Insets::symmetric(28.0, 18.0)
                } else {
                    Insets::symmetric(16.0, 12.0)
                }
            })
            .animated(Transition::ease_in_out(Duration::from_millis(280)))
    }

    fn stack_gap(&self) -> Binding<f32> {
        self.expanded
            .binding()
            .map(|expanded| if expanded { 28.0 } else { 16.0 })
            .animated(
                Transition::ease_in_out(Duration::from_millis(300))
                    .direction(PlaybackDirection::Normal),
            )
    }

    fn hint_color(&self) -> Binding<Color> {
        self.expanded
            .binding()
            .map(|expanded| {
                if expanded {
                    Color::hexa(0xDBEAFEFF)
                } else {
                    Color::hexa(0x7C2D12FF)
                }
            })
            .animated(Transition::default())
    }

    fn toggle(&mut self) {
        self.expanded.update(|expanded| *expanded = !*expanded);
    }

    fn view(&self) -> tgui::Element<Self> {
        Column::new()
            .padding(Insets::all(24.0))
            .gap(self.stack_gap())
            .child(
                Text::new(
                    "Declarative transitions for color, opacity, offset and layout".to_string(),
                )
                .color(self.hint_color()),
            )
            .child(
                Button::new(Text::new(self.expanded.binding().map(|expanded| {
                    if expanded {
                        "Collapse card".to_string()
                    } else {
                        "Expand card".to_string()
                    }
                })))
                .width(self.card_width())
                .padding(self.card_padding())
                .background(self.card_color())
                .opacity(self.card_opacity())
                .offset(self.card_offset())
                .on_click(Command::new(Self::toggle)),
            )
            .into()
    }
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .title("tgui animation showcase")
        .window_size(960, 640)
        .with_view_model(AnimationShowcaseVm::new)
        .bind_title(AnimationShowcaseVm::title)
        .bind_clear_color(AnimationShowcaseVm::clear_color)
        .root_view(AnimationShowcaseVm::view)
        .run()
}
