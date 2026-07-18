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
//! - backdrop + panel 的动画：backdrop 走 `opacity` fade，panel 走 scene-only
//!   `offset` 位移（left/right/top/bottom），避免滑动帧重复布局；
//! - Esc 关闭 / on_close 派发 / focus return 由挂在外层 Stack 上的
//!   `DrawerDescriptor` + collect 阶段 sentinel overlay 完成；
//! - `close_on_backdrop_click` 在 collect 阶段按当前 open 信号注入 backdrop hit
//!   region，避免初始关闭的 Drawer 打开后缺少点击 handler。

use parking_lot::{Mutex, RwLock};
use std::sync::Arc;

use crate::foundation::color::Color;
use crate::foundation::view_model::ValueCommand;
use crate::log::Log;
use crate::ui::layout::{pct, Axis, Insets, LayoutStyle, Value};
use crate::ui::theme::{StyleContext, WidgetState};
use crate::ui::unit::Dp;
use crate::ui::widget::common::{Point, VisualStyle};
use crate::ui::widget::container::{
    set_layout_inset, set_layout_length, set_layout_lengths, IntoLengthValue,
};
use crate::ui::widget::container::{Flex, Stack};
use crate::ui::widget::core::Element;
use crate::ui::widget::style::{ContainerStyle, DrawerStyle, StyleResolver, StyleSheet};
use crate::ui::widget::{CursorStyle, FocusScopeOptions, WidgetId};

use super::descriptor::DrawerDescriptor;
use super::placement::DrawerPlacement;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct DrawerRuntimeMetrics {
    width: Dp,
    height: Dp,
    padding: Insets,
}

impl From<&DrawerStyle> for DrawerRuntimeMetrics {
    fn from(style: &DrawerStyle) -> Self {
        Self {
            width: style.width,
            height: style.height,
            padding: style.padding,
        }
    }
}

