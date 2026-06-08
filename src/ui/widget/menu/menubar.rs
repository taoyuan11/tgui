//! [`MenuBar`] widget builder。
//!
//! MenuBar 不是独立的 `WidgetKind`，而是一个 builder——`From<MenuBar<VM>>`
//! 把它转成 `Flex<VM>`，每个 entry 是一个挂了 `MenuDescriptor` 的 Button
//! （参考 `RadioGroup`）。每个 entry 上的 Menu 都关联同一个 `MenuBarGroupId`
//! 以便 runtime 在它们之间做 Left/Right 切换（step 6 落地）。
//!
//! ```ignore
//! MenuBar::new(state.menubar_active.signal())
//!     .on_active_change(cmd!(|vm, idx| vm.menubar_active.set(idx)))
//!     .entry("文件", vec![MenuItem::new("新建")...])
//!     .entry("编辑", vec![MenuItem::new("撤销")...])
//! ```

use crate::foundation::view_model::{Command, CommandContext, ValueCommand};
use crate::ui::layout::Axis;
use crate::ui::layout::Value;
use crate::ui::theme::{StateValue, StyleContext};
use crate::ui::widget::button::Button;
use crate::ui::widget::container::Flex;
use crate::ui::widget::core::Element;
use crate::ui::widget::overlay::{Alignment, Placement};
use crate::ui::widget::style::{
    ButtonStyle, ContainerStyle, MenuBarStyle, MenuStyle, StyleResolver,
};

use super::super::common::ButtonVariantKind;
use super::types::{MenuBarGroupId, MenuItem};
use super::widget::Menu;

/// MenuBar 单个顶级条目。
#[derive(Clone)]
pub struct MenuBarEntry<VM> {
    label: Value<String>,
    items: Vec<MenuItem<VM>>,
    disabled: Value<bool>,
}

impl<VM> MenuBarEntry<VM> {
    pub fn new(label: impl Into<Value<String>>) -> Self {
        Self {
            label: label.into(),
            items: Vec::new(),
            disabled: Value::Static(false),
        }
    }

    pub fn items(mut self, items: Vec<MenuItem<VM>>) -> Self {
        self.items = items;
        self
    }

    pub fn item(mut self, item: MenuItem<VM>) -> Self {
        self.items.push(item);
        self
    }

    pub fn disable(mut self, disable: impl Into<Value<bool>>) -> Self {
        self.disabled = disable.into();
        self
    }
}

/// MenuBar widget。
///
/// `active_index` 是当前展开的条目下标——`None` 表示无菜单展开。点击某个 entry
/// 会调用 `on_active_change(Some(index) 或 None)` 切换。
pub struct MenuBar<VM> {
    entries: Vec<MenuBarEntry<VM>>,
    active_index: Option<Value<Option<usize>>>,
    on_active_change: Option<ValueCommand<VM, Option<usize>>>,
    group: MenuBarGroupId,
    style: Option<StyleResolver<MenuBarStyle>>,
    menu_style: Option<StyleResolver<MenuStyle>>,
}

impl<VM> MenuBar<VM> {
    /// 创建 MenuBar，绑定外部 `Signal<Option<usize>>` 作为 active_index 数据源。
    pub fn new(active_index: impl Into<Value<Option<usize>>>) -> Self {
        Self {
            entries: Vec::new(),
            active_index: Some(active_index.into()),
            on_active_change: None,
            group: MenuBarGroupId::next(),
            style: None,
            menu_style: None,
        }
    }

    /// 创建由 runtime 自动维护 active entry 的 MenuBar。
    pub fn uncontrolled() -> Self {
        Self {
            entries: Vec::new(),
            active_index: None,
            on_active_change: None,
            group: MenuBarGroupId::next(),
            style: None,
            menu_style: None,
        }
    }

    /// 设置 active_index 变更回调。点击 entry 触发：
    /// - 若当前 active_index == Some(我自己)，调用 `cmd(None)`（收起）；
    /// - 否则调用 `cmd(Some(我))`（切到我）。
    /// 若没接此 callback，MenuBar 的 entry 仍可视、但点击不会打开下拉。
    pub fn on_active_change(mut self, command: ValueCommand<VM, Option<usize>>) -> Self {
        self.on_active_change = Some(command);
        self
    }

    /// 追加一个 entry。
    pub fn entry(mut self, label: impl Into<Value<String>>, items: Vec<MenuItem<VM>>) -> Self {
        self.entries.push(MenuBarEntry::new(label).items(items));
        self
    }

