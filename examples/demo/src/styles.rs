use std::sync::OnceLock;

use tgui::prelude::*;

/// Demo chrome switches as one coherent palette when the resolved system mode changes.
///
/// `StyleContext::theme` may contain an in-progress interpolation for the subset of
/// theme colors that animate. Other foreground tokens switch immediately, which can
/// temporarily pair light text with dark custom surfaces. The component gallery uses
/// the canonical token set for the resolved mode so all of its custom chrome changes
/// atomically; built-in controls keep their framework-managed transitions.
fn demo_colors(ctx: &StyleContext<'_>) -> &'static ColorScheme {
    static LIGHT: OnceLock<ColorScheme> = OnceLock::new();
    static DARK: OnceLock<ColorScheme> = OnceLock::new();

    match ctx.mode {
        ResolvedThemeMode::Light => LIGHT.get_or_init(ColorScheme::light),
        ResolvedThemeMode::Dark => DARK.get_or_init(ColorScheme::dark),
    }
}

fn text_style(ctx: &StyleContext<'_>, typography: TextStyle, color: Color) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography = typography;
    style.color = color.into();
    style
}

pub(crate) fn title_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    text_style(
        ctx,
        ctx.theme.typography.headline.clone(),
        demo_colors(ctx).on_background,
    )
}

pub(crate) fn page_description_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    text_style(
        ctx,
        ctx.theme.typography.body.clone(),
        demo_colors(ctx).on_surface_muted,
    )
}

pub(crate) fn section_title_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    text_style(
        ctx,
        ctx.theme.typography.title.clone(),
        demo_colors(ctx).on_surface,
    )
}

pub(crate) fn usage_title_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    text_style(
        ctx,
        ctx.theme.typography.label.clone(),
        demo_colors(ctx).on_surface,
    )
}

pub(crate) fn status_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    text_style(
        ctx,
        ctx.theme.typography.body_small.clone(),
        demo_colors(ctx).on_surface_muted,
    )
}

pub(crate) fn field_label_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    text_style(
        ctx,
        ctx.theme.typography.label.clone(),
        demo_colors(ctx).on_surface,
    )
}

pub(crate) fn error_text_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    text_style(
        ctx,
        ctx.theme.typography.label_small.clone(),
        demo_colors(ctx).error,
    )
}

pub(crate) fn code_text_style(ctx: &StyleContext<'_>) -> TextWidgetStyle {
    text_style(
        ctx,
        ctx.theme.typography.code.clone(),
        demo_colors(ctx).on_surface,
    )
}

pub(crate) fn root_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(demo_colors(ctx).background.into());
    style
}

pub(crate) fn sidebar_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(demo_colors(ctx).surface.into());
    style.surface.border_color = Some(demo_colors(ctx).outline_muted.into());
    style.surface.border_width = Some(ctx.theme.border.thin.into());
    style
}

pub(crate) fn nav_item_style(ctx: &StyleContext<'_>, active: bool) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    if active {
        style.surface.background = Some(demo_colors(ctx).primary_container.into());
    }
    style.surface.border_radius = Some(ctx.theme.radius.lg.into());
    style
}

pub(crate) fn nav_icon_style(ctx: &StyleContext<'_>, active: bool) -> IconStyle {
    let mut style = IconStyle::default_for_theme(ctx.theme);
    style.color = if active {
        demo_colors(ctx).on_primary_container
    } else {
        demo_colors(ctx).on_surface_muted
    }
    .into();
    style.size = ctx.theme.spacing.md + ctx.theme.spacing.xxs;
    style
}

pub(crate) fn nav_title_style(ctx: &StyleContext<'_>, active: bool) -> TextWidgetStyle {
    text_style(
        ctx,
        ctx.theme.typography.label.clone(),
        if active {
            demo_colors(ctx).on_primary_container
        } else {
            demo_colors(ctx).on_surface
        },
    )
}

pub(crate) fn nav_description_style(ctx: &StyleContext<'_>, active: bool) -> TextWidgetStyle {
    text_style(
        ctx,
        ctx.theme.typography.label_small.clone(),
        if active {
            demo_colors(ctx).on_primary_container
        } else {
            demo_colors(ctx).on_surface_muted
        },
    )
}

pub(crate) fn usage_card_style(ctx: &StyleContext<'_>) -> CardStyle {
    let mut style = CardStyle::default_for_theme(ctx.theme);
    style.background = demo_colors(ctx).surface.into();
    style.border = demo_colors(ctx).outline_muted.into();
    style.border_width = ctx.theme.border.thin;
    style.radius = ctx.theme.radius.lg;
    style.shadow = ctx.theme.elevation.none.clone();
    style.padding = Insets::all(ctx.theme.spacing.md);
    style.gap = ctx.theme.spacing.sm + ctx.theme.spacing.xs;
    style
}

pub(crate) fn preview_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(demo_colors(ctx).surface_low.into());
    style.surface.border_radius = Some(ctx.theme.radius.md.into());
    style
}

