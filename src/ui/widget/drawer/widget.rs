//! [`Drawer`] widget builder。
//!
//! ```ignore
//! Drawer::new(state.drawer_open.signal())
//!     .on_open_change(ValueCommand::new(|vm, open| vm.drawer_open.set(open)))
//!     .placement(DrawerPlacement::Left)
//!     .content(
//!         Flex::new(Axis::Vertical)
//!             .child(Text::new("Navigation"))
//!             .child(Button::new("Home"))
//!             .child(Button::new("Settings"))
//!             .into()
//!     )
//! ```
//!
//! 实现要点：
//! - `Drawer::From<Element>` 把自身展开为一个 `position_absolute + 全屏 fill`
//!   的 Stack，里面放两个常驻 child：backdrop（半透明遮罩）+ panel（侧边栏面板）；
//! - panel 根据 placement 决定从哪个边缘滑出，使用 `position_absolute` 定位；
//! - 外层 Stack 在打开时挂动态 active 的
//!   `FocusScopeOptions::{trap, auto_focus_first}`，使 Tab 在 drawer 内循环，并让
//!   backdrop 点击留在 trap scope 内；
//! - backdrop + panel 的动画：backdrop 走 `opacity` fade，panel 走对应方向的位移
//!   （left/right/top/bottom）+ `opacity` 组合实现 slide + fade；
//! - Esc 关闭 / on_close 派发 / focus return 由挂在外层 Stack 上的
//!   `DrawerDescriptor` + collect 阶段 sentinel overlay 完成；
//! - `close_on_backdrop_click` 在 collect 阶段按当前 open 信号注入 backdrop hit
//!   region，避免初始关闭的 Drawer 打开后缺少点击 handler。

use std::time::Duration;

use crate::animation::Transition;
use crate::foundation::color::Color;
use crate::foundation::view_model::ValueCommand;
use crate::log::Log;
use crate::theme::ResolvedThemeMode;
use crate::ui::layout::{pct, Axis, Insets, LayoutStyle, Value};
use crate::ui::unit::Dp;
use crate::ui::widget::container::{set_layout_length, set_layout_lengths, IntoLengthValue};
use crate::ui::widget::container::{Flex, Stack};
use crate::ui::widget::core::Element;
use crate::ui::widget::style::{ContainerStyle, DrawerStyle};
use crate::ui::widget::{CursorStyle, FocusScopeOptions, WidgetId};

use super::descriptor::DrawerDescriptor;
use super::placement::DrawerPlacement;

const DRAWER_SLIDE_DURATION_MS: u64 = 250;

/// Drawer presentation mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawerMode {
    Overlay,
    Push,
}

impl Default for DrawerMode {
    fn default() -> Self {
        Self::Overlay
    }
}

/// 从屏幕边缘滑出的侧边栏 builder。
pub struct Drawer<VM> {
    open: Value<bool>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    placement: DrawerPlacement,
    mode: DrawerMode,
    content: Option<Element<VM>>,
    close_on_escape: bool,
    close_on_backdrop_click: bool,
    return_focus_to: Option<WidgetId>,
    auto_focus_first: bool,
    style: Option<DrawerStyle>,
}

