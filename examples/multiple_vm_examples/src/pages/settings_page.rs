use std::sync::Arc;
use tgui::prelude::*;

#[derive(Clone)]
pub struct SettingsPage {
    enabled: Observable<bool>,
    on_change: Option<Arc<dyn Fn(bool) + Send + Sync>>
}

impl SettingsPage {
    pub fn new(context: &ViewModelContext, on_change: Option<Arc<dyn Fn(bool) + Send + Sync>>) -> Self {
        Self {
            enabled: context.observable(false),
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
            .background(Color::hex(0x2E7D32))
            .border(dp(1.0), Color::WHITE)
            .child(el![
                Text::new("设置页").font_size(sp(24.0)).color(Color::WHITE),
                Text::new(
                    self.enabled
                        .binding()
                        .map(|enabled| format!("当前状态：{}", if enabled { "已启用" } else { "已关闭" }))
                )
                .color(Color::WHITE),
                Button::new(Text::new("切换状态")).on_click(Command::new(Self::toggle)),
            ])
            .into()
    }
}
