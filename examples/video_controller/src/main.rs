use std::time::Duration;

use tgui::{
    Application, Button, Column, Command, ContentFit, Insets, Observable, Row, Text,
    Video, VideoControllerHandle, VideoPlaybackStatus, ViewModelContext,
};

struct VideoControllerVm {
    controller: VideoControllerHandle,
    last_event: Observable<String>,
    video_state: Observable<String>
}

impl VideoControllerVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            controller: ctx.video_controller(),
            last_event: ctx.observable("waiting".to_string()),
            video_state: ctx.observable("empty".to_string()),
        }
    }

    fn play(&mut self) {
        self.controller.play();
    }

    fn pause(&mut self) {
        self.controller.pause();
    }

    fn resume(&mut self) {
        self.controller.resume();
    }

    fn replay(&mut self) {
        self.controller.replay();
    }

    fn seek_middle(&mut self) {
        self.controller.seek_percent(0.8);
    }

    fn toggle_mute(&mut self) {
        let muted = self.controller.snapshot().muted;
        self.controller.set_muted(!muted);
    }

    fn on_play(&mut self) {
        self.last_event.set("play".to_string());
        self.video_state.set("play".to_string());
    }

    fn on_pause(&mut self) {
        self.last_event.set("pause".to_string());
        self.video_state.set("pause".to_string());
    }

    fn on_resume(&mut self) {
        self.last_event.set("resume".to_string());
    }

    fn on_end(&mut self) {
        self.last_event.set("end".to_string());
    }

    fn on_seek(&mut self) {
        self.last_event.set("seek".to_string());
    }

    fn on_loading(&mut self) {
        self.video_state.set("loading".to_string());
    }

    fn status_text(&self) -> tgui::Binding<String> {
        self.controller.status_binding().map(|status| match status {
            VideoPlaybackStatus::Idle => "status: idle".to_string(),
            VideoPlaybackStatus::Loading => "status: loading".to_string(),
            VideoPlaybackStatus::Ready => "status: ready".to_string(),
            VideoPlaybackStatus::Playing => "status: playing".to_string(),
            VideoPlaybackStatus::Paused => "status: paused".to_string(),
            VideoPlaybackStatus::Ended => "status: ended".to_string(),
            VideoPlaybackStatus::Error(error) => format!("status: error ({error})"),
        })
    }

    fn progress_text(&self) -> tgui::Binding<String> {
        let controller = self.controller.clone();
        self.controller.progress_binding().map(move |progress| {
            let snapshot = controller.snapshot();
            let position = format_duration(snapshot.position);
            let duration = snapshot
                .duration
                .map(format_duration)
                .unwrap_or_else(|| "--:--".to_string());
            format!("progress: {:>3.0}% ({position} / {duration})", progress * 100.0)
        })
    }

    fn muted_text(&self) -> tgui::Binding<String> {
        self.controller
            .muted_binding()
            .map(|muted| format!("muted: {muted}"))
    }

    fn view(&self) -> tgui::Element<Self> {
        Column::new()
            .fill_size()
            .padding(Insets::all(24.0))
            .gap(14.0)
            .child(Text::new("Video controller").font_size(28.0))
            .child(Text::new(self.status_text()).font_size(16.0))
            .child(Text::new(self.progress_text()).font_size(14.0))
            .child(Text::new(self.muted_text()).font_size(14.0))
            .child(Text::new(self.video_state.binding()).font_size(14.0))
            .child(
                Text::new(
                    self.last_event
                        .binding()
                        .map(|value| format!("last event: {value}")),
                )
                .font_size(14.0),
            )
            .child(
                Row::new()
                    .gap(10.0)
                    .child(Button::new(Text::new("Play")).on_click(Command::new(Self::play)))
                    .child(Button::new(Text::new("Pause")).on_click(Command::new(Self::pause)))
                    .child(Button::new(Text::new("Resume")).on_click(Command::new(Self::resume)))
                    .child(Button::new(Text::new("Replay")).on_click(Command::new(Self::replay)))
                    .child(
                        Button::new(Text::new("Seek 90%"))
                            .on_click(Command::new(Self::seek_middle)),
                    )
                    .child(
                        Button::new(Text::new("Toggle mute"))
                            .on_click(Command::new(Self::toggle_mute)),
                    ),
            )
            .child(
                //TODO: Bugs -> 播放到最后几秒时卡住，并且状态依然时 playing
                Video::from_url(
                    "https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4"
                )
                    .controller(self.controller.clone())
                    .height(320.0)
                    .fill_width()
                    .fit(ContentFit::Contain)
                    .aspect_ratio(16.0 / 9.0)
                    .on_play(Command::new(Self::on_play))
                    .on_pause(Command::new(Self::on_pause))
                    .on_resume(Command::new(Self::on_resume))
                    .on_end(Command::new(Self::on_end))
                    .on_seek(Command::new(Self::on_seek))
                    .on_loading(Command::new(Self::on_loading))
            )
            .into()
    }
}

fn format_duration(value: Duration) -> String {
    let seconds = value.as_secs();
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .title("tgui video controller")
        .window_size(1080, 720)
        .with_view_model(VideoControllerVm::new)
        .root_view(VideoControllerVm::view)
        .run()
}
