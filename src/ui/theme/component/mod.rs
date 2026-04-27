mod button;
mod dialog;
mod input;
mod panel;
mod scrollbar;
mod switch;
mod text;
mod tooltip;

use crate::foundation::color::Color;
use crate::ui::theme::color::ColorScheme;
use crate::ui::theme::motion::MotionScale;
use crate::ui::theme::shape::{BorderScale, ElevationScale, RadiusScale};
use crate::ui::theme::spacing::SpaceScale;
use crate::ui::theme::state::Stateful;
use crate::ui::theme::typography::TypeScale;
use crate::ui::unit::Dp;

pub use button::{ButtonStyle, ButtonTheme, ButtonVariant};
pub use dialog::DialogTheme;
pub use input::{InputStyle, InputTheme};
pub use panel::PanelTheme;
pub use scrollbar::ScrollbarTheme;
pub use switch::{SwitchStyle, SwitchTheme};
pub use text::TextTheme;
pub use tooltip::TooltipTheme;

#[derive(Clone, Debug, PartialEq)]
pub struct ComponentTheme {
    pub button: ButtonTheme,
    pub input: InputTheme,
    pub text: TextTheme,
    pub switch: SwitchTheme,
    pub panel: PanelTheme,
    pub dialog: DialogTheme,
    pub tooltip: TooltipTheme,
    pub scrollbar: ScrollbarTheme,
}

