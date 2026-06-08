use std::time::Duration;

use tgui::prelude::*;

fn stateful<T: Clone>(value: T) -> StateValue<T> {
    StateValue::new(value)
}

fn text_style(ctx: &StyleContext<'_>, size: Sp, color: Color) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = size;
    style.color = color.into();
    style
}

struct AnimationVm {
    expanded: State<bool>,
}

impl AnimationVm {

    fn title(&self) -> Signal<String> {
        self.expanded.signal().map(|expanded| {
            if expanded {
                "tgui animation showcase - expanded".to_string()
            } else {
                "tgui animation showcase - compact".to_string()
            }
        })
    }

    fn clear_color(&self) -> Signal<Color> {
        self.expanded
            .signal()
            .map(|expanded| {
                if expanded {
                    Color::hexa(0x08111FFF)
                } else {
                    Color::hexa(0x1A1024FF)
                }
            })
            .animated(Transition::ease_in_out(Duration::from_millis(340)))
    }

    fn card_width(&self) -> Signal<Dp> {
        self.expanded
            .signal()
            .map(|expanded| if expanded { dp(520.0) } else { dp(320.0) })
            .animated(Transition::ease_in_out(Duration::from_millis(320)))
    }

    fn card_padding(&self) -> Signal<Insets> {
        self.expanded
            .signal()
            .map(|expanded| {
                if expanded {
                    Insets::symmetric(dp(28.0), dp(22.0))
                } else {
                    Insets::symmetric(dp(18.0), dp(14.0))
                }
            })
            .animated(Transition::ease_in_out(Duration::from_millis(300)))
    }

    fn card_radius(&self) -> Signal<Dp> {
        self.expanded
            .signal()
            .map(|expanded| if expanded { dp(24.0) } else { dp(14.0) })
            .animated(Transition::ease_out(Duration::from_millis(260)))
    }

    fn card_background(&self) -> Signal<Color> {
        self.expanded
            .signal()
            .map(|expanded| {
                if expanded {
                    Color::hexa(0x0F766EFF)
                } else {
                    Color::hexa(0x9333EAFF)
                }
            })
            .animated(Transition::ease_in_out(Duration::from_millis(280)))
    }

    fn card_offset(&self) -> Signal<Point> {
        self.expanded
            .signal()
            .map(|expanded| {
                if expanded {
                    Point::new(dp(0.0), dp(0.0))
                } else {
                    Point::new(dp(0.0), dp(28.0))
                }
            })
            .animated(Transition::ease_in_out(Duration::from_millis(280)))
    }

    fn body_opacity(&self) -> Signal<f32> {
        self.expanded
            .signal()
            .map(|expanded| if expanded { 1.0 } else { 0.72 })
            .animated(Transition::ease_out(Duration::from_millis(220)))
    }

    fn action_label(&self) -> Signal<String> {
        self.expanded.signal().map(|expanded| {
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

}

impl ViewModel for AnimationVm {

    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            expanded: ctx.state(false),
        }
    }

    fn view(&self) -> Element<Self> {
        Stack::new()
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(24.0)))
            .center()
            .child(
                Flex::new(Axis::Vertical)
                    .width(self.card_width())
                    .padding(self.card_padding())
                    .gap(dp(16.0))
                    .style_full({
                        let background = self.card_background();
                        let radius = self.card_radius();
                        let offset = self.card_offset();
                        move |ctx| {
                            let mut style = ContainerStyle::default_for_theme(ctx.theme);
                            style.surface.background = Some(background.clone().into());
                            style.surface.border_color = Some(Color::hexa(0xE2E8F055).into());
                            style.surface.border_width = Some(dp(1.0).into());
                            style.surface.border_radius = Some(radius.clone().into());
                            style.surface.offset = offset.clone().into();
                            style
                        }
                    })
                    .child(
                        Text::new("Declarative transitions")
                            .style_full(|ctx| text_style(ctx, sp(26.0), Color::hexa(0xF8FAFCFF))),
                    )
                    .child(
                        Text::new("This single boolean drives animated width, padding, radius, color, offset, opacity, and window clear color.")
                            .style_full({
                                let opacity = self.body_opacity();
                                move |ctx| {
                                    let mut style =
                                        text_style(ctx, sp(15.0), Color::hexa(0xECFEFFFF));
                                    style.surface.opacity = opacity.clone().into();
                                    style
                                }
                            }),
                    )
                    .child(
                        Button::new(self.action_label())
                            .width(pct(100.0))
                            .style_full(|ctx| ButtonStyle {
                                surface: WidgetSurfaceStyle::default(),
                                background: stateful(Color::hexa(0x0F172AFF).into()),
                                foreground: stateful(Color::WHITE.into()),
                                border: stateful(Color::TRANSPARENT.into()),
                                focus_ring: None,
                                border_width: dp(0.0).into(),
                                radius: dp(12.0).into(),
                                padding_x: dp(16.0),
                                padding_y: dp(10.0),
                                min_height: dp(40.0),
                                text_style: TextWidgetStyle::default_for_theme(ctx.theme)
                                    .typography,
                            })
                            .on_click(Command::new(Self::toggle)),
                    ),
            )
            .into()
    }

}

fn main() -> Result<(), TguiError> {
    Application::new()
        .window_size(dp(980.0), dp(680.0))
        .with_view_model(AnimationVm::new)
        .bind_title(AnimationVm::title)
        .bind_clear_color(AnimationVm::clear_color)
        .root_view(AnimationVm::view)
        .run()
}
