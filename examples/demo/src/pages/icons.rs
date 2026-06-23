use crate::app::App;
use crate::demo_section::{self, UsageDemo};
use crate::styles;
use tgui::prelude::*;

const CODE_ICON_GRID: &str = r#"Icon::builtin(BuiltinIcon::Search)
    .size(dp(28.0))
    .style(|style, _ctx| {
        style.color = Color::hexa(0x2563EBFF).into();
    })"#;

const CODE_ICON_SIZES: &str = r#"Flex::horizontal().gap(dp(16.0)).align(Align::Center).child(el![
    Icon::builtin(BuiltinIcon::Bell).size(dp(18.0)),
    Icon::builtin(BuiltinIcon::Bell).size(dp(28.0)),
    Icon::builtin(BuiltinIcon::Bell).size(dp(40.0)),
])"#;

const CODE_ICON_COLORS: &str = r#"Flex::horizontal().gap(dp(12.0)).child(el![
    Icon::builtin(BuiltinIcon::Success).style(|style, _| {
        style.color = Color::hexa(0x22C55EFF).into();
    }),
    Icon::builtin(BuiltinIcon::Warning).style(|style, _| {
        style.color = Color::hexa(0xF59E0BFF).into();
    }),
    Icon::builtin(BuiltinIcon::Error).style(|style, _| {
        style.color = Color::hexa(0xEF4444FF).into();
    }),
])"#;

const COMMON_ICONS: &[(BuiltinIcon, &str)] = &[
    (BuiltinIcon::Search, "Search"),
    (BuiltinIcon::Home, "Home"),
    (BuiltinIcon::Settings, "Settings"),
    (BuiltinIcon::Bell, "Bell"),
    (BuiltinIcon::Mail, "Mail"),
    (BuiltinIcon::User, "User"),
    (BuiltinIcon::Lock, "Lock"),
    (BuiltinIcon::Unlock, "Unlock"),
    (BuiltinIcon::Eye, "Eye"),
    (BuiltinIcon::EyeOff, "EyeOff"),
    (BuiltinIcon::Info, "Info"),
    (BuiltinIcon::Success, "Success"),
    (BuiltinIcon::Warning, "Warning"),
    (BuiltinIcon::Error, "Error"),
    (BuiltinIcon::Calendar, "Calendar"),
    (BuiltinIcon::Clock, "Clock"),
    (BuiltinIcon::Image, "Image"),
    (BuiltinIcon::File, "File"),
    (BuiltinIcon::Folder, "Folder"),
    (BuiltinIcon::Upload, "Upload"),
    (BuiltinIcon::Download, "Download"),
    (BuiltinIcon::Edit, "Edit"),
    (BuiltinIcon::Copy, "Copy"),
    (BuiltinIcon::Trash, "Trash"),
    (BuiltinIcon::RefreshCw, "RefreshCw"),
    (BuiltinIcon::ExternalLink, "ExternalLink"),
    (BuiltinIcon::Filter, "Filter"),
    (BuiltinIcon::SortAsc, "SortAsc"),
    (BuiltinIcon::SortDesc, "SortDesc"),
    (BuiltinIcon::Menu, "Menu"),
    (BuiltinIcon::MoreHorizontal, "MoreHorizontal"),
    (BuiltinIcon::ChevronLeft, "ChevronLeft"),
    (BuiltinIcon::ChevronRight, "ChevronRight"),
    (BuiltinIcon::ChevronUp, "ChevronUp"),
    (BuiltinIcon::ChevronDown, "ChevronDown"),
    (BuiltinIcon::Plus, "Plus"),
    (BuiltinIcon::Minus, "Minus"),
    (BuiltinIcon::Close, "Close"),
    (BuiltinIcon::Check, "Check"),
    (BuiltinIcon::Star, "Star"),
    (BuiltinIcon::StarHalf, "StarHalf"),
    (BuiltinIcon::Heart, "Heart"),
    (BuiltinIcon::Palette, "Palette"),
    (BuiltinIcon::MapPin, "MapPin"),
    (BuiltinIcon::Link, "Link"),
    (BuiltinIcon::Play, "Play"),
    (BuiltinIcon::Pause, "Pause"),
    (BuiltinIcon::VolumeUp, "VolumeUp"),
    (BuiltinIcon::VolumeDown, "VolumeDown"),
    (BuiltinIcon::VolumeOff, "VolumeOff"),
];

