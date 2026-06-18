//! Tooltip widget——通过 hover 触发的浮层提示。
//!
//! Tooltip 不是独立的 `WidgetKind`，而是任意 widget 的可选修饰（`Element::tooltip`）。
//! 任何 widget 的 builder 通过 `.tooltip("text")` 给底层 `Element` 挂上 tooltip 描述符，
//! collect 阶段统一在 widget 的 trigger frame 之上调用 Overlay 引擎渲染。
//!
//! `Tooltip::new(text)` 走轻量文本 primitive 路径；`Tooltip::content(element)` 走
//! nested scene 路径，可承载任意 Element 子树。

use std::sync::Arc;
use std::time::Duration;

use crate::theme::StyleContext;
use crate::ui::layout::Value;
use crate::ui::widget::core::Element;
use crate::ui::widget::overlay::{FlipPolicy, Placement};
use crate::ui::widget::style::{StyleResolver, TooltipStyle};

/// 默认 hover 延迟。与 Material / Web 平台习惯一致。
pub const TOOLTIP_DEFAULT_DELAY: Duration = Duration::from_millis(500);

/// Tooltip 描述符。挂在 `Element::tooltip` 上，由 collect 阶段渲染。
pub struct Tooltip<VM = ()> {
    pub(crate) content: TooltipContent<VM>,
    pub(crate) placement: Placement,
    pub(crate) flip_policy: FlipPolicy,
    pub(crate) delay: Duration,
    pub(crate) style: Option<StyleResolver<TooltipStyle>>,
}

pub(crate) enum TooltipContent<VM = ()> {
    Text(Value<String>),
    Element(Box<Element<VM>>),
}

impl<VM> Clone for TooltipContent<VM> {
    fn clone(&self) -> Self {
        match self {
            Self::Text(text) => Self::Text(text.clone()),
            Self::Element(element) => Self::Element(element.clone()),
        }
    }
}

impl<VM> Clone for Tooltip<VM> {
    fn clone(&self) -> Self {
        Self {
            content: self.content.clone(),
            placement: self.placement,
            flip_policy: self.flip_policy,
            delay: self.delay,
            style: self.style.clone(),
        }
    }
}

impl<VM> Tooltip<VM> {
    /// 构造 Tooltip，使用主题默认样式与 500ms hover 延迟。
    pub fn new(text: impl Into<Value<String>>) -> Self {
        Self {
            content: TooltipContent::Text(text.into()),
            placement: Placement::top(),
            flip_policy: FlipPolicy::FlipSide,
            delay: TOOLTIP_DEFAULT_DELAY,
            style: None,
        }
    }

    /// 使用任意 Element 子树作为 Tooltip 内容。
    pub fn content(content: impl Into<Element<VM>>) -> Self {
        Self {
            content: TooltipContent::Element(Box::new(content.into())),
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

    /// Patch the theme-derived style.
    pub fn style(
        mut self,
        mutator: impl Fn(&mut TooltipStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| TooltipStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    /// Replace the full resolved style.
    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> TooltipStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    /// 按当前主题解析最终样式（用户未提供则取主题默认值）。
    pub(crate) fn resolved_style(&self, context: &StyleContext<'_>) -> TooltipStyle {
        let mut base = TooltipStyle::default_for_theme(context.theme);
        context.theme.components.tooltip.apply(&mut base, context);
        self.style
            .as_ref()
            .map(|resolver| resolver.resolve_from(base.clone(), context))
            .unwrap_or(base)
    }

    pub(crate) fn scope<RootVm: 'static>(
        self,
        selector: Arc<dyn for<'a> Fn(&'a mut RootVm) -> &'a mut VM + Send + Sync>,
    ) -> Tooltip<RootVm>
    where
        VM: 'static,
    {
        Tooltip {
            content: match self.content {
                TooltipContent::Text(text) => TooltipContent::Text(text),
                TooltipContent::Element(content) => {
                    TooltipContent::Element(Box::new(content.scope_with_selector(selector)))
                }
            },
            placement: self.placement,
            flip_policy: self.flip_policy,
            delay: self.delay,
            style: self.style,
        }
    }
}
