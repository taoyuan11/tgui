use std::path::PathBuf;

use crate::app::{App, audio_status_text};
use crate::demo_section::{self, UsageDemo};
use crate::styles;
use tgui::prelude::*;

const CODE_IMAGE_PATH: &str = r#"Image::from_path(demo_image_path())
    .size(dp(260.0), dp(150.0))
    .style_full(image_style)"#;

const CODE_IMAGE_EVENTS: &str = r#"Image::from_path(demo_image_path())
    .on_success(Command::new(|app: &mut App| {
        app.toast_status.set("图片加载完成".to_string());
    }))"#;

const CODE_CANVAS_GRADIENT: &str = r#"Canvas::new(CanvasRecorder::build(|canvas| {
    canvas
        .set_fill(CanvasLinearGradient::new(start, end, stops))
        .begin_path()
        .move_to(24.0, 20.0)
        .line_to(208.0, 20.0)
        .close_path()
        .fill();
}))"#;

const CODE_CANVAS_PATH: &str = r#"canvas
    .set_fill(Color::hexa(0x22C55EFF))
    .set_stroke(CanvasStroke::new(dp(3.0), Color::hexa(0x14532DFF)))
    .begin_path()
    .move_to(44.0, 146.0)
    .quad_to(116.0, 92.0, 188.0, 146.0)
    .fill_and_stroke();"#;

const CODE_AUDIO_SAFE_LOAD: &str = r#"Input::new(app.input_text.clone())
    .placeholder("输入音频文件路径或 URL")

Button::new("加载").on_click(Command::new(App::load_audio_from_input))"#;

const CODE_AUDIO_PLAYBACK: &str = r#"Audio::new(app.audio_controller.clone())

Button::new("播放").on_click(Command::new(|app: &mut App| {
    app.audio_controller.play();
}))"#;

const CODE_VIDEO_SAFE_LOAD: &str = r#"Input::new(app.video_player.source.clone())
    .placeholder("输入视频文件路径或 URL")

Button::new("加载").on_click(Command::new(|app: &mut App| {
    app.video_player.load_from_input();
}))"#;

const CODE_VIDEO_SURFACE: &str = r#"VideoSurface::new(app.video_player.controller.clone())
    .size(dp(360.0), dp(202.0))"#;

pub(crate) fn page(app: &App) -> Element<App> {
    demo_section::page(
        "Media & Canvas",
        "媒体页面展示图片、Canvas，以及默认不加载本机资源的音视频控制器。",
        vec![
            image_component(app),
            canvas_component(app),
            audio_component(app),
            video_component(app),
        ],
    )
}

fn image_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Image",
        "Image 支持本地路径、URL 和内存字节；本 demo 使用仓库已有图片资源。",
        vec![
            UsageDemo::new(
                "image/path",
                "本地图片",
                "从示例资源目录加载图片，并应用圆角样式。",
                Image::from_path(demo_image_path())
                    .size(dp(260.0), dp(150.0))
                    .style_full(styles::image_style),
                CODE_IMAGE_PATH,
            ),
            UsageDemo::new(
                "image/events",
                "媒体事件",
                "加载成功或失败可以回调到 ViewModel。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Image::from_path(demo_image_path())
                        .size(dp(180.0), dp(100.0))
                        .style_full(styles::image_style)
                        .on_success(Command::new(|app: &mut App| {
                            app.toast_status.set("图片加载完成".to_string());
                        })),
                    Text::new(app.toast_status.signal()).style_full(styles::status_style),
                ]),
                CODE_IMAGE_EVENTS,
            ),
        ],
    )
}

fn canvas_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Canvas",
        "Canvas 使用 recorder 构建 retained 绘图命令，最终落到 renderer primitive。",
        vec![
            UsageDemo::new(
                "canvas/gradient",
                "渐变矩形",
                "线性渐变、路径和描边可以组合生成基础图形。",
                demo_canvas_gradient(),
                CODE_CANVAS_GRADIENT,
            ),
            UsageDemo::new(
                "canvas/path",
                "曲线路径",
                "二次贝塞尔路径适合绘制简单图标、形状和装饰。",
                demo_canvas_path(),
                CODE_CANVAS_PATH,
            ),
        ],
    )
}

