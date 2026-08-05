//! 菜单 widget 的运行时状态（键盘导航 / 高亮 cursor）。
//!
//! Tooltip 模块（`runtime/tooltip.rs`）的姐妹文件——所有方法挂在
//! `BoundRuntimeHandler<VM>` 上，被 `runtime/input.rs` 的键盘分发以及
//! `runtime/scene_runtime.rs::widget_state_map`、`collect/menu.rs` 共同使用。
//!
//! 覆盖：
//! - 找到当前最顶层"已打开"的 Menu overlay（按 `OverlayLayer::Menu` 判定）；
//! - 以路径（`Vec<usize>`）表示 cursor：长度=1=最外层、>1=进入了嵌套 submenu；
//! - Up/Down 调整当前层（路径末元素），跳过 separator / disabled，循环；
//! - Right：cursor 在 Submenu 项上时入栈，进入 submenu 第一个可点项；否则
//!   走 MenuBar Left/Right；Left：弹栈直到深度=1，再走 MenuBar 切条目；
//! - Enter / Space 沿路径找到叶子 item，触发 on_select 并 on_open_change(false)；
//! - 字母 type-ahead 在当前层跳到首字母匹配项；
//! - 全局 KeyChord 派发：扫整棵 resolved 树（含 submenu 递归）找 shortcut 命中。

use crate::foundation::view_model::ValueCommand;
use crate::platform::keyboard::{Key, KeyCode, ModifiersState};
use crate::runtime::overlay::OverlayLayer;
use crate::ui::widget::{
    menu_item_state_owner, GesturePhase, GestureSource, HitInteraction, KeyChord, LongPressEvent,
    MenuItemKind, MenuItemState, Rect, ResolvedWidgetKind, WidgetId, WidgetStateMap,
};
use smallvec::SmallVec;

use super::BoundRuntimeHandler;

/// 单个 menu 的可点选项快照。
#[allow(dead_code)] // Kept for menu keyboard navigation hit-rect dispatch.
pub(super) struct MenuKeyboardItem<VM> {
    pub option_index: usize,
    pub rect: Rect,
    pub on_select: Option<crate::foundation::view_model::Command<VM>>,
    pub on_open_change: Option<ValueCommand<VM, bool>>,
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    pub(super) fn menu_trigger_ancestor(&self, mut widget_id: WidgetId) -> Option<WidgetId> {
        let layout = self.cached_scene.as_ref()?.layout.as_ref()?;
        loop {
            if layout
                .resolved_widget(widget_id)
                .and_then(|resolved| resolved.menu.as_ref())
                .is_some_and(|menu| !menu.disabled.resolve())
            {
                return Some(widget_id);
            }
            widget_id = layout.parent_of(widget_id)?;
        }
    }

    pub(super) fn context_menu_trigger_ancestor(
        &self,
        mut widget_id: WidgetId,
    ) -> Option<WidgetId> {
        let layout = self.cached_scene.as_ref()?.layout.as_ref()?;
        loop {
            if layout
                .resolved_widget(widget_id)
                .and_then(|resolved| resolved.context_menu.as_ref())
                .is_some_and(|menu| !menu.disabled.resolve())
            {
                return Some(widget_id);
            }
            widget_id = layout.parent_of(widget_id)?;
        }
    }

    pub(super) fn is_menu_layer_source(&self, widget_id: WidgetId) -> bool {
        self.cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .and_then(|layout| layout.resolved_widget(widget_id))
            .is_some_and(|resolved| resolved.menu.is_some() || resolved.context_menu.is_some())
    }

    pub(super) fn close_menu_source_for_activation(&mut self, widget_id: WidgetId) -> bool {
        if self.close_context_menu(widget_id) {
            return true;
        }
        if self
            .cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .and_then(|layout| layout.resolved_widget(widget_id))
            .is_some_and(|resolved| resolved.menu.is_some())
        {
            return self.set_menu_open_state(widget_id, false);
        }
        false
    }

