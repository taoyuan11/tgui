use tgui::prelude::*;

fn with_alpha(color: Color, factor: f32) -> Color {
    let alpha = ((color.a as f32) * factor.clamp(0.0, 1.0)).round() as u8;
    Color::rgba(color.r, color.g, color.b, alpha)
}

pub(crate) fn text_style(ctx: &StyleContext<'_>, size: Sp) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = size;
    style
}

fn primary_text_color(mode: ResolvedThemeMode) -> Color {
    match mode {
        ResolvedThemeMode::Light => Color::hexa(0x0F172AFF),
        ResolvedThemeMode::Dark => Color::hexa(0xF8FAFCFF),
    }
}

pub(crate) fn title_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = text_style(ctx, sp(28.0));
    style.typography.weight = FontWeight::SemiBold;
    style.color = primary_text_color(ctx.mode).into();
    style
}

pub(crate) fn section_title_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = text_style(ctx, sp(20.0));
    style.typography.weight = FontWeight::SemiBold;
    style.color = primary_text_color(ctx.mode).into();
    style
}

pub(crate) fn usage_title_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mut style = text_style(ctx, sp(16.0));
    style.typography.weight = FontWeight::SemiBold;
    style.color = primary_text_color(ctx.mode).into();
    style
}

pub(crate) fn status_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    muted_text_style(ctx, sp(14.0))
}

pub(crate) fn muted_text_style(ctx: &StyleContext<'_>, size: Sp) -> TextWidgetStyle {
    let mode = ctx.mode;
    let mut style = text_style(ctx, size);
    style.color = match mode {
        ResolvedThemeMode::Light => Color::hexa(0x475569FF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0xCBD5E1FF).into(),
    };
    style
}

pub(crate) fn code_text_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    let mode = ctx.mode;
    let mut style = text_style(ctx, sp(13.0));
    style.color = match mode {
        ResolvedThemeMode::Light => Color::hexa(0x1F2937FF).into(),
        ResolvedThemeMode::Dark => Color::hexa(0xE5E7EBFF).into(),
    };
    style
}

pub(crate) fn root_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mode = ctx.mode;
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(
        match mode {
            ResolvedThemeMode::Light => Color::hexa(0xF8FAFCFF),
            ResolvedThemeMode::Dark => Color::hexa(0x0B1120FF),
        }
        .into(),
    );
    style
}

pub(crate) fn sidebar_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mode = ctx.mode;
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(
        match mode {
            ResolvedThemeMode::Light => Color::hexa(0xFFFFFFFF),
            ResolvedThemeMode::Dark => Color::hexa(0x111827FF),
        }
        .into(),
    );
    style.surface.border_color = Some(
        match mode {
            ResolvedThemeMode::Light => Color::hexa(0xE2E8F0FF),
            ResolvedThemeMode::Dark => Color::hexa(0x243044FF),
        }
        .into(),
    );
    style.surface.border_width = Some(dp(1.0).into());
    style
}

pub(crate) fn nav_item_style(
    ctx: &StyleContext<'_>,
    active: bool,
    accent: u32,
) -> ContainerStyle {
    let mode = ctx.mode;
    let accent = Color::hexa(accent);
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(
        match (mode, active) {
            (ResolvedThemeMode::Light, true) => with_alpha(accent, 0.12),
            (ResolvedThemeMode::Dark, true) => with_alpha(accent, 0.18),
            (ResolvedThemeMode::Light, false) => Color::hexa(0xF8FAFC00),
            (ResolvedThemeMode::Dark, false) => Color::hexa(0x11182700),
        }
        .into(),
    );
    style.surface.border_color = Some(
        match (mode, active) {
            (ResolvedThemeMode::Light, true) => with_alpha(accent, 0.38),
            (ResolvedThemeMode::Dark, true) => with_alpha(accent, 0.52),
            (ResolvedThemeMode::Light, false) => Color::hexa(0xE2E8F000),
            (ResolvedThemeMode::Dark, false) => Color::hexa(0x33415500),
        }
        .into(),
    );
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(8.0).into());
    if active {
        style.surface.shadow = Some(
            Shadow {
                offset_x: dp(0.0),
                offset_y: dp(8.0),
                blur: dp(18.0),
                spread: dp(-12.0),
                color: with_alpha(
                    accent,
                    match mode {
                        ResolvedThemeMode::Light => 0.30,
                        ResolvedThemeMode::Dark => 0.42,
                    },
                ),
            }
            .into(),
        );
    }
    style
}

