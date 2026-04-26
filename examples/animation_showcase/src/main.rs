use std::time::Duration;

use tgui::{Application, Axis, Binding, Button, Color, Command, Flex, Insets, Observable, Point, Stack, Text, TguiError, Transition, ViewModel, ViewModelContext, dp, pct, sp, Dp};

struct AnimationVm {
    expanded: Observable<bool>,
}

impl AnimationVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            expanded: ctx.observable(false),
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
                    Color::hexa(0x08111FFF)
                } else {
                    Color::hexa(0x1A1024FF)
                }
            })
            .animated(Transition::ease_in_out(Duration::from_millis(340)))
    }

    fn card_width(&self) -> Binding<Dp> {
        self.expanded
            .binding()
            .map(|expanded| if expanded { dp(520.0) } else { dp(320.0) })
            .animated(Transition::ease_in_out(Duration::from_millis(320)))
    }

    fn card_padding(&self) -> Binding<Insets> {
        self.expanded
            .binding()
            .map(|expanded| {
                if expanded {
                    Insets::symmetric(dp(28.0), dp(22.0))
                } else {
                    Insets::symmetric(dp(18.0), dp(14.0))
                }
            })
            .animated(Transition::ease_in_out(Duration::from_millis(300)))
    }

    fn card_radius(&self) -> Binding<Dp> {
        self.expanded
            .binding()
            .map(|expanded| if expanded { dp(24.0) } else { dp(14.0) })
            .animated(Transition::ease_out(Duration::from_millis(260)))
    }

    fn card_background(&self) -> Binding<Color> {
        self.expanded
            .binding()
            .map(|expanded| {
                if expanded {
                    Color::hexa(0x0F766EFF)
                } else {
                    Color::hexa(0x9333EAFF)
                }
            })
            .animated(Transition::ease_in_out(Duration::from_millis(280)))
    }

    fn card_offset(&self) -> Binding<Point> {
        self.expanded
            .binding()
            .map(|expanded| {
                if expanded {
                    Point::new(dp(0.0), dp(0.0))
                } else {
                    Point::new(dp(0.0), dp(28.0))
                }
            })
            .animated(Transition::ease_in_out(Duration::from_millis(280)))
    }

    fn body_opacity(&self) -> Binding<f32> {
        self.expanded
            .binding()
            .map(|expanded| if expanded { 1.0 } else { 0.72 })
            .animated(Transition::ease_out(Duration::from_millis(220)))
    }

    fn action_label(&self) -> Binding<String> {
        self.expanded.binding().map(|expanded| {
            if expanded {
                "Collapse".to_string()
            } else {
                "Expand".to_string()
            }
        })
    }

    fn toggle(&mut self) {
        self.expanded.update(|expanded| *expanded = !*expanded);
    }

    fn view(&self) -> tgui::Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(24.0)))
            .center()
            .child(
                Flex::new(Axis::Vertical)
                    .width(self.card_width())
                    .padding(self.card_padding())
                    .gap(dp(16.0))
                    .background(self.card_background())
                    .border(dp(1.0), Color::hexa(0xE2E8F055))
                    .border_radius(self.card_radius())
                    .offset(self.card_offset())
                    .child(
                        Text::new("Declarative transitions")
                            .font_size(sp(26.0))
                            .color(Color::hexa(0xF8FAFCFF)),
                    )
                    .child(
                        Text::new("This single boolean drives animated width, padding, radius, color, offset, opacity, and window clear color.")
                            .font_size(sp(15.0))
                            .opacity(self.body_opacity())
                            .color(Color::hexa(0xECFEFFFF)),
                    )
                    .child(
                        Button::new(Text::new(self.action_label()))
                            .width(pct(100.0))
                            .background(Color::hexa(0x0F172AFF))
                            .border_radius(dp(12.0))
                            .on_click(Command::new(Self::toggle)),
                    ),
            )
            .into()
    }
}

impl ViewModel for AnimationVm {}

fn main() -> Result<(), TguiError> {
    Application::new()
        .window_size(dp(980.0), dp(680.0))
        .with_view_model(AnimationVm::new)
        .bind_title(AnimationVm::title)
        .bind_clear_color(AnimationVm::clear_color)
        .root_view(AnimationVm::view)
        .run()
}