impl<VM: 'static> Drawer<VM> {
    /// 用绑定 open 状态的 `Signal<bool>`（或常量 bool）创建 Drawer。
    pub fn new(open: impl Into<Value<bool>>) -> Self {
        Self {
            open: open.into(),
            on_open_change: None,
            placement: DrawerPlacement::default(),
            mode: DrawerMode::Overlay,
            content: None,
            close_on_escape: true,
            close_on_backdrop_click: true,
            return_focus_to: None,
            auto_focus_first: true,
            style: None,
        }
    }

    /// 关闭时（Esc / backdrop click / overlay close）触发的回调。
    /// 值参为新的 `open`（恒为 `false`）。
    pub fn on_open_change(mut self, command: ValueCommand<VM, bool>) -> Self {
        self.on_open_change = Some(command);
        self
    }

    /// 设置 Drawer 从哪个边缘滑出（默认 Left）。
    pub fn placement(mut self, placement: DrawerPlacement) -> Self {
        self.placement = placement;
        self
    }

    /// 设置 Drawer 模式。`Push` 只有放在 [`DrawerHost`] 中才会启用。
    pub fn mode(mut self, mode: DrawerMode) -> Self {
        self.mode = mode;
        self
    }

    /// 设置内容区元素（任意 widget 子树）。
    pub fn content(mut self, content: impl Into<Element<VM>>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// 是否允许 Esc 关闭（默认 `true`）。
    pub fn close_on_escape(mut self, on: bool) -> Self {
        self.close_on_escape = on;
        self
    }

    /// 是否允许点击 backdrop 关闭（默认 `true`）。
    pub fn close_on_backdrop_click(mut self, on: bool) -> Self {
        self.close_on_backdrop_click = on;
        self
    }

    /// 关闭后把焦点还给指定 widget。
    pub fn return_focus_to(mut self, widget_id: WidgetId) -> Self {
        self.return_focus_to = Some(widget_id);
        self
    }

    /// 打开时是否自动聚焦 drawer 内第一个可聚焦控件（默认 `true`）。
    pub fn auto_focus_first(mut self, auto_focus_first: bool) -> Self {
        self.auto_focus_first = auto_focus_first;
        self
    }

    /// 覆盖默认主题样式。
    pub fn style(mut self, style: DrawerStyle) -> Self {
        self.style = Some(style);
        self
    }
}