    pub(in crate::runtime) fn resolved_menu_open_state(&self, menu_id: WidgetId) -> bool {
        let Some(cached) = self.cached_scene.as_ref() else {
            return self
                .menu_open_states
                .get(&menu_id)
                .copied()
                .unwrap_or(false);
        };
        let Some(layout) = cached.layout.as_ref() else {
            return self
                .menu_open_states
                .get(&menu_id)
                .copied()
                .unwrap_or(false);
        };
        let Some(resolved) = layout.resolved_widget(menu_id) else {
            return self
                .menu_open_states
                .get(&menu_id)
                .copied()
                .unwrap_or(false);
        };
        let Some(menu) = resolved.menu.as_ref() else {
            return self
                .menu_open_states
                .get(&menu_id)
                .copied()
                .unwrap_or(false);
        };
        if let Some(open) = &menu.open {
            return open.resolve();
        }
        if let (Some(group), Some(index)) = (menu.menubar_group, menu.menubar_index) {
            return self
                .menubar_active_states
                .get(&group.raw())
                .copied()
                .flatten()
                == Some(index);
        }
        self.menu_open_states
            .get(&menu_id)
            .copied()
            .unwrap_or(false)
    }

    pub(in crate::runtime) fn set_menu_open_state(
        &mut self,
        menu_id: WidgetId,
        open: bool,
    ) -> bool {
        let Some((is_controlled, group_index, disabled, on_open_change)) = self
            .cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .and_then(|layout| layout.resolved_widget(menu_id))
            .and_then(|resolved| resolved.menu.as_ref())
            .map(|menu| {
                (
                    menu.open.is_some(),
                    menu.menubar_group
                        .zip(menu.menubar_index)
                        .map(|(group, index)| (group.raw(), index)),
                    menu.disabled.resolve(),
                    menu.on_open_change.clone(),
                )
            })
        else {
            return false;
        };
        if open && disabled {
            return false;
        }
        let previous = self.resolved_menu_open_state(menu_id);
        if previous == open {
            return false;
        }

        if is_controlled {
            let Some(command) = on_open_change.as_ref() else {
                return false;
            };
            if !open {
                self.menu_keyboard_cursor.remove(&menu_id);
            }
            self.execute_value_command(command, open);
        } else if let Some((group, index)) = group_index {
            self.menubar_active_states
                .insert(group, open.then_some(index));
        } else {
            self.menu_open_states.insert(menu_id, open);
        }
        if !open {
            self.menu_keyboard_cursor.remove(&menu_id);
        }
        self.invalidate_scene_with_reason("menu_open_state");
        if !is_controlled {
            if let Some(command) = on_open_change.as_ref() {
                self.execute_value_command(command, open);
            }
        }
        true
    }

    pub(in crate::runtime) fn toggle_menu_open_state(&mut self, menu_id: WidgetId) -> bool {
        let next = !self.resolved_menu_open_state(menu_id);
        self.set_menu_open_state(menu_id, next)
    }

    pub(in crate::runtime) fn open_context_menu_at(
        &mut self,
        widget_id: WidgetId,
        position: crate::ui::widget::Point,
    ) -> bool {
        self.open_context_menu_with_event(LongPressEvent {
            widget_id,
            source: GestureSource::Mouse,
            phase: GesturePhase::Recognized,
            start_position: position,
            position,
            finger_id: None,
        })
    }

    pub(in crate::runtime) fn open_context_menu_with_event(
        &mut self,
        event: LongPressEvent,
    ) -> bool {
        self.open_context_menu_at_anchor(event.widget_id, event.position, Some(event))
    }

    pub(in crate::runtime) fn open_context_menu_semantically(
        &mut self,
        target_widget_id: WidgetId,
    ) -> bool {
        let Some(widget_id) = self.context_menu_trigger_ancestor(target_widget_id) else {
            return false;
        };
        let Some(position) = self.context_menu_semantic_anchor(target_widget_id, widget_id) else {
            return false;
        };
        self.open_context_menu_at_anchor(widget_id, position, None)
    }

    fn context_menu_semantic_anchor(
        &self,
        target_widget_id: WidgetId,
        owner_widget_id: WidgetId,
    ) -> Option<crate::ui::widget::Point> {
        let cached = self.cached_scene.as_ref()?;
        let layout = cached.layout.as_ref()?;
        let bounds = cached
            .computed
            .hit_regions
            .iter()
            .chain(cached.computed.overlay_hit_regions.iter())
            .find(|region| {
                region.interaction.widget_id() == target_widget_id
                    || region
                        .focus
                        .as_ref()
                        .is_some_and(|focus| focus.widget_id == target_widget_id)
            })
            .map(|region| region.rect)
            .or_else(|| layout.widget_bounds(target_widget_id))
            .or_else(|| layout.widget_bounds(owner_widget_id))?;
        Some(crate::ui::widget::Point::new(bounds.x, bounds.bottom()))
    }

