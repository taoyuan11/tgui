//! DrawerDescriptor —— 挂在 `Element::drawer` 上的修饰符。
//!
//! 与 `ModalDescriptor` 同位：collect 阶段由 `src/ui/widget/core/resolved/collect/drawer.rs`
//! 读取此描述符，按 `open` 状态解算动画 visibility、注册 sentinel overlay（用于 Esc
//! 关闭 / focus 返回 / `on_close` 派发），并向 ComputedScene 注入"backdrop / panel
//! visibility"通道。
//!
//! panel / backdrop 本身是 Drawer builder 在 `Element` 主树里构造的普通 widget
//! 子树（详见 `widget.rs`）。

use crate::foundation::view_model::ValueCommand;
use crate::theme::StyleContext;
use crate::ui::layout::Value;
use crate::ui::theme::WidgetState;
use crate::ui::widget::common::VisualStyle;
use crate::ui::widget::style::{DrawerStyle, StyleResolver};
use crate::ui::widget::StyleSheet;
use crate::ui::widget::WidgetId;

use super::placement::DrawerPlacement;
use super::widget::DrawerMode;

/// 挂在 `Element::drawer` 上的 Drawer 状态描述符。
pub(crate) struct DrawerDescriptor<VM> {
    pub(crate) open: Value<bool>,
    pub(crate) on_open_change: Option<ValueCommand<VM, bool>>,
    pub(crate) placement: DrawerPlacement,
    pub(crate) mode: DrawerMode,
    pub(crate) close_on_escape: bool,
    pub(crate) close_on_backdrop_click: bool,
    pub(crate) return_focus_to: Option<WidgetId>,
    pub(crate) backdrop_widget_id: WidgetId,
    pub(crate) panel_widget_id: WidgetId,
    pub(crate) style: Option<StyleResolver<DrawerStyle>>,
}

impl<VM> Clone for DrawerDescriptor<VM> {
    fn clone(&self) -> Self {
        Self {
            open: self.open.clone(),
            on_open_change: self.on_open_change.clone(),
            placement: self.placement,
            mode: self.mode,
            close_on_escape: self.close_on_escape,
            close_on_backdrop_click: self.close_on_backdrop_click,
            return_focus_to: self.return_focus_to,
            backdrop_widget_id: self.backdrop_widget_id,
            panel_widget_id: self.panel_widget_id,
            style: self.style.clone(),
        }
    }
}

impl<VM> DrawerDescriptor<VM> {
    /// 把 `on_open_change` 重新映射到父 view model。其它字段不变。
    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: std::sync::Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> DrawerDescriptor<RootVm>
    where
        VM: 'static,
    {
        DrawerDescriptor {
            open: self.open,
            on_open_change: self.on_open_change.map(|cmd| cmd.scope(selector)),
            placement: self.placement,
            mode: self.mode,
            close_on_escape: self.close_on_escape,
            close_on_backdrop_click: self.close_on_backdrop_click,
            return_focus_to: self.return_focus_to,
            backdrop_widget_id: self.backdrop_widget_id,
            panel_widget_id: self.panel_widget_id,
            style: self.style,
        }
    }

    /// 按当前主题解析最终样式。
    pub(crate) fn resolved_style(&self, context: &StyleContext<'_>) -> DrawerStyle {
        let style_sheet = StyleSheet::default();
        self.resolved_style_with_sheet(
            context,
            &style_sheet,
            &VisualStyle::default(),
            WidgetState::default(),
        )
    }

    pub(crate) fn resolved_style_with_sheet(
        &self,
        context: &StyleContext<'_>,
        style_sheet: &StyleSheet,
        visual: &VisualStyle,
        state: WidgetState,
    ) -> DrawerStyle {
        let mut base = DrawerStyle::default_for_theme(context.theme);
        context.theme.components.drawer.apply(&mut base, context);
        style_sheet.apply_drawer(&mut base, context, visual);
        style_sheet.apply_drawer_state(&mut base, context, visual, state);
        self.style
            .as_ref()
            .map(|resolver| resolver.resolve_from(base.clone(), context))
            .unwrap_or(base)
    }
}
