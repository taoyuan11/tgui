#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::error::Error;
use std::path::PathBuf;
use std::sync::OnceLock;

use tgui::logging::{configure_logging, LogConfig, LogFileConfig};
use tgui::platform::AppPaths;
use tgui::prelude::*;

const APP_ID: &str = "com.tgui.packaging-example";
static PATHS: OnceLock<AppPaths> = OnceLock::new();

struct PackagingVm {
    message: String,
    #[cfg(feature = "media")]
    audio: tgui::audio::AudioController,
    #[cfg(feature = "media")]
    video: tgui::video::VideoController,
}

impl ViewModel for PackagingVm {
    fn new(_ctx: &ViewModelContext) -> Self {
        let paths = PATHS.get().expect("paths initialized before application");
        #[cfg(feature = "media")]
        let (audio, video) = {
            let audio = tgui::audio::AudioController::new(_ctx);
            audio
                .load(paths.resource("assets/tone.wav").unwrap())
                .expect("load packaged audio");
            let video = tgui::video::VideoController::new(_ctx);
            video
                .load(paths.resource("assets/clip.mkv").unwrap())
                .expect("load packaged video");
            (audio, video)
        };
        Self {
            message: std::fs::read_to_string(paths.resource("assets/welcome.txt").unwrap())
                .expect("read packaged text"),
            #[cfg(feature = "media")]
            audio,
            #[cfg(feature = "media")]
            video,
        }
    }

    fn view(&self) -> Element<Self> {
        let paths = PATHS.get().unwrap();
        let content = Flex::vertical()
            .width(pct(100.0))
            .padding(Insets::all(dp(24.0)))
            .gap(dp(12.0))
            .child(Text::new(format!(
                "tgui Packaging {}",
                env!("CARGO_PKG_VERSION")
            )))
            .child(Text::new(self.message.clone()).width(pct(100.0)))
            .child(
                Image::new(MediaSource::path(
                    paths.resource("assets/icon.png").unwrap(),
                ))
                .size(dp(96.0), dp(96.0)),
            )
            .child(Text::new(
                "Resources loaded from the installed application.",
            ))
            .child(Text::new(
                "User data and logs are stored outside the installation.",
            ));
        #[cfg(feature = "media")]
        let content = content
            .child(
                Button::new("Play packaged audio")
                    .on_click(Command::new(|vm: &mut Self| vm.audio.play())),
            )
            .child(
                Button::new("Pause audio").on_click(Command::new(|vm: &mut Self| vm.audio.pause())),
            )
            .child(
                tgui::video::Video::new(self.video.clone())
                    .width(dp(400.0))
                    .height(dp(240.0)),
            );
        ScrollView::new()
            .size(pct(100.0), pct(100.0))
            .overflow_x(Overflow::Hidden)
            .child(content)
            .into()
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut smoke = false;
    let mut paths = AppPaths::new(APP_ID)?;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--smoke-test" {
            smoke = true;
        } else if arg == "--resource-dir" {
            let directory = args
                .next()
                .ok_or("--resource-dir requires an absolute path")?;
            paths = paths.with_resource_dir(PathBuf::from(directory))?;
        } else {
            return Err(format!("unknown argument: {}", arg.to_string_lossy()).into());
        }
    }
    std::fs::create_dir_all(paths.data_dir())?;
    std::fs::create_dir_all(paths.log_dir())?;
    configure_logging(LogConfig {
        level: LogLevel::Info,
        file: Some(LogFileConfig {
            log_dir: paths.log_dir().into(),
            file_name: "packaging.log".into(),
            max_file_size_bytes: 1024 * 1024,
            max_files: 2,
        }),
    })?;
    if smoke {
        smoke_test(&paths)?;
        println!(
            "packaging smoke test passed ({})",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    PATHS.set(paths).map_err(|_| "paths already initialized")?;
    Application::new()
        .app_id(APP_ID)
        .title("tgui Packaging")
        .window_icon(include_bytes!("../assets/icon.png"))
        .window_size(dp(720.0), dp(680.0))
        .with_view_model(PackagingVm::new)
        .root_view(PackagingVm::view)
        .run()?;
    Ok(())
}

fn smoke_test(paths: &AppPaths) -> Result<(), Box<dyn Error>> {
    let message = std::fs::read_to_string(paths.resource("assets/welcome.txt")?)?;
    if !message.contains("Packaged resources are available") {
        return Err("incorrect packaged text".into());
    }
    let icon = std::fs::read(paths.resource("assets/icon.png")?)?;
    if !icon.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("incorrect packaged icon".into());
    }
    // These markers also let the installation/upgrade test assert data preservation.
    let marker = paths.data_dir().join("installation-smoke.txt");
    if marker.exists() {
        if std::fs::read_to_string(&marker)? != "preserve across upgrade" {
            return Err("user data changed".into());
        }
    } else {
        std::fs::write(&marker, "preserve across upgrade")?;
    }
    std::fs::write(
        paths.log_dir().join("smoke.log"),
        "resource and user path checks passed\n",
    )?;
    #[cfg(feature = "media")]
    media_smoke(paths)?;
    Ok(())
}

#[cfg(feature = "media")]
fn media_smoke(paths: &AppPaths) -> Result<(), Box<dyn Error>> {
    use ffmpeg_next as ffmpeg;
    ffmpeg::init()?;
    let mut input = ffmpeg::format::input(&paths.resource("assets/tone.wav")?)?;
    let stream = input
        .streams()
        .best(ffmpeg::media::Type::Audio)
        .ok_or("missing audio stream")?;
    let index = stream.index();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?
        .decoder()
        .audio()?;
    let mut audio_frames = 0;
    for (stream, packet) in input.packets() {
        if stream.index() == index {
            decoder.send_packet(&packet)?;
            let mut frame = ffmpeg::util::frame::Audio::empty();
            while decoder.receive_frame(&mut frame).is_ok() {
                audio_frames += 1;
            }
        }
    }
    decoder.send_eof()?;
    while decoder
        .receive_frame(&mut ffmpeg::util::frame::Audio::empty())
        .is_ok()
    {
        audio_frames += 1;
    }
    if audio_frames == 0 {
        return Err("no audio frames decoded".into());
    }

    let mut input = ffmpeg::format::input(&paths.resource("assets/clip.mkv")?)?;
    let stream = input
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or("missing video stream")?;
    let index = stream.index();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?
        .decoder()
        .video()?;
    let mut video_frames = 0;
    for (stream, packet) in input.packets() {
        if stream.index() == index {
            decoder.send_packet(&packet)?;
            let mut frame = ffmpeg::util::frame::Video::empty();
            while decoder.receive_frame(&mut frame).is_ok() {
                video_frames += 1;
            }
        }
    }
    decoder.send_eof()?;
    while decoder
        .receive_frame(&mut ffmpeg::util::frame::Video::empty())
        .is_ok()
    {
        video_frames += 1;
    }
    if video_frames == 0 {
        return Err("no video frames decoded".into());
    }
    println!("decoded {audio_frames} audio frames and {video_frames} video frames");
    Ok(())
}
