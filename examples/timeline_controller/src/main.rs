use std::time::Duration;

use tgui::{
    AnimatedValue, AnimationControllerHandle, AnimationCurve, AnimationSpec, Application, Button,
    Color, Axis, Command, Dp, Flex, Insets, Keyframes, Observable, Playback,
    PlaybackDirection, Point, Text, TguiError, ViewModel, ViewModelContext, dp, pct, sp,
};

struct TimelineVm {
    status: Observable<String>,
    card_color: AnimatedValue<Color>,
    card_offset: AnimatedValue<Point>,
    card_width: AnimatedValue<Dp>,
    card_padding: AnimatedValue<Insets>,
    card_opacity: AnimatedValue<f32>,
    timeline: AnimationControllerHandle,
}

impl TimelineVm {

    fn play(&mut self) {
        self.timeline.play();
    }

    fn pause(&mut self) {
        self.timeline.pause();
        self.status.set("Paused".to_string());
    }

    fn resume(&mut self) {
        self.timeline.resume();
        self.status.set("Running".to_string());
    }

    fn restart(&mut self) {
        self.timeline.restart();
        self.status.set("Restarted".to_string());
    }

    fn reverse(&mut self) {
        self.timeline.reverse();
        self.status.set("Reversed".to_string());
    }

    fn seek_middle(&mut self) {
        self.timeline.seek_percent(0.5);
        self.status.set("Jumped to 50%".to_string());
    }

}

impl ViewModel for TimelineVm {

    fn new(ctx: &ViewModelContext) -> Self {
        let status = ctx.observable("Idle".to_string());
        let card_color = ctx.animated_value(Color::hexa(0x2563EBFF));
        let card_offset = ctx.animated_value(Point::new(dp(0.0), dp(0.0)));
        let card_width = ctx.animated_value(dp(280.0));
        let card_padding = ctx.animated_value(Insets::symmetric(dp(18.0), dp(14.0)));
        let card_opacity = ctx.animated_value(1.0);

        let on_start = status.clone();
        let on_repeat = status.clone();
        let on_complete = status.clone();
        let on_stop = status.clone();

        let timeline = ctx
            .timeline()
            .playback(
                Playback::default()
                    .repeat(2)
                    .direction(PlaybackDirection::Alternate)
                    .delay(Duration::from_millis(80)),
            )
            .track(
                card_color.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(1200))
                        .curve(AnimationCurve::EaseInOutCubic)
                        .at(Duration::ZERO, Color::hexa(0x2563EBFF))
                        .at(Duration::from_millis(500), Color::hexa(0x0F766EFF))
                        .at(Duration::from_millis(1200), Color::hexa(0x9333EAFF)),
                ),
            )
            .track(
                card_offset.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(1200))
                        .curve(AnimationCurve::EaseInOutCubic)
                        .at(Duration::ZERO, Point::new(dp(0.0), dp(0.0)))
                        .at(Duration::from_millis(400), Point::new(dp(0.0), dp(18.0)))
                        .at(Duration::from_millis(1200), Point::new(dp(0.0), dp(-12.0))),
                ),
            )
            .track(
                card_width.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(1200))
                        .curve(AnimationCurve::EaseInOutCubic)
                        .at(Duration::ZERO, dp(280.0))
                        .at(Duration::from_millis(600), dp(440.0))
                        .at(Duration::from_millis(1200), dp(340.0)),
                ),
            )
            .track(
                card_padding.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(1200))
                        .curve(AnimationCurve::EaseInOutCubic)
                        .at(Duration::ZERO, Insets::symmetric(dp(18.0), dp(14.0)))
                        .at(Duration::from_millis(520), Insets::symmetric(dp(30.0), dp(22.0)))
                        .at(Duration::from_millis(1200), Insets::symmetric(dp(22.0), dp(16.0))),
                ),
            )
            .track(
                card_opacity.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(1200))
                        .curve(AnimationCurve::EaseInOutCubic)
                        .at(Duration::ZERO, 1.0)
                        .at(Duration::from_millis(280), 0.72)
                        .at(Duration::from_millis(1200), 1.0),
                ),
            )
            .on_start(move || on_start.set("Running".to_string()))
            .on_repeat(move || on_repeat.set("Looped".to_string()))
            .on_complete(move || on_complete.set("Completed".to_string()))
            .on_stop(move || on_stop.set("Stopped".to_string()))
            .build();

        Self {
            status,
            card_color,
            card_offset,
            card_width,
            card_padding,
            card_opacity,
            timeline,
        }
    }

    fn view(&self) -> tgui::Element<Self> {
        Flex::new(Axis::Vertical)
            .size(pct(100.0), pct(100.0))
            .padding(Insets::all(dp(24.0)))
            .gap(dp(16.0))
            .child(
                Text::new("Timeline controller")
                    .font_size(sp(28.0))
                    .color(Color::hexa(0xF8FAFCFF)),
            )
            .child(
                Text::new(
                    self.status
                        .binding()
                        .map(|status| format!("Status: {status}")),
                )
                    .font_size(sp(16.0))
                    .color(Color::hexa(0xCBD5E1FF)),
            )
            .child(
                Flex::new(Axis::Horizontal)
                    .gap(dp(10.0))
                    .child(Button::new(Text::new("Play")).on_click(Command::new(Self::play)))
                    .child(Button::new(Text::new("Pause")).on_click(Command::new(Self::pause)))
                    .child(Button::new(Text::new("Resume")).on_click(Command::new(Self::resume)))
                    .child(Button::new(Text::new("Restart")).on_click(Command::new(Self::restart)))
                    .child(Button::new(Text::new("Reverse")).on_click(Command::new(Self::reverse)))
                    .child(
                        Button::new(Text::new("Seek 50%"))
                            .on_click(Command::new(Self::seek_middle)),
                    ),
            )
            .child(
                Button::new(Text::new("Timeline-driven card"))
                    .width(self.card_width.binding())
                    .padding(self.card_padding.binding())
                    .background(self.card_color.binding())
                    .border_radius(dp(18.0))
                    .opacity(self.card_opacity.binding())
                    .offset(self.card_offset.binding()),
            )
            .into()
    }

}

fn main() -> Result<(), TguiError> {
    Application::new()
        .title("tgui timeline controller")
        .window_size(dp(1080.0), dp(720.0))
        .with_view_model(TimelineVm::new)
        .root_view(TimelineVm::view)
        .run()
}
