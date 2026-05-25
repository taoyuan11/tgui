//! Menu 公开数据类型：MenuItem 构造器、菜单项种类、图标、快捷键。
//!
//! 这些类型同时被 `Menu`、`ContextMenu`、`MenuBar` 三个 widget 共享。
//! 与 `Tooltip` 一致——Menu / ContextMenu 不是独立的 `WidgetKind`，而是挂在
//! `Element::menu` / `Element::context_menu` 上的修饰符。MenuBar 则是
//! 一个纯 builder，组装出带 menu 的若干 Button（参考 `RadioGroup`）。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::foundation::view_model::Command;
use crate::platform::keyboard::{Key, KeyCode, ModifiersState, NamedKey};
use crate::ui::layout::Value;

/// 菜单项类型。
///
/// - `Action`：普通可点击命令；
/// - `Separator`：分隔线，不可命中；
/// - `Checkable`：带勾选状态的命令，触发后由调用方更新 `checked`；
/// - `Submenu`：本身不触发命令，hover / 方向键展开 `children`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItemKind {
    Action,
    Separator,
    Checkable,
    Submenu,
}

/// 菜单项前缀图标。
///
/// 目前仅支持 SVG 字节数据，与 `Image` widget 复用 `media/svg` 栅格化通道。
#[derive(Clone)]
pub enum MenuIcon {
    /// 内嵌 SVG 字节数据。当前 collect 阶段尚不渲染 SVG（OverlayPrimitive
    /// 不支持 Image），仅占位——后续接入 overlay image 时启用。
    Svg(Arc<[u8]>),
    /// 单个 Unicode 字符 / emoji glyph。collect 阶段直接以文本渲染，
    /// 是 SVG 落地前的过渡方案。例如 `MenuIcon::glyph('📁')`。
    Glyph(char),
}

impl MenuIcon {
    /// 用静态 SVG 字节切片构造图标（暂未在 collect 阶段渲染，见 enum 上的注释）。
    pub fn svg(bytes: &'static [u8]) -> Self {
        MenuIcon::Svg(Arc::from(bytes))
    }

    /// 用任意 SVG 字节数据构造图标。
    pub fn svg_owned(bytes: Vec<u8>) -> Self {
        MenuIcon::Svg(Arc::from(bytes.into_boxed_slice()))
    }

    /// 用单个 glyph 字符构造图标（emoji / 字体图标）。
    pub fn glyph(ch: char) -> Self {
        MenuIcon::Glyph(ch)
    }
}

impl std::fmt::Debug for MenuIcon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MenuIcon::Svg(bytes) => f
                .debug_struct("MenuIcon::Svg")
                .field("len", &bytes.len())
                .finish(),
            MenuIcon::Glyph(ch) => f.debug_tuple("MenuIcon::Glyph").field(ch).finish(),
        }
    }
}

/// 键盘组合键描述符。
///
/// 由 `MenuItem::shortcut()` 注册；菜单 widget 在 mount 时把它加入全局
/// 快捷键表，unmount 时反注册。`shortcut_hint` 字段则是纯显示用的字符串。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub(crate) mods: ModifiersState,
    pub(crate) key: ChordKey,
}

/// `KeyChord` 内部的键位标识，可来自字符键（`KeyCode`）或具名键（`NamedKey`）。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ChordKey {
    Code(KeyCode),
    Named(NamedKey),
}

impl KeyChord {
    /// 创建一个仅含键位的 chord（无 modifier）。
    pub fn new(key: KeyCode) -> Self {
        Self {
            mods: ModifiersState::empty(),
            key: ChordKey::Code(key),
        }
    }

    /// 创建一个具名键的 chord，例如 F1 / Escape。
    pub fn named(key: NamedKey) -> Self {
        Self {
            mods: ModifiersState::empty(),
            key: ChordKey::Named(key),
        }
    }

    /// 在 chord 上叠加 Ctrl（在 macOS 上对应 Command 由调用方决定）。
    pub fn ctrl(mut self) -> Self {
        self.mods |= ModifiersState::CONTROL;
        self
    }

    /// 在 chord 上叠加 Shift。
    pub fn shift(mut self) -> Self {
        self.mods |= ModifiersState::SHIFT;
        self
    }

    /// 在 chord 上叠加 Alt。
    pub fn alt(mut self) -> Self {
        self.mods |= ModifiersState::ALT;
        self
    }

    /// 在 chord 上叠加 Meta (Super / Command)。
    pub fn meta(mut self) -> Self {
        self.mods |= ModifiersState::META;
        self
    }