impl<VM: 'static> From<Drawer<VM>> for Element<VM> {
    fn from(drawer: Drawer<VM>) -> Element<VM> {
        let Drawer {
            open,
            on_open_change,
            placement,
            mode,
            content,
            close_on_escape,
            close_on_backdrop_click,
            return_focus_to,
            auto_focus_first,
            style,
        } = drawer;

        if mode == DrawerMode::Push {
            Log::with_tag("tgui-drawer").debug(format_args!(
                "Drawer::mode(Push) requires DrawerHost; falling back to Overlay"
            ));
        }

        // -----------------------------------------------------------------
        // 如果是静态 false，直接返回空元素，避免渲染蒙层
        // -----------------------------------------------------------------
        if let Value::Static(false) = open {
            return Stack::<VM>::new()
                .size(Dp::ZERO, Dp::ZERO)
                .position_absolute()
                .into();
        }

        // -----------------------------------------------------------------
        // 动画过渡配置
        // -----------------------------------------------------------------
        let slide_transition =
            Transition::ease_in_out(Duration::from_millis(DRAWER_SLIDE_DURATION_MS));

        // backdrop fade 动画
        let backdrop_visibility: Value<f32> = match open.clone() {
            Value::Static(open_now) => Value::Static(if open_now { 1.0 } else { 0.0 }),
            Value::Signal(signal) => Value::Signal(
                signal
                    .map(|o| if o { 1.0 } else { 0.0 })
                    .animated(slide_transition),
            ),
        };

        // -----------------------------------------------------------------
        // backdrop：覆盖整个区域的半透明 scrim
        // 根据 open 状态动态设置背景色，确保关闭时完全透明
        // -----------------------------------------------------------------
        let backdrop_color_base = match &style {
            Some(s) => s.backdrop_color.clone(),
            None => Value::Static(Color::rgba(0, 0, 0, 0x80)),
        };

        // 根据 open 状态动态设置背景色
        // 关键：无论是 Static 还是 Signal，关闭时都必须是透明色
        let backdrop_color = match open.clone() {
            Value::Static(open_now) => {
                if open_now {
                    backdrop_color_base
                } else {
                    Value::Static(Color::TRANSPARENT)
                }
            }
            Value::Signal(signal) => {
                // 对于 Signal，根据 open 状态动态切换颜色
                let base_color = match backdrop_color_base {
                    Value::Static(c) => c,
                    Value::Signal(_) => {
                        // 如果 backdrop_color 本身也是 Signal，取第一个值
                        // 这种情况很少见，通常 backdrop_color 是静态的
                        Color::rgba(0, 0, 0, 0x80)
                    }
                };
                Value::Signal(signal.map(
                    move |open| {
                        if open {
                            base_color
                        } else {
                            Color::TRANSPARENT
                        }
                    },
                ))
            }
        };

        let backdrop = Stack::<VM>::new()
            .size(pct(100.0), pct(100.0))
            .position_absolute()
            .left(Dp::ZERO)
            .top(Dp::ZERO)
            .opacity(backdrop_visibility.clone())
            .style(move |mode| {
                let mut s = ContainerStyle::default_for(mode);
                s.surface.background = Some(backdrop_color.clone());
                s.surface.border_color = Some(Color::TRANSPARENT.into());
                s.surface.border_width = Some(Dp::ZERO.into());
                s.surface.border_radius = Some(Dp::ZERO.into());
                s
            });
        let backdrop_element: Element<VM> = backdrop.into();
        let backdrop_widget_id = backdrop_element.id;

        // -----------------------------------------------------------------
        // panel：侧边栏面板，根据 placement 决定位置和滑动方向
        // -----------------------------------------------------------------
        let drawer_style_for_panel = style.clone();
        let resolved_style_for_layout = style
            .clone()
            .unwrap_or_else(|| DrawerStyle::default_for(ResolvedThemeMode::Light));

        // 根据 placement 计算 panel 的位置和尺寸
        let (panel_width, panel_height) = match placement {
            DrawerPlacement::Left | DrawerPlacement::Right => {
                (Some(resolved_style_for_layout.width), None)
            }
            DrawerPlacement::Top | DrawerPlacement::Bottom => {
                (None, Some(resolved_style_for_layout.height))
            }
        };

        // 计算 slide 动画的位移值
        // open=true 时位置为 0，open=false 时位置为负的宽度/高度（隐藏在屏幕外）
        // 注意：left/right/top/bottom 都使用负值来表示"移出屏幕"
        let slide_offset: Value<Dp> = match open.clone() {
            Value::Static(open_now) => {
                if open_now {
                    Value::Static(Dp::ZERO)
                } else {
                    match placement {
                        DrawerPlacement::Left => Value::Static(-resolved_style_for_layout.width),
                        DrawerPlacement::Right => Value::Static(-resolved_style_for_layout.width),
                        DrawerPlacement::Top => Value::Static(-resolved_style_for_layout.height),
                        DrawerPlacement::Bottom => Value::Static(-resolved_style_for_layout.height),
                    }
                }
            }
            Value::Signal(signal) => {
                let width = resolved_style_for_layout.width;
                let height = resolved_style_for_layout.height;
                Value::Signal(
                    signal
                        .map(move |o| {
                            if o {
                                Dp::ZERO
                            } else {
                                match placement {
                                    DrawerPlacement::Left => -width,
                                    DrawerPlacement::Right => -width,
                                    DrawerPlacement::Top => -height,
                                    DrawerPlacement::Bottom => -height,
                                }
                            }
                        })
                        .animated(slide_transition),
                )
            }
        };

        // 构建 panel 容器
        let mut panel: Flex<VM> = Flex::new(Axis::Vertical)
            .position_absolute()
            .cursor(CursorStyle::Default)
            // 不在 panel 上设置 opacity，由外层 Stack 统一控制
            .style(move |mode| {
                let resolved = drawer_style_for_panel
                    .clone()
                    .unwrap_or_else(|| DrawerStyle::default_for(mode));
                let mut s = ContainerStyle::default_for(mode);
                s.surface.background = Some(resolved.background.clone());
                s.surface.border_color = Some(resolved.border.clone());
                s.surface.border_width = Some(resolved.border_width.clone());
                s.surface.border_radius = Some(resolved.radius.clone());
                s.surface.shadow = Some(resolved.shadow.into());
                s
            });
        // 根据 placement 设置位置和尺寸
        match placement {
            DrawerPlacement::Left => {
                panel = panel
                    .left(slide_offset)
                    .top(Dp::ZERO)
                    .width(panel_width.unwrap())
                    .height(pct(100.0));
            }
            DrawerPlacement::Right => {
                panel = panel
                    .right(slide_offset)
                    .top(Dp::ZERO)
                    .width(panel_width.unwrap())
                    .height(pct(100.0));
            }
            DrawerPlacement::Top => {
                panel = panel
                    .left(Dp::ZERO)
                    .top(slide_offset)
                    .width(pct(100.0))
                    .height(panel_height.unwrap());
            }
            DrawerPlacement::Bottom => {
                panel = panel
                    .left(Dp::ZERO)
                    .bottom(slide_offset)
                    .width(pct(100.0))
                    .height(panel_height.unwrap());
            }
        }

        // 设置 padding
        if resolved_style_for_layout.padding != Insets::ZERO {
            panel = panel.padding(resolved_style_for_layout.padding);
        }

        // 添加内容
        if let Some(content_element) = content {
            panel = panel.child(content_element);
        }

        let panel_element: Element<VM> = panel.into();
        let panel_widget_id = panel_element.id;

        // -----------------------------------------------------------------
        // 外层 Stack：全屏绝对定位容器，backdrop 仅在打开时捕获点击。
        // -----------------------------------------------------------------
        let outer: Stack<VM> = Stack::<VM>::new()
            .size(pct(100.0), pct(100.0))
            .position_absolute()
            .left(Dp::ZERO)
            .top(Dp::ZERO)
            .focus_scope(
                FocusScopeOptions::new()
                    .trap(true)
                    .auto_focus_first(auto_focus_first)
                    .active(open.clone()),
            )
            .child(backdrop_element)
            .child(panel_element);

        let mut outer_element: Element<VM> = outer.into();
        outer_element.drawer = Some(Box::new(DrawerDescriptor {
            open,
            on_open_change,
            placement,
            mode: DrawerMode::Overlay,
            close_on_escape,
            close_on_backdrop_click,
            return_focus_to,
            backdrop_widget_id,
            panel_widget_id,
            style,
        }));
        outer_element
    }
}

