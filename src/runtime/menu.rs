//! 菜单 widget 的运行时状态（键盘导航 / 高亮 cursor）。
//!
//! Tooltip 模块（`runtime/tooltip.rs`）的姐妹文件——所有方法挂在
//! `BoundRuntimeHandler<VM>` 上，被 `runtime/input.rs` 的键盘分发以及
//! `runtime/scene_runtime.rs::widget_state_map` 共同使用。
//!
//! 当前覆盖：
//! - 找到当前最顶层"已打开"的 Menu overlay（按 `OverlayLayer::Menu` 判定）；
//! - 收集 menu 的可点选 items（HitInteraction::SelectOption + 非 Disabled）；
//! - Up/Down 在 items 间循环游走（自动跳过 separator / disabled）；
//! - Enter / Space 触发 cursor 项的 on_select 并通过 on_open_change(false) 关菜单；
//! - widget_state_map 把 cursor 项标 hovered=true 让 collect 用 hover 背景渲染。
//!
//! 未覆盖（待后续）：Left/Right 在 MenuBar 内切换条目；submenu 嵌套展开；字母 type-ahead。

use std::collections::HashMap;

use crate::foundation::view_model::ValueCommand;
use crate::platform::keyboard::{Key, KeyCode, ModifiersState};
use crate::runtime::overlay::OverlayLayer;
use crate::ui::widget::{HitInteraction, KeyChord, MenuItemState, Rect, WidgetId, WidgetStateMap};

use super::BoundRuntimeHandler;

/// 单个 menu 的可点选项快照。
pub(super) struct MenuKeyboardItem<VM> {
    pub option_index: usize,
    pub rect: Rect,
    pub on_select: Option<crate::foundation::view_model::Command<VM>>,
    pub on_open_change: Option<ValueCommand<VM, bool>>,
}

