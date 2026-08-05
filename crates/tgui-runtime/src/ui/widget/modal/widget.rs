//! [`Modal`] widget builder。
//!
//! ```ignore
//! Modal::new(state.confirm_open.signal())
//!     .on_open_change(ValueCommand::new(|vm, open| vm.confirm_open.set(open)))
//!     .title("Confirm")
//!     .content(Text::new("Are you sure?").into())
//!     .action(ModalAction::new("Cancel").on_click(Command::new(|vm: &mut Vm| vm.confirm_open.set(false))))
//!     .action(ModalAction::primary("OK").on_click(Command::new(|vm: &mut Vm| { vm.confirmed.set(true); vm.confirm_open.set(false); })))
//! ```
//!
//! 实现要点：
//! - `Modal::From<Element>` 把自身展开为一个 `position_absolute + 全屏 fill`
//!   的 Stack，里面放两个常驻 child：backdrop（半透明遮罩）+ centered card；
//! - card 走 Flex(vertical)，按 title / content / actions 三段排列；
//! - 外层 Stack 挂动态 active 的 `FocusScopeOptions::{trap, auto_focus_first}`，
//!   打开后聚焦主按钮 / 首个控件，并使 Tab 在 modal 内循环；
//! - backdrop + card 的 `opacity` 由 `open` 派生，运行时按当前
//!   `Theme.motion.normal_ms` 应用 ease-out；reduced-motion 直接落到终值；
//! - Esc 关闭 / on_close 派发 / focus return 由挂在外层 Stack 上的
//!   `ModalDescriptor` + collect 阶段 sentinel overlay 完成；
//! - `close_on_backdrop_click` 由 backdrop 自己的 `on_click` 命令直接驱动。

use std::sync::Arc;

use parking_lot::{Mutex, RwLock};

use crate::foundation::color::Color;
use crate::foundation::view_model::{Command, CommandContext, ValueCommand};
use crate::ui::layout::{pct, Align, Axis, Insets, Length, Overflow, Value};
use crate::ui::theme::{StyleContext, TextStyle, WidgetState};
use crate::ui::unit::Dp;
use crate::ui::widget::button::Button;
use crate::ui::widget::common::VisualStyle;
use crate::ui::widget::container::{Flex, Stack};
use crate::ui::widget::core::Element;
use crate::ui::widget::style::{
    ButtonStyle, ContainerStyle, ModalStyle, StyleResolver, StyleSheet, TextWidgetStyle,
};
use crate::ui::widget::text::Text;
use crate::ui::widget::{FocusScopeOptions, WidgetId};

use super::action::ModalAction;
use super::descriptor::ModalDescriptor;

#[derive(Clone, Debug, PartialEq)]
struct ModalRuntimeMetrics {
    min_width: Dp,
    max_width: Dp,
    max_height: Dp,
    margin: Insets,
    padding: Insets,
    title_padding: Insets,
    title_text_style: TextStyle,
    content_padding: Insets,
    actions_gap: Dp,
    actions_padding: Insets,
    enter_scale: f32,
}

impl Default for ModalRuntimeMetrics {
    fn default() -> Self {
        Self {
            min_width: Dp::ZERO,
            max_width: Dp::ZERO,
            max_height: Dp::ZERO,
            margin: Insets::ZERO,
            padding: Insets::ZERO,
            title_padding: Insets::ZERO,
            title_text_style: TextStyle::default(),
            content_padding: Insets::ZERO,
            actions_gap: Dp::ZERO,
            actions_padding: Insets::ZERO,
            enter_scale: 0.96,
        }
    }
}

impl From<&ModalStyle> for ModalRuntimeMetrics {
    fn from(style: &ModalStyle) -> Self {
        Self {
            min_width: style.min_width,
            max_width: style.max_width,
            max_height: style.max_height,
            margin: style.margin,
            padding: style.padding,
            title_padding: style.title_padding,
            title_text_style: style.title_text_style.clone(),
            content_padding: style.content_padding,
            actions_gap: style.actions_gap,
            actions_padding: style.actions_padding,
            enter_scale: style.enter_scale.clamp(0.01, 16.0),
        }
    }
}

