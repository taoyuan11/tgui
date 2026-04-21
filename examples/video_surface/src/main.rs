#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;

use tgui::{
    el, Application, Button, Color, Column, Command, Input, Insets, Observable, Row, Stack, Text,
    ValueCommand, VideoController, VideoSource, VideoSurface, ViewModelContext,
};

struct VideoVm {
    controller: VideoController,
    source: Observable<String>,
}

impl VideoVm {
    fn new(ctx: &ViewModelContext) -> Self {
        let controller = VideoController::new(ctx);
        controller.set_buffer_memory_limit_bytes(160 * 1024 * 1024);
        Self {
            controller,
            source: ctx.observable(String::from("D:\\CloudMusic\\MV\\郭顶 - 凄美地.mp4")),
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
        let status = self
            .controller
            .playback_state()
            .map(|state| format!("{state:?}"));
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
            .child(el![
                Text::new(
                    self.source
                        .binding()
                        .map(|source| format!("Source: {}", source)),
                )
                .font_size(14.0)
                .color(Color::WHITE),
                VideoSurface::new(self.controller.clone())
                    .fill_width()
                    .height(360.0)
                    .background(Color::hexa(0x020617FF))
                    .border_radius(12.0)
                    .border(1.0, Color::hexa(0x334155FF)),
                Row::new().gap(8.0).child(el![
                    Stack::new()
                        .padding(Insets::symmetric(12.0, 8.0))
                        .background(Color::hexa(0x2563EBFF))
                        .border_radius(8.0)
                        .child(Text::new("Play").color(Color::WHITE))
                        .on_click(Command::new(|vm: &mut VideoVm| {
                            eprintln!("[vm] play clicked");
                            vm.play()
                        })),
                    Stack::new()
                        .padding(Insets::symmetric(12.0, 8.0))
                        .background(Color::hexa(0x475569FF))
                        .border_radius(8.0)
                        .child(Text::new("Pause").color(Color::WHITE))
                        .on_click(Command::new(Self::pause)),
                    Stack::new()
                        .padding(Insets::symmetric(12.0, 8.0))
                        .background(Color::hexa(0xEA580CFF))
                        .border_radius(8.0)
                        .child(Text::new("to 95%").color(Color::WHITE))
                        .on_click(Command::new(|video_vm: &mut VideoVm| {
                            video_vm.set_progress(0.95)
                        })),
                    Stack::new()
                        .padding(Insets::symmetric(12.0, 8.0))
                        .background(Color::hexa(0x7C3AEDFF))
                        .border_radius(8.0)
                        .child(Text::new("Mute").color(Color::WHITE))
                        .on_click(Command::new(Self::mute)),
                    Stack::new()
                        .padding(Insets::symmetric(12.0, 8.0))
                        .background(Color::hexa(0x0F766EFF))
                        .border_radius(8.0)
                        .child(Text::new("Unmute").color(Color::WHITE))
                        .on_click(Command::new(Self::unmute)),
                ]),
                Row::new().gap(12.0).child(el![
                    Text::new(status).color(Color::hexa(0xE2E8F0FF)),
                    Text::new(position).color(Color::hexa(0xCBD5E1FF)),
                    Text::new("/").color(Color::hexa(0x64748BFF)),
                    Text::new(duration).color(Color::hexa(0xCBD5E1FF))
                ]),
                Row::new().fill_width().gap(10.0).child(el![
                    Input::new(Text::new(self.source.binding()))
                        .fill_width()
                        .placeholder_with_str("PleaseEnterTheVideoSourcePath")
                        .on_change(ValueCommand::new(
                            |video_vm: &mut VideoVm, value: String| { video_vm.source.set(value) }
                        )),
                    Button::new(Text::new("LoadSource")).on_click(Command::new(
                        |video_vm: &mut VideoVm| {
                            let _ = video_vm
                                .controller
                                .load(parse_video_source(video_vm.source.get().clone()));
                        }
                    ))
                ])
            ])
            .into()
    }
}

fn parse_video_source(value: String) -> VideoSource {
    if value.starts_with("http") {
        VideoSource::Url(value)
    } else {
        VideoSource::File(PathBuf::from(value))
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