pub(crate) fn nav_badge_style(
    ctx: &StyleContext<'_>,
    active: bool,
    accent: u32,
) -> ContainerStyle {
    let mode = ctx.mode;
    let accent = Color::hexa(accent);
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(
        if active {
            accent
        } else {
            match mode {
                ResolvedThemeMode::Light => with_alpha(accent, 0.14),
                ResolvedThemeMode::Dark => with_alpha(accent, 0.22),
            }
        }
        .into(),
    );
    style.surface.border_color = Some(
        match mode {
            ResolvedThemeMode::Light => with_alpha(accent, if active { 0.0 } else { 0.24 }),
            ResolvedThemeMode::Dark => with_alpha(accent, if active { 0.0 } else { 0.36 }),
        }
        .into(),
    );
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(8.0).into());
    style
}

pub(crate) fn nav_badge_text_style(ctx: &StyleContext<'_>, active: bool) -> TextWidgetStyle {
    let mode = ctx.mode;
    let mut style = text_style(ctx, sp(13.0));
    style.typography.weight = FontWeight::SemiBold;
    style.color = if active {
        Color::hexa(0xFFFFFFFF).into()
    } else {
        match mode {
            ResolvedThemeMode::Light => Color::hexa(0x334155FF),
            ResolvedThemeMode::Dark => Color::hexa(0xE2E8F0FF),
        }
        .into()
    };
    style
}

pub(crate) fn nav_title_style(ctx: &StyleContext<'_>, active: bool) -> TextWidgetStyle {
    let mode = ctx.mode;
    let mut style = text_style(ctx, sp(14.0));
    style.typography.weight = FontWeight::SemiBold;
    style.color = match (mode, active) {
        (ResolvedThemeMode::Light, true) => Color::hexa(0x0F172AFF),
        (ResolvedThemeMode::Light, false) => Color::hexa(0x334155FF),
        (ResolvedThemeMode::Dark, true) => Color::hexa(0xF8FAFCFF),
        (ResolvedThemeMode::Dark, false) => Color::hexa(0xCBD5E1FF),
    }
    .into();
    style
}

pub(crate) fn nav_description_style(ctx: &StyleContext<'_>, active: bool) -> TextWidgetStyle {
    let mode = ctx.mode;
    let mut style = text_style(ctx, sp(12.0));
    style.color = match (mode, active) {
        (ResolvedThemeMode::Light, true) => Color::hexa(0x475569FF),
        (ResolvedThemeMode::Light, false) => Color::hexa(0x64748BFF),
        (ResolvedThemeMode::Dark, true) => Color::hexa(0xD7DEE8FF),
        (ResolvedThemeMode::Dark, false) => Color::hexa(0x94A3B8FF),
    }
    .into();
    style
}

pub(crate) fn component_card_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mode = ctx.mode;
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(
        match mode {
            ResolvedThemeMode::Light => Color::hexa(0xFFFFFFFF),
            ResolvedThemeMode::Dark => Color::hexa(0x111827FF),
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

pub(crate) fn usage_card_style(ctx: &StyleContext<'_>) -> ContainerStyle {
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
            ResolvedThemeMode::Dark => Color::hexa(0x253145FF),
        }
        .into(),
    );
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(8.0).into());
    style
}

pub(crate) fn preview_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mode = ctx.mode;
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(
        match mode {
            ResolvedThemeMode::Light => Color::hexa(0xFFFFFFFF),
            ResolvedThemeMode::Dark => Color::hexa(0x172033FF),
        }
        .into(),
    );
    style.surface.border_color = Some(
        match mode {
            ResolvedThemeMode::Light => Color::hexa(0xE5E7EBFF),
            ResolvedThemeMode::Dark => Color::hexa(0x334155FF),
        }
        .into(),
    );
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(8.0).into());
    style
}

pub(crate) fn code_block_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mode = ctx.mode;
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(
        match mode {
            ResolvedThemeMode::Light => Color::hexa(0xF1F5F9FF),
            ResolvedThemeMode::Dark => Color::hexa(0x020617FF),
        }
        .into(),
    );
    style.surface.border_color = Some(
        match mode {
            ResolvedThemeMode::Light => Color::hexa(0xCBD5E1FF),
            ResolvedThemeMode::Dark => Color::hexa(0x334155FF),
        }
        .into(),
    );
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(8.0).into());
    style
}