pub(crate) fn code_block_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(demo_colors(ctx).background.into());
    style.surface.border_color = Some(demo_colors(ctx).outline_muted.into());
    style.surface.border_width = Some(ctx.theme.border.thin.into());
    style.surface.border_radius = Some(ctx.theme.radius.lg.into());
    style
}

pub(crate) fn shadow_showcase_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(demo_colors(ctx).surface_overlay.into());
    style.surface.border_color = Some(demo_colors(ctx).outline_muted.into());
    style.surface.border_width = Some(ctx.theme.border.thin.into());
    style.surface.border_radius = Some(ctx.theme.radius.full.into());
    style.surface.shadow = Some(ctx.theme.elevation.lg.clone().into());
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typography_and_colors_follow_the_active_theme() {
        let light = Theme::light();
        let dark = Theme::dark();
        let light_context = StyleContext::from_theme(&light);
        let dark_context = StyleContext::from_theme(&dark);

        let light_title = title_style(&light_context);
        let dark_title = title_style(&dark_context);
        assert_eq!(light_title.typography, light.typography.headline);
        assert_eq!(dark_title.typography, dark.typography.headline);
        assert_eq!(light_title.color.resolve(), light.colors.on_background);
        assert_eq!(dark_title.color.resolve(), dark.colors.on_background);

        let light_card = usage_card_style(&light_context);
        let dark_card = usage_card_style(&dark_context);
        assert_eq!(light_card.background.resolve(), light.colors.surface);
        assert_eq!(dark_card.background.resolve(), dark.colors.surface);
        assert_eq!(light_card.radius, light.radius.lg);
        assert_eq!(light_card.shadow, light.elevation.none);
        assert_eq!(dark_card.shadow, dark.elevation.none);
    }

    #[test]
    fn shell_surfaces_switch_together_from_dark_to_light_mode() {
        let light_tokens = ColorScheme::light();
        let dark_tokens = ColorScheme::dark();

        // Reproduce the first frame of the framework theme transition: the resolved
        // mode and non-animated foregrounds are light, while animated surface tokens
        // can still carry their dark starting values.
        let mut transitioning_to_light = Theme::light();
        transitioning_to_light.colors.background = dark_tokens.background;
        transitioning_to_light.colors.surface = dark_tokens.surface;
        transitioning_to_light.colors.surface_low = dark_tokens.surface_low;
        let light_context = StyleContext::from_theme(&transitioning_to_light);

        let light_root = root_style(&light_context);
        let light_sidebar = sidebar_style(&light_context);
        let light_card = usage_card_style(&light_context);
        let light_preview = preview_style(&light_context);
        assert_eq!(
            light_root.surface.background.unwrap().resolve(),
            light_tokens.background
        );
        assert_eq!(
            light_sidebar.surface.background.unwrap().resolve(),
            light_tokens.surface
        );
        assert_eq!(light_card.background.resolve(), light_tokens.surface);
        assert_eq!(
            light_preview.surface.background.unwrap().resolve(),
            light_tokens.surface_low
        );
        assert!(light_preview.surface.border_color.is_none());
        assert!(light_preview.surface.border_width.is_none());
        assert_eq!(
            title_style(&light_context).color.resolve(),
            light_tokens.on_background
        );

        let dark_theme = Theme::dark();
        let dark_context = StyleContext::from_theme(&dark_theme);
        let dark_root = root_style(&dark_context);
        let dark_sidebar = sidebar_style(&dark_context);
        let dark_card = usage_card_style(&dark_context);
        let dark_preview = preview_style(&dark_context);
        assert_eq!(
            dark_root.surface.background.unwrap().resolve(),
            dark_tokens.background
        );
        assert_eq!(
            dark_sidebar.surface.background.unwrap().resolve(),
            dark_tokens.surface
        );
        assert_eq!(dark_card.background.resolve(), dark_tokens.surface);
        assert_eq!(
            dark_preview.surface.background.unwrap().resolve(),
            dark_tokens.surface_low
        );
        assert!(dark_preview.surface.border_color.is_none());
        assert!(dark_preview.surface.border_width.is_none());

        assert_ne!(light_tokens.background, dark_tokens.background);
        assert_ne!(light_tokens.surface, dark_tokens.surface);
    }

    #[test]
    fn active_navigation_uses_primary_theme_tokens() {
        for theme in [Theme::light(), Theme::dark()] {
            let context = StyleContext::from_theme(&theme);
            let item = nav_item_style(&context, true);
            let active_icon = nav_icon_style(&context, true);
            let inactive_icon = nav_icon_style(&context, false);

            assert_eq!(
                item.surface.background.unwrap().resolve(),
                theme.colors.primary_container
            );
            assert_eq!(
                active_icon.color.resolve(),
                theme.colors.on_primary_container
            );
            assert_eq!(inactive_icon.color.resolve(), theme.colors.on_surface_muted);
            assert_eq!(
                nav_title_style(&context, true).color.resolve(),
                theme.colors.on_primary_container
            );
        }
    }
}