fn audio_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Audio",
        "Audio 是隐形播放组件；默认不加载任何本机路径，用户显式输入后再加载。",
        vec![
            UsageDemo::new(
                "audio/safe-load",
                "安全加载",
                "输入文件路径或 URL 后点击加载，空输入只更新状态。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Audio::new(app.audio_controller.clone()),
                    Input::new(app.input_text.clone())
                        .width(dp(420.0))
                        .placeholder("输入音频文件路径或 URL"),
                    Button::new("加载").on_click(Command::new(App::load_audio_from_input)),
                    Text::new(app.audio_status.signal()).style_full(styles::status_style),
                ]),
                CODE_AUDIO_SAFE_LOAD,
            ),
            UsageDemo::new(
                "audio/playback",
                "播放控制",
                "播放、暂停和状态文本都由 AudioController 提供。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Flex::horizontal().gap(dp(8.0)).child(el![
                        Button::new("播放").on_click(Command::new(|app: &mut App| {
                            app.audio_controller.play();
                        })),
                        Button::new("暂停").on_click(Command::new(|app: &mut App| {
                            app.audio_controller.pause();
                        })),
                    ]),
                    Text::new(app.audio_controller.playback_state().map(audio_status_text))
                        .style_full(styles::status_style),
                ]),
                CODE_AUDIO_PLAYBACK,
            ),
        ],
    )
}

fn video_component(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Video",
        "VideoSurface 参与布局和渲染；默认只显示空 surface 与安全加载控件。",
        vec![
            UsageDemo::new(
                "video/safe-load",
                "安全加载",
                "输入视频文件路径或 URL 后加载，避免启动 demo 时依赖本机文件。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    Input::new(app.video_player.source.clone())
                        .width(dp(420.0))
                        .placeholder("输入视频文件路径或 URL"),
                    Button::new("加载").on_click(Command::new(|app: &mut App| {
                        app.video_player.load_from_input();
                    })),
                    Text::new(app.video_player.status.signal()).style_full(styles::status_style),
                ]),
                CODE_VIDEO_SAFE_LOAD,
            ),
            UsageDemo::new(
                "video/surface",
                "视频 Surface",
                "VideoSurface 保持固定宽高，播放控制由 VideoController 提供。",
                Flex::vertical().gap(dp(8.0)).child(el![
                    VideoSurface::new(app.video_player.controller.clone())
                        .size(dp(360.0), dp(202.0)),
                    Flex::horizontal().gap(dp(8.0)).child(el![
                        Button::new("播放").on_click(Command::new(|app: &mut App| {
                            app.video_player.controller.play();
                        })),
                        Button::new("暂停").on_click(Command::new(|app: &mut App| {
                            app.video_player.controller.pause();
                        })),
                    ]),
                    Text::new(app.video_player.playback_status()).style_full(styles::status_style),
                ]),
                CODE_VIDEO_SURFACE,
            ),
        ],
    )
}

fn demo_image_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../background_effects/assets/juequling_shushu.jpg")
}

fn demo_canvas_gradient() -> Element<App> {
    Canvas::new(CanvasRecorder::build(|canvas| {
        canvas
            .set_fill(CanvasLinearGradient::new(
                Point::new(24.0, 20.0),
                Point::new(208.0, 128.0),
                vec![
                    CanvasGradientStop::new(0.0, Color::hexa(0x38BDF8FF)),
                    CanvasGradientStop::new(1.0, Color::hexa(0x1D4ED8FF)),
                ],
            ))
            .set_stroke(CanvasStroke::new(dp(3.0), Color::hexa(0xE0F2FEFF)))
            .begin_path()
            .move_to(24.0, 20.0)
            .line_to(208.0, 20.0)
            .line_to(208.0, 128.0)
            .line_to(24.0, 128.0)
            .close_path()
            .fill_and_stroke();
    }))
    .size(dp(232.0), dp(160.0))
    .style_full(styles::canvas_style)
    .into()
}

fn demo_canvas_path() -> Element<App> {
    Canvas::new(CanvasRecorder::build(|canvas| {
        canvas
            .set_fill(Color::hexa(0x22C55EFF))
            .set_stroke(CanvasStroke::new(dp(3.0), Color::hexa(0x14532DFF)))
            .begin_path()
            .move_to(44.0, 44.0)
            .quad_to(116.0, 0.0, 188.0, 44.0)
            .line_to(188.0, 112.0)
            .line_to(44.0, 112.0)
            .close_path()
            .fill_and_stroke();
    }))
    .size(dp(232.0), dp(150.0))
    .style_full(styles::canvas_style)
    .into()
}