/// Host that enables [`DrawerMode::Push`] by laying out content and drawer panel as siblings.
pub struct DrawerHost<VM> {
    content: Element<VM>,
    drawer: Drawer<VM>,
    layout: LayoutStyle,
}

impl<VM: 'static> DrawerHost<VM> {
    pub fn new(content: impl Into<Element<VM>>, drawer: Drawer<VM>) -> Self {
        Self {
            content: content.into(),
            drawer,
            layout: LayoutStyle::default(),
        }
    }

    pub fn size(mut self, width: impl IntoLengthValue, height: impl IntoLengthValue) -> Self {
        set_layout_lengths(&mut self.layout, width, height);
        self
    }

    pub fn width(mut self, width: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.width, width);
        self
    }

    pub fn height(mut self, height: impl IntoLengthValue) -> Self {
        set_layout_length(&mut self.layout.height, height);
        self
    }

    pub fn grow(mut self, grow: impl Into<Value<f32>>) -> Self {
        self.layout.grow = grow.into();
        self
    }

    pub fn shrink(mut self, shrink: impl Into<Value<f32>>) -> Self {
        self.layout.shrink = shrink.into();
        self
    }
}

impl<VM: 'static> From<DrawerHost<VM>> for Element<VM> {
    fn from(host: DrawerHost<VM>) -> Element<VM> {
        let DrawerHost {
            content,
            drawer,
            layout,
        } = host;
        let mut element = match drawer.mode {
            DrawerMode::Overlay => Stack::<VM>::new()
                .size(pct(100.0), pct(100.0))
                .child(content)
                .child(Element::from(drawer))
                .into(),
            DrawerMode::Push => build_push_drawer_host(content, drawer),
        };
        element.layout = layout;
        element
    }
}

