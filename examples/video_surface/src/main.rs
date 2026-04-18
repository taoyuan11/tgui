use std::path::PathBuf;

use tgui::{
    Application, Color, Column, Command, Insets, Row, Stack, Text, VideoController, VideoSource,
    VideoSurface, ViewModelContext,
};

struct VideoVm {
    controller: VideoController,
    source_label: String,
}

impl VideoVm {
    fn new(ctx: &ViewModelContext) -> Self {
        let controller = VideoController::new(ctx);
        // let source = parse_video_source("D:\\CloudMusic\\MV\\郭顶 - 凄美地.mp4".to_string());
        let source = parse_video_source("http://47.109.31.100:19900/%E9%83%AD%E9%A1%B6%20-%20%E5%87%84%E7%BE%8E%E5%9C%B0.mp4".to_string());
        let source_label = source_label(&source);
        let _ = controller.load(source);

        Self {
            controller,
            source_label,
        }
    }

    fn play(&mut self) {
        self.controller.play();
    }

    fn pause(&mut self) {
        self.controller.pause();
    }

    fn mute(&mut self) {
        self.controller.set_muted(true);
    }

    fn unmute(&mut self) {
        self.controller.set_muted(false);
    }

    fn set_progress(&mut self, progress: f64) {
        if let Some(duration) = self.controller.duration().get() {
            let progress = std::time::Duration::from_secs_f64(duration.as_secs_f64() * progress);
            self.controller.seek(progress);
        }
    }

    fn view(&self) -> tgui::Element<Self> {
        let status = self.controller.playback_state().map(|state| format!("{state:?}"));
        let position = self
            .controller
            .position()
            .map(|position| format_duration(position));
        let duration = self.controller.duration().map(|duration| {
            duration
                .map(format_duration)
                .unwrap_or_else(|| "--:--".to_string())
        });

        Column::new()
            .padding(Insets::all(20.0))
            .gap(12.0)
            .background(Color::hexa(0x0F172AFF))
            .child(
                Text::new(format!("Source: {}", self.source_label))
                    .font_size(14.0)
                    .color(Color::WHITE),
            )
            .child(
                VideoSurface::new(self.controller.clone())
                    .fill_width()
                    .height(360.0)
                    .background(Color::hexa(0x020617FF))
                    .border_radius(12.0)
                    .border(1.0, Color::hexa(0x334155FF)),
            )
            .child(
                Row::new()
                    .gap(8.0)
                    .child(
                        Stack::new()
                            .padding(Insets::symmetric(12.0, 8.0))
                            .background(Color::hexa(0x2563EBFF))
                            .border_radius(8.0)
                            .child(Text::new("Play").color(Color::WHITE))
                            .on_click(Command::new(Self::play)),
                    )
                    .child(
                        Stack::new()
                            .padding(Insets::symmetric(12.0, 8.0))
                            .background(Color::hexa(0x475569FF))
                            .border_radius(8.0)
                            .child(Text::new("Pause").color(Color::WHITE))
                            .on_click(Command::new(Self::pause)),
                    )
                    .child(
                        Stack::new()
                            .padding(Insets::symmetric(12.0, 8.0))
                            .background(Color::hexa(0xEA580CFF))
                            .border_radius(8.0)
                            .child(Text::new("to 95%").color(Color::WHITE))
                            .on_click(Command::new(|video_vm: &mut VideoVm|{
                                video_vm.set_progress(0.95)
                            })),
                    )
                    .child(
                        Stack::new()
                            .padding(Insets::symmetric(12.0, 8.0))
                            .background(Color::hexa(0x7C3AEDFF))
                            .border_radius(8.0)
                            .child(Text::new("Mute").color(Color::WHITE))
                            .on_click(Command::new(Self::mute)),
                    )
                    .child(
                        Stack::new()
                            .padding(Insets::symmetric(12.0, 8.0))
                            .background(Color::hexa(0x0F766EFF))
                            .border_radius(8.0)
                            .child(Text::new("Unmute").color(Color::WHITE))
                            .on_click(Command::new(Self::unmute)),
                    )
            )
            .child(
                Row::new()
                    .gap(12.0)
                    .child(Text::new(status).color(Color::hexa(0xE2E8F0FF)))
                    .child(Text::new(position).color(Color::hexa(0xCBD5E1FF)))
                    .child(Text::new("/").color(Color::hexa(0x64748BFF)))
                    .child(Text::new(duration).color(Color::hexa(0xCBD5E1FF))),
            )
            .into()
    }
}


fn parse_video_source(value: String) -> VideoSource {
    if value.starts_with("http://") || value.starts_with("https://") {
        VideoSource::Url(value)
    } else {
        VideoSource::File(PathBuf::from(value))
    }
}

fn source_label(source: &VideoSource) -> String {
    match source {
        VideoSource::File(path) => path.display().to_string(),
        VideoSource::Url(url) => url.clone()
    }
}

fn format_duration(duration: std::time::Duration) -> String {
    let total_seconds = duration.as_secs();
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}

fn main() -> Result<(), tgui::TguiError> {
    Application::new()
        .title("tgui VideoSurface")
        .window_size(960, 640)
        .with_view_model(VideoVm::new)
        .root_view(VideoVm::view)
        .run()
}
