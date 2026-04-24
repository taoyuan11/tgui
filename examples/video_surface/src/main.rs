#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::path::PathBuf;

use tgui::{
    dp, el, sp, Application, Button, Color, Column, Command, Input, Insets, Observable, Row,
    Text, ValueCommand, VideoController, VideoSource, VideoSurface, ViewModelContext,
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
            // source: ctx.observable(String::from("D:\\CloudMusic\\MV\\郭顶 - 凄美地.mp4")),
            source: ctx.observable(String::from("https://cn-jsnt-ct-01-01.bilivideo.com/upgcxcode/85/57/37691985785/37691985785-1-16.mp4?e=ig8euxZM2rNcNbRVhwdVhwdlhWdVhwdVhoNvNC8BqJIzNbfqXBvEqxTEto8BTrNvN0GvT90W5JZMkX_YN0MvXg8gNEV4NC8xNEV4N03eN0B5tZlqNxTEto8BTrNvNeZVuJ10Kj_g2UB02J0mN0B5tZlqNCNEto8BTrNvNC7MTX502C8f2jmMQJ6mqF2fka1mqx6gqj0eN0B599M=&uipk=5&os=bcache&mid=0&oi=2882915941&deadline=1777026992&platform=pc&trid=00008c9a0b26e40e433494404a9fc3b42f1u&gen=playurlv3&og=cos&nbs=1&upsig=17af2a95637bd18b06c59d5f2f104d5d&uparams=e,uipk,os,mid,oi,deadline,platform,trid,gen,og,nbs&cdnid=4309&bvc=vod&nettype=0&bw=427231&lrs=78&f=u_0_0&qn_dyeid=3b985d16adf341e3000935b869eb2b90&agrr=1&buvid=&build=0&dl=0&orderid=0,3")),
        }
    }

    fn play(&mut self) {
        self.controller.play();
    }

    fn pause(&mut self) {
        self.controller.pause();
    }

    fn replay(&mut self) {
        self.controller.replay();
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
            .padding(Insets::all(dp(20.0)))
            .gap(dp(12.0))
            .background(Color::hexa(0x0F172AFF))
            .child(el![
                Text::new(
                    self.source
                        .binding()
                        .map(|source| format!("Source: {}", source)),
                )
                .font_size(sp(14.0))
                .color(Color::WHITE),
                VideoSurface::new(self.controller.clone())
                    .fill_width()
                    .height(dp(360.0))
                    .border_radius(dp(12.0))
                    .border(dp(1.0), Color::hexa(0x334155FF)),
                Row::new().gap(dp(8.0)).child(el![
                    Button::new(Text::new("Play"))
                        .on_click(Command::new(Self::play)),
                    Button::new(Text::new("Pause"))
                        .on_click(Command::new(Self::pause)),
                    Button::new(Text::new("Replay"))
                        .on_click(Command::new(Self::replay)),
                    Button::new(Text::new("to 95%"))
                        .on_click(Command::new(|video_vm: &mut VideoVm| {
                            video_vm.set_progress(0.95)
                        })),
                    Button::new(Text::new("Mute"))
                        .on_click(Command::new(Self::mute)),
                    Button::new(Text::new("Unmute"))
                        .on_click(Command::new(Self::unmute)),
                ]),
                Row::new().gap(dp(12.0)).child(el![
                    Text::new(status).color(Color::hexa(0xE2E8F0FF)),
                    Text::new(position).color(Color::hexa(0xCBD5E1FF)),
                    Text::new("/").color(Color::hexa(0x64748BFF)),
                    Text::new(duration).color(Color::hexa(0xCBD5E1FF))
                ]),
                Column::new().fill_width().gap(dp(10.0)).child(el![
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
                                .load(parse_video_source(video_vm.source.get().clone(), Some(vec![
                                ("Referer".to_string(), "https://www.bilibili.com/".to_string())
                            ])));
                        }
                    ))
                ])
            ])
            .into()
    }
}

fn parse_video_source(value: String, header: Option<Vec<(String, String)>>) -> VideoSource {
    if value.starts_with("http") {
        let mut source = VideoSource::url(value);
        if let Some(header) = header {
            source = source.with_headers(header);
        }
        source
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
        .window_size(dp(960.0), dp(640.0))
        .with_view_model(VideoVm::new)
        .root_view(VideoVm::view)
        .run()
}