    /// 追加已构造好的 entry。
    pub fn entries(mut self, entries: Vec<MenuBarEntry<VM>>) -> Self {
        self.entries.extend(entries);
        self
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut MenuBarStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| MenuBarStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> MenuBarStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    pub fn menu_style(
        mut self,
        mutator: impl Fn(&mut MenuStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.menu_style = Some(StyleResolver::mutate(
            |context| MenuStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn menu_style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> MenuStyle + Send + Sync + 'static,
    ) -> Self {
        self.menu_style = Some(StyleResolver::full(resolver));
        self
    }
}

impl<VM> From<MenuBar<VM>> for Element<VM>
where
    VM: 'static,
{
    fn from(bar: MenuBar<VM>) -> Element<VM> {
        let MenuBar {
            entries,
            active_index,
            on_active_change,
            group,
            style,
            menu_style,
        } = bar;

        let layout_style = resolve_menu_bar_style_for_layout(style.as_ref());
        let mut children: Vec<Element<VM>> = Vec::with_capacity(entries.len());
        for (index, entry) in entries.into_iter().enumerate() {
            let MenuBarEntry {
                label,
                items,
                disabled,
            } = entry;

            // 每个 entry 的下拉是否展开 = active_index == Some(index)；uncontrolled
            // 模式下不写 open，让 runtime 通过 MenuBarGroupId 维护 active entry。
            let entry_open = active_index.clone().map(|active_index| match active_index {
                Value::Static(value) => Value::Static(value == Some(index)),
                Value::Signal(signal) => Value::Signal(signal.map(move |idx| idx == Some(index))),
            });

            // 点击事件：toggle 我这一项
            let entry_style = style.clone();
            let mut button = Button::new(label)
                .disable(disabled.clone())
                .height(layout_style.height)
                .min_width(layout_style.entry_min_width)
                .style_full(move |context| {
                    menu_bar_entry_button_style(
                        resolve_menu_bar_style(entry_style.as_ref(), context),
                        context,
                    )
                });
            if let Some(on_change) = on_active_change.clone() {
                if let Some(active_signal) = active_index.clone() {
                    let click_cmd =
                        Command::new_with_context(move |vm: &mut VM, ctx: &CommandContext<VM>| {
                            let current = active_signal.resolve();
                            let next = if current == Some(index) {
                                None
                            } else {
                                Some(index)
                            };
                            on_change.execute_with_context(vm, next, ctx);
                        });
                    button = button.on_click(click_cmd);
                }
            }

            // 菜单 entry 切换关闭回调：用户从菜单内部触发 close（外部点击 / Esc / item 选中）时，
            // 重置 active_index 到 None。
            let mut menu = Menu::new(button)
                .items(items)
                .placement(Placement::bottom().align(Alignment::Start))
                .menubar_binding(group, index);
            if let Some(entry_open) = entry_open {
                menu = menu.open(entry_open);
            }
            if let Some(on_change) = on_active_change.clone() {
                menu = menu.menubar_set_active_command(on_change);
            }
            if let Some(style) = menu_style.clone() {
                menu = menu.style_full(move |context| resolve_menu_style(Some(&style), context));
            }
            if let Some(on_change) = on_active_change.clone() {
                menu = menu.on_open_change(ValueCommand::new_with_context(
                    move |vm: &mut VM, open: bool, ctx: &CommandContext<VM>| {
                        if !open {
                            on_change.execute_with_context(vm, None, ctx);
                        }
                    },
                ));
            }

            children.push(menu.into());
        }

        let root_style = style.clone();
        Flex::new(Axis::Horizontal)
            .height(layout_style.height)
            .padding(layout_style.padding)
            .gap(layout_style.entry_gap)
            .style_full(move |context| {
                menu_bar_container_style(
                    resolve_menu_bar_style(root_style.as_ref(), context),
                    context,
                )
            })
            .child(children)
            .into()
    }
}

fn resolve_menu_bar_style(
    style: Option<&StyleResolver<MenuBarStyle>>,
    context: &StyleContext<'_>,
) -> MenuBarStyle {
    let mut base = MenuBarStyle::default_for_theme(context.theme);
    context.theme.components.menu_bar.apply(&mut base, context);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

fn resolve_menu_bar_style_for_layout(style: Option<&StyleResolver<MenuBarStyle>>) -> MenuBarStyle {
    let theme = crate::ui::theme::Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_menu_bar_style(style, &context)
}

fn resolve_menu_style(
    style: Option<&StyleResolver<MenuStyle>>,
    context: &StyleContext<'_>,
) -> MenuStyle {
    let mut base = MenuStyle::default_for_theme(context.theme);
    context.theme.components.menu.apply(&mut base, context);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}

fn menu_bar_container_style(style: MenuBarStyle, context: &StyleContext<'_>) -> ContainerStyle {
    let mut container = ContainerStyle::default_for_theme(context.theme);
    container.surface = style.surface;
    container.surface.background = Some(style.background);
    container.surface.border_color = Some(style.border);
    container.surface.border_width = Some(style.border_width);
    container.surface.border_radius = Some(style.radius);
    container
}

fn menu_bar_entry_button_style(style: MenuBarStyle, context: &StyleContext<'_>) -> ButtonStyle {
    let mut button = ButtonStyle::default_for_theme(context.theme, ButtonVariantKind::Ghost);
    button.background = style.entry_background;
    button.background.open = Some(style.entry_active_background);
    button.foreground = style.entry_foreground;
    button.border = StateValue::new(crate::foundation::color::Color::TRANSPARENT.into());
    button.border_width = crate::ui::unit::Dp::ZERO.into();
    button.radius = style.radius;
    button.padding_x = style.entry_padding_x;
    button.padding_y = crate::ui::unit::Dp::ZERO;
    button.min_height = style.height;
    button.text_style = style.text_style;
    button
}
