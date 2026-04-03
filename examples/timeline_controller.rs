use std::time::Duration;

use tgui::{
    AnimationControllerHandle, AnimationCurve, AnimationSpec, Application, Button, Color, Column,
    Command, Insets, Keyframes, Playback, PlaybackDirection, Point, Row, Text, ViewModelContext,
};

struct TimelineControllerVm {
    status: tgui::Observable<String>,
    card_color: tgui::AnimatedValue<Color>,
    card_offset: tgui::AnimatedValue<Point>,
    card_width: tgui::AnimatedValue<f32>,
    card_padding: tgui::AnimatedValue<Insets>,
    card_opacity: tgui::AnimatedValue<f32>,
    timeline: AnimationControllerHandle,
}

impl TimelineControllerVm {
    fn new(context: &ViewModelContext) -> Self {
        let status = context.observable("Idle".to_string());
        let card_color = context.animated_value(Color::hexa(0x2563EBFF));
        let card_offset = context.animated_value(Point { x: 0.0, y: 0.0 });
        let card_width = context.animated_value(220.0);
        let card_padding = context.animated_value(Insets::symmetric(16.0, 12.0));
        let card_opacity = context.animated_value(1.0);

        let on_start = status.clone();
        let on_repeat = status.clone();
        let on_complete = status.clone();
        let on_stop = status.clone();

        let timeline = context
            .timeline()
            .playback(
                Playback::default()
                    .repeat(2)
                    .direction(PlaybackDirection::Alternate)
                    .delay(Duration::from_millis(60)),
            )
            .track(
                card_color.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(900))
                        .curve(AnimationCurve::EaseInOutCubic)
                        .at(Duration::ZERO, Color::hexa(0x2563EBFF))
                        .at(Duration::from_millis(420), Color::hexa(0xF97316FF))
                        .at(Duration::from_millis(900), Color::hexa(0x14B8A6FF)),
                ),
            )
            .track(
                card_offset.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(900))
                        .curve(AnimationCurve::EaseInOutCubic)
                        .at(Duration::ZERO, Point { x: 0.0, y: 0.0 })
                        .at(Duration::from_millis(320), Point { x: 0.0, y: 18.0 })
                        .at(Duration::from_millis(900), Point { x: 0.0, y: -10.0 }),
                ),
            )
            .track(
                card_width.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(900))
                        .curve(AnimationCurve::EaseInOutCubic)
                        .at(Duration::ZERO, 220.0)
                        .at(Duration::from_millis(400), 320.0)
                        .at(Duration::from_millis(900), 260.0),
                ),
            )
            .track(
                card_padding.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(900))
                        .curve(AnimationCurve::EaseInOutCubic)
                        .at(Duration::ZERO, Insets::symmetric(16.0, 12.0))
                        .at(Duration::from_millis(480), Insets::symmetric(28.0, 18.0))
                        .at(Duration::from_millis(900), Insets::symmetric(20.0, 14.0)),
                ),
            )
            .track(
                card_opacity.clone(),
                AnimationSpec::from(
                    Keyframes::timed(Duration::from_millis(900))
                        .curve(AnimationCurve::EaseInOutCubic)
                        .at(Duration::ZERO, 1.0)
                        .at(Duration::from_millis(300), 0.75)
                        .at(Duration::from_millis(900), 1.0),
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

    fn reverse(&mut self) {
        self.timeline.reverse();
        self.status.set("Reversed".to_string());
    }

    fn faster(&mut self) {
        self.timeline.set_speed(1.8);
        self.timeline.restart();
        self.status.set("1.8x speed".to_string());
    }

    fn view(&self) -> tgui::Element<Self> {
        Column::new()
            .padding(Insets::all(24.0))
            .gap(18.0)
            .child(Text::new("Command-style timeline controller".to_string()).font_size(24.0))
            .child(Text::new(
                self.status
                    .binding()
                    .map(|value| format!("Status: {value}")),
            ))
            .child(
                Row::new()
                    .gap(10.0)
                    .child(Button::new(Text::new("Play")).on_click(Command::new(Self::play)))
                    .child(Button::new(Text::new("Pause")).on_click(Command::new(Self::pause)))
                    .child(Button::new(Text::new("Resume")).on_click(Command::new(Self::resume)))
                    .child(Button::new(Text::new("Reverse")).on_click(Command::new(Self::reverse)))
                    .child(Button::new(Text::new("1.8x")).on_click(Command::new(Self::faster))),
            )
            .child(
                Button::new(Text::new("Timeline-driven card"))
                    .width(self.card_width.binding())
                    .padding(self.card_padding.binding())
                    .background(self.card_color.binding())
                    .opacity(self.card_opacity.binding())
                    .offset(self.card_offset.binding()),
            )
            .into()
    }
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .title("tgui timeline controller")
        .window_size(960, 640)
        .with_view_model(TimelineControllerVm::new)
        .root_view(TimelineControllerVm::view)
        .run()
}