    fn open_context_menu_at_anchor(
        &mut self,
        widget_id: WidgetId,
        position: crate::ui::widget::Point,
        event: Option<LongPressEvent>,
    ) -> bool {
        let Some((disabled, on_show, on_open_change)) = self
            .cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .and_then(|layout| layout.resolved_widget(widget_id))
            .and_then(|resolved| resolved.context_menu.as_ref())
            .map(|menu| {
                (
                    menu.disabled.resolve(),
                    menu.on_show.clone(),
                    menu.on_open_change.clone(),
                )
            })
        else {
            return false;
        };
        if disabled {
            return false;
        }

        let was_open = self
            .context_menu_anchor_states
            .insert(widget_id, position)
            .is_some();
        self.invalidate_scene_with_reason("context_menu_open_state");
        if was_open {
            return true;
        }
        if let (Some(command), Some(event)) = (on_show.as_ref(), event) {
            self.execute_value_command(command, event);
        }
        if let Some(command) = on_open_change.as_ref() {
            self.execute_value_command(command, true);
        }
        true
    }

    pub(in crate::runtime) fn close_context_menu(&mut self, widget_id: WidgetId) -> bool {
        let on_open_change = self
            .cached_scene
            .as_ref()
            .and_then(|cached| cached.layout.as_ref())
            .and_then(|layout| layout.resolved_widget(widget_id))
            .and_then(|resolved| resolved.context_menu.as_ref())
            .and_then(|menu| menu.on_open_change.clone());
        if self.context_menu_anchor_states.remove(&widget_id).is_none() {
            return false;
        }
        self.menu_keyboard_cursor.remove(&widget_id);
        self.invalidate_scene_with_reason("context_menu_open_state");
        if let Some(command) = on_open_change.as_ref() {
            self.execute_value_command(command, false);
        }
        true
    }