    /// 判断给定的修饰符 + 物理键是否命中本 chord。
    pub fn matches(&self, mods: ModifiersState, key: &Key, code: KeyCode) -> bool {
        if self.mods != mods {
            return false;
        }
        match &self.key {
            ChordKey::Code(target) => *target == code,
            ChordKey::Named(target) => matches!(key, Key::Named(named) if named == target),
        }
    }
}

/// 公开的菜单项构造器。
///
/// 链式构造完毕后传给 `Menu::items()` / `ContextMenu::items()` / `MenuBar` entry。
/// 转 `MenuItemState<VM>` 由内部 descriptor 完成；调用方不直接接触 state 类型。
#[derive(Clone)]
pub struct MenuItem<VM> {
    pub(crate) kind: MenuItemKind,
    pub(crate) label: Option<Value<String>>,
    pub(crate) icon: Option<MenuIcon>,
    pub(crate) shortcut_hint: Option<Value<String>>,
    pub(crate) shortcut: Option<KeyChord>,
    pub(crate) disabled: Value<bool>,
    pub(crate) checked: Option<Value<bool>>,
    pub(crate) on_select: Option<Command<VM>>,
    pub(crate) submenu: Vec<MenuItem<VM>>,
}

impl<VM> MenuItem<VM> {
    /// 创建一个普通可点击菜单项。
    pub fn new(label: impl Into<Value<String>>) -> Self {
        Self {
            kind: MenuItemKind::Action,
            label: Some(label.into()),
            icon: None,
            shortcut_hint: None,
            shortcut: None,
            disabled: Value::Static(false),
            checked: None,
            on_select: None,
            submenu: Vec::new(),
        }
    }

    /// 创建一根水平分隔线，不可命中。
    pub fn separator() -> Self {
        Self {
            kind: MenuItemKind::Separator,
            label: None,
            icon: None,
            shortcut_hint: None,
            shortcut: None,
            disabled: Value::Static(true),
            checked: None,
            on_select: None,
            submenu: Vec::new(),
        }
    }

    /// 创建一个勾选项。初始勾选状态由 `checked` 参数指定，触发后由
    /// `on_select` 回调负责更新外部 `State<bool>`。
    pub fn checkable(label: impl Into<Value<String>>) -> Self {
        Self {
            kind: MenuItemKind::Checkable,
            label: Some(label.into()),
            icon: None,
            shortcut_hint: None,
            shortcut: None,
            disabled: Value::Static(false),
            checked: Some(Value::Static(false)),
            on_select: None,
            submenu: Vec::new(),
        }
    }

    /// 创建一个 submenu 父项。`children` 是该子菜单的所有项。
    pub fn submenu(label: impl Into<Value<String>>, children: Vec<MenuItem<VM>>) -> Self {
        Self {
            kind: MenuItemKind::Submenu,
            label: Some(label.into()),
            icon: None,
            shortcut_hint: None,
            shortcut: None,
            disabled: Value::Static(false),
            checked: None,
            on_select: None,
            submenu: children,
        }
    }

    /// 设置图标。
    pub fn icon(mut self, icon: MenuIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// 设置右侧快捷键提示文本（仅显示，不绑全局键）。
    pub fn shortcut_hint(mut self, hint: impl Into<Value<String>>) -> Self {
        self.shortcut_hint = Some(hint.into());
        self
    }

    /// 绑定真正的全局快捷键。如果同时未设 `shortcut_hint`，会自动按
    /// `mods + key` 派生一份显示文本。
    pub fn shortcut(mut self, chord: KeyChord) -> Self {
        self.shortcut = Some(chord);
        self
    }

    /// 标记禁用状态。
    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        self.disabled = disable.into();
        self
    }

    /// 设置勾选状态（仅对 `Checkable` 类型有意义）。
    pub fn checked(mut self, checked: impl Into<Value<bool>>) -> Self {
        self.checked = Some(checked.into());
        self
    }

    /// 设置选中回调。
    pub fn on_select(mut self, command: Command<VM>) -> Self {
        self.on_select = Some(command);
        self
    }
}

/// MenuBar 内部分组标识，用于把同一条 MenuBar 上的若干个 entries 关联起来，
/// 让 runtime 可以在它们之间响应 Left/Right 切换。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuBarGroupId(u64);

impl MenuBarGroupId {
    pub(crate) fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    pub(crate) fn raw(self) -> u64 {
        self.0
    }
}