impl ComponentTheme {
    pub fn from_tokens(
        colors: &ColorScheme,
        typography: &TypeScale,
        spacing: &SpaceScale,
        radius: &RadiusScale,
        border: &BorderScale,
        elevation: &ElevationScale,
        _motion: &MotionScale,
    ) -> Self {
        let disabled_surface = colors.disabled;
        let disabled_content = colors.on_disabled;
        Self {
            button: ButtonTheme {
                primary: ButtonVariant {
                    container: Stateful {
                        normal: colors.primary,
                        hovered: colors.primary_container,
                        pressed: colors.primary_container,
                        focused: colors.primary,
                        disabled: disabled_surface,
                    },
                    content: Stateful {
                        normal: colors.on_primary,
                        hovered: colors.on_primary_container,
                        pressed: colors.on_primary_container,
                        focused: colors.on_primary,
                        disabled: disabled_content,
                    },
                    border: Stateful {
                        normal: colors.primary,
                        hovered: colors.primary_container,
                        pressed: colors.primary_container,
                        focused: colors.focus_ring,
                        disabled: disabled_surface,
                    },
                    border_width: Dp::ZERO,
                    radius: radius.md,
                    padding_x: spacing.sm,
                    padding_y: spacing.xs,
                    min_height: spacing.xl,
                    text_style: typography.label.clone(),
                },
                secondary: ButtonVariant {
                    container: Stateful {
                        normal: colors.surface_high,
                        hovered: colors.surface_overlay,
                        pressed: colors.surface_low,
                        focused: colors.surface_high,
                        disabled: disabled_surface,
                    },
                    content: Stateful {
                        normal: colors.on_surface,
                        hovered: colors.on_surface,
                        pressed: colors.on_surface,
                        focused: colors.on_surface,
                        disabled: disabled_content,
                    },
                    border: Stateful {
                        normal: colors.outline,
                        hovered: colors.outline,
                        pressed: colors.outline_muted,
                        focused: colors.focus_ring,
                        disabled: disabled_surface,
                    },
                    border_width: border.thin,
                    radius: radius.md,
                    padding_x: spacing.sm,
                    padding_y: spacing.xs,
                    min_height: spacing.xl,
                    text_style: typography.label.clone(),
                },
                ghost: ButtonVariant {
                    container: Stateful {
                        normal: Color::TRANSPARENT,
                        hovered: colors.surface_low,
                        pressed: colors.surface_high,
                        focused: colors.surface_low,
                        disabled: Color::TRANSPARENT,
                    },
                    content: Stateful {
                        normal: colors.on_surface,
                        hovered: colors.on_surface,
                        pressed: colors.on_surface,
                        focused: colors.on_surface,
                        disabled: disabled_content,
                    },
                    border: Stateful {
                        normal: Color::TRANSPARENT,
                        hovered: Color::TRANSPARENT,
                        pressed: Color::TRANSPARENT,
                        focused: colors.focus_ring,
                        disabled: Color::TRANSPARENT,
                    },
                    border_width: Dp::ZERO,
                    radius: radius.md,
                    padding_x: spacing.sm,
                    padding_y: spacing.xs,
                    min_height: spacing.xl,
                    text_style: typography.label.clone(),
                },
                danger: ButtonVariant {
                    container: Stateful {
                        normal: colors.error,
                        hovered: colors.error,
                        pressed: colors.error,
                        focused: colors.error,
                        disabled: disabled_surface,
                    },
                    content: Stateful {
                        normal: colors.on_error,
                        hovered: colors.on_error,
                        pressed: colors.on_error,
                        focused: colors.on_error,
                        disabled: disabled_content,
                    },
                    border: Stateful {
                        normal: colors.error,
                        hovered: colors.error,
                        pressed: colors.error,
                        focused: colors.focus_ring,
                        disabled: disabled_surface,
                    },
                    border_width: Dp::ZERO,
                    radius: radius.md,
                    padding_x: spacing.sm,
                    padding_y: spacing.xs,
                    min_height: spacing.xl,
                    text_style: typography.label.clone(),
                },
            },
            input: InputTheme {
                background: Stateful {
                    normal: colors.surface_low,
                    hovered: colors.surface,
                    pressed: colors.surface,
                    focused: colors.surface,
                    disabled: colors.disabled,
                },
                text: Stateful {
                    normal: colors.on_surface,
                    hovered: colors.on_surface,
                    pressed: colors.on_surface,
                    focused: colors.on_surface,
                    disabled: colors.on_disabled,
                },
                placeholder: Stateful {
                    normal: colors.on_surface_muted,
                    hovered: colors.on_surface_muted,
                    pressed: colors.on_surface_muted,
                    focused: colors.on_surface_muted,
                    disabled: colors.on_disabled,
                },
                border: Stateful {
                    normal: colors.outline,
                    hovered: colors.outline_muted,
                    pressed: colors.outline,
                    focused: colors.focus_ring,
                    disabled: colors.disabled,
                },
                cursor: colors.on_surface,
                selection: colors.selection,
                radius: radius.md,
                padding_x: spacing.md,
                padding_y: spacing.sm,
                min_height: spacing.xl,
                text_style: typography.body.clone(),
            },
            text: TextTheme {
                default: typography.body.clone(),
                muted_color: colors.on_surface_muted,
                primary_color: colors.primary,
            },
            switch: SwitchTheme {
                track: Stateful {
                    normal: colors.surface_high,
                    hovered: colors.surface_overlay,
                    pressed: colors.surface_low,
                    focused: colors.surface_overlay,
                    disabled: colors.disabled,
                },
                track_checked: Stateful {
                    normal: colors.primary,
                    hovered: colors.primary,
                    pressed: colors.primary,
                    focused: colors.primary,
                    disabled: colors.disabled,
                },
                thumb: Stateful {
                    normal: colors.on_surface,
                    hovered: colors.on_surface,
                    pressed: colors.on_surface,
                    focused: colors.on_surface,
                    disabled: colors.on_disabled,
                },
                thumb_checked: Stateful {
                    normal: colors.on_primary,
                    hovered: colors.on_primary,
                    pressed: colors.on_primary,
                    focused: colors.on_primary,
                    disabled: colors.on_disabled,
                },
                border: Stateful {
                    normal: colors.outline_muted,
                    hovered: colors.outline,
                    pressed: colors.outline,
                    focused: colors.focus_ring,
                    disabled: colors.disabled,
                },
                border_checked: Stateful {
                    normal: colors.primary,
                    hovered: colors.primary,
                    pressed: colors.primary,
                    focused: colors.focus_ring,
                    disabled: colors.disabled,
                },
                border_width: border.thin,
                radius: radius.full,
                padding: crate::ui::layout::Insets::all(spacing.xxs),
                width: spacing.xl + spacing.sm + spacing.xxs,
                height: spacing.lg,
            },
            panel: PanelTheme {
                background: colors.surface,
                border_color: colors.outline_muted,
                border_width: border.thin,
                radius: radius.lg,
                shadow: elevation.sm.clone(),
            },
            dialog: DialogTheme {
                background: colors.surface_overlay,
                scrim: colors.scrim,
                border_color: colors.outline,
                radius: radius.xl,
                shadow: elevation.lg.clone(),
                title_style: typography.title.clone(),
                body_style: typography.body.clone(),
            },
            tooltip: TooltipTheme {
                background: colors.surface_overlay,
                text: colors.on_surface,
                radius: radius.sm,
                padding_x: spacing.sm,
                padding_y: spacing.xs,
                text_style: typography.label_small.clone(),
            },
            scrollbar: ScrollbarTheme {
                track: Stateful {
                    normal: colors.surface_low,
                    hovered: colors.surface_low,
                    pressed: colors.surface_low,
                    focused: colors.surface_low,
                    disabled: colors.surface_low,
                },
                thumb: Stateful {
                    normal: colors.outline.with_alpha_factor(0.72),
                    hovered: colors.on_surface_muted,
                    pressed: colors.on_surface,
                    focused: colors.on_surface_muted,
                    disabled: colors.disabled,
                },
                width: spacing.sm,
                radius: radius.full,
            },
        }
    }
}