    /// 在当前 cached scene 上找最顶层的 Menu overlay；返回它对应的 widget_id。
    /// 当 submenu 嵌套打开时 overlay_close_handlers 会含有合成 overlay_id（不
    /// 对应任何真实 widget），筛选时优先取 `return_focus_to` —— 那是真实的
    /// trigger widget_id，跨嵌套保持一致。
    pub(super) fn topmost_open_menu_id(&self) -> Option<WidgetId> {
        let cached = self.cached_scene.as_ref()?;
        for handle in cached.computed.overlay_close_handlers.iter().rev() {
            if handle.layer != OverlayLayer::Menu {
                continue;
            }
            if let Some(source_id) = handle.source_widget_id {
                let resolved_source = cached
                    .layout
                    .as_ref()
                    .and_then(|layout| layout.resolved_widget(source_id));
                if let Some(ResolvedWidgetKind::Select { open, .. }) =
                    resolved_source.map(|resolved| &resolved.kind)
                {
                    let source_is_open = open
                        .as_ref()
                        .map(|open| open.resolve())
                        .or_else(|| self.select_open_states.get(&source_id).copied())
                        .unwrap_or(false);
                    if source_is_open {
                        // Select owns its own option navigation. Do not let the generic Menu
                        // router consume its arrow/activation keys or reach an underlying menu.
                        return None;
                    }
                    continue;
                }
                let source_is_open = resolved_source
                    .map(|resolved| {
                        if resolved.menu.is_some() {
                            self.resolved_menu_open_state(source_id)
                        } else if resolved.context_menu.is_some() {
                            self.context_menu_anchor_states.contains_key(&source_id)
                        } else {
                            true
                        }
                    })
                    .unwrap_or(true);
                if !source_is_open {
                    continue;
                }
            }
            if let Some(target) = handle.return_focus_to {
                return Some(target);
            }
            let candidate = WidgetId::from_raw(handle.overlay_id.0);
            if let Some(layout) = cached.layout.as_ref() {
                if layout.resolved_widget(candidate).is_some() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    /// 扫描当前 cached scene 拿出某 menu 的所有可点选 items（按 option_index 升序）。
    /// 仅返回非 Disabled 的项；Separator / Disabled item 会被自动过滤掉。
    #[allow(dead_code)] // Kept for menu keyboard navigation hit-rect dispatch.
    pub(super) fn menu_keyboard_items(&self, menu_id: WidgetId) -> Vec<MenuKeyboardItem<VM>> {
        let Some(cached) = self.cached_scene.as_ref() else {
            return Vec::new();
        };
        let mut items: Vec<MenuKeyboardItem<VM>> = cached
            .computed
            .overlay_hit_regions
            .iter()
            .filter_map(|region| match &region.interaction {
                HitInteraction::SelectOption {
                    id,
                    option_index,
                    on_select,
                    on_open_change,
                    ..
                } if *id == menu_id => Some(MenuKeyboardItem {
                    option_index: *option_index,
                    rect: region.rect,
                    on_select: on_select.clone(),
                    on_open_change: on_open_change.clone(),
                }),
                _ => None,
            })
            .collect();
        items.sort_by_key(|item| item.option_index);
        items
    }

    /// 沿 `path` 在 resolved menu 的 items 树里走到目标项（不含路径最末元素之外的更深）。
    /// path = [3, 1] → 返回 items[3].submenu[1]。失败返回 None。
    fn menu_item_at_path(&self, menu_id: WidgetId, path: &[usize]) -> Option<MenuItemSnapshot<VM>> {
        if path.is_empty() {
            return None;
        }
        let mut current = self.menu_items(menu_id)?.get(path[0])?;
        for idx in &path[1..] {
            current = current.submenu.get(*idx)?;
        }
        Some(MenuItemSnapshot {
            kind: current.kind,
            disabled: current.disabled.resolve(),
            has_submenu: !current.submenu.is_empty(),
            on_select: current.on_select.clone(),
            label: current.label.as_ref().map(|l| l.resolve()),
        })
    }

    pub(super) fn activate_menu_accessibility_item(
        &mut self,
        menu_id: WidgetId,
        path: &[usize],
    ) -> bool {
        let Some(snapshot) = self.menu_item_at_path(menu_id, path) else {
            return false;
        };
        if snapshot.disabled {
            return false;
        }
        if matches!(snapshot.kind, MenuItemKind::Submenu) && snapshot.has_submenu {
            self.menu_keyboard_cursor.insert(menu_id, path.to_vec());
            self.invalidate_computed_scene();
            return true;
        }
        let _ = self.close_menu_source_for_activation(menu_id);
        if let Some(command) = snapshot.on_select {
            self.execute_command(&command);
        }
        true
    }

    pub(super) fn set_menu_accessibility_item_expanded(
        &mut self,
        menu_id: WidgetId,
        path: &[usize],
        expanded: bool,
    ) -> bool {
        let Some(snapshot) = self.menu_item_at_path(menu_id, path) else {
            return false;
        };
        if snapshot.disabled
            || !matches!(snapshot.kind, MenuItemKind::Submenu)
            || !snapshot.has_submenu
        {
            return false;
        }
        if expanded {
            self.menu_keyboard_cursor.insert(menu_id, path.to_vec());
        } else if path.len() == 1 {
            self.menu_keyboard_cursor.remove(&menu_id);
        } else {
            self.menu_keyboard_cursor
                .insert(menu_id, path[..path.len() - 1].to_vec());
        }
        self.invalidate_computed_scene();
        true
    }

    fn menu_items(&self, menu_id: WidgetId) -> Option<&[MenuItemState<VM>]> {
        let resolved = self
            .cached_scene
            .as_ref()?
            .layout
            .as_ref()?
            .resolved_widget(menu_id)?;
        resolved
            .menu
            .as_ref()
            .map(|menu| menu.items.as_slice())
            .or_else(|| {
                resolved
                    .context_menu
                    .as_ref()
                    .map(|menu| menu.items.as_slice())
            })
    }

    /// 返回某菜单某层级（`parent_path` 指定）下所有可"游走"的索引：跳过 Separator
    /// 与 disabled 项。parent_path 空表示根菜单。
    fn selectable_indices(&self, menu_id: WidgetId, parent_path: &[usize]) -> SmallVec<[usize; 8]> {
        let Some(root_items) = self.menu_items(menu_id) else {
            return SmallVec::new();
        };
        // 走到目标父级 items 列表。
        let mut items = root_items;
        for idx in parent_path {
            let Some(parent) = items.get(*idx) else {
                return SmallVec::new();
            };
            items = &parent.submenu;
        }
        items
            .iter()
            .enumerate()
            .filter_map(|(i, it)| {
                if it.disabled.resolve() {
                    return None;
                }
                match it.kind {
                    MenuItemKind::Separator => None,
                    _ => Some(i),
                }
            })
            .collect()
    }

    /// 让 cursor 在当前层（路径末层）前进一位（dir = +1 下、-1 上），循环到首尾。
    /// 返回是否真的有 menu 在打开（用来决定是否吞键）。
    pub(super) fn advance_menu_keyboard_cursor(&mut self, dir: i32) -> bool {
        let Some(menu_id) = self.topmost_open_menu_id() else {
            return false;
        };
        let mut path = self
            .menu_keyboard_cursor
            .get(&menu_id)
            .cloned()
            .unwrap_or_default();
        // 父路径=去掉当前层；当前层是 path.last()（若有）。
        let parent_path: Vec<usize> = if path.is_empty() {
            Vec::new()
        } else {
            path[..path.len() - 1].to_vec()
        };
        let candidates = self.selectable_indices(menu_id, &parent_path);
        if candidates.is_empty() {
            return true;
        }
        let cur_pos = path
            .last()
            .and_then(|idx| candidates.iter().position(|c| c == idx));
        let next_pos = match cur_pos {
            Some(pos) => {
                let n = candidates.len() as i32;
                let mut p = pos as i32 + dir;
                p = ((p % n) + n) % n;
                p as usize
            }
            None => {
                if dir >= 0 {
                    0
                } else {
                    candidates.len() - 1
                }
            }
        };
        if path.is_empty() {
            path.push(candidates[next_pos]);
        } else {
            *path.last_mut().unwrap() = candidates[next_pos];
        }
        self.menu_keyboard_cursor.insert(menu_id, path);
        self.invalidate_computed_scene();
        true
    }

    /// Right 键：若 cursor 在 Submenu 项上，则入栈进入 submenu 第一个可点项；
    /// 否则交给 MenuBar Left/Right。返回是否吞键。
    pub(super) fn enter_submenu_or_advance_menubar(&mut self, dir: i32) -> bool {
        let Some(menu_id) = self.topmost_open_menu_id() else {
            return false;
        };
        let path = self
            .menu_keyboard_cursor
            .get(&menu_id)
            .cloned()
            .unwrap_or_default();
        if !path.is_empty() {
            // 检查 cursor 是否落在 Submenu 项。
            if let Some(snapshot) = self.menu_item_at_path(menu_id, &path) {
                if matches!(snapshot.kind, MenuItemKind::Submenu) && snapshot.has_submenu {
                    let children = self.selectable_indices(menu_id, &path);
                    if let Some(first) = children.first().copied() {
                        let mut new_path = path;
                        new_path.push(first);
                        self.menu_keyboard_cursor.insert(menu_id, new_path);
                        self.invalidate_computed_scene();
                        return true;
                    }
                }
            }
        }
        // 不能深入 → fall through 到 MenuBar 横向切换。
        self.advance_menubar_active(dir)
    }

    /// Left 键：若 cursor 已在 submenu（深度>1），弹栈；否则交给 MenuBar Left/Right。
    pub(super) fn leave_submenu_or_advance_menubar(&mut self, dir: i32) -> bool {
        let Some(menu_id) = self.topmost_open_menu_id() else {
            return false;
        };
        let path = self
            .menu_keyboard_cursor
            .get(&menu_id)
            .cloned()
            .unwrap_or_default();
        if path.len() > 1 {
            let mut new_path = path;
            new_path.pop();
            self.menu_keyboard_cursor.insert(menu_id, new_path);
            self.invalidate_computed_scene();
            return true;
        }
        self.advance_menubar_active(dir)
    }

    /// 在 MenuBar 内左右切换：找当前打开 menu 的 menubar_group + menubar_index，
    /// 在同 group 的其它 menu 里找下一个（cycle），调用其 menubar_set_active
    /// 命令切换 active_index。返回是否处理了。
    pub(super) fn advance_menubar_active(&mut self, dir: i32) -> bool {
        let Some(menu_id) = self.topmost_open_menu_id() else {
            return false;
        };
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let Some(layout) = cached.layout.as_ref() else {
            return false;
        };
        let Some(current) = layout.resolved_widget(menu_id) else {
            return false;
        };
        let Some(current_menu) = current.menu.as_ref() else {
            return false;
        };
        let (Some(group), Some(current_idx)) =
            (current_menu.menubar_group, current_menu.menubar_index)
        else {
            return false;
        };
        // 扫所有同 group 的 menu，按 menubar_index 排序，取相邻 entry。
        let mut peers = layout
            .all_widget_ids()
            .filter_map(|id| {
                let resolved = layout.resolved_widget(id)?;
                let menu = resolved.menu.as_ref()?;
                (menu.menubar_group == Some(group) && !menu.disabled.resolve())
                    .then(|| menu.menubar_index)?
            })
            .collect::<SmallVec<[_; 8]>>();
        peers.sort();
        peers.dedup();
        if peers.len() < 2 {
            return true;
        }
        let pos = peers
            .iter()
            .position(|&idx| idx == current_idx)
            .unwrap_or(0);
        let n = peers.len() as i32;
        let mut np = pos as i32 + dir;
        np = ((np % n) + n) % n;
        let target = peers[np as usize];
        // 切换 active_index → 受控 MenuBar 调用用户命令，未受控 MenuBar 写 runtime state。
        if let Some(set_active) = current_menu.menubar_set_active.clone() {
            self.execute_value_command(&set_active, Some(target));
        } else {
            self.menubar_active_states.insert(group.raw(), Some(target));
        }
        // cursor 清掉，避免下次打开继承上次位置
        self.menu_keyboard_cursor.remove(&menu_id);
        self.invalidate_computed_scene();
        true
    }

    /// 字母 type-ahead：在当前层 items 里找首字母匹配项，把 cursor 移过去。
    /// 命中返回 true，未命中返回 false。
    pub(super) fn type_ahead_menu_cursor(&mut self, letter: char) -> bool {
        let Some(menu_id) = self.topmost_open_menu_id() else {
            return false;
        };
        let path = self
            .menu_keyboard_cursor
            .get(&menu_id)
            .cloned()
            .unwrap_or_default();
        let parent_path: Vec<usize> = if path.is_empty() {
            Vec::new()
        } else {
            path[..path.len() - 1].to_vec()
        };
        // 在父级 items 列表里找匹配。
        let Some(root_items) = self.menu_items(menu_id) else {
            return false;
        };
        let mut items = root_items;
        for idx in &parent_path {
            let Some(parent) = items.get(*idx) else {
                return false;
            };
            items = &parent.submenu;
        }
        let target = letter.to_ascii_lowercase();
        let candidates = items
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| {
                if item.disabled.resolve() {
                    return None;
                }
                match item.kind {
                    MenuItemKind::Separator => None,
                    _ => {
                        let label = item.label.as_ref()?.resolve();
                        let first = label
                            .chars()
                            .find(|c| c.is_alphanumeric())?
                            .to_ascii_lowercase();
                        (first == target).then_some(idx)
                    }
                }
            })
            .collect::<SmallVec<[_; 8]>>();
        if candidates.is_empty() {
            return false;
        }
        let current_last = path.last().copied();
        let next = match current_last {
            Some(cur) => candidates
                .iter()
                .copied()
                .find(|idx| *idx > cur)
                .or_else(|| candidates.first().copied())
                .unwrap(),
            None => candidates[0],
        };
        let mut new_path = parent_path;
        new_path.push(next);
        self.menu_keyboard_cursor.insert(menu_id, new_path);
        self.invalidate_computed_scene();
        true
    }

    /// 触发 cursor 路径上叶子项的 on_select + on_open_change(false)。
    /// 返回是否处理了（用来吞键）。
    pub(super) fn activate_menu_keyboard_cursor(&mut self) -> bool {
        let Some(menu_id) = self.topmost_open_menu_id() else {
            return false;
        };
        let path = self
            .menu_keyboard_cursor
            .get(&menu_id)
            .cloned()
            .unwrap_or_default();
        if path.is_empty() {
            return true;
        }
        let Some(snapshot) = self.menu_item_at_path(menu_id, &path) else {
            return true;
        };
        if snapshot.disabled {
            return true;
        }
        // 在 Submenu 项上回车=进入 submenu，等同 Right。
        if matches!(snapshot.kind, MenuItemKind::Submenu) && snapshot.has_submenu {
            return self.enter_submenu_or_advance_menubar(1);
        }
        // Close while the resolved source is still available. Item commands may request a root
        // rebuild, which intentionally drops the cached layout.
        let _ = self.close_menu_source_for_activation(menu_id);
        if let Some(command) = snapshot.on_select {
            self.execute_command(&command);
        }
        true
    }

    /// 在构造 WidgetStateMap 时调用：把当前每个 menu 的 cursor 项标 hovered=true，
    /// 这样 collect 阶段会用 item_background 的 hover 颜色渲染。
    /// 路径每一层都标 hovered=true，保证嵌套 submenu 自动展开。
    pub(super) fn apply_menu_keyboard_cursor_to_states(&self, states: &mut WidgetStateMap) {
        let open_menus = if let Some(cached) = self.cached_scene.as_ref() {
            cached
                .computed
                .overlay_close_handlers
                .iter()
                .filter(|h| h.layer == OverlayLayer::Menu)
                .filter_map(|h| h.source_widget_id)
                .collect::<SmallVec<[_; 4]>>()
        } else {
            SmallVec::new()
        };
        for (menu_id, path) in &self.menu_keyboard_cursor {
            if !open_menus.contains(menu_id) {
                continue;
            }
            // 路径每一层都标 hovered=true，让 submenu 自动展开。最末层是真正的 cursor。
            let mut parent_path = SmallVec::<[usize; 4]>::new();
            for cursor_opt_index in path {
                let state_owner = menu_item_state_owner(*menu_id, &parent_path);
                let mut state = states.get_select_option(state_owner, *cursor_opt_index);
                state.hovered = true;
                states.set_select_option(state_owner, *cursor_opt_index, state);
                parent_path.push(*cursor_opt_index);
            }
        }
    }

    /// 全局快捷键派发：扫描 cached resolved 树里所有挂了 menu / context_menu 的
    /// element，比对每个 MenuItem 的 `shortcut` chord。命中即执行 on_select。
    /// 返回是否消费了该按键。
    pub(super) fn dispatch_global_menu_shortcut(
        &mut self,
        mods: ModifiersState,
        key: &Key,
        code: KeyCode,
    ) -> bool {
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let Some(layout) = cached.layout.as_ref() else {
            return false;
        };
        let mut matched_commands =
            SmallVec::<[crate::foundation::view_model::Command<VM>; 2]>::new();
        for id in layout.all_widget_ids() {
            let Some(resolved) = layout.resolved_widget(id) else {
                continue;
            };
            if let Some(menu) = resolved.menu.as_ref() {
                if !menu.disabled.resolve() {
                    collect_shortcut_matches(&menu.items, mods, key, code, &mut matched_commands);
                }
            }
            if let Some(ctx_menu) = resolved.context_menu.as_ref() {
                if !ctx_menu.disabled.resolve() {
                    collect_shortcut_matches(
                        &ctx_menu.items,
                        mods,
                        key,
                        code,
                        &mut matched_commands,
                    );
                }
            }
        }
        if matched_commands.is_empty() {
            return false;
        }
        for command in &matched_commands {
            self.execute_command(command);
        }
        self.invalidate_computed_scene();
        true
    }
}

/// menu_item_at_path 返回的简易快照（避免暴露 MenuItemState 的具体生命周期）。
struct MenuItemSnapshot<VM> {
    kind: MenuItemKind,
    disabled: bool,
    has_submenu: bool,
    on_select: Option<crate::foundation::view_model::Command<VM>>,
    #[allow(dead_code)]
    label: Option<String>,
}

fn collect_shortcut_matches<VM>(
    items: &[MenuItemState<VM>],
    mods: ModifiersState,
    key: &Key,
    code: KeyCode,
    out: &mut SmallVec<[crate::foundation::view_model::Command<VM>; 2]>,
) where
    VM: 'static,
{
    for item in items {
        if item.disabled.resolve() {
            continue;
        }
        if let (Some(chord), Some(command)) = (item.shortcut.as_ref(), item.on_select.as_ref()) {
            if chord.matches(mods, key, code) {
                out.push(command.clone());
            }
        }
        if !item.submenu.is_empty() {
            collect_shortcut_matches(&item.submenu, mods, key, code, out);
        }
    }
}

#[allow(dead_code)]
fn _keychord_marker(_c: &KeyChord) {}
