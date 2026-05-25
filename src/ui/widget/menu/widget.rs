//! [`Menu`] widget builder。
//!
//! Menu 是一个修饰符——它接受一个 trigger child（通常是 `Button`）作为可见
//! widget，然后通过 `Element::menu = Some(MenuDescriptor)` 把菜单挂上去。
//! 真正的浮层渲染发生在 collect 阶段（见 `collect/menu.rs`）。
//!
//! ```ignore
//! Menu::new(Button::new("文件"))
//!     .items(vec![
//!         MenuItem::new("新建").on_select(cmd!(...)),
//!         MenuItem::separator(),
//!         MenuItem::new("退出").on_select(cmd!(...)),
//!     ])
//!     .open(state.menu_open.signal())
//!     .on_open_change(cmd!(|vm: &mut Vm, open: bool| vm.menu_open.set(open)))
//! ```

use crate::foundation::view_model::ValueCommand;
use crate::ui::layout::Value;
use crate::ui::widget::core::Element;
use crate::ui::widget::overlay::{Alignment, FlipPolicy, Placement};
use crate::ui::widget::style::MenuStyle;

use super::descriptor::{MenuDescriptor, MenuItemState};
use super::types::{MenuBarGroupId, MenuItem};

/// Menu widget builder。挂在任意 trigger element 上，被点击或键盘激活时
/// 弹出下拉菜单。
pub struct Menu<VM> {
    trigger: Element<VM>,
    items: Vec<MenuItem<VM>>,
    open: Option<Value<bool>>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    placement: Placement,
    flip_policy: FlipPolicy,
    disabled: Value<bool>,
    style: Option<MenuStyle>,
    menubar_group: Option<MenuBarGroupId>,
    menubar_index: Option<usize>,
    menubar_set_active: Option<ValueCommand<VM, Option<usize>>>,
}

impl<VM> Menu<VM> {
    /// 用 trigger child（通常是 `Button`、`Text`、`Image` 等）创建 Menu。
    pub fn new(trigger: impl Into<Element<VM>>) -> Self {
        Self {
            trigger: trigger.into(),
            items: Vec::new(),
            open: None,
            on_open_change: None,
            placement: Placement::bottom().align(Alignment::Start),
            flip_policy: FlipPolicy::FlipSide,
            disabled: Value::Static(false),
            style: None,
            menubar_group: None,
            menubar_index: None,
            menubar_set_active: None,
        }
    }

    /// 设置所有菜单项。
    pub fn items(mut self, items: Vec<MenuItem<VM>>) -> Self {
        self.items = items;
        self
    }

    /// 追加单个菜单项。
    pub fn item(mut self, item: MenuItem<VM>) -> Self {
        self.items.push(item);
        self
    }

    /// 绑定外部 `Signal<bool>` 控制菜单的展开/收起。
    /// 若未提供，runtime 会维护内部开闭状态（step 6 落地）。
    pub fn open(mut self, open: impl Into<Value<bool>>) -> Self {
        self.open = Some(open.into());
        self
    }

    /// 菜单开/关切换回调。点击 trigger、外部点击、Esc 关闭等都会触发此命令。
    pub fn on_open_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_open_change = Some(command);
        self
    }

    /// 自定义浮层相对 trigger 的方位（默认 `Placement::bottom() + Alignment::Start`）。
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// 自定义翻转策略（默认 `FlipPolicy::FlipSide`）。
    pub fn flip_policy(mut self, policy: FlipPolicy) -> Self {
        self.flip_policy = policy;
        self
    }

    /// 禁用整个菜单（trigger 仍可见但不响应）。
    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        self.disabled = disable.into();
        self
    }

    /// 覆盖默认主题样式。
    pub fn style(mut self, style: MenuStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// MenuBar 内部接口：把本 Menu 绑定到某个 MenuBar 分组的指定 entry 上。
    pub(crate) fn menubar_binding(mut self, group: MenuBarGroupId, index: usize) -> Self {
        self.menubar_group = Some(group);
        self.menubar_index = Some(index);
        self
    }

    /// MenuBar 内部接口：注入 active_index 切换命令，让 runtime 的 Left/Right
    /// 在 MenuBar 之间直接切换条目。
    pub(crate) fn menubar_set_active_command(
        mut self,
        command: ValueCommand<VM, Option<usize>>,
    ) -> Self {
        self.menubar_set_active = Some(command);
        self
    }
}

impl<VM> From<Menu<VM>> for Element<VM> {
    fn from(menu: Menu<VM>) -> Element<VM> {
        let Menu {
            mut trigger,
            items,
            open,
            on_open_change,
            placement,
            flip_policy,
            disabled,
            style,
            menubar_group,
            menubar_index,
            menubar_set_active,
        } = menu;

        let descriptor = MenuDescriptor {
            items: items.into_iter().map(MenuItemState::from_public).collect(),
            open: open.unwrap_or(Value::Static(false)),
            on_open_change,
            placement,
            flip_policy,
            disabled,
            style,
            menubar_group,
            menubar_index,
            menubar_set_active,
        };
        trigger.menu = Some(descriptor);
        trigger
    }
}
