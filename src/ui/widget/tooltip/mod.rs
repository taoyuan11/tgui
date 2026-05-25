//! Tooltip widget——通过 hover 触发的浮层提示。
//!
//! Tooltip 不是独立的 `WidgetKind`，而是任意 widget 的可选修饰（`Element::tooltip`）。
//! 任何 widget 的 builder 通过 `.tooltip("text")` 给底层 `Element` 挂上 tooltip 描述符，
//! collect 阶段统一在 widget 的 trigger frame 之上调用 Overlay 引擎渲染。
//!
//! 本轮约束：
//! - 仅支持纯文本内容（`Value<String>`）；Element 子树作为内容留下一轮；
//! - `delay` 仅作用于 hover（默认 500ms）；focus / 长按不走 hover delay；
//! - 内容仍为纯文本，浮层渲染支持主题默认样式与三角指针。

use std::time::Duration;

use crate::theme::ResolvedThemeMode;
use crate::ui::layout::Value;
use crate::ui::widget::overlay::{FlipPolicy, Placement};
use crate::ui::widget::style::TooltipStyle;

/// 默认 hover 延迟。与 Material / Web 平台习惯一致。
pub const TOOLTIP_DEFAULT_DELAY: Duration = Duration::from_millis(500);

/// Tooltip 描述符。挂在 `Element::tooltip` 上，由 collect 阶段渲染。
#[derive(Clone)]
pub struct Tooltip {
    pub(crate) text: Value<String>,
    pub(crate) placement: Placement,
    pub(crate) flip_policy: FlipPolicy,
    pub(crate) delay: Duration,
    pub(crate) style: Option<TooltipStyle>,
}

impl Tooltip {
    /// 构造 Tooltip，使用主题默认样式与 500ms hover 延迟。
    pub fn new(text: impl Into<Value<String>>) -> Self {
        Self {
            text: text.into(),
            placement: Placement::top(),
            flip_policy: FlipPolicy::FlipSide,
            delay: TOOLTIP_DEFAULT_DELAY,
            style: None,
        }
    }

    /// 自定义 placement（默认 `Placement::top()`）。
    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    /// 自定义翻转策略（默认 `FlipPolicy::FlipSide`）。
    pub fn flip_policy(mut self, policy: FlipPolicy) -> Self {
        self.flip_policy = policy;
        self
    }

    /// 自定义 hover 延迟。`Duration::ZERO` 表示 hover 即显示。
    /// 该值仅影响鼠标 hover；focus 与长按不会等待此延迟。
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// 覆盖默认主题样式。
    pub fn style(mut self, style: TooltipStyle) -> Self {
        self.style = Some(style);
        self
    }

    /// 按主题模式解析最终样式（用户未提供则取主题默认值）。
    pub(crate) fn resolved_style(&self, mode: ResolvedThemeMode) -> TooltipStyle {
        self.style
            .clone()
            .unwrap_or_else(|| TooltipStyle::default_for(mode))
    }
}