/// 应用内阻塞式对话框 builder。
pub struct Modal<VM> {
    open: Value<bool>,
    on_open_change: Option<ValueCommand<VM, bool>>,
    title: Option<Value<String>>,
    content: Option<Element<VM>>,
    actions: Vec<ModalAction<VM>>,
    close_on_escape: bool,
    close_on_backdrop_click: bool,
    return_focus_to: Option<WidgetId>,
    auto_focus_first: bool,
    style: Option<StyleResolver<ModalStyle>>,
}

impl<VM: 'static> Modal<VM> {
    /// 用绑定 open 状态的 `Signal<bool>`（或常量 bool）创建 Modal。
    pub fn new(open: impl Into<Value<bool>>) -> Self {
        Self {
            open: open.into(),
            on_open_change: None,
            title: None,
            content: None,
            actions: Vec::new(),
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

    /// 设置标题文本。
    pub fn title(mut self, title: impl Into<Value<String>>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// 设置内容区元素（任意 widget 子树）。
    pub fn content(mut self, content: impl Into<Element<VM>>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// 追加一个动作按钮。
    pub fn action(mut self, action: ModalAction<VM>) -> Self {
        self.actions.push(action);
        self
    }

    /// 设置所有动作按钮。
    pub fn actions(mut self, actions: Vec<ModalAction<VM>>) -> Self {
        self.actions = actions;
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

    /// 打开时是否自动聚焦 modal 内第一个可聚焦控件（默认 `true`）。
    pub fn auto_focus_first(mut self, auto_focus_first: bool) -> Self {
        self.auto_focus_first = auto_focus_first;
        self
    }

    /// Patch the theme-derived style.
    pub fn style(
        mut self,
        mutator: impl Fn(&mut ModalStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| ModalStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    /// Replace the full resolved style.
    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> ModalStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }
}

impl<VM: 'static> From<Modal<VM>> for Element<VM> {
    fn from(modal: Modal<VM>) -> Element<VM> {
        let Modal {
            open,
            on_open_change,
            title,
            content,
            actions,
            close_on_escape,
            close_on_backdrop_click,
            return_focus_to,
            auto_focus_first,
            style,
        } = modal;

        // -----------------------------------------------------------------
        // visibility target：只派生目标值。transition 在 runtime layout 中
        // 按当前 StyleContext 注入，避免构建树时冻结 Theme.motion，也让
        // reduced-motion / 0ms theme 可以移除已有 transition。
        // -----------------------------------------------------------------
        let visibility_value: Value<f32> = match open.clone() {
            Value::Static(open_now) => Value::Static(if open_now { 1.0 } else { 0.0 }),
            Value::Signal(signal) => Value::Signal(signal.map(|o| if o { 1.0 } else { 0.0 })),
        };
        let runtime_metrics = Arc::new(RwLock::new(ModalRuntimeMetrics::default()));

        // -----------------------------------------------------------------
        // close 命令：把 on_open_change(false) 包成 Command<VM>，用于 backdrop
        // 点击。actions 内的命令由调用方自己设置（typically 同样调用 close）。
        // -----------------------------------------------------------------
        let close_command: Option<Command<VM>> = on_open_change.clone().map(|cmd| {
            Command::new_with_context(move |vm: &mut VM, ctx: &CommandContext<VM>| {
                cmd.execute_with_context(vm, false, ctx);
            })
        });

        // -----------------------------------------------------------------
        // backdrop：覆盖整个 modal 区域的半透明 scrim。
        // Opacity 在 runtime layout 中读取 live motion token。
        // 背景色由 ContainerStyle.surface.background 设置。
        // -----------------------------------------------------------------
        let backdrop_style = style.clone();
        let backdrop_visibility = visibility_value.clone();
        let mut backdrop = Stack::<VM>::new()
            .size(pct(100.0), pct(100.0))
            .position_absolute()
            .left(Dp::ZERO)
            .top(Dp::ZERO)
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_modal_style_with_sheet(
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
            })
            .opacity(visibility_value.clone())
            .runtime_layout(move |_, _, context, _, visual| {
                visual.opacity = backdrop_visibility
                    .clone()
                    .with_default_transition(context.motion_normal_transition());
            });
        if close_on_backdrop_click {
            if let Some(close_cmd) = close_command.clone() {
                backdrop = backdrop.on_click(close_cmd);
            }
        }
        let backdrop_element: Element<VM> = backdrop.into();
        let backdrop_widget_id = backdrop_element.id;

        // -----------------------------------------------------------------
        // card：标题 + 内容 + 动作三段，居中。
        // -----------------------------------------------------------------
        let modal_style_for_card = style.clone();
        let title_value_for_render = title.clone();
        let card_runtime_metrics = runtime_metrics.clone();
        let card_open = open.clone();
        let card_scale_cache = Arc::new(Mutex::new(None::<(f32, Value<f32>)>));
        let card_visibility = visibility_value.clone();
        let card_base_visibility = card_visibility.clone();
        let mut card: Flex<VM> = Flex::new(Axis::Vertical)
            .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                let resolved = resolve_modal_style_with_sheet(
                    modal_style_for_card.as_ref(),
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
                let metrics = card_runtime_metrics.read().clone();
                layout.min_width = Some(Value::Static(Length::Px(metrics.min_width)));
                layout.max_width = Some(Value::Static(Length::Px(metrics.max_width)));
                layout.max_height = Some(Value::Static(Length::Px(metrics.max_height)));
                layout.margin = Value::Static(metrics.margin);
                container.padding = Some(Value::Static(metrics.padding));
                let transition = context.motion_normal_transition();
                visual.scale = match &card_open {
                    Value::Static(open_now) => {
                        Value::Static(if *open_now { 1.0 } else { metrics.enter_scale })
                    }
                    Value::Signal(signal) => {
                        let mut cache = card_scale_cache.lock();
                        let value = match cache.as_ref() {
                            Some((enter_scale, value)) if *enter_scale == metrics.enter_scale => {
                                value.clone()
                            }
                            _ => {
                                let enter_scale = metrics.enter_scale;
                                let value = Value::Signal(signal.clone().map(move |open| {
                                    if open {
                                        1.0
                                    } else {
                                        enter_scale
                                    }
                                }));
                                *cache = Some((enter_scale, value.clone()));
                                value
                            }
                        };
                        value.with_default_transition(transition)
                    }
                };
                visual.opacity = card_visibility.clone().with_default_transition(transition);
            })
            .opacity(card_base_visibility)
            .overflow(Overflow::Hidden);

        // title 段（如果给了 title）
        if let Some(title_value) = title_value_for_render {
            let title_metrics = runtime_metrics.clone();
            let title_text_metrics = runtime_metrics.clone();
            let title_element: Element<VM> = Flex::<VM>::new(Axis::Horizontal)
                .runtime_layout(move |_, container, _, _, _| {
                    container.padding = Some(Value::Static(title_metrics.read().title_padding));
                })
                .child(Text::new(title_value).style_full(move |context| {
                    let mut s = TextWidgetStyle::default_for_theme(context.theme);
                    s.typography = title_text_metrics.read().title_text_style.clone();
                    s
                }))
                .into();
            card = card.child(title_element);
        }

        // content 段（如果给了 content）
        if let Some(content_element) = content {
            let content_metrics = runtime_metrics.clone();
            let wrapped: Element<VM> = Stack::<VM>::new()
                .runtime_layout(move |_, container, _, _, _| {
                    container.padding = Some(Value::Static(content_metrics.read().content_padding));
                })
                .child(content_element)
                .into();
            card = card.child(wrapped);
        }

        // actions 段（如果有 action）
        if !actions.is_empty() {
            let actions_metrics = runtime_metrics.clone();
            let mut actions_row = Flex::<VM>::new(Axis::Horizontal)
                .runtime_layout(move |_, container, _, _, _| {
                    let metrics = actions_metrics.read();
                    container.padding = Some(Value::Static(metrics.actions_padding));
                    container.gap = Value::Static(Length::Px(metrics.actions_gap));
                })
                .justify(crate::ui::layout::Justify::End);
            let total = actions.len();
            for (idx, action) in actions.into_iter().enumerate() {
                let ModalAction {
                    label,
                    on_click,
                    primary,
                    disabled,
                } = action;
                let action_style = primary;
                let mut button = Button::new(label).disable(disabled);
                if let Some(cmd) = on_click {
                    button = button.on_click(cmd);
                }
                // Positive tab_index values win over the default 0 bucket, so primary
                // actions use 1 while secondary actions stay in tree order.
                let _ = idx;
                let _ = total;
                let _ = action_style;
                // ButtonStyle: primary 走 Primary 变体，否则 Secondary。
                if primary {
                    button = button.style_full(|context| {
                        ButtonStyle::default_for_theme(
                            context.theme,
                            crate::ui::widget::common::ButtonVariantKind::Primary,
                        )
                    });
                } else {
                    button = button.style_full(|context| {
                        ButtonStyle::default_for_theme(
                            context.theme,
                            crate::ui::widget::common::ButtonVariantKind::Secondary,
                        )
                    });
                }
                let mut button_element: Element<VM> = button.into();
                if primary {
                    button_element = button_element.tab_index(1);
                } else {
                    button_element = button_element.tab_index(0);
                }
                actions_row = actions_row.child(button_element);
            }
            card = card.child(Element::from(actions_row));
        }

        let card_element: Element<VM> = card.into();
        let card_widget_id = card_element.id;

        // -----------------------------------------------------------------
        // 外层 Stack：全屏 + 同时承担"居中 card" 的对齐职责。backdrop 走
        // `position_absolute` 不参与 align/justify 流，永远撑满；card 是 in-flow
        // 子节点，按 Stack 的 align/justify=Center 居中。
        // 这样省去一层 Centered Stack，降低 collect 递归深度（生产环境堆栈更
        // 安全，测试也不需要更大的 stack size）。
        // -----------------------------------------------------------------
        let outer_style = style.clone();
        let outer_runtime_metrics = runtime_metrics;
        let outer: Stack<VM> = Stack::<VM>::new()
            .size(pct(100.0), pct(100.0))
            .runtime_layout(move |_, _, context, style_sheet, visual| {
                let resolved = resolve_modal_style_with_sheet(
                    outer_style.as_ref(),
                    context,
                    style_sheet,
                    visual,
                    WidgetState::default(),
                );
                *outer_runtime_metrics.write() = ModalRuntimeMetrics::from(&resolved);
            })
            .align(Align::Center)
            .justify(crate::ui::layout::Justify::Center)
            .focus_scope(
                FocusScopeOptions::new()
                    .trap(true)
                    .auto_focus_first(auto_focus_first)
                    .suppress_interactions_when_inactive()
                    .hide_from_accessibility_when_inactive()
                    .active(open.clone()),
            )
            .child(backdrop_element)
            .child(card_element);

        let mut outer_element: Element<VM> = outer.into();
        outer_element.visual.accessibility_label = title;
        outer_element.modal = Some(Box::new(ModalDescriptor {
            open,
            on_open_change,
            close_on_escape,
            close_on_backdrop_click,
            return_focus_to,
            backdrop_widget_id,
            card_widget_id,
            style,
        }));
        outer_element
    }
}

impl<VM> Modal<VM> {
    #[doc(hidden)]
    pub fn _backdrop_id_of(element: &Element<VM>) -> Option<WidgetId> {
        element.modal.as_ref().map(|m| m.backdrop_widget_id)
    }
    #[doc(hidden)]
    pub fn _card_id_of(element: &Element<VM>) -> Option<WidgetId> {
        element.modal.as_ref().map(|m| m.card_widget_id)
    }
}

fn resolve_modal_style_with_sheet(
    style: Option<&StyleResolver<ModalStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> ModalStyle {
    let mut base = ModalStyle::default_for_theme(context.theme);
    context.theme.components.modal.apply(&mut base, context);
    style_sheet.apply_modal(&mut base, context, visual);
    style_sheet.apply_modal_state(&mut base, context, visual, state);
    style
        .map(|resolver| resolver.resolve_from(base.clone(), context))
        .unwrap_or(base)
}
