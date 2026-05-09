use std::sync::Arc;
use tgui::prelude::*;

fn text_style(mode: ResolvedThemeMode, size: Sp, color: Color) -> TextWidgetStyle {
    let mut style = TextWidgetStyle::default_for(mode);
    style.typography.size = size;
    style.color = color.into();
    style
}

fn panel_style(mode: ResolvedThemeMode) -> ContainerStyle {
    let mut style = ContainerStyle::default_for(mode);
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
            .style(panel_style)
            .child(el![
                Text::new("设置页").style(|mode| text_style(mode, sp(24.0), Color::WHITE)),
                Text::new(
                    self.enabled
                        .signal()
                        .map(|enabled| format!("当前状态：{}", if enabled { "已启用" } else { "已关闭" }))
                )
                .style(|mode| text_style(mode, sp(16.0), Color::WHITE)),
                Button::new("切换状态").on_click(Command::new(Self::toggle)),
            ])
            .into()
    }
}