pub(crate) fn shadow_showcase_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(Color::hexa(0xFFFFFFFF).into());
    style.surface.border_radius = Some(dp(50.0).into());
    style.surface.shadow = Some(
        Shadow {
            offset_x: dp(0.0),
            offset_y: dp(7.0),
            blur: dp(30.0),
            spread: dp(0.0),
            color: Color::hex(0x64646F),
        }
        .into(),
    );
    style
}

pub(crate) fn accent_tooltip_style(ctx: &StyleContext<'_>) -> TooltipStyle {
    let mut style = TooltipStyle::default_for_theme(ctx.theme);
    style.background = Color::hexa(0x4F9CF9FF);
    style.foreground = Color::hexa(0x0B1220FF);
    style.radius = dp(8.0);
    style
}

pub(crate) fn popover_panel_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mode = ctx.mode;
    let mut style = component_card_style(ctx);
    style.surface.shadow = Some(
        Shadow {
            offset_x: dp(0.0),
            offset_y: dp(8.0),
            blur: dp(24.0),
            spread: dp(0.0),
            color: match mode {
                ResolvedThemeMode::Light => Color::rgba(0, 0, 0, 31),
                ResolvedThemeMode::Dark => Color::rgba(0, 0, 0, 102),
            },
        }
        .into(),
    );
    style.surface.background_blur = dp(16.0).into();
    style
}

pub(crate) fn image_style(ctx: &StyleContext<'_>) -> ImageStyle {
    let mut style = ImageStyle::default_for_theme(ctx.theme);
    style.surface.border_radius = Some(dp(8.0).into());
    style
}

pub(crate) fn canvas_style(ctx: &StyleContext<'_>) -> CanvasStyle {
    let mut style = CanvasStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(Color::rgb(15, 23, 42).into());
    style.surface.border_color = Some(Color::rgb(51, 65, 85).into());
    style.surface.border_width = Some(dp(1.0).into());
    style.surface.border_radius = Some(dp(8.0).into());
    style
}

pub(crate) fn modern_toast_style(ctx: &StyleContext<'_>) -> ToastStyle {
    let mode = ctx.mode;
    let mut style = ToastStyle::default_for_theme(ctx.theme);
    style.radius = Value::Static(dp(8.0));
    style.border_width = Value::Static(dp(0.0));
    style.padding = Insets::all(dp(12.0));
    style.gap = dp(8.0);
    style.shadow = Shadow {
        offset_x: dp(0.0),
        offset_y: dp(8.0),
        blur: dp(28.0),
        spread: dp(0.0),
        color: match mode {
            ResolvedThemeMode::Light => Color::rgba(0, 0, 0, 40),
            ResolvedThemeMode::Dark => Color::rgba(0, 0, 0, 120),
        },
    };
    style.success_icon_background = Value::Static(Color::hexa(0x10B981FF));
    style.success_icon_foreground = Value::Static(Color::hexa(0xFFFFFFFF));
    style.error_icon_background = Value::Static(Color::hexa(0xEF4444FF));
    style.error_icon_foreground = Value::Static(Color::hexa(0xFFFFFFFF));
    style.warning_icon_background = Value::Static(Color::hexa(0xF59E0BFF));
    style.warning_icon_foreground = Value::Static(Color::hexa(0xFFFFFFFF));
    style.info_icon_background = Value::Static(Color::hexa(0x3B82F6FF));
    style.info_icon_foreground = Value::Static(Color::hexa(0xFFFFFFFF));
    style.title_text_style.weight = FontWeight::SemiBold;
    style.action_button.radius = Value::Static(dp(6.0));
    style.min_width = dp(200.0);
    style.max_width = dp(320.0);
    style.stack_gap = dp(12.0);
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_title_style_changes_between_theme_modes() {
        let light = Theme::light();
        let dark = Theme::dark();
        let light_color = usage_title_style(&StyleContext::from_theme(&light))
            .color
            .resolve();
        let dark_color = usage_title_style(&StyleContext::from_theme(&dark))
            .color
            .resolve();

        assert_eq!(light_color, Color::hexa(0x0F172AFF));
        assert_eq!(dark_color, Color::hexa(0xF8FAFCFF));
        assert_ne!(light_color, dark_color);
    }
}
