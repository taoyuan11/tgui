//! Drawer 在 collect 阶段的 sentinel overlay 发射。
//!
//! Drawer 的内容（backdrop + panel）是 in-tree widget 子树（见
//! `src/ui/widget/drawer/widget.rs`），其本身已经走主场景渲染 + hit-test。这里
//! 仅做一件事：当 `drawer.open == true` 时，向 `OverlayLayer::Modal` 注册一个
//! **空内容 sentinel overlay**，piggyback 现有 overlay close 机制以获得：
//! - `close_on_escape`：runtime 自动接管 Esc 键 → 调 `on_close`；
//! - `on_close` 派发：把 `on_open_change(false)` 送到 ViewModel；
//! - `return_focus_to`（如配置）：关闭时把焦点回还到指定 widget。
//! - backdrop click：按当前 `open` 信号动态注入遮罩命中区，避免初始关闭时
//!   构建出的 backdrop 永远没有点击 handler。
//!
//! sentinel overlay 没有 primitives / hits，仅注册 close handler，不影响
//! 主场景渲染。

use std::time::Duration;

use crate::animation::{AnimationKey, Transition, WidgetProperty};
use crate::foundation::view_model::Command;
use crate::ui::unit::Dp;
use crate::ui::widget::common::{
    ComputedScene, DefaultActivation, HitGeometry, HitInteraction, HitRegion, InteractionHandlers,
    Rect,
};
use crate::ui::widget::overlay::{
    collect::emit_overlay, Anchor, Overlay, OverlayContent, OverlayId, OverlayLayer,
};

use super::super::scene::CollectContext;
use super::super::types::ResolvedElement;
use super::CollectVisualState;
use crate::ui::widget::drawer::{DrawerDescriptor, DrawerPlacement};

/// 把 Drawer 的 WidgetId 异或一个固定 tag 得到 overlay id，避免和 Menu/Modal 等
/// 共用 `OverlayId::new(self.id.raw())` 时撞 ID。
const DRAWER_OVERLAY_TAG: u64 = 0x4452415745525F49; // "DRAWER_I"

fn drawer_panel_rect<VM>(
    computed: &ComputedScene<VM>,
    viewport: Rect,
    drawer: &DrawerDescriptor<VM>,
    context: &CollectContext<'_, '_>,
) -> Rect {
    if let Some(rect) = computed
        .hit_regions
        .iter()
        .rev()
        .find_map(|hit| match &hit.interaction {
            HitInteraction::Widget { id, .. } if *id == drawer.panel_widget_id => Some(hit.rect),
            _ => None,
        })
    {
        return rect;
    }

    let style = drawer.resolved_style_with_sheet(
        &context.style_context,
        context.style_sheet,
        &crate::ui::widget::common::VisualStyle::default(),
        crate::ui::theme::WidgetState::default(),
    );
    match drawer.placement {
        DrawerPlacement::Left => Rect::new(
            viewport.x,
            viewport.y,
            style.width.min(viewport.width),
            viewport.height,
        ),
        DrawerPlacement::Right => {
            let width = style.width.min(viewport.width);
            Rect::new(
                viewport.x + viewport.width - width,
                viewport.y,
                width,
                viewport.height,
            )
        }
        DrawerPlacement::Top => Rect::new(
            viewport.x,
            viewport.y,
            viewport.width,
            style.height.min(viewport.height),
        ),
        DrawerPlacement::Bottom => {
            let height = style.height.min(viewport.height);
            Rect::new(
                viewport.x,
                viewport.y + viewport.height - height,
                viewport.width,
                height,
            )
        }
    }
}

fn push_backdrop_hit_region<VM: 'static>(
    computed: &mut ComputedScene<VM>,
    context: &CollectContext<'_, '_>,
    drawer: &DrawerDescriptor<VM>,
    rect: Rect,
    interactions: InteractionHandlers<VM>,
) {
    if rect.width <= Dp::ZERO || rect.height <= Dp::ZERO {
        return;
    }
    computed.overlay_hit_regions.push(HitRegion {
        rect,
        clip_rect: Some(context.viewport),
        geometry: HitGeometry::Rect,
        scope_path: context.focus_scope_path(),
        focus: None,
        interaction: HitInteraction::Widget {
            id: drawer.backdrop_widget_id,
            interactions,
            focusable: false,
            default_activation: DefaultActivation::None,
        },
    });
}

fn emit_drawer_backdrop_hit_regions<VM: 'static>(
    computed: &mut ComputedScene<VM>,
    context: &CollectContext<'_, '_>,
    drawer: &DrawerDescriptor<VM>,
) {
    if !drawer.close_on_backdrop_click {
        return;
    }
    let Some(on_open_change) = drawer.on_open_change.clone() else {
        return;
    };

    let mut interactions = InteractionHandlers::default();
    interactions.on_click = Some(Command::new_with_context(move |vm, ctx| {
        on_open_change.execute_with_context(vm, false, ctx);
    }));

    let viewport = context.viewport;
    let panel = drawer_panel_rect(computed, viewport, drawer, context);
    let top = Rect::new(viewport.x, viewport.y, viewport.width, panel.y - viewport.y);
    let bottom = Rect::new(
        viewport.x,
        panel.y + panel.height,
        viewport.width,
        viewport.y + viewport.height - panel.y - panel.height,
    );
    let left = Rect::new(viewport.x, panel.y, panel.x - viewport.x, panel.height);
    let right = Rect::new(
        panel.x + panel.width,
        panel.y,
        viewport.x + viewport.width - panel.x - panel.width,
        panel.height,
    );

    for rect in [top, bottom, left, right] {
        push_backdrop_hit_region(computed, context, drawer, rect, interactions.clone());
    }
}

impl<VM: 'static> ResolvedElement<VM> {
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
        emit_drawer_backdrop_hit_regions(computed, context, drawer);

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