impl<VM: 'static> BoundRuntimeHandler<VM> {
    /// 在当前 cached scene 上找最顶层的 Menu overlay；返回它的 widget_id（即
    /// `OverlayCloseHandle::overlay_id` 还原回的 trigger / menu source）。
    pub(super) fn topmost_open_menu_id(&self) -> Option<WidgetId> {
        let cached = self.cached_scene.as_ref()?;
        for handle in cached.computed.overlay_close_handlers.iter().rev() {
            if handle.layer == OverlayLayer::Menu {
                return Some(WidgetId::from_raw(handle.overlay_id.0));
            }
        }
        None
    }

    /// 扫描当前 cached scene 拿出某 menu 的所有可点选 items（按 option_index 升序）。
    /// 仅返回非 Disabled 的项；Separator / Disabled item 会被自动过滤掉。
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

    /// 让 cursor 在 items 上前进一位（dir = +1 下、-1 上），循环到首尾。
    /// 返回是否真的有 menu 在打开（用来决定是否吞键）。
    pub(super) fn advance_menu_keyboard_cursor(&mut self, dir: i32) -> bool {
        let Some(menu_id) = self.topmost_open_menu_id() else {
            return false;
        };
        let items = self.menu_keyboard_items(menu_id);
        if items.is_empty() {
            return true; // 仍吞键避免冒泡到 focus 切换
        }
        let current = self.menu_keyboard_cursor.get(&menu_id).copied();
        let cur_pos =
            current.and_then(|opt_idx| items.iter().position(|item| item.option_index == opt_idx));
        let next_pos = match cur_pos {
            Some(pos) => {
                let n = items.len() as i32;
                let mut p = pos as i32 + dir;
                p = ((p % n) + n) % n;
                p as usize
            }
            None => {
                if dir >= 0 {
                    0
                } else {
                    items.len() - 1
                }
            }
        };
        let chosen_opt_index = items[next_pos].option_index;
        self.menu_keyboard_cursor.insert(menu_id, chosen_opt_index);
        self.invalidate_computed_scene();
        true
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
        let Some(set_active) = current_menu.menubar_set_active.clone() else {
            return false;
        };
        // 扫所有同 group 的 menu，按 menubar_index 排序，取相邻 entry。
        let ids: Vec<WidgetId> = layout.all_widget_ids().collect();
        let mut peers: Vec<usize> = ids
            .into_iter()
            .filter_map(|id| {
                let resolved = layout.resolved_widget(id)?;
                let menu = resolved.menu.as_ref()?;
                (menu.menubar_group == Some(group)).then(|| menu.menubar_index)?
            })
            .collect();
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
        // 切换 active_index → user state 改 → 下帧 entry_open 切换。
        self.execute_value_command(&set_active, Some(target));
        // cursor 清掉，避免下次打开继承上次位置
        self.menu_keyboard_cursor.remove(&menu_id);
        self.invalidate_computed_scene();
        true
    }

    /// 字母 type-ahead：扫顶层菜单的 items，找到第一个 label 以 `letter`
    /// 开头（不区分大小写、跳过 separator/disabled）的项，把 cursor 移过去。
    /// 命中返回 true，未命中返回 false（来键照常走原有路径）。
    pub(super) fn type_ahead_menu_cursor(&mut self, letter: char) -> bool {
        let Some(menu_id) = self.topmost_open_menu_id() else {
            return false;
        };
        let Some(cached) = self.cached_scene.as_ref() else {
            return false;
        };
        let Some(layout) = cached.layout.as_ref() else {
            return false;
        };
        let Some(resolved) = layout.resolved_widget(menu_id) else {
            return false;
        };
        let Some(menu) = resolved.menu.as_ref() else {
            return false;
        };
        let target = letter.to_ascii_lowercase();
        let current_cursor = self.menu_keyboard_cursor.get(&menu_id).copied();
        let candidates: Vec<usize> = menu
            .items
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| {
                if item.disabled.resolve() {
                    return None;
                }
                match item.kind {
                    crate::ui::widget::MenuItemKind::Separator => None,
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
            .collect();
        if candidates.is_empty() {
            return false;
        }
        // 若有当前 cursor，跳到 cursor 之后的下一个；否则跳到第一个。
        let next = match current_cursor {
            Some(cur) => candidates
                .iter()
                .copied()
                .find(|idx| *idx > cur)
                .or_else(|| candidates.first().copied())
                .unwrap(),
            None => candidates[0],
        };
        self.menu_keyboard_cursor.insert(menu_id, next);
        self.invalidate_computed_scene();
        true
    }

    /// 触发 cursor 项的 on_select + on_open_change(false)。
    /// 返回是否处理了（用来吞键）。
    pub(super) fn activate_menu_keyboard_cursor(&mut self) -> bool {
        let Some(menu_id) = self.topmost_open_menu_id() else {
            return false;
        };
        let items = self.menu_keyboard_items(menu_id);
        let cursor_idx = self.menu_keyboard_cursor.get(&menu_id).copied();
        let chosen = match cursor_idx {
            Some(opt_idx) => items.into_iter().find(|item| item.option_index == opt_idx),
            None => items.into_iter().next(),
        };
        let Some(item) = chosen else {
            return true;
        };
        if let Some(command) = item.on_select {
            self.execute_command(&command);
        }
        if let Some(close) = item.on_open_change {
            self.execute_value_command(&close, false);
        }
        self.menu_keyboard_cursor.remove(&menu_id);
        self.invalidate_computed_scene();
        true
    }

    /// 在构造 WidgetStateMap 时调用：把当前每个 menu 的 cursor 项标 hovered=true，
    /// 这样 collect 阶段会用 item_background 的 hover 颜色渲染。
    pub(super) fn apply_menu_keyboard_cursor_to_states(&self, states: &mut WidgetStateMap) {
        let open_menus: HashMap<WidgetId, ()> = if let Some(cached) = self.cached_scene.as_ref() {
            cached
                .computed
                .overlay_close_handlers
                .iter()
                .filter(|h| h.layer == OverlayLayer::Menu)
                .map(|h| (WidgetId::from_raw(h.overlay_id.0), ()))
                .collect()
        } else {
            HashMap::new()
        };
        for (menu_id, cursor_opt_index) in &self.menu_keyboard_cursor {
            if !open_menus.contains_key(menu_id) {
                continue;
            }
            let mut state = states.get_select_option(*menu_id, *cursor_opt_index);
            state.hovered = true;
            states.set_select_option(*menu_id, *cursor_opt_index, state);
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
        let ids: Vec<WidgetId> = layout.all_widget_ids().collect();
        let mut matched_commands: Vec<crate::foundation::view_model::Command<VM>> = Vec::new();
        for id in ids {
            let Some(resolved) = layout.resolved_widget(id) else {
                continue;
            };
            if let Some(menu) = resolved.menu.as_ref() {
                collect_shortcut_matches(&menu.items, mods, key, code, &mut matched_commands);
            }
            if let Some(ctx_menu) = resolved.context_menu.as_ref() {
                collect_shortcut_matches(&ctx_menu.items, mods, key, code, &mut matched_commands);
            }
        }
        if matched_commands.is_empty() {
            return false;
        }
        // 同一 chord 同时绑定多处时按 widget 树顺序全部执行（典型场景下只会一个）。
        for command in &matched_commands {
            self.execute_command(command);
        }
        self.invalidate_computed_scene();
        true
    }
}

fn collect_shortcut_matches<VM>(
    items: &[MenuItemState<VM>],
    mods: ModifiersState,
    key: &Key,
    code: KeyCode,
    out: &mut Vec<crate::foundation::view_model::Command<VM>>,
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