fn build_push_drawer_host<VM: 'static>(content: Element<VM>, drawer: Drawer<VM>) -> Element<VM> {
    let Drawer {
        open,
        on_open_change,
        placement,
        mode: _,
        content: drawer_content,
        close_on_escape,
        close_on_backdrop_click: _,
        return_focus_to,
        auto_focus_first,
        style,
    } = drawer;

    let slide_transition = Transition::ease_in_out(Duration::from_millis(DRAWER_SLIDE_DURATION_MS));
    let drawer_style_for_panel = style.clone();
    let resolved_style_for_layout = style
        .clone()
        .unwrap_or_else(|| DrawerStyle::default_for(ResolvedThemeMode::Light));
    let target_extent = match placement {
        DrawerPlacement::Left | DrawerPlacement::Right => resolved_style_for_layout.width,
        DrawerPlacement::Top | DrawerPlacement::Bottom => resolved_style_for_layout.height,
    };
    let panel_extent: Value<Dp> = match open.clone() {
        Value::Static(open_now) => Value::Static(if open_now { target_extent } else { Dp::ZERO }),
        Value::Signal(signal) => Value::Signal(
            signal
                .map(move |open| if open { target_extent } else { Dp::ZERO })
                .animated(slide_transition),
        ),
    };

    let mut panel: Flex<VM> = Flex::new(Axis::Vertical)
        .cursor(CursorStyle::Default)
        .overflow(crate::ui::layout::Overflow::Hidden)
        .focus_scope(
            FocusScopeOptions::new()
                .trap(true)
                .auto_focus_first(auto_focus_first)
                .active(open.clone()),
        )
        .style(move |mode| {
            let resolved = drawer_style_for_panel
                .clone()
                .unwrap_or_else(|| DrawerStyle::default_for(mode));
            let mut s = ContainerStyle::default_for(mode);
            s.surface.background = Some(resolved.background.clone());
            s.surface.border_color = Some(resolved.border.clone());
            s.surface.border_width = Some(resolved.border_width.clone());
            s.surface.border_radius = Some(resolved.radius.clone());
            s.surface.shadow = Some(resolved.shadow.into());
            s
        });

    match placement {
        DrawerPlacement::Left | DrawerPlacement::Right => {
            panel = panel.width(panel_extent).height(pct(100.0));
        }
        DrawerPlacement::Top | DrawerPlacement::Bottom => {
            panel = panel.width(pct(100.0)).height(panel_extent);
        }
    }
    if resolved_style_for_layout.padding != Insets::ZERO {
        panel = panel.padding(resolved_style_for_layout.padding);
    }
    if let Some(content) = drawer_content {
        panel = panel.child(content);
    }
    let panel_element: Element<VM> = panel.into();
    let panel_widget_id = panel_element.id;
    let main_content: Element<VM> = Stack::<VM>::new()
        .grow(1.0)
        .shrink(1.0)
        .child(content)
        .into();

    let root = match placement {
        DrawerPlacement::Left => Flex::horizontal().child(panel_element).child(main_content),
        DrawerPlacement::Right => Flex::horizontal().child(main_content).child(panel_element),
        DrawerPlacement::Top => Flex::vertical().child(panel_element).child(main_content),
        DrawerPlacement::Bottom => Flex::vertical().child(main_content).child(panel_element),
    };
    let mut root_element: Element<VM> = root.into();
    root_element.drawer = Some(Box::new(DrawerDescriptor {
        open,
        on_open_change,
        placement,
        mode: DrawerMode::Push,
        close_on_escape,
        close_on_backdrop_click: false,
        return_focus_to,
        backdrop_widget_id: root_element.id,
        panel_widget_id,
        style,
    }));
    root_element
}

impl<VM> Drawer<VM> {
    /// 暴露内部常量，方便 tests 引用。
    pub(crate) const _DRAWER_SLIDE_DURATION_MS: u64 = DRAWER_SLIDE_DURATION_MS;
    #[doc(hidden)]
    pub fn _backdrop_id_of(element: &Element<VM>) -> Option<WidgetId> {
        element.drawer.as_ref().map(|d| d.backdrop_widget_id)
    }
    #[doc(hidden)]
    pub fn _panel_id_of(element: &Element<VM>) -> Option<WidgetId> {
        element.drawer.as_ref().map(|d| d.panel_widget_id)
    }
}
