//! Drawer 在 collect 阶段的 sentinel overlay 发射。
//!
//! Drawer 的内容（backdrop + panel）是 in-tree widget 子树（见
//! `src/ui/widget/drawer/widget.rs`），其本身已经走主场景渲染 + hit-test。这里
//! 仅做一件事：当 `drawer.open == true` 时，向 `OverlayLayer::Modal` 注册一个
//! **空内容 sentinel overlay**，piggyback 现有 overlay close 机制以获得：
//! - `close_on_escape`：runtime 自动接管 Esc 键 → 调 `on_close`；
//! - `on_close` 派发：把 `on_open_change(false)` 送到 ViewModel；
//! - `return_focus_to`（如配置）：关闭时把焦点回还到指定 widget。
//!
//! sentinel overlay 没有 primitives / hits，仅注册 close handler，不影响
//! 主场景渲染。

use std::time::Duration;

use crate::animation::{AnimationKey, Transition, WidgetProperty};
use crate::ui::unit::Dp;
use crate::ui::widget::common::ComputedScene;
use crate::ui::widget::overlay::{
    collect::emit_overlay, Anchor, Overlay, OverlayContent, OverlayId, OverlayLayer,
};

use super::super::scene::CollectContext;
use super::super::types::ResolvedElement;
use super::CollectVisualState;

/// 把 Drawer 的 WidgetId 异或一个固定 tag 得到 overlay id，避免和 Menu/Modal 等
/// 共用 `OverlayId::new(self.id.raw())` 时撞 ID。
const DRAWER_OVERLAY_TAG: u64 = 0x4452415745525F49; // "DRAWER_I"

impl<VM> ResolvedElement<VM> {
    pub(super) fn emit_drawer_close_overlay_if_open(
        &self,
        context: &mut CollectContext<'_, '_>,
        computed: &mut ComputedScene<VM>,
        _visual: &CollectVisualState,
    ) {
        let Some(drawer) = &self.drawer else { return };
        // 只有当用户主动开 drawer 时才注册 close handler。
        if !drawer.open.resolve() {
            return;
        }
        // 没有任何 close 触发条件就跳过。
        if !drawer.close_on_escape && drawer.on_open_change.is_none() {
            return;
        }

        // 哪怕 `close_on_escape == false` 也允许 sentinel overlay 注册，
        // 因为 on_close 命令（用于 overlay 关闭时其它清理）仍然有用——但本场景
        // 主要还是为 Esc 关闭服务。
        let overlay_id = OverlayId::new(self.id.raw() ^ DRAWER_OVERLAY_TAG);
        let viewport = context.viewport;
        let mut overlay = Overlay::<VM>::new(overlay_id, Anchor::Rect(viewport))
            .source_widget(self.id)
            .layer(OverlayLayer::Modal)
            .close_on_escape(drawer.close_on_escape)
            .close_on_outside_click(false); // backdrop 自己处理点击
        if let Some(cmd) = drawer.on_open_change.clone() {
            overlay = overlay.on_close(cmd);
        }
        if let Some(target) = drawer.return_focus_to {
            overlay = overlay.return_focus_to(target);
        }

        // 触发动画通道：用 DrawerVisibility 让 AnimationEngine 保持一个解析任务，
        // 即使本 emit 不主动消费 visibility 值。这样下次 open=false 时 collect
        // 阶段仍能 schedule wakeup（slide-out 动画）。
        let _ = context.animations.resolve_f32(
            AnimationKey::Widget {
                id: self.id.raw(),
                property: WidgetProperty::DrawerVisibility,
            },
            1.0,
            Some(Transition::ease_in_out(Duration::from_millis(250))),
            context.now,
        );

        let _ = emit_overlay(
            computed,
            viewport,
            overlay,
            (Dp::ZERO, Dp::ZERO),
            OverlayContent::Primitives(Vec::new()),
        );
    }
}
