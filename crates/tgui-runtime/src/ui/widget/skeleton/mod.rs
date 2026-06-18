use crate::theme::{StyleContext, WidgetState};
use crate::ui::layout::{pct, LayoutStyle, Value};
use crate::ui::theme::Theme;
use crate::ui::unit::{dp, Dp};

use super::common::VisualStyle;
use super::core::Element;
use super::p3_support::{
    impl_p3_layout_api, merge_layout, resolve_component_style_with_sheet, with_visual_identity,
};
use super::style::{ContainerStyle, ProgressBarStyle, SkeletonStyle, StyleResolver, StyleSheet};
use super::{Flex, ProgressBar, Stack, WidgetKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkeletonShape {
    Rect,
    Circle,
    Line,
}

pub struct Skeleton<VM> {
    shape: SkeletonShape,
    lines: usize,
    style: Option<StyleResolver<SkeletonStyle>>,
    layout: LayoutStyle,
    visual: VisualStyle,
    key: Option<WidgetKey>,
    _marker: std::marker::PhantomData<fn() -> VM>,
}

impl<VM> Skeleton<VM> {
    pub fn rect() -> Self {
        Self::new(SkeletonShape::Rect)
    }

    pub fn circle() -> Self {
        Self::new(SkeletonShape::Circle)
    }

    pub fn line() -> Self {
        Self::new(SkeletonShape::Line)
    }

    pub fn lines(lines: usize) -> Self {
        Self {
            lines: lines.max(1),
            ..Self::new(SkeletonShape::Line)
        }
    }

    fn new(shape: SkeletonShape) -> Self {
        Self {
            shape,
            lines: 1,
            style: None,
            layout: LayoutStyle::default(),
            visual: VisualStyle::default(),
            key: None,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn style(
        mut self,
        mutator: impl Fn(&mut SkeletonStyle, &StyleContext<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::mutate(
            |context| SkeletonStyle::default_for_theme(context.theme),
            mutator,
        ));
        self
    }

    pub fn style_full(
        mut self,
        resolver: impl Fn(&StyleContext<'_>) -> SkeletonStyle + Send + Sync + 'static,
    ) -> Self {
        self.style = Some(StyleResolver::full(resolver));
        self
    }

    impl_p3_layout_api!(layout);
}

impl<VM: 'static> From<Skeleton<VM>> for Element<VM> {
    fn from(skeleton: Skeleton<VM>) -> Self {
        let layout_style = resolve_skeleton_style_for_layout(skeleton.style.as_ref());
        let visual_identity = skeleton.visual.clone();
        let block = |width: Dp, height: Dp, radius: Dp| -> Element<VM> {
            let style = skeleton.style.clone();
            let shimmer_style = skeleton.style.clone();
            let shimmer_identity = visual_identity.clone();
            with_visual_identity(
                Stack::new()
                    .size(width, height)
                    .style_full_with_style_sheet(move |context, style_sheet, visual, state| {
                        let resolved = resolve_skeleton_style_with_sheet(
                            style.as_ref(),
                            context,
                            style_sheet,
                            visual,
                            state,
                        );
                        let mut container = ContainerStyle::default_for_theme(context.theme);
                        container.surface.background = Some(resolved.base);
                        container.surface.border_radius = Some(Value::Static(radius));
                        container
                    })
                    .child(with_visual_identity(
                        ProgressBar::<VM>::indeterminate(true)
                            .position_absolute()
                            .left(pct(0.0))
                            .top(pct(0.0))
                            .width(pct(100.0))
                            .height(pct(100.0))
                            .style_full_with_style_sheet(
                                move |context, style_sheet, visual, state| {
                                    let resolved = resolve_skeleton_style_with_sheet(
                                        shimmer_style.as_ref(),
                                        context,
                                        style_sheet,
                                        visual,
                                        state,
                                    );
                                    let mut progress =
                                        ProgressBarStyle::default_for_theme(context.theme);
                                    progress.track_color =
                                        Value::Static(crate::foundation::color::Color::TRANSPARENT);
                                    progress.fill_color = resolved.highlight;
                                    progress.radius = Value::Static(radius);
                                    progress.height = height;
                                    progress.min_width = Dp::ZERO;
                                    progress.indeterminate_segment_ratio = 0.42;
                                    progress
                                },
                            )
                            .into(),
                        &visual_identity,
                    ))
                    .into(),
                &shimmer_identity,
            )
        };
        let mut root = if skeleton.lines > 1 {
            let lines = (0..skeleton.lines)
                .map(|index| {
                    let width = if index + 1 == skeleton.lines {
                        dp(160.0)
                    } else {
                        dp(220.0)
                    };
                    block(width, layout_style.line_height, layout_style.radius)
                })
                .collect::<Vec<_>>();
            Flex::vertical().gap(layout_style.gap).child(lines).into()
        } else {
            match skeleton.shape {
                SkeletonShape::Circle => block(dp(40.0), dp(40.0), dp(999.0)),
                SkeletonShape::Line => {
                    block(dp(220.0), layout_style.line_height, layout_style.radius)
                }
                SkeletonShape::Rect => block(dp(220.0), dp(120.0), layout_style.radius),
            }
        };
        root.key = skeleton.key;
        root = with_visual_identity(root, &skeleton.visual);
        root.layout = merge_layout(root.layout, skeleton.layout);
        root
    }
}

fn resolve_skeleton_style(
    style: Option<&StyleResolver<SkeletonStyle>>,
    context: &StyleContext<'_>,
) -> SkeletonStyle {
    let style_sheet = StyleSheet::default();
    resolve_skeleton_style_with_sheet(
        style,
        context,
        &style_sheet,
        &VisualStyle::default(),
        WidgetState::default(),
    )
}

fn resolve_skeleton_style_with_sheet(
    style: Option<&StyleResolver<SkeletonStyle>>,
    context: &StyleContext<'_>,
    style_sheet: &StyleSheet,
    visual: &VisualStyle,
    state: WidgetState,
) -> SkeletonStyle {
    resolve_component_style_with_sheet(
        style,
        context,
        style_sheet,
        visual,
        state,
        SkeletonStyle::default_for_theme(context.theme),
        |base, context| context.theme.components.skeleton.apply(base, context),
        |sheet, base, context, visual| sheet.apply_skeleton(base, context, visual),
        |sheet, base, context, visual, state| {
            sheet.apply_skeleton_state(base, context, visual, state)
        },
    )
}

fn resolve_skeleton_style_for_layout(
    style: Option<&StyleResolver<SkeletonStyle>>,
) -> SkeletonStyle {
    let theme = Theme::default();
    let context = StyleContext::from_theme(&theme);
    resolve_skeleton_style(style, &context)
}