pub(crate) fn page(app: &App) -> Element<App> {
    demo_section::page(
        "Icons",
        "内置图标现在覆盖导航、文件、表单、媒体、反馈和常用操作场景，全部通过 Icon::builtin 使用。",
        vec![icon_gallery(app), icon_variants(app)],
    )
}

fn icon_gallery(app: &App) -> Element<App> {
    demo_section::component_doc_stacked(
        app,
        "Builtin Icons",
        "BuiltinIcon 提供一组轻量 SVG 图标，可继承主题色，也可单独设置颜色和尺寸。",
        vec![UsageDemo::new(
            "icons/gallery",
            "常用图标总览",
            "每个图标项展示公开枚举名，适合直接复制到业务组件里。",
            icon_grid(),
            CODE_ICON_GRID,
        )],
    )
}

fn icon_variants(app: &App) -> Element<App> {
    demo_section::component_doc(
        app,
        "Icon Styling",
        "Icon 使用统一 style API 控制颜色和尺寸，仍然参与普通布局。",
        vec![
            UsageDemo::new(
                "icons/sizes",
                "尺寸",
                "同一图标可以用组件尺寸快速适配工具栏、按钮或空状态。",
                Flex::horizontal()
                    .gap(dp(16.0))
                    .align(Align::Center)
                    .child(el![
                        Icon::builtin(BuiltinIcon::Bell).size(dp(18.0)),
                        Icon::builtin(BuiltinIcon::Bell).size(dp(28.0)),
                        Icon::builtin(BuiltinIcon::Bell).size(dp(40.0)),
                    ]),
                CODE_ICON_SIZES,
            ),
            UsageDemo::new(
                "icons/colors",
                "语义颜色",
                "通过 IconStyle::color 可表达成功、警告、危险等状态。",
                Flex::horizontal()
                    .gap(dp(12.0))
                    .align(Align::Center)
                    .child(el![
                        Icon::builtin(BuiltinIcon::Success)
                            .size(dp(30.0))
                            .style(|style, _| {
                                style.color = Color::hexa(0x22C55EFF).into();
                            }),
                        Icon::builtin(BuiltinIcon::Warning)
                            .size(dp(30.0))
                            .style(|style, _| {
                                style.color = Color::hexa(0xF59E0BFF).into();
                            }),
                        Icon::builtin(BuiltinIcon::Error)
                            .size(dp(30.0))
                            .style(|style, _| {
                                style.color = Color::hexa(0xEF4444FF).into();
                            }),
                    ]),
                CODE_ICON_COLORS,
            ),
        ],
    )
}

fn icon_grid() -> Element<App> {
    let items = COMMON_ICONS
        .iter()
        .copied()
        .map(|(icon, label)| icon_tile(icon, label))
        .collect::<Vec<_>>();

    Flex::horizontal()
        .width(pct(100.0))
        .gap(dp(10.0))
        .wrap(Wrap::Wrap)
        .child(items)
        .into()
}

fn icon_tile(icon: BuiltinIcon, label: &'static str) -> Element<App> {
    Flex::vertical()
        .width(dp(112.0))
        .height(dp(106.0))
        .gap(dp(8.0))
        .center()
        .padding(Insets::all(dp(10.0)))
        .style_full(icon_tile_style)
        .child(Icon::builtin(icon).size(dp(28.0)).style(|style, _ctx| {
            style.color = Color::hexa(0x2563EBFF).into();
        }))
        .child(
            Text::new(label)
                .max_width(dp(96.0))
                .align_self(Align::Center)
                .style_full(icon_label_style)
                .user_select(true),
        )
        .into()
}

fn icon_tile_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mode = ctx.mode;
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(
        match mode {
            ResolvedThemeMode::Light => Color::hexa(0xF8FAFCFF),
            ResolvedThemeMode::Dark => Color::hexa(0x0F172AFF),
        }
        .into(),
    );
    style.surface.border_color = Some(
        match mode {
            ResolvedThemeMode::Light => Color::hexa(0xE2E8F0FF),
            ResolvedThemeMode::Dark => Color::hexa(0x334155FF),
        }
        .into(),
    );
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(8.0).into());
    style
}

fn icon_label_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = styles::muted_text_style(ctx, sp(12.0));
    style.typography.weight = FontWeight::Medium;
    style
}
