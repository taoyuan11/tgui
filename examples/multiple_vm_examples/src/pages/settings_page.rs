use std::sync::Arc;
use tgui::prelude::*;

fn text_style(ctx: &StyleContext<'_>, size: Sp, color: Color) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for_theme(ctx.theme);
    style.typography.size = size;
    style.color = color.into();
    style
}

fn panel_style(ctx: &StyleContext<'_>) -> ContainerStyle {
    let mut style = ContainerStyle::default_for_theme(ctx.theme);
    style.surface.background = Some(Color::hex(0x2E7D32).into());
    style.surface.border_color = Some(Color::WHITE.into());
    style.surface.border_width = Some(dp(1.0).into());
    style
}

#[derive(Clone)]
pub struct SettingsPage {
    enabled: State<bool>,
    on_change: Option<Arc<dyn Fn(bool) + Send + Sync>>
}

impl SettingsPage {
    pub fn new(context: &ViewModelContext, on_change: Option<Arc<dyn Fn(bool) + Send + Sync>>) -> Self {
        Self {
            enabled: context.state(false),
            on_change
        }
    }

    fn toggle(&mut self) {
        let enabled = self.enabled.update(|enabled| {
            *enabled = !*enabled;
            *enabled
        });
        if let Some(on_change) = &self.on_change {
            on_change(enabled)
        }
    }

    pub fn view(&self) -> Element<Self> {
        Flex::new(Axis::Vertical)
            .size(pct(60.0), pct(60.0))
            .padding(Insets::all(dp(20.0)))
            .style_full(panel_style)
            .child(el![
                Text::new("设置页").style_full(|ctx| text_style(ctx, sp(24.0), Color::WHITE)),
                Text::new(
                    self.enabled
                        .signal()
                        .map(|enabled| format!("当前状态：{}", if enabled { "已启用" } else { "已关闭" }))
                )
                .style_full(|ctx| text_style(ctx, sp(16.0), Color::WHITE)),
                Button::new("切换状态").on_click(Command::new(Self::toggle)),
            ])
            .into()
    }
}