/// Drawer presentation mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrawerMode {
    #[default]
    Overlay,
    Push,
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
    style: Option<StyleResolver<DrawerStyle>>,
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

    /// Patch the theme-derived style.
    pub fn style(
        mut self,
        mutator: impl Fn(&mut DrawerStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| DrawerStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    /// Replace the full resolved style.
    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> DrawerStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
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
        // Motion targets stay transition-free until runtime layout, where the
        // current Theme.motion and reduced-motion preference are available.
        let backdrop_visibility: Value<f32> = match open.clone() {
            Value::Static(open_now) => Value::Static(if open_now { 1.0 } else { 0.0 }),
            Value::Signal(signal) => Value::Signal(signal.map(|o| if o { 1.0 } else { 0.0 })),
        };

        // -----------------------------------------------------------------
        // backdrop：覆盖整个区域的半透明 scrim
        // 根据 open 状态动态设置背景色，确保关闭时完全透明
        // -----------------------------------------------------------------
        let backdrop_style = style.clone();
        let backdrop_opacity = backdrop_visibility.clone();
        let backdrop = Stack::<VM>::new()
            .size(pct(100.0), pct(100.0))
            .position_absolute()
            .left(Dp::ZERO)
            .top(Dp::ZERO)
            .runtime_layout(move |_, _, context, _, visual| {
                visual.opacity = backdrop_opacity
                    .clone()
                    .with_default_transition(context.motion_normal_transition());
            })
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_drawer_style_with_sheet(
                    backdrop_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                );
                let mut s = ContainerStyle::default_for_theme(context.theme);
                s.surface.background = Some(resolved.backdrop_color);
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
        let runtime_metrics = Arc::new(RwLock::new(DrawerRuntimeMetrics::default()));
        let panel_runtime_metrics = runtime_metrics.clone();
        let panel_open = open.clone();
        let panel_offset_cache = Arc::new(Mutex::new(None::<(Dp, Value<Point>)>));

        // 构建 panel 容器
        let mut panel: Flex<VM> = Flex::new(Axis::Vertical)
            .position_absolute()
            .cursor(CursorStyle::Default)
            // Panel 保持不透明并只做平移；backdrop 独立淡入淡出。
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_drawer_style_with_sheet(
                    drawer_style_for_panel.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    state,
                );
                let mut s = ContainerStyle::default_for_theme(context.theme);
                s.surface.background = Some(resolved.background.clone());
                s.surface.border_color = Some(resolved.border.clone());
                s.surface.border_width = Some(resolved.border_width.clone());
                s.surface.border_radius = Some(resolved.radius.clone());
                s.surface.shadow = Some(resolved.shadow.into());
                s
            })
            .runtime_layout(move |layout, container, context, _, visual| {
                let metrics = *panel_runtime_metrics.read();
                let hidden_extent = match placement {
                    DrawerPlacement::Left | DrawerPlacement::Right => metrics.width,
                    DrawerPlacement::Top | DrawerPlacement::Bottom => metrics.height,
                };
                let hidden_offset = match placement {
                    DrawerPlacement::Left => Point::new(-hidden_extent, Dp::ZERO),
                    DrawerPlacement::Right => Point::new(hidden_extent, Dp::ZERO),
                    DrawerPlacement::Top => Point::new(Dp::ZERO, -hidden_extent),
                    DrawerPlacement::Bottom => Point::new(Dp::ZERO, hidden_extent),
                };
                visual.offset = match &panel_open {
                    Value::Static(open) => {
                        Value::Static(if *open { Point::ZERO } else { hidden_offset })
                    }
                    Value::Signal(signal) => {
                        let mut cache = panel_offset_cache.lock();
                        let value = match cache.as_ref() {
                            Some((cached_extent, value)) if *cached_extent == hidden_extent => {
                                value.clone()
                            }
                            _ => {
                                let value = Value::Signal(signal.clone().map(move |open| {
                                    if open {
                                        Point::ZERO
                                    } else {
                                        hidden_offset
                                    }
                                }));
                                *cache = Some((hidden_extent, value.clone()));
                                value
                            }
                        };
                        value.with_default_transition(context.motion_slow_transition())
                    }
                };
                match placement {
                    DrawerPlacement::Left => {
                        set_layout_inset(&mut layout.left, Dp::ZERO);
                        set_layout_inset(&mut layout.top, Dp::ZERO);
                        set_layout_length(&mut layout.width, metrics.width);
                        set_layout_length(&mut layout.height, pct(100.0));
                    }
                    DrawerPlacement::Right => {
                        set_layout_inset(&mut layout.right, Dp::ZERO);
                        set_layout_inset(&mut layout.top, Dp::ZERO);
                        set_layout_length(&mut layout.width, metrics.width);
                        set_layout_length(&mut layout.height, pct(100.0));
                    }
                    DrawerPlacement::Top => {
                        set_layout_inset(&mut layout.left, Dp::ZERO);
                        set_layout_inset(&mut layout.top, Dp::ZERO);
                        set_layout_length(&mut layout.width, pct(100.0));
                        set_layout_length(&mut layout.height, metrics.height);
                    }
                    DrawerPlacement::Bottom => {
                        set_layout_inset(&mut layout.left, Dp::ZERO);
                        set_layout_inset(&mut layout.bottom, Dp::ZERO);
                        set_layout_length(&mut layout.width, pct(100.0));
                        set_layout_length(&mut layout.height, metrics.height);
                    }
                }
                container.padding = Some(Value::Static(metrics.padding));
            });

        // 添加内容
        if let Some(content_element) = content {
            panel = panel.child(content_element);
        }

        let panel_element: Element<VM> = panel.into();
        let panel_widget_id = panel_element.id;

        // -----------------------------------------------------------------
        // 外层 Stack：全屏绝对定位容器，backdrop 仅在打开时捕获点击。
        // -----------------------------------------------------------------
        let outer_style = style.clone();
        let outer_runtime_metrics = runtime_metrics;
        let outer: Stack<VM> = Stack::<VM>::new()
            .size(pct(100.0), pct(100.0))
            .runtime_layout(move |_, _, context, style_sheet, visual| {
                let resolved = resolve_drawer_style_with_sheet(
                    outer_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                *outer_runtime_metrics.write() = DrawerRuntimeMetrics::from(&resolved);
            })
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

    let drawer_style_for_panel = style.clone();
    let runtime_metrics = Arc::new(RwLock::new(DrawerRuntimeMetrics::default()));
    let panel_runtime_metrics = runtime_metrics.clone();
    let panel_open = open.clone();
    let panel_extent_cache = Arc::new(Mutex::new(None::<(Dp, Value<Dp>)>));

    let mut panel: Flex<VM> = Flex::new(Axis::Vertical)
        .cursor(CursorStyle::Default)
        .overflow(crate::ui::layout::Overflow::Hidden)
        .focus_scope(
            FocusScopeOptions::new()
                .trap(true)
                .auto_focus_first(auto_focus_first)
                .active(open.clone()),
        )
        .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
            let resolved = resolve_drawer_style_with_sheet(
                drawer_style_for_panel.as_ref(),
                context,
                style_sheet,
                visual,
                state,
            );
            let mut s = ContainerStyle::default_for_theme(context.theme);
            s.surface.background = Some(resolved.background.clone());
            s.surface.border_color = Some(resolved.border.clone());
            s.surface.border_width = Some(resolved.border_width.clone());
            s.surface.border_radius = Some(resolved.radius.clone());
            s.surface.shadow = Some(resolved.shadow.into());
            s
        })
        .runtime_layout(move |layout, container, context, _, _| {
            let metrics = *panel_runtime_metrics.read();
            let target_extent = match placement {
                DrawerPlacement::Left | DrawerPlacement::Right => metrics.width,
                DrawerPlacement::Top | DrawerPlacement::Bottom => metrics.height,
            };
            let panel_extent = match &panel_open {
                Value::Static(open) => Value::Static(if *open { target_extent } else { Dp::ZERO }),
                Value::Signal(signal) => {
                    let mut cache = panel_extent_cache.lock();
                    let value = match cache.as_ref() {
                        Some((cached_extent, value)) if *cached_extent == target_extent => {
                            value.clone()
                        }
                        _ => {
                            let value = Value::Signal(signal.clone().map(move |open| {
                                if open {
                                    target_extent
                                } else {
                                    Dp::ZERO
                                }
                            }));
                            *cache = Some((target_extent, value.clone()));
                            value
                        }
                    };
                    value.with_default_transition(context.motion_slow_transition())
                }
            };
            match placement {
                DrawerPlacement::Left | DrawerPlacement::Right => {
                    set_layout_length(&mut layout.width, panel_extent);
                    set_layout_length(&mut layout.height, pct(100.0));
                }
                DrawerPlacement::Top | DrawerPlacement::Bottom => {
                    set_layout_length(&mut layout.width, pct(100.0));
                    set_layout_length(&mut layout.height, panel_extent);
                }
            }
            container.padding = Some(Value::Static(metrics.padding));
        });
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
    let root_style = style.clone();
    let root_runtime_metrics = runtime_metrics;
    let root = root.runtime_layout(move |_, _, context, style_sheet, visual| {
        let resolved = resolve_drawer_style_with_sheet(
            root_style.as_ref(),
            context,
            style_sheet,
            visual,
            WidgetState::default(),
        );
        *root_runtime_metrics.write() = DrawerRuntimeMetrics::from(&resolved);
    });
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
    #[doc(hidden)]
    pub fn _backdrop_id_of(element: &Element<VM>) -> Option<WidgetId> {
        element.drawer.as_ref().map(|d| d.backdrop_widget_id)
    }
    #[doc(hidden)]
    pub fn _panel_id_of(element: &Element<VM>) -> Option<WidgetId> {
        element.drawer.as_ref().map(|d| d.panel_widget_id)
    }
}

fn resolve_drawer_style_with_sheet(
    style: Option<&StyleResolver<DrawerStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> DrawerStyle {
    let mut base = DrawerStyle::default_for_theme(context.theme);
    context.theme.components.drawer.apply(&mut base, context);
    style_sheet.apply_drawer(&mut base, context, visual);
    style_sheet.apply_drawer_state(&mut base, context, visual, state);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}
